use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusVocabulary {
    pub total_docs: usize,
    pub doc_frequencies: HashMap<String, u32>,
}

impl Default for CorpusVocabulary {
    fn default() -> Self {
        Self {
            total_docs: 0,
            doc_frequencies: HashMap::new(),
        }
    }
}

pub struct TfidfEngine {
    vocab: CorpusVocabulary,
    stopwords: HashSet<&'static str>,
    short_acronym_allowlist: HashSet<&'static str>,
    storage_path: Option<PathBuf>,
    dirty: bool,
}

impl TfidfEngine {
    pub fn new(storage_path: Option<PathBuf>) -> Self {
        let stopwords = [
            // English Grammar, Articles & Prepositions
            "the", "and", "this", "that", "with", "from", "for", "have", "been", "they",
            "will", "would", "there", "their", "what", "about", "which", "when", "make",
            "like", "time", "just", "know", "take", "people", "into", "year", "your",
            "good", "some", "could", "them", "other", "than", "then", "now", "look",
            "only", "come", "its", "over", "think", "also", "back", "after", "use",
            "two", "how", "our", "work", "first", "well", "way", "even", "new", "want",
            "because", "any", "these", "give", "day", "most", "us", "are", "were", "was",
            "has", "had", "can", "may", "page", "total", "date", "document", "file",
            "using", "used", "present", "same", "before", "getting", "based", "each",
            "such", "more", "very", "here", "where", "while", "should", "must", "shall",
            "between", "through", "under", "above", "below", "within", "without", "both",
            "again", "further", "once", "down", "during", "behind", "around", "among",
            "since", "still", "until", "upon", "therefore", "across", "along", "already",
            // Pronouns & Possessives
            "you", "your", "yours", "yourself", "yourselves", "he", "him", "his", "himself",
            "she", "her", "hers", "herself", "they", "them", "their", "theirs", "we", "us",
            "our", "ours", "who", "whom", "whose", "which", "what", "someone", "something",
            // Conversational & Contraction noise
            "don", "doesn", "didn", "hasn", "haven", "hadn", "won", "wouldn", "shan",
            "shouldn", "cannot", "couldn", "mustn", "needn", "ain", "aren", "isn",
            "wasn", "weren", "gonna", "wanna", "gotta", "yeah", "yep", "nope", "okay",
            "alright", "sure", "maybe", "please", "thank", "thanks", "hello", "hey",
            "listen", "tell", "told", "talk", "came", "went", "gone", "seem", "thing",
            "things", "stuff", "guy", "guys", "man", "woman", "boy", "girl", "sir",
            // German stopwords
            "der", "die", "das", "den", "dem", "des", "ein", "eine", "einer", "eines",
            "einem", "einen", "und", "oder", "aber", "mit", "von", "zu", "im", "in",
            "am", "an", "auf", "aus", "bei", "nach", "uber", "über", "unter", "vor",
            "zwischen", "ich", "du", "er", "sie", "es", "wir", "ihr", "mich", "dich",
            "ihn", "uns", "euch", "ihnen", "sein", "seine", "nicht", "auch", "so",
            "wie", "wenn", "dass", "da", "hier", "dort", "ja", "nein", "doch", "noch",
            "schon", "nur", "immer", "wieder", "sehr", "etwas", "nichts", "alles",
            // Extensions & code noise
            "txt", "pdf", "docx", "png", "jpg", "jpeg", "webp", "md", "json", "csv",
            "self", "true", "false", "none", "null", "return", "import", "class", "def",
        ]
        .into_iter()
        .collect();

        let short_acronym_allowlist = [
            "pdf", "doc", "sql", "api", "git", "csv", "jpg", "png", "svg", "xml",
            "app", "bot", "vtt", "srt", "cpu", "gpu", "ram", "rom", "ssh", "cli",
            "sdk", "zip", "tar", "log", "css", "vue", "npm", "pip", "url", "uri",
            "dns", "ssl", "tls", "key", "web", "net", "ai", "ml", "dev", "ops",
        ]
        .into_iter()
        .collect();

        let vocab = if let Some(ref path) = storage_path {
            Self::load_vocab(path).unwrap_or_default()
        } else {
            CorpusVocabulary::default()
        };

        Self {
            vocab,
            stopwords,
            short_acronym_allowlist,
            storage_path,
            dirty: false,
        }
    }

    fn load_vocab(path: &Path) -> Option<CorpusVocabulary> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(ref path) = self.storage_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let json = serde_json::to_string(&self.vocab)
                .map_err(std::io::Error::other)?;
            std::fs::write(path, json)?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Register document terms in corpus vocabulary for inverse document frequency (IDF)
    pub fn register_document_terms(&mut self, text: &str, extra_symbols: &[String]) {
        let terms = self.extract_candidate_terms(text, extra_symbols);
        let unique_terms: HashSet<String> = terms.into_keys().collect();

        self.vocab.total_docs += 1;
        for term in unique_terms {
            *self.vocab.doc_frequencies.entry(term).or_insert(0) += 1;
        }

        // Keep vocabulary bounded to max 5,000 terms to guarantee memory < 10MB
        if self.vocab.doc_frequencies.len() > 5000 {
            self.prune_vocabulary();
        }

        self.dirty = true;
    }

