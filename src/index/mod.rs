use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexItem {
    pub file: String,
    pub category: String,
    pub keywords: Vec<String>,
    pub path: String,
    pub hash: String,
    #[serde(default)]
    pub indexed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub item: IndexItem,
    pub score: f64,
    pub match_type: String, // "exact", "partial", "suggested"
}

pub struct DocumentIndex {
    file_path: PathBuf,
    items: Vec<IndexItem>,
    dirty: bool,
}

impl DocumentIndex {
    pub fn load<P: AsRef<Path>>(index_path: P) -> Self {
        let file_path = index_path.as_ref().to_path_buf();
        let items = if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                serde_json::from_str(&content).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Self {
            file_path,
            items,
            dirty: false,
        }
    }

    pub fn compute_file_hash<P: AsRef<Path>>(path: P) -> Option<String> {
        let mut file = File::open(path).ok()?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).ok()?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Some(format!("{:x}", hasher.finalize()))
    }

    pub fn find_by_hash(&self, hash: &str) -> Option<&IndexItem> {
        self.items.iter().find(|item| item.hash == hash)
    }

    #[allow(dead_code)]
    pub fn find_by_path(&self, path: &str) -> Option<&IndexItem> {
        self.items.iter().find(|item| item.path == path)
    }

    pub fn upsert(&mut self, item: IndexItem) {
        if let Some(pos) = self.items.iter().position(|i| i.hash == item.hash || i.path == item.path) {
            self.items[pos] = item;
        } else {
            self.items.push(item);
        }
        self.dirty = true;
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if !self.dirty {
            return Ok(());
        }

        let json = serde_json::to_string_pretty(&self.items)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let temp_path = format!("{}.tmp", self.file_path.display());
        std::fs::write(&temp_path, json)?;
        std::fs::rename(temp_path, &self.file_path)?;

        self.dirty = false;
        Ok(())
    }

    pub fn all(&self) -> &[IndexItem] {
        &self.items
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_clean = query.trim().to_lowercase();
        if query_clean.is_empty() {
            return Vec::new();
        }

        let query_terms: Vec<&str> = query_clean.split_whitespace().collect();
        let mut exact_matches = Vec::new();
        let mut fuzzy_matches = Vec::new();

        for item in &self.items {
            let cat_lower = item.category.to_lowercase();
            let file_lower = item.file.to_lowercase();
            let path_lower = item.path.to_lowercase();
            let kws_lower: Vec<String> = item.keywords.iter().map(|k| k.to_lowercase()).collect();

            let mut score: f64 = 0.0;
            let mut match_type = "partial";

            // Direct substring matches
            if cat_lower.contains(&query_clean) {
                score = score.max(1.0);
                match_type = "exact";
            }
            if file_lower.contains(&query_clean) {
                score = score.max(0.95);
                match_type = "exact";
            }
            if kws_lower.iter().any(|k| k.contains(&query_clean)) {
                score = score.max(0.9);
                match_type = "exact";
            }
            if path_lower.contains(&query_clean) {
                score = score.max(0.85);
            }

            // Term hit scoring
            let term_hits = query_terms.iter().filter(|&t| {
                cat_lower.contains(t)
                    || file_lower.contains(t)
                    || kws_lower.iter().any(|k| k.contains(t))
            }).count();

            if term_hits > 0 {
                let term_score = 0.7 + (term_hits as f64 / query_terms.len() as f64) * 0.25;
                score = score.max(term_score);
            }

            if score > 0.0 {
                exact_matches.push(SearchResult {
                    item: item.clone(),
                    score,
                    match_type: match_type.to_string(),
                });
            } else {
                // Fuzzy similarity
                let best_sim = kws_lower.iter().map(|k| strsim::jaro_winkler(&query_clean, k))
                    .chain(std::iter::once(strsim::jaro_winkler(&query_clean, &file_lower)))
                    .chain(std::iter::once(strsim::jaro_winkler(&query_clean, &cat_lower)))
                    .fold(0.0f64, |acc, v| acc.max(v));

                if best_sim >= 0.70 {
                    fuzzy_matches.push(SearchResult {
                        item: item.clone(),
                        score: best_sim * 0.7,
                        match_type: "suggested".to_string(),
                    });
                }
            }
        }

        if !exact_matches.is_empty() {
            exact_matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            exact_matches.truncate(limit);
            exact_matches
        } else {
            fuzzy_matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            fuzzy_matches.truncate(limit);
            fuzzy_matches
        }
    }
}
