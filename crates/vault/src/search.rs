use crate::graph_store::{GraphStoreError, VaultGraphStore};
use crate::types::{GraphSearchResult, TextSearchResult, VaultEdge};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("graph store error: {0}")]
    Store(#[from] GraphStoreError),
}

pub type Result<T> = std::result::Result<T, SearchError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FusionSearchOpts {
    pub graph_boost: Option<f64>,
    pub max_hops: Option<usize>,
}

pub struct VaultGraphSearch<'a> {
    store: &'a VaultGraphStore,
}

impl<'a> VaultGraphSearch<'a> {
    pub fn new(store: &'a VaultGraphStore) -> Self {
        Self { store }
    }

    pub fn page_rank(&self) -> Result<HashMap<i64, f64>> {
        let edges = self.store.get_all_edges()?;
        let mut node_ids: HashSet<i64> = self.store.get_all_node_ids()?.into_iter().collect();
        for edge in &edges {
            node_ids.insert(edge.source_id);
            if let Some(target_id) = edge.target_id {
                node_ids.insert(target_id);
            }
        }
        let n = node_ids.len();
        if n == 0 {
            return Ok(HashMap::new());
        }
        let damping = 0.85;
        let mut out_degree: HashMap<i64, usize> = HashMap::new();
        let mut reverse: HashMap<i64, Vec<i64>> = HashMap::new();
        for edge in &edges {
            if let Some(target_id) = edge.target_id {
                *out_degree.entry(edge.source_id).or_default() += 1;
                reverse.entry(target_id).or_default().push(edge.source_id);
            }
        }
        let mut rank: HashMap<i64, f64> = node_ids.iter().map(|id| (*id, 1.0 / n as f64)).collect();
        for _ in 0..20 {
            let dangling_sum: f64 = node_ids
                .iter()
                .filter(|id| out_degree.get(id).copied().unwrap_or(0) == 0)
                .map(|id| rank.get(id).copied().unwrap_or(0.0))
                .sum();
            let mut next = HashMap::new();
            for id in &node_ids {
                let mut sum = dangling_sum / n as f64;
                if let Some(sources) = reverse.get(id) {
                    for src in sources {
                        sum += rank.get(src).copied().unwrap_or(0.0)
                            / out_degree.get(src).copied().unwrap_or(1) as f64;
                    }
                }
                next.insert(*id, (1.0 - damping) / n as f64 + damping * sum);
            }
            rank = next;
        }
        Ok(rank)
    }

    pub fn neighbors(
        &self,
        node_id: i64,
        max_hops: usize,
        edge_type: Option<&str>,
    ) -> Result<Vec<GraphSearchResult>> {
        let hops = max_hops.min(3);
        let adjacency = build_adjacency(&self.store.get_all_edges()?, edge_type);
        let mut visited = HashSet::from([node_id]);
        let mut frontier = VecDeque::from([(node_id, 0usize)]);
        let mut results = Vec::new();

        while let Some((current, distance)) = frontier.pop_front() {
            if distance >= hops {
                continue;
            }
            for neighbor_id in adjacency.get(&current).cloned().unwrap_or_default() {
                if !visited.insert(neighbor_id) {
                    continue;
                }
                let Some(node) = self.store.get_node_by_id(neighbor_id)? else {
                    continue;
                };
                let hop_distance = distance + 1;
                frontier.push_back((neighbor_id, hop_distance));
                results.push(self.result_for_node(
                    node,
                    Some(hop_distance),
                    "bfs",
                    None,
                    None,
                    None,
                )?);
            }
        }
        Ok(results)
    }

