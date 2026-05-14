use crate::types::{SearchResult, is_stopword};

pub fn find_all_positions(text: &str, term: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    if term.is_empty() {
        return positions;
    }

    let mut start = 0;
    while let Some(idx) = text[start..].find(term) {
        let absolute = start + idx;
        positions.push(absolute);
        start = absolute + 1;
        if start >= text.len() {
            break;
        }
    }
    positions
}

pub fn count_adjacent_pairs(position_lists: &[Vec<usize>], terms: &[String], gap: usize) -> usize {
    if position_lists.len() < 2 || terms.len() < 2 {
        return 0;
    }

    let mut total = 0;
    let pairs = position_lists.len().min(terms.len()) - 1;
    for i in 0..pairs {
        let left = &position_lists[i];
        let right = &position_lists[i + 1];
        let left_len = terms[i].len();
        let mut j = 0;
        for p in left {
            let min_start = p + left_len;
            let max_start = min_start + gap;
            while j < right.len() && right[j] < min_start {
                j += 1;
            }
            if j < right.len() && right[j] <= max_start {
                total += 1;
                j += 1;
            }
        }
    }
    total
}

pub fn find_min_span(position_lists: &[Vec<usize>]) -> Option<usize> {
    if position_lists.is_empty() || position_lists.iter().any(Vec::is_empty) {
        return None;
    }
    if position_lists.len() == 1 {
        return Some(0);
    }

    let sorted: Vec<Vec<usize>> = position_lists
        .iter()
        .map(|positions| {
            let mut positions = positions.clone();
            positions.sort_unstable();
            positions
        })
        .collect();
    let mut ptrs = vec![0usize; sorted.len()];
    let mut min_span = usize::MAX;

    loop {
        let mut cur_min = usize::MAX;
        let mut cur_max = 0usize;
        let mut min_idx = 0usize;

        for (i, positions) in sorted.iter().enumerate() {
            let val = positions[ptrs[i]];
            if val < cur_min {
                cur_min = val;
                min_idx = i;
            }
            if val > cur_max {
                cur_max = val;
            }
        }

        min_span = min_span.min(cur_max - cur_min);
        ptrs[min_idx] += 1;
        if ptrs[min_idx] >= sorted[min_idx].len() {
            break;
        }
    }

    Some(min_span)
}

pub fn apply_proximity_reranking(results: &[SearchResult], query: &str) -> Vec<SearchResult> {
    let all_terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .map(ToString::to_string)
        .collect();
    let filtered: Vec<String> = all_terms
        .iter()
        .filter(|w| !is_stopword(w))
        .cloned()
        .collect();
    let terms = if filtered.is_empty() {
        all_terms
    } else {
        filtered
    };

    let mut scored: Vec<(SearchResult, f64)> = results
        .iter()
        .cloned()
        .map(|result| {
            if terms.is_empty() {
                return (result, 0.0);
            }

            let title_lower = result.title.to_lowercase();
            let title_hits = terms
                .iter()
                .filter(|term| title_lower.contains(term.as_str()))
                .count();
            let title_weight = if result.content_type == context_mode_core::ContentType::Code {
                0.6
            } else {
                0.3
            };
            let title_boost = if title_hits > 0 {
                title_weight * (title_hits as f64 / terms.len() as f64)
            } else {
                0.0
            };

            let mut proximity_boost = 0.0;
            let mut phrase_boost = 0.0;
            if terms.len() >= 2 {
                let content = result.content.to_lowercase();
                let positions: Vec<Vec<usize>> = terms
                    .iter()
                    .map(|term| find_all_positions(&content, term))
                    .collect();
                if !positions.iter().any(Vec::is_empty) {
                    if let Some(min_span) = find_min_span(&positions) {
                        proximity_boost =
                            1.0 / (1.0 + min_span as f64 / content.len().max(1) as f64);
                    }
                    let adjacent_pairs = count_adjacent_pairs(&positions, &terms, 30);
                    phrase_boost = 0.5 * (adjacent_pairs as f64 / 4.0).min(1.0);
                }
            }

            (result, title_boost + proximity_boost + phrase_boost)
        })
        .collect();

    scored.sort_by(|(a_result, a_boost), (b_result, b_boost)| {
        b_boost
            .partial_cmp(a_boost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a_result
                    .rank
                    .partial_cmp(&b_result.rank)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    scored.into_iter().map(|(result, _)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_pairs_consumes_right_position_once() {
        let positions = vec![vec![0, 4], vec![8]];
        let terms = vec!["foo".to_string(), "bar".to_string()];
        assert_eq!(count_adjacent_pairs(&positions, &terms, 30), 1);
    }

    #[test]
    fn min_span_covers_one_position_from_each_list() {
        assert_eq!(
            find_min_span(&[vec![1, 50], vec![10, 55], vec![20, 60]]),
            Some(10)
        );
    }
}
