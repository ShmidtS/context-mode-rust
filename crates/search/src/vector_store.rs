use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("embedding must not be empty")]
    EmptyEmbedding,
    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorRecord {
    pub id: String,
    pub embedding: Vec<f32>,
    pub model_name: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorSearchResult {
    pub id: String,
    pub similarity: f32,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Default, Clone)]
pub struct VectorStore {
    records: Vec<VectorRecord>,
    dimensions: Option<usize>,
}

impl VectorStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_records(records: Vec<VectorRecord>) -> Result<Self, VectorStoreError> {
        let mut store = Self::new();
        for record in records {
            store.insert(record)?;
        }
        Ok(store)
    }

    pub fn insert(&mut self, record: VectorRecord) -> Result<(), VectorStoreError> {
        validate_embedding(&record.embedding)?;
        self.validate_dimensions(record.embedding.len())?;
        self.records.push(record);
        Ok(())
    }

    pub fn upsert(&mut self, record: VectorRecord) -> Result<(), VectorStoreError> {
        validate_embedding(&record.embedding)?;
        self.validate_dimensions(record.embedding.len())?;

        if let Some(existing) = self.records.iter_mut().find(|item| item.id == record.id) {
            *existing = record;
        } else {
            self.records.push(record);
        }
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.records.len();
        self.records.retain(|record| record.id != id);
        before != self.records.len()
    }

    pub fn clear(&mut self) {
        self.records.clear();
        self.dimensions = None;
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[VectorRecord] {
        &self.records
    }

    pub fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError> {
        validate_embedding(query_embedding)?;
        self.validate_query_dimensions(query_embedding.len())?;

        let mut scored: Vec<(usize, f32)> = self
            .records
            .iter()
            .enumerate()
            .filter_map(|(i, record)| {
                let similarity = cosine_similarity(query_embedding, &record.embedding);
                (similarity >= min_similarity).then_some((i, similarity))
            })
            .collect();

        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(limit);

        let results: Vec<VectorSearchResult> = scored
            .into_iter()
            .map(|(i, similarity)| {
                let record = &self.records[i];
                VectorSearchResult {
                    id: record.id.clone(),
                    similarity,
                    metadata: record.metadata.clone(),
                }
            })
            .collect();
        Ok(results)
    }

    fn validate_dimensions(&mut self, actual: usize) -> Result<(), VectorStoreError> {
        match self.dimensions {
            Some(expected) if expected != actual => {
                Err(VectorStoreError::DimensionMismatch { expected, actual })
            }
            Some(_) => Ok(()),
            None => {
                self.dimensions = Some(actual);
                Ok(())
            }
        }
    }

    fn validate_query_dimensions(&self, actual: usize) -> Result<(), VectorStoreError> {
        match self.dimensions {
            Some(expected) if expected != actual => {
                Err(VectorStoreError::DimensionMismatch { expected, actual })
            }
            _ => Ok(()),
        }
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;

    for (left, right) in a.iter().zip(b) {
        dot += left * right;
        norm_a += left * left;
        norm_b += right * right;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

fn validate_embedding(embedding: &[f32]) -> Result<(), VectorStoreError> {
    if embedding.is_empty() {
        Err(VectorStoreError::EmptyEmbedding)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_ranks_exact_match_first() {
        let mut store = VectorStore::new();
        store
            .insert(VectorRecord {
                id: "a".into(),
                embedding: vec![1.0, 0.0],
                model_name: None,
                metadata: HashMap::new(),
            })
            .unwrap();
        store
            .insert(VectorRecord {
                id: "b".into(),
                embedding: vec![0.0, 1.0],
                model_name: None,
                metadata: HashMap::new(),
            })
            .unwrap();

        let results = store.search_similar(&[1.0, 0.0], 10, 0.0).unwrap();

        assert_eq!(results[0].id, "a");
        assert_eq!(results[0].similarity, 1.0);
    }
}
