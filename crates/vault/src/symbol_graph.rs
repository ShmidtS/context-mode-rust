use crate::ast_parser::{AstResult, Symbol, cross_reference_map};
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolNode {
    pub id: String,
    pub file_path: String,
    pub symbol: Symbol,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum EdgeKind {
    Import,
    SameName,
    Reference,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SymbolGraph {
    pub nodes: Vec<SymbolNode>,
    pub edges: Vec<SymbolEdge>,
    pub file_index: HashMap<String, Vec<usize>>,
}

pub fn build_symbol_graph(results: &HashMap<String, AstResult>) -> SymbolGraph {
    let mut graph = SymbolGraph::default();
    let mut id_counter = 0usize;

    for (path, result) in results {
        let mut indices = Vec::new();
        for sym in &result.symbols {
            let id = format!("{}#{}", path, id_counter);
            id_counter += 1;
            indices.push(graph.nodes.len());
            graph.nodes.push(SymbolNode {
                id: id.clone(),
                file_path: path.clone(),
                symbol: sym.clone(),
            });
        }
        graph.file_index.insert(path.clone(), indices);
    }

    let refs = cross_reference_map(results);
    for (from_path, to_paths) in &refs {
        for to_path in to_paths {
            if let Some(from_indices) = graph.file_index.get(from_path) {
                if let Some(to_indices) = graph.file_index.get(to_path) {
                    for &fi in from_indices {
                        for &ti in to_indices {
                            graph.edges.push(SymbolEdge {
                                from: graph.nodes[fi].id.clone(),
                                to: graph.nodes[ti].id.clone(),
                                kind: EdgeKind::Reference,
                            });
                        }
                    }
                }
            }
        }
    }

    graph
}

pub fn find_symbol_nodes<'a>(graph: &'a SymbolGraph, name: &str) -> Vec<&'a SymbolNode> {
    graph
        .nodes
        .iter()
        .filter(|n| n.symbol.name == name)
        .collect()
}

pub fn neighbors<'a>(graph: &'a SymbolGraph, node_id: &str) -> Vec<&'a SymbolNode> {
    let mut ids = std::collections::HashSet::new();
    for edge in &graph.edges {
        if edge.from == node_id {
            ids.insert(edge.to.clone());
        }
        if edge.to == node_id {
            ids.insert(edge.from.clone());
        }
    }
    graph.nodes.iter().filter(|n| ids.contains(&n.id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast_parser::parse_file;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn graph_builds() {
        let mut results = HashMap::new();
        results.insert(
            "a.rs".into(),
            parse_file(Path::new("a.rs"), "pub fn foo() {}\nuse b;"),
        );
        results.insert(
            "b.rs".into(),
            parse_file(Path::new("b.rs"), "pub fn foo() {}"),
        );
        let graph = build_symbol_graph(&results);
        assert!(!graph.nodes.is_empty());
        assert!(!graph.edges.is_empty());
    }

    #[test]
    fn find_nodes() {
        let mut results = HashMap::new();
        results.insert("a.rs".into(), parse_file(Path::new("a.rs"), "fn foo() {}"));
        let graph = build_symbol_graph(&results);
        let found = find_symbol_nodes(&graph, "foo");
        assert_eq!(found.len(), 1);
    }
}
