use crate::types::is_stopword;
use regex::Regex;
use rusqlite::{Connection, params};
use std::collections::{HashMap, VecDeque};

pub fn levenshtein(a: &str, b: &str) -> usize {
    if a.is_empty() {
        return b.chars().count();
    }
    if b.is_empty() {
        return a.chars().count();
    }

    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, a_ch) in a.iter().enumerate() {
        let mut curr = vec![i + 1];
        for (j, b_ch) in b.iter().enumerate() {
            let cost = if a_ch == b_ch {
                prev[j]
            } else {
                1 + prev[j].min(curr[j]).min(prev[j + 1])
            };
            curr.push(cost);
        }
        prev = curr;
    }
    prev[b.len()]
}

pub fn max_edit_distance(word_length: usize) -> usize {
    if word_length <= 4 {
        1
    } else if word_length <= 12 {
        2
    } else {
        3
    }
}

#[derive(Debug, Clone, Default)]
pub struct FuzzyCache {
    entries: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

impl FuzzyCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    fn get_promoted(&mut self, key: &str) -> Option<Option<String>> {
        let value = self.entries.get(key).cloned()?;
        self.order.retain(|existing| existing != key);
        self.order.push_back(key.to_string());
        Some(value)
    }

    fn insert(&mut self, key: String, value: Option<String>, max_size: usize) {
        if self.entries.contains_key(&key) {
            self.order.retain(|existing| existing != &key);
        } else if self.entries.len() >= max_size {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, value);
    }
}

pub fn extract_words(content: &str) -> Vec<String> {
    let token_re = Regex::new(r"[^\p{L}\p{N}_-]+").expect("valid token regex");
    let mut words: Vec<String> = token_re
        .split(&content.to_lowercase())
        .filter(|word| word.len() >= 3 && !is_stopword(word))
        .map(ToString::to_string)
        .collect();
    words.sort();
    words.dedup();
    words
}

pub fn extract_and_store_vocabulary(
    conn: &Connection,
    content: &str,
    fuzzy_cache: &mut FuzzyCache,
) -> rusqlite::Result<usize> {
    let words = extract_words(content);
    let mut inserted = 0;
    for word in words {
        inserted += conn.execute(
            "INSERT OR IGNORE INTO vocabulary (word) VALUES (?1)",
            params![word],
        )?;
    }
    if inserted > 0 {
        fuzzy_cache.clear();
    }
    Ok(inserted)
}

pub fn fuzzy_correct(
    conn: &Connection,
    fuzzy_cache: &mut FuzzyCache,
    max_cache_size: usize,
    query: &str,
) -> rusqlite::Result<Option<String>> {
    let word = query.to_lowercase().trim().to_string();
    if word.len() < 3 {
        return Ok(None);
    }

    if let Some(cached) = fuzzy_cache.get_promoted(&word) {
        return Ok(cached);
    }

    let max_dist = max_edit_distance(word.len());
    let min_len = word.len().saturating_sub(max_dist);
    let max_len = word.len() + max_dist;
    let mut stmt =
        conn.prepare("SELECT word FROM vocabulary WHERE length(word) BETWEEN ?1 AND ?2")?;
    let candidates = stmt
        .query_map(params![min_len as i64, max_len as i64], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut best_word: Option<String> = None;
    let mut best_dist = max_dist + 1;
    let mut exact_match = false;
    for candidate in candidates {
        if candidate == word {
            exact_match = true;
            break;
        }
        let dist = levenshtein(&word, &candidate);
        if dist < best_dist {
            best_dist = dist;
            best_word = Some(candidate);
        }
    }

    let result = if exact_match {
        None
    } else if best_dist <= max_dist {
        best_word
    } else {
        None
    };
    fuzzy_cache.insert(word, result.clone(), max_cache_size);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_counts_edits() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(max_edit_distance(4), 1);
        assert_eq!(max_edit_distance(12), 2);
        assert_eq!(max_edit_distance(13), 3);
    }

    #[test]
    fn fuzzy_correct_returns_near_vocabulary_word() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE vocabulary (word TEXT PRIMARY KEY)", [])
            .unwrap();
        let mut cache = FuzzyCache::new();
        extract_and_store_vocabulary(&conn, "database connection failure", &mut cache).unwrap();
        assert_eq!(
            fuzzy_correct(&conn, &mut cache, 256, "databse").unwrap(),
            Some("database".to_string())
        );
        assert_eq!(
            fuzzy_correct(&conn, &mut cache, 256, "database").unwrap(),
            None
        );
    }
}
