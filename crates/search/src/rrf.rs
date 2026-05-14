use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

pub const DEFAULT_RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RrfScored<T> {
    pub item: T,
    pub rrf_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedId {
    pub id: String,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FusedId {
    pub id: String,
    pub rrf_score: f64,
}

pub fn reciprocal_rank_fuse<T, K, F>(
    result_lists: &[Vec<T>],
    key_fn: F,
    k: f64,
) -> Vec<RrfScored<T>>
where
    T: Clone,
    K: Eq + Hash,
    F: Fn(&T) -> K,
{
    let mut scores: HashMap<K, (T, f64)> = HashMap::new();
    let k = if k.is_sign_positive() {
        k
    } else {
        DEFAULT_RRF_K
    };

    for results in result_lists {
        for (index, item) in results.iter().enumerate() {
            let score = 1.0 / (k + index as f64 + 1.0);
            scores
                .entry(key_fn(item))
                .and_modify(|(_, total)| *total += score)
                .or_insert_with(|| (item.clone(), score));
        }
    }

    let mut fused: Vec<_> = scores
        .into_values()
        .map(|(item, rrf_score)| RrfScored { item, rrf_score })
        .collect();
    fused.sort_by(|a, b| b.rrf_score.total_cmp(&a.rrf_score));
    fused
}

pub fn fuse_ranked_ids(result_lists: &[Vec<RankedId>], k: f64) -> Vec<FusedId> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let k = if k.is_sign_positive() {
        k
    } else {
        DEFAULT_RRF_K
    };

    for results in result_lists {
        for item in results {
            let rank = item.rank.max(1) as f64;
            *scores.entry(item.id.clone()).or_insert(0.0) += 1.0 / (k + rank);
        }
    }

    let mut fused: Vec<_> = scores
        .into_iter()
        .map(|(id, rrf_score)| FusedId { id, rrf_score })
        .collect();
    fused.sort_by(|a, b| b.rrf_score.total_cmp(&a.rrf_score));
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_combines_duplicate_keys() {
        let a = vec!["a".to_string(), "b".to_string()];
        let b = vec!["b".to_string(), "c".to_string()];

        let fused = reciprocal_rank_fuse(&[a, b], |item| item.clone(), DEFAULT_RRF_K);

        assert_eq!(fused[0].item, "b");
        assert!(fused[0].rrf_score > fused[1].rrf_score);
    }
}
