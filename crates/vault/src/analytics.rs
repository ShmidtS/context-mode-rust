use crate::graph_store::{GraphStoreError, VaultGraphStore};
use crate::search::{SearchError, VaultGraphSearch};
use crate::types::{VaultEdge, VaultNode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("graph store error: {0}")]
    Store(#[from] GraphStoreError),
    #[error("search error: {0}")]
    Search(#[from] SearchError),
}

pub type Result<T> = std::result::Result<T, AnalyticsError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GodNode {
    pub id: i64,
    pub title: String,
    pub path: String,
    pub in_degree: i64,
    pub out_degree: i64,
    pub page_rank: Option<f64>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurprisingConnection {
    pub source_path: String,
    pub target_path: String,
    pub edge_type: String,
    pub unexpectedness: f64,
    pub source_tags: Vec<String>,
    pub target_tags: Vec<String>,
    pub context: Option<String>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommunityHint {
    pub id: usize,
    pub representative_path: String,
    pub node_count: usize,
    pub internal_edges: usize,
    pub external_edges: usize,
    pub avg_internal_density: f64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuggestedQuestion {
    pub question: String,
    pub relevance: f64,
    pub related_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenEstimate {
    pub raw_tokens: usize,
    pub graph_tokens: usize,
    pub reduction_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphAnalysisResult {
    pub god_nodes: Vec<GodNode>,
    pub surprising_connections: Vec<SurprisingConnection>,
    pub community_hints: Vec<CommunityHint>,
    pub suggested_questions: Vec<SuggestedQuestion>,
    pub summary: String,
    pub markdown_report: String,
    pub token_estimate: Option<TokenEstimate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalyzeOpts {
    pub god_node_limit: Option<usize>,
    pub surprise_limit: Option<usize>,
    pub community_limit: Option<usize>,
    pub question_limit: Option<usize>,
}

impl Default for AnalyzeOpts {
    fn default() -> Self {
        Self {
            god_node_limit: Some(10),
            surprise_limit: Some(10),
            community_limit: Some(5),
            question_limit: Some(5),
        }
    }
}

pub fn analyze_graph(
    store: &VaultGraphStore,
    opts: Option<AnalyzeOpts>,
) -> Result<GraphAnalysisResult> {
    let opts = opts.unwrap_or_default();
    let nodes = store.get_all_nodes()?;
    let edges = store.get_all_edges()?;
    let tags = store.get_node_tag_map()?;
    let search = VaultGraphSearch::new(store);
    let page_rank = search.page_rank()?;
    let god_nodes = find_god_nodes(&nodes, &tags, &page_rank, opts.god_node_limit.unwrap_or(10));
    let surprising_connections =
        find_surprising_connections(&nodes, &edges, &tags, opts.surprise_limit.unwrap_or(10));
    let community_hints =
        find_community_hints(&nodes, &edges, &tags, opts.community_limit.unwrap_or(5));
    let suggested_questions = suggest_questions(
        &god_nodes,
        &surprising_connections,
        opts.question_limit.unwrap_or(5),
    );
    let summary = format!(
        "Graph has {} nodes, {} edges, {} god nodes, {} surprising connections.",
        nodes.len(),
        edges.len(),
        god_nodes.len(),
        surprising_connections.len()
    );
    let token_estimate = Some(compute_token_estimate(
        nodes.len(),
        &god_nodes,
        &surprising_connections,
        &community_hints,
        &suggested_questions,
    ));
    let markdown_report = generate_markdown_report(
        &summary,
        nodes.len(),
        edges.len(),
        &god_nodes,
        &surprising_connections,
        &community_hints,
        &suggested_questions,
    );
    Ok(GraphAnalysisResult {
        god_nodes,
        surprising_connections,
        community_hints,
        suggested_questions,
        summary,
        markdown_report,
        token_estimate,
    })
}

fn find_god_nodes(
    nodes: &[VaultNode],
    tags: &HashMap<i64, Vec<String>>,
    page_rank: &HashMap<i64, f64>,
    limit: usize,
) -> Vec<GodNode> {
    let n = nodes.len().max(1) as f64;
    let mut scored: Vec<_> = nodes
        .iter()
        .map(|node| {
            let pr = page_rank.get(&node.id).copied().unwrap_or(0.0);
            let score = pr * 0.7 + (node.in_degree as f64 / n) * 0.3;
            (score, node)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, node)| GodNode {
            id: node.id,
            title: node.title.clone(),
            path: node.note_path.clone(),
            in_degree: node.in_degree,
            out_degree: node.out_degree,
            page_rank: page_rank.get(&node.id).copied(),
            tags: tags.get(&node.id).cloned().unwrap_or_default(),
        })
        .collect()
}

fn find_surprising_connections(
    nodes: &[VaultNode],
    edges: &[VaultEdge],
    tags: &HashMap<i64, Vec<String>>,
    limit: usize,
) -> Vec<SurprisingConnection> {
    let node_map: HashMap<i64, &VaultNode> = nodes.iter().map(|n| (n.id, n)).collect();
    let mut scored = Vec::new();
    for edge in edges {
        let Some(target_id) = edge.target_id else {
            continue;
        };
        let (Some(source), Some(target)) =
            (node_map.get(&edge.source_id), node_map.get(&target_id))
        else {
            continue;
        };
        let source_tags = tags.get(&source.id).cloned().unwrap_or_default();
        let target_tags = tags.get(&target.id).cloned().unwrap_or_default();
        let tag_sim = tag_similarity(&source_tags, &target_tags);
        let path_sim = (path_module_prefix(&source.note_path)
            == path_module_prefix(&target.note_path)) as u8 as f64;
        let unexpectedness = (1.0 - tag_sim) * 0.5 + (1.0 - path_sim) * 0.5;
        if unexpectedness < 0.3 {
            continue;
        }
        let explanation = format!(
            "Unexpected because {} (score={unexpectedness:.2}).",
            explain_reasons(source, target, &source_tags, &target_tags)
        );
        scored.push((
            unexpectedness,
            SurprisingConnection {
                source_path: source.note_path.clone(),
                target_path: target.note_path.clone(),
                edge_type: edge.edge_type.clone(),
                unexpectedness,
                source_tags,
                target_tags,
                context: edge.context.clone(),
                explanation: Some(explanation),
            },
        ));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(limit).map(|(_, s)| s).collect()
}

fn find_community_hints(
    nodes: &[VaultNode],
    edges: &[VaultEdge],
    tags: &HashMap<i64, Vec<String>>,
    limit: usize,
) -> Vec<CommunityHint> {
    let node_map: HashMap<i64, &VaultNode> = nodes.iter().map(|n| (n.id, n)).collect();
    let mut parent: HashMap<i64, i64> = nodes.iter().map(|n| (n.id, n.id)).collect();

    for edge in edges {
        let Some(target_id) = edge.target_id else {
            continue;
        };
        let (Some(source), Some(target)) =
            (node_map.get(&edge.source_id), node_map.get(&target_id))
        else {
            continue;
        };
        let share_tag = tag_similarity(
            tags.get(&source.id).map(Vec::as_slice).unwrap_or(&[]),
            tags.get(&target.id).map(Vec::as_slice).unwrap_or(&[]),
        ) > 0.0;
        let share_path =
            path_module_prefix(&source.note_path) == path_module_prefix(&target.note_path);
        if share_tag || share_path {
            union(&mut parent, source.id, target.id);
        }
    }

    let mut groups: HashMap<i64, HashSet<i64>> = HashMap::new();
    for node in nodes {
        let root = find(&mut parent, node.id);
        groups.entry(root).or_default().insert(node.id);
    }

    let mut hints = Vec::new();
    for (idx, members) in groups.values().filter(|g| g.len() >= 2).enumerate() {
        let mut internal = 0;
        let mut external = 0;
        for edge in edges {
            let Some(target_id) = edge.target_id else {
                continue;
            };
            let src_in = members.contains(&edge.source_id);
            let tgt_in = members.contains(&target_id);
            if src_in && tgt_in {
                internal += 1;
            } else if src_in || tgt_in {
                external += 1;
            }
        }
        let mut all_tags = HashSet::new();
        for id in members {
            for tag in tags.get(id).cloned().unwrap_or_default() {
                all_tags.insert(tag);
            }
        }
        let member_nodes: Vec<_> = members
            .iter()
            .filter_map(|id| node_map.get(id).copied())
            .collect();
        let max_possible = member_nodes.len() * member_nodes.len().saturating_sub(1) / 2;
        hints.push(CommunityHint {
            id: idx,
            representative_path: member_nodes
                .first()
                .map(|n| n.note_path.clone())
                .unwrap_or_default(),
            node_count: member_nodes.len(),
            internal_edges: internal,
            external_edges: external,
            avg_internal_density: internal as f64 / max_possible.max(1) as f64,
            tags: all_tags.into_iter().collect(),
        });
    }
    hints.sort_by(|a, b| {
        b.avg_internal_density
            .partial_cmp(&a.avg_internal_density)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hints.truncate(limit);
    hints
}

fn suggest_questions(
    god_nodes: &[GodNode],
    surprising: &[SurprisingConnection],
    limit: usize,
) -> Vec<SuggestedQuestion> {
    let mut out = Vec::new();
    for node in god_nodes.iter().take(limit) {
        out.push(SuggestedQuestion {
            question: format!("Why is {} central to this vault?", node.title),
            relevance: node.page_rank.unwrap_or(0.0),
            related_nodes: vec![node.path.clone()],
        });
    }
    for edge in surprising.iter().take(limit.saturating_sub(out.len())) {
        out.push(SuggestedQuestion {
            question: format!(
                "Why does {} connect to {}?",
                edge.source_path, edge.target_path
            ),
            relevance: edge.unexpectedness,
            related_nodes: vec![edge.source_path.clone(), edge.target_path.clone()],
        });
    }
    out.truncate(limit);
    out
}

fn tag_similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let set: HashSet<&String> = a.iter().collect();
    let intersection = b.iter().filter(|t| set.contains(*t)).count();
    intersection as f64 / a.len().max(b.len()) as f64
}

fn path_module_prefix(path: &str) -> String {
    path.split('/').take(2).collect::<Vec<_>>().join("/")
}

fn extension(path: &str) -> &str {
    path.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

fn explain_reasons(
    source: &VaultNode,
    target: &VaultNode,
    source_tags: &[String],
    target_tags: &[String],
) -> String {
    let mut reasons = Vec::new();
    if source_tags.is_empty()
        || target_tags.is_empty()
        || tag_similarity(source_tags, target_tags) == 0.0
    {
        reasons.push("no shared tags".to_string());
    }
    if path_module_prefix(&source.note_path) != path_module_prefix(&target.note_path) {
        reasons.push(format!(
            "cross-module ({} -> {})",
            path_module_prefix(&source.note_path),
            path_module_prefix(&target.note_path)
        ));
    }
    if extension(&source.note_path) != extension(&target.note_path) {
        reasons.push(format!(
            "cross-file-type (.{} -> .{})",
            extension(&source.note_path),
            extension(&target.note_path)
        ));
    }
    if reasons.is_empty() {
        reasons.push("low tag similarity".to_string());
    }
    reasons.join(", ")
}

fn find(parent: &mut HashMap<i64, i64>, id: i64) -> i64 {
    let p = *parent.get(&id).unwrap_or(&id);
    if p == id {
        id
    } else {
        let root = find(parent, p);
        parent.insert(id, root);
        root
    }
}

fn union(parent: &mut HashMap<i64, i64>, a: i64, b: i64) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent.insert(ra, rb);
    }
}

fn compute_token_estimate(
    nodes: usize,
    god_nodes: &[GodNode],
    surprising: &[SurprisingConnection],
    communities: &[CommunityHint],
    questions: &[SuggestedQuestion],
) -> TokenEstimate {
    let raw_tokens = nodes * 400;
    let graph_tokens = 200
        + god_nodes.len() * 50
        + surprising.len() * 30
        + communities.len() * 40
        + questions.len() * 20;
    TokenEstimate {
        raw_tokens,
        graph_tokens,
        reduction_ratio: (raw_tokens as f64 / graph_tokens.max(1) as f64 * 10.0).round() / 10.0,
    }
}

fn generate_markdown_report(
    summary: &str,
    node_count: usize,
    edge_count: usize,
    god_nodes: &[GodNode],
    surprising: &[SurprisingConnection],
    communities: &[CommunityHint],
    questions: &[SuggestedQuestion],
) -> String {
    let mut lines = vec![
        "# Graph Analysis Report".to_string(),
        String::new(),
        format!("- **Nodes:** {node_count} | **Edges:** {edge_count}"),
        String::new(),
    ];
    if !god_nodes.is_empty() {
        lines.push("## God Nodes (Architectural Hubs)".to_string());
        lines.push(String::new());
        for node in god_nodes.iter().take(5) {
            lines.push(format!(
                "- **{}** ({}) — PR={:.3}, in={}, out={}",
                node.title,
                node.path,
                node.page_rank.unwrap_or(0.0),
                node.in_degree,
                node.out_degree
            ));
        }
        lines.push(String::new());
    }
    if !surprising.is_empty() {
        lines.push("## Surprising Connections".to_string());
        lines.push(String::new());
        for edge in surprising.iter().take(5) {
            lines.push(format!(
                "- `{}` -> `{}` ({}) — unexpectedness={:.2}",
                edge.source_path, edge.target_path, edge.edge_type, edge.unexpectedness
            ));
        }
        lines.push(String::new());
    }
    if !communities.is_empty() {
        lines.push("## Community Hints".to_string());
        lines.push(String::new());
        for community in communities {
            lines.push(format!(
                "- **{}** — {} nodes, density={:.2}",
                community.representative_path, community.node_count, community.avg_internal_density
            ));
        }
        lines.push(String::new());
    }
    if !questions.is_empty() {
        lines.push("## Suggested Questions".to_string());
        lines.push(String::new());
        for question in questions {
            lines.push(format!("- {}", question.question));
        }
        lines.push(String::new());
    }
    lines.push("## Summary".to_string());
    lines.push(String::new());
    lines.push(summary.to_string());
    lines.push(String::new());
    lines.join("\n")
}
