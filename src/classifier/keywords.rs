use std::collections::{HashMap, HashSet};

pub struct KeywordExtractor {
    stopwords: HashSet<&'static str>,
}

impl Default for KeywordExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl KeywordExtractor {
    pub fn new() -> Self {
        let stopwords = [
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
            "txt", "pdf", "docx", "png", "jpg", "jpeg", "webp", "md", "json", "csv",
        ]
        .into_iter()
        .collect();

        Self { stopwords }
    }

    pub fn extract(&self, text: &str, max_keywords: usize) -> Vec<String> {
        let max_k = max_keywords.clamp(3, 10);
        let mut freq: HashMap<String, usize> = HashMap::new();

        for word in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
            let letter_count = word.chars().filter(|c| c.is_alphabetic()).count();
            if word.len() >= 3 && letter_count >= 3 && !self.stopwords.contains(word) {
                *freq.entry(word.to_string()).or_insert(0) += 1;
            }
        }

        let mut scored_words: Vec<(String, usize)> = freq
            .into_iter()
            .map(|(w, count)| {
                let score = count * 10 + if w.len() >= 6 { 2 } else { 0 };
                (w, score)
            })
            .collect();

        scored_words.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut keywords: Vec<String> = scored_words
            .into_iter()
            .take(max_k)
            .map(|(w, _)| w)
            .collect();

        if keywords.len() < 3 {
            let fallbacks = ["document", "records", "file"];
            for fb in fallbacks {
                if !keywords.contains(&fb.to_string()) {
                    keywords.push(fb.to_string());
                }
                if keywords.len() >= 3 {
                    break;
                }
            }
        }

        keywords
    }

    pub fn clean_keywords(&self, keywords: Vec<String>, text: &str, max_keywords: usize) -> Vec<String> {
        let mut cleaned = Vec::new();
        for k in keywords {
            let clean = k.trim().to_lowercase();
            if !clean.is_empty() && !cleaned.contains(&clean) {
                cleaned.push(clean);
            }
        }

        let max_k = max_keywords.clamp(3, 10);
        if cleaned.len() > max_k {
            cleaned.truncate(max_k);
        }

        if cleaned.len() < 3 && !text.is_empty() {
            let extracted = self.extract(text, max_k);
            for word in extracted {
                if !cleaned.contains(&word) {
                    cleaned.push(word);
                }
                if cleaned.len() >= 3 {
                    break;
                }
            }
        }

        cleaned
    }
}