    pub fn backlinks(
        &self,
        node_id: i64,
        edge_type: Option<&str>,
    ) -> Result<Vec<GraphSearchResult>> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        for edge in self.store.get_edges_by_target(node_id)? {
            if edge_type.is_some_and(|t| edge.edge_type != t) || !seen.insert(edge.source_id) {
                continue;
            }
            if let Some(node) = self.store.get_node_by_id(edge.source_id)? {
                results.push(self.result_for_node(
                    node,
                    Some(1),
                    "backlinks",
                    edge.context,
                    None,
                    None,
                )?);
            }
        }
        Ok(results)
    }

    pub fn tag_cluster(&self, tag: &str) -> Result<Vec<GraphSearchResult>> {
        let tagged = self.store.get_nodes_by_tag_hierarchy(tag)?;
        let adjacency = build_adjacency(&self.store.get_all_edges()?, None);
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for node in &tagged {
            seen.insert(node.id);
            results.push(self.result_for_node(
                node.clone(),
                Some(0),
                "tag-cluster",
                None,
                None,
                None,
            )?);
        }
        for node in tagged {
            for neighbor_id in adjacency.get(&node.id).cloned().unwrap_or_default() {
                if !seen.insert(neighbor_id) {
                    continue;
                }
                if let Some(neighbor) = self.store.get_node_by_id(neighbor_id)? {
                    results.push(self.result_for_node(
                        neighbor,
                        Some(1),
                        "tag-cluster",
                        None,
                        None,
                        None,
                    )?);
                }
            }
        }
        Ok(results)
    }

    pub fn fusion_search(
        &self,
        query: &str,
        text_results: &[TextSearchResult],
        opts: Option<FusionSearchOpts>,
    ) -> Result<Vec<GraphSearchResult>> {
        let graph_boost = opts.as_ref().and_then(|o| o.graph_boost).unwrap_or(2.0);
        let max_hops = opts.as_ref().and_then(|o| o.max_hops).unwrap_or(2).min(3);
        let mut scores: HashMap<i64, (f64, Option<usize>, Option<usize>)> = HashMap::new();
        let mut seeds = Vec::new();
        for (rank, result) in text_results.iter().take(5).enumerate() {
            if let Some(node) = self.find_node_by_text_result(result)? {
                seeds.push(node.id);
                let score = 1.0 / (60.0 + rank as f64);
                scores
                    .entry(node.id)
                    .and_modify(|e| {
                        e.0 += score;
                        e.1 = Some(rank);
                    })
                    .or_insert((score, Some(rank), Some(0)));
            }
        }
        if seeds.is_empty() {
            for node in self.store.search_nodes(query)? {
                seeds.push(node.id);
                scores.insert(node.id, (1.0, None, Some(0)));
            }
        }
        let adjacency = build_adjacency(&self.store.get_all_edges()?, None);
        let mut visited: HashSet<i64> = seeds.iter().copied().collect();
        let mut frontier: VecDeque<(i64, usize)> = seeds.into_iter().map(|id| (id, 0)).collect();
        while let Some((id, distance)) = frontier.pop_front() {
            if distance >= max_hops {
                continue;
            }
            for neighbor in adjacency.get(&id).cloned().unwrap_or_default() {
                if !visited.insert(neighbor) {
                    continue;
                }
                let hop = distance + 1;
                frontier.push_back((neighbor, hop));
                let graph_score = graph_boost * (1.0 / (60.0 + hop as f64));
                scores
                    .entry(neighbor)
                    .and_modify(|e| {
                        e.0 += graph_score;
                        if e.2.is_none_or(|old| hop < old) {
                            e.2 = Some(hop);
                        }
                    })
                    .or_insert((graph_score, None, Some(hop)));
            }
        }
        let pr = self.page_rank()?;
        let mut entries: Vec<_> = scores.into_iter().collect();
        entries.sort_by(|a, b| {
            b.1.0
                .partial_cmp(&a.1.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut out = Vec::new();
        for (id, (score, text_rank, hop)) in entries {
            if let Some(node) = self.store.get_node_by_id(id)? {
                let mut result = self.result_for_node(
                    node,
                    hop,
                    "rrf-graph",
                    None,
                    Some(score),
                    pr.get(&id).copied(),
                )?;
                result.text_rank = text_rank;
                out.push(result);
            }
        }
        Ok(out)
    }

    fn find_node_by_text_result(
        &self,
        result: &TextSearchResult,
    ) -> Result<Option<crate::types::VaultNode>> {
        if let Some(node) = self.store.get_node_by_note_path(&result.source)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.store.get_node_by_title(&result.title)? {
            return Ok(Some(node));
        }
        let pattern = format!("%{}%", result.title);
        if let Some((id, _)) = self.store.find_node_by_title_like(&pattern)? {
            return self.store.get_node_by_id(id).map_err(Into::into);
        }
        Ok(None)
    }

    fn result_for_node(
        &self,
        node: crate::types::VaultNode,
        hop_distance: Option<usize>,
        layer: &str,
        snippet: Option<String>,
        fusion_score: Option<f64>,
        page_rank: Option<f64>,
    ) -> Result<GraphSearchResult> {
        let tags = self
            .store
            .get_tags_by_node(node.id)?
            .into_iter()
            .map(|t| t.tag)
            .collect();
        let frontmatter = node
            .frontmatter
            .as_deref()
            .and_then(parse_frontmatter_strings);
        Ok(GraphSearchResult {
            id: node.id,
            title: node.title,
            path: node.note_path,
            hop_distance,
            backlink_count: node.in_degree,
            tags,
            frontmatter,
            snippet,
            fusion_score,
            page_rank,
            text_rank: None,
            match_layer: layer.to_string(),
            origin: "vault-graph".to_string(),
        })
    }
}

fn build_adjacency(edges: &[VaultEdge], edge_type: Option<&str>) -> HashMap<i64, Vec<i64>> {
    let mut adj = HashMap::new();
    for edge in edges {
        if edge_type.is_some_and(|t| edge.edge_type != t) {
            continue;
        }
        if let Some(target_id) = edge.target_id {
            adj.entry(edge.source_id)
                .or_insert_with(Vec::new)
                .push(target_id);
        }
    }
    adj
}

fn parse_frontmatter_strings(raw: &str) -> Option<HashMap<String, String>> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let obj = value.as_object()?;
    Some(
        obj.iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect(),
    )
}
