use std::collections::{HashMap, HashSet};

pub struct KeywordExtractor {
    stopwords: HashSet<&'static str>,
    short_acronym_allowlist: HashSet<&'static str>,
}

impl Default for KeywordExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl KeywordExtractor {
    pub fn new() -> Self {
        let stopwords = [
            // English Articles, Prepositions & Conjunctions
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
            "almost", "always", "neither", "either", "together", "several",

            // English Pronouns & Possessives
            "you", "your", "yours", "yourself", "yourselves",
            "he", "him", "his", "himself",
            "she", "her", "hers", "herself",
            "it", "its", "itself",
            "they", "them", "their", "theirs", "themselves",
            "we", "us", "our", "ours", "ourselves",
            "who", "whom", "whose", "which", "what", "whatever", "whoever", "whomever",
            "somebody", "someone", "something", "somewhere",
            "anybody", "anyone", "anything", "anywhere",
            "everybody", "everyone", "everything", "everywhere",
            "nobody", "nothing", "nowhere", "none",

            // Conversational auxiliaries, contractions & dialog noise
            "don", "doesn", "didn", "hasn", "haven", "hadn", "won", "wouldn", "shan",
            "shouldn", "cannot", "couldn", "mustn", "needn", "ain", "aren", "isn",
            "wasn", "weren", "gonna", "wanna", "gotta", "yeah", "yep", "nope", "okay",
            "alright", "sure", "maybe", "please", "thank", "thanks", "hello", "hey",
            "bye", "goodbye", "sorry", "excuse", "listen", "tell", "told", "talk",
            "came", "went", "gone", "knew", "thought", "felt", "needed", "made",
            "took", "gave", "found", "seem", "seemed", "thing", "things", "stuff",
            "person", "man", "woman", "guy", "guys", "boy", "girl", "sir", "madam",

            // Common German Stopwords (vital for German subtitles/dialogue like Dark)
            "der", "die", "das", "den", "dem", "des", "ein", "eine", "einer", "eines",
            "einem", "einen", "und", "oder", "aber", "mit", "von", "zu", "im", "in",
            "am", "an", "auf", "aus", "bei", "nach", "uber", "über", "unter", "vor",
            "zwischen", "ich", "du", "er", "sie", "es", "wir", "ihr", "mich", "dich",
            "ihn", "uns", "euch", "ihnen", "sein", "seine", "ihr", "ihre", "mein",
            "meine", "dein", "deine", "nicht", "auch", "so", "wie", "wenn", "dass",
            "da", "hier", "dort", "ja", "nein", "doch", "noch", "schon", "nur",
            "immer", "wieder", "sehr", "etwas", "nichts", "alles", "alle", "allen",
            "aller", "war", "waren", "wird", "werden", "wurde", "wurden", "hat",
            "hatte", "haben", "hatten", "ist", "sind", "bist", "seid", "kann",
            "können", "konnte", "muss", "müssen", "musste", "will", "wollen",
            "wollte", "soll", "sollen", "sollte",

            // Spanish & French Basic Stopwords
            "el", "la", "los", "las", "un", "una", "unos", "unas", "del", "para",
            "por", "con", "les", "des", "dans", "sur", "avec", "pour",

            // File extension noise
            "txt", "pdf", "docx", "png", "jpg", "jpeg", "webp", "md", "json", "csv",
            "mkv", "mp4", "avi", "mov", "webm", "srt", "vtt",
        ]
        .into_iter()
        .collect();

        let short_acronym_allowlist = [
            "pdf", "doc", "sql", "api", "git", "csv", "jpg", "png", "svg", "xml",
            "app", "bot", "vtt", "srt", "cpu", "gpu", "ram", "rom", "ssh", "cli",
            "sdk", "zip", "tar", "log", "css", "vue", "npm", "pip", "url", "uri",
            "dns", "ssl", "tls", "key", "web", "net", "ai", "ml", "dev", "ops",
            "cad", "mp4", "mkv", "mp3", "wav", "pan", "ssn", "tax", "ocr", "law",
            "pin",
        ]
        .into_iter()
        .collect();

        Self {
            stopwords,
            short_acronym_allowlist,
        }
    }

    /// Extract top keywords using frequency, semantic length weighting, and strict stopword rejection
    pub fn extract(&self, text: &str, max_keywords: usize) -> Vec<String> {
        let max_k = max_keywords.clamp(3, 10);
        let mut freq: HashMap<String, usize> = HashMap::new();

        // Tokenize while normalizing contractions (e.g. don't -> don't split into standalone "don")
        for word in text.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '_') {
            let clean = word.trim_matches('_');
            if clean.is_empty() {
                continue;
            }

            // Reject conversational stopwords
            if self.stopwords.contains(clean) {
                continue;
            }

            let len = clean.chars().count();
            let is_acronym = self.short_acronym_allowlist.contains(clean);

            // General words must be at least 4 letters; 3-letter words only permitted if in technical acronym allowlist
            if (len >= 4 && len <= 28) || (len == 3 && is_acronym) {
                let letter_count = clean.chars().filter(|c| c.is_alphabetic()).count();
                if letter_count >= 2 {
                    *freq.entry(clean.to_string()).or_insert(0) += 1;
                }
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

        let mut keywords: Vec<String> = scored_words
            .into_iter()
            .take(max_k)
            .map(|(w, _)| w)
            .collect();

        if keywords.len() < 3 {
            let fallbacks = ["document", "records", "archive", "content"];
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

    /// Clean, filter, and deduplicate keywords, strictly removing conversational stop words and invalid short tokens
    pub fn clean_keywords(&self, keywords: Vec<String>, text: &str, max_keywords: usize) -> Vec<String> {
        let mut cleaned = Vec::new();
        for k in keywords {
            let clean = k.trim().to_lowercase();
            if clean.is_empty()
                || clean.contains("redacted")
                || self.stopwords.contains(clean.as_str())
            {
                continue;
            }

            let len = clean.chars().count();
            let is_acronym = self.short_acronym_allowlist.contains(clean.as_str());
            // Filter out 1, 2, or 3-letter non-acronym words
            if len < 3 || (len == 3 && !is_acronym) || len > 32 {
                continue;
            }

            // Exclude purely numeric tokens
            if clean.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            if !cleaned.contains(&clean) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_conversational_dialogue_words() {
        let extractor = KeywordExtractor::new();
        // Spoken conversational dialogue with words like "you", "die", "who", "don't", "all", "what"
        let dialogue = "You said you will die! Who are you? Don't look at me, all of them went away!";
        let keywords = extractor.extract(dialogue, 5);

        // Conversational noise must NEVER appear as keywords
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
}