    /// Extract top keywords for a document using TF-IDF
    pub fn extract_keywords(
        &self,
        text: &str,
        extra_symbols: &[String],
        max_keywords: usize,
    ) -> Vec<String> {
        let max_k = max_keywords.clamp(3, 10);
        let term_counts = self.extract_candidate_terms(text, extra_symbols);
        if term_counts.is_empty() {
            return Vec::new();
        }

        let total_words: usize = term_counts.values().sum();
        let total_words_f = total_words.max(1) as f64;
        let n_docs = self.vocab.total_docs.max(1) as f64;

        let symbol_set: HashSet<String> = extra_symbols.iter().map(|s| s.to_lowercase()).collect();

        let mut scored: Vec<(String, f64)> = term_counts
            .into_iter()
            .map(|(term, count)| {
                // Sublinear term frequency: 1.0 + ln(count)
                let tf = 1.0 + (count as f64).ln();
                let tf_norm = tf / (1.0 + total_words_f.ln());

                // Smooth IDF: ln(1 + N / (1 + df)) + 1.0
                let df = self.vocab.doc_frequencies.get(&term).copied().unwrap_or(1) as f64;
                let idf = ((1.0 + n_docs) / (1.0 + df)).ln() + 1.0;

                let mut score = tf_norm * idf;

                // Boost code symbols and functions
                if symbol_set.contains(&term) {
                    score *= 1.45;
                }

                // Favor medium-length distinctive words (len 5-14)
                if term.len() >= 5 && term.len() <= 16 {
                    score *= 1.15;
                }

                (term, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut keywords: Vec<String> = scored
            .into_iter()
            .take(max_k)
            .map(|(term, _)| term)
            .collect();

        // Ensure at least 3 keywords if text is present
        if keywords.len() < 3 {
            for sym in extra_symbols {
                let clean = sym.to_lowercase();
                if clean.len() >= 3 && !keywords.contains(&clean) {
                    keywords.push(clean);
                    if keywords.len() >= max_k {
                        break;
                    }
                }
            }
        }

        keywords
    }

    fn extract_candidate_terms(
        &self,
        text: &str,
        extra_symbols: &[String],
    ) -> HashMap<String, usize> {
        let mut counts = HashMap::new();

        // 1. Candidate terms from text
        for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
            let clean = word.trim_matches('_').to_lowercase();
            let len = clean.chars().count();
            let is_acronym = self.short_acronym_allowlist.contains(clean.as_str());

            if (len >= 4 && len <= 32) || (len == 3 && is_acronym) {
                let alpha_count = clean.chars().filter(|c| c.is_alphabetic()).count();
                if alpha_count >= 2 && !self.stopwords.contains(clean.as_str()) {
                    *counts.entry(clean).or_insert(0) += 1;
                }
            }
        }

        // 2. Extra weight for explicit code symbols and functions
        for sym in extra_symbols {
            let lower = sym.to_lowercase();
            if lower.len() >= 2 && !self.stopwords.contains(lower.as_str()) {
                *counts.entry(lower).or_insert(0) += 3;
            }
        }

        counts
    }

    fn prune_vocabulary(&mut self) {
        // Keep terms with df >= 2 or highest df
        let mut entries: Vec<(String, u32)> = self.vocab.doc_frequencies.drain().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(4000);
        self.vocab.doc_frequencies = entries.into_iter().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfidf_keyword_extraction() {
        let mut engine = TfidfEngine::new(None);

        let doc1 = "Hospital patient discharge summary and prescription for medicine dosage.";
        let doc2 = "Convolutional neural network image classification using deep learning tensors.";
        let doc3 = "Tax invoice total amount gstin billing and price details.";

        engine.register_document_terms(doc1, &[]);
        engine.register_document_terms(doc2, &[]);
        engine.register_document_terms(doc3, &[]);

        let kws1 = engine.extract_keywords(doc1, &[], 5);
        assert!(kws1.contains(&"prescription".to_string()) || kws1.contains(&"discharge".to_string()) || kws1.contains(&"hospital".to_string()));

        let kws2 = engine.extract_keywords(doc2, &[], 5);
        assert!(kws2.contains(&"neural".to_string()) || kws2.contains(&"convolutional".to_string()) || kws2.contains(&"classification".to_string()));
    }

    #[test]
    fn test_tfidf_symbol_boost() {
        let engine = TfidfEngine::new(None);
        let code = "a = 10\nb = 20\nresult = a + b";
        let symbols = vec!["add_numbers".to_string(), "calculate_sum".to_string()];

        let kws = engine.extract_keywords(code, &symbols, 5);
        assert!(kws.contains(&"add_numbers".to_string()) || kws.contains(&"calculate_sum".to_string()));
    }
}
