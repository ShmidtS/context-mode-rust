use std::collections::HashMap;

use crate::types::{VaultEdge, VaultNode, VaultTag};

/// In-memory vault graph storing nodes, edges and tags.
#[derive(Debug, Clone, Default)]
pub struct VaultGraph {
    nodes: HashMap<i64, VaultNode>,
    edges: Vec<VaultEdge>,
    tags: HashMap<i64, Vec<VaultTag>>,
    next_id: i64,
}

impl VaultGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, mut node: VaultNode) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        node.id = id;
        self.nodes.insert(id, node);
        id
    }

    pub fn get_node(&self, id: i64) -> Option<&VaultNode> {
        self.nodes.get(&id)
    }

    pub fn add_edge(&mut self, mut edge: VaultEdge) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        edge.id = id;
        if let Some(source) = self.nodes.get_mut(&edge.source_id) {
            source.out_degree += 1;
        }
        if let Some(target_id) = edge.target_id {
            if let Some(target) = self.nodes.get_mut(&target_id) {
                target.in_degree += 1;
            }
        }
        self.edges.push(edge);
        id
    }

    pub fn edges_from(&self, source_id: i64) -> Vec<&VaultEdge> {
        self.edges
            .iter()
            .filter(|e| e.source_id == source_id)
            .collect()
    }

    pub fn edges_to(&self, target_id: i64) -> Vec<&VaultEdge> {
        self.edges
            .iter()
            .filter(|e| e.target_id == Some(target_id))
            .collect()
    }

    pub fn add_tag(&mut self, node_id: i64, tag: VaultTag) {
        self.tags.entry(node_id).or_default().push(tag);
    }

    pub fn tags_for(&self, node_id: i64) -> Vec<&VaultTag> {
        self.tags
            .get(&node_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn all_nodes(&self) -> Vec<&VaultNode> {
        self.nodes.values().collect()
    }

    pub fn all_edges(&self) -> &[VaultEdge] {
        &self.edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::VaultConfidence;

    fn make_node(vault_path: &str, note_path: &str, title: &str) -> VaultNode {
        VaultNode {
            id: 0,
            vault_path: vault_path.into(),
            note_path: note_path.into(),
            title: title.into(),
            frontmatter: None,
            content_hash: "abc".into(),
            file_mtime: 0.0,
            out_degree: 0,
            in_degree: 0,
            source_id: None,
            indexed_at: "2024-01-01".into(),
            source_type: "vault".into(),
            connector_meta: None,
        }
    }

    #[test]
    fn test_add_and_get_node() {
        let mut graph = VaultGraph::new();
        let node = make_node("/vault", "a.md", "Note A");
        let id = graph.add_node(node);
        assert_eq!(graph.get_node(id).unwrap().title, "Note A");
    }

    #[test]
    fn test_edges_update_degrees() {
        let mut graph = VaultGraph::new();
        let a = graph.add_node(make_node("/vault", "a.md", "A"));
        let b = graph.add_node(make_node("/vault", "b.md", "B"));

        let edge = VaultEdge {
            id: 0,
            source_id: a,
            target_id: Some(b),
            target_name: "B".into(),
            alias: None,
            line_number: None,
            context: None,
            edge_type: "wikilink".into(),
            confidence: VaultConfidence::EXTRACTED,
        };
        graph.add_edge(edge);

        assert_eq!(graph.get_node(a).unwrap().out_degree, 1);
        assert_eq!(graph.get_node(b).unwrap().in_degree, 1);
        assert_eq!(graph.edges_from(a).len(), 1);
        assert_eq!(graph.edges_to(b).len(), 1);
    }

    #[test]
    fn test_tags() {
        let mut graph = VaultGraph::new();
        let a = graph.add_node(make_node("/vault", "a.md", "A"));
        graph.add_tag(a, VaultTag { id: 1, tag: "rust".into() });
        graph.add_tag(a, VaultTag { id: 2, tag: "code".into() });
        let tags = graph.tags_for(a);
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t.tag == "rust"));
    }
}
