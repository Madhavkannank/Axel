use std::collections::HashMap;
use crate::classifier::nlp::{is_lexically_valid, is_recognized_acronym, is_stopword, tokenize};

pub struct KeywordExtractor;

impl Default for KeywordExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl KeywordExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract top keywords using normalized token frequency, length salience,
    /// and strict stopword / gibberish rejection.
    pub fn extract(&self, text: &str, max_keywords: usize) -> Vec<String> {
        let max_k = if max_keywords == 0 { 10 } else { max_keywords.min(50) };
        let mut freq: HashMap<String, usize> = HashMap::new();

        let tokens = tokenize(text);
        for clean in tokens {
            if is_stopword(&clean) {
                continue;
            }

            let len = clean.chars().count();
            let is_acronym = is_recognized_acronym(&clean);

            if ((len >= 4 && len <= 28) || (len == 3 && is_acronym)) && is_lexically_valid(&clean) {
                *freq.entry(clean).or_insert(0) += 1;
            }
        }

        let mut scored_words: Vec<(String, usize)> = freq
            .into_iter()
            .map(|(w, count)| {
                // Weight descriptive, topical nouns (length 5..=18) higher than short conversational fragments
                let length_boost = if w.len() >= 5 && w.len() <= 18 { 4 } else { 0 };
                let score = count * 10 + length_boost;
                (w, score)
            })
            .collect();

        scored_words.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let keywords: Vec<String> = scored_words
            .into_iter()
            .take(max_k)
            .map(|(w, _)| w)
            .collect();

        keywords
    }

    /// Clean, filter, and deduplicate keywords, strictly removing conversational stop words and invalid tokens.
    pub fn clean_keywords(&self, keywords: Vec<String>, text: &str, max_keywords: usize) -> Vec<String> {
        let mut cleaned = Vec::new();
        for k in keywords {
            let clean = k.trim().to_lowercase();
            if clean.is_empty()
                || clean.contains("redacted")
                || is_stopword(&clean)
                || !is_lexically_valid(&clean)
            {
                continue;
            }

            if !cleaned.contains(&clean) {
                cleaned.push(clean);
            }
        }

        let max_k = if max_keywords == 0 { 10 } else { max_keywords.min(50) };
        if cleaned.len() > max_k {
            cleaned.truncate(max_k);
        }

        if cleaned.is_empty() && !text.is_empty() {
            let extracted = self.extract(text, max_k);
            for word in extracted {
                if !cleaned.contains(&word) {
                    cleaned.push(word);
                }
                if cleaned.len() >= max_k {
                    break;
                }
            }
        }

        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_conversational_dialogue_words() {
        let extractor = KeywordExtractor::new();
        let dialogue = "You said you will die! Who are you? Don't look at me, all of them went away!";
        let keywords = extractor.extract(dialogue, 5);

        assert!(!keywords.contains(&"you".to_string()));
        assert!(!keywords.contains(&"die".to_string()));
        assert!(!keywords.contains(&"who".to_string()));
        assert!(!keywords.contains(&"don".to_string()));
        assert!(!keywords.contains(&"all".to_string()));
    }

    #[test]
    fn test_german_stopword_rejection() {
        let extractor = KeywordExtractor::new();
        let german_text = "Der Die Das Ueberlebenden Episode Dark Staffel";
        let keywords = extractor.extract(german_text, 5);

        assert!(!keywords.contains(&"die".to_string()));
        assert!(!keywords.contains(&"der".to_string()));
        assert!(!keywords.contains(&"das".to_string()));
    }

    #[test]
    fn test_respects_custom_max_keywords_and_complex_words() {
        let extractor = KeywordExtractor::new();
        let text = "The architectural strengths and engineering calculations require database optimization and distributed transaction pipelines.";

        // Should respect max_keywords = 1 without forcing minimum 3 (Audit finding 41)
        let kw1 = extractor.extract(text, 1);
        assert_eq!(kw1.len(), 1);

        // Should accept complex natural words like "strengths" (Audit finding 17)
        assert!(is_lexically_valid("strengths"));
        assert!(is_lexically_valid("twelfths"));
        assert!(is_lexically_valid("розробка")); // Ukrainian "development"
    }
}
