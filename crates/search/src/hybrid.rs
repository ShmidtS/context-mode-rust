use crate::rrf::DEFAULT_RRF_K;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HybridRankedId {
    pub doc_id: String,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HybridScore {
    pub doc_id: String,
    pub score: f64,
}

pub fn rrf_score(rank: usize, k: f64) -> f64 {
    let k = if k.is_sign_positive() {
        k
    } else {
        DEFAULT_RRF_K
    };
    1.0 / (k + rank.max(1) as f64)
}

pub fn hybrid_rrf(
    fts5: &[HybridRankedId],
    secondary: Option<&[HybridRankedId]>,
    alpha: f64,
    k: f64,
) -> Vec<HybridScore> {
    let alpha = alpha.clamp(0.0, 1.0);
    let bm25_weight = 1.0 - alpha;
    let secondary_weight = if secondary.is_some() { alpha } else { 0.0 };
    let mut scores: HashMap<String, f64> = HashMap::new();

    for item in fts5 {
        *scores.entry(item.doc_id.clone()).or_insert(0.0) += bm25_weight * rrf_score(item.rank, k);
    }

    if let Some(secondary) = secondary {
        for item in secondary {
            *scores.entry(item.doc_id.clone()).or_insert(0.0) +=
                secondary_weight * rrf_score(item.rank, k);
        }
    }

    let mut fused = scores
        .into_iter()
        .map(|(doc_id, score)| HybridScore { doc_id, score })
        .collect::<Vec<_>>();
    fused.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.doc_id.cmp(&b.doc_id))
    });
    fused
}

pub fn hybrid_rrf_pairs(
    fts5: &[(String, usize)],
    secondary: Option<&[(String, usize)]>,
    alpha: f64,
    k: f64,
) -> Vec<HybridScore> {
    let fts5 = fts5
        .iter()
        .map(|(doc_id, rank)| HybridRankedId {
            doc_id: doc_id.clone(),
            rank: *rank,
        })
        .collect::<Vec<_>>();
    let secondary = secondary.map(|items| {
        items
            .iter()
            .map(|(doc_id, rank)| HybridRankedId {
                doc_id: doc_id.clone(),
                rank: *rank,
            })
            .collect::<Vec<_>>()
    });

    hybrid_rrf(&fts5, secondary.as_deref(), alpha, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_use_rrf_formula() {
        assert_eq!(rrf_score(1, 60.0), 1.0 / 61.0);
    }

    #[test]
    fn accepts_empty_secondary_ranker() {
        let scores = hybrid_rrf_pairs(
            &[("a".into(), 1), ("b".into(), 2)],
            None,
            0.7,
            DEFAULT_RRF_K,
        );

        assert_eq!(scores[0].doc_id, "a");
        assert!(scores[0].score > scores[1].score);
    }

    #[test]
    fn combines_secondary_with_alpha() {
        let scores = hybrid_rrf_pairs(
            &[("a".into(), 1), ("b".into(), 2)],
            Some(&[("b".into(), 1)]),
            0.7,
            DEFAULT_RRF_K,
        );

        assert_eq!(scores[0].doc_id, "b");
    }
}
