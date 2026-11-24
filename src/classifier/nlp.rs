use std::collections::HashSet;
use std::sync::OnceLock;

static STOPWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
static SHORT_ACRONYMS: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn get_stopwords() -> &'static HashSet<&'static str> {
    STOPWORDS.get_or_init(|| {
        [
            // English grammatical markers, prepositions, conjunctions
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
            "almost", "always", "neither", "either", "together", "several", "such",
            // Pronouns & Possessives
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
            // Conversational & Dialogue auxiliaries, spoken noise
            "don", "doesn", "didn", "hasn", "haven", "hadn", "won", "wouldn", "shan",
            "shouldn", "cannot", "couldn", "mustn", "needn", "ain", "aren", "isn",
            "wasn", "weren", "gonna", "wanna", "gotta", "yeah", "yep", "nope", "okay",
            "alright", "sure", "maybe", "please", "thank", "thanks", "hello", "hey",
            "bye", "goodbye", "sorry", "excuse", "listen", "tell", "told", "talk",
            "came", "went", "gone", "knew", "thought", "felt", "needed", "made",
            "took", "gave", "found", "seem", "seemed", "thing", "things", "stuff",
            "person", "man", "woman", "guy", "guys", "boy", "girl", "sir", "madam",
            // Multilingual: German (e.g. Dark audio/subtitles)
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
            // Multilingual: Spanish / French
            "el", "la", "los", "las", "un", "una", "unos", "unas", "del", "para",
            "por", "con", "les", "des", "dans", "sur", "avec", "pour",
            // Common extensions & metadata noise
            "txt", "pdf", "docx", "doc", "png", "jpg", "jpeg", "webp", "md", "json", "csv",
            "mkv", "mp4", "avi", "mov", "webm", "srt", "vtt",
            // Code keywords & identifiers
            "self", "true", "false", "none", "null", "return", "import", "class", "def",
            "const", "let", "var", "function", "fn", "pub", "struct", "impl",
        ]
        .into_iter()
        .collect()
    })
}

fn get_short_acronyms() -> &'static HashSet<&'static str> {
    SHORT_ACRONYMS.get_or_init(|| {
        [
            "cv", "qps", "api", "sql", "git", "csv", "xml", "app", "bot", "vtt", "srt",
            "cpu", "gpu", "ram", "rom", "ssh", "cli", "sdk", "zip", "tar", "log", "css",
            "vue", "npm", "pip", "url", "uri", "dns", "ssl", "tls", "key", "web", "net",
            "ai", "ml", "os", "db", "cs", "it", "dev", "ops", "cad", "mp4", "mkv", "mp3",
            "wav", "pan", "ssn", "tax", "ocr", "law", "pin", "gpa", "rag",
        ]
        .into_iter()
        .collect()
    })
}

/// Tokenizes text into normalized lexical tokens, splitting by non-alphanumeric delimiters
/// and camelCase / PascalCase word boundaries (e.g. "DataScience" -> ["data", "science"]).
pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        // Skip non-alphanumerics
        while i < n && !chars[i].is_alphanumeric() {
            i += 1;
        }
        if i >= n {
            break;
        }

        let start = i;
        while i < n && chars[i].is_alphanumeric() {
            // Check camelCase transition: lower followed by upper (e.g. dataScience)
            if i + 1 < n
                && chars[i].is_lowercase()
                && chars[i + 1].is_uppercase()
            {
                i += 1;
                break;
            }
            // Check digit-alpha transition (e.g. Lecture1 -> Lecture, 1 or 5thSem -> 5th, Sem)
            if i + 1 < n {
                let curr_is_digit = chars[i].is_ascii_digit();
                let next_is_alpha = chars[i + 1].is_alphabetic();
                let curr_is_alpha = chars[i].is_alphabetic();
                let next_is_digit = chars[i + 1].is_ascii_digit();

                // Split at Letter->Digit (Lecture1 -> Lecture, 1) unless it's a known unit like 5th
                if curr_is_alpha && next_is_digit {
                    i += 1;
                    break;
                }
                // Split at Digit->Letter unless it's an ordinal suffix like "5th", "1st", "2nd", "3rd"
                if curr_is_digit && next_is_alpha {
                    // Check if remaining slice starts with st, nd, rd, th
                    let remaining: String = chars[i + 1..].iter().take(2).collect();
                    let rem_lower = remaining.to_lowercase();
                    if rem_lower == "th" || rem_lower == "st" || rem_lower == "nd" || rem_lower == "rd" {
                        // Keep ordinal together (e.g. "5th")
                        i += 3;
                        break;
                    } else {
                        i += 1;
                        break;
                    }
                }
            }
            i += 1;
        }

        let token_str: String = chars[start..i].iter().collect();
        let clean = token_str.trim().to_lowercase();
        if !clean.is_empty() {
            tokens.push(clean);
        }
    }

    tokens
}

/// Normalizes English inflectional suffixes (plurals, gerunds, past tense) to base morphemes.
pub fn stem(word: &str) -> String {
    let w = word.trim().to_lowercase();
    let len = w.chars().count();
    if len <= 3 {
        return w;
    }

    // -ies -> -y (summaries -> summary, realities -> reality)
    if w.ends_with("ies") && len >= 5 {
        return format!("{}y", &w[..len - 3]);
    }

    // -sses -> -ss (classes -> class, processes -> process)
    if w.ends_with("sses") && len >= 6 {
        return w[..len - 2].to_string();
    }

    // -ves -> -f or -ve (leaves -> leaf, shelves -> shelf)
    if w.ends_with("ves") && len >= 5 {
        return format!("{}f", &w[..len - 3]);
    }

    // -es -> base for sibilants (boxes -> box, branches -> branch, invoices -> invoice)
    if w.ends_with("es") && len >= 5 {
        let stem = &w[..len - 2];
        if stem.ends_with("ch") || stem.ends_with("sh") || stem.ends_with('x') || stem.ends_with('z') || stem.ends_with('s') {
            return stem.to_string();
        }
        // e.g. invoices -> invoice, slides -> slide
        return w[..len - 1].to_string();
    }

    // -s -> base (notes -> note, lectures -> lecture, algorithms -> algorithm)
    if w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") && !w.ends_with("is") && len >= 4 {
        return w[..len - 1].to_string();
    }

    // -ing -> base (billing -> bill, recording -> record, consulting -> consult)
    if w.ends_with("ing") && len >= 6 {
        let stem = &w[..len - 3];
        // Handle double consonant (e.g. running -> run, programming -> program)
        if stem.len() >= 4 {
            let last_two: Vec<char> = stem.chars().rev().take(2).collect();
            if last_two.len() == 2 && last_two[0] == last_two[1] && !matches!(last_two[0], 's' | 'l') {
                return stem[..stem.len() - 1].to_string();
            }
        }
        return stem.to_string();
    }

    // -ed -> base (prescribed -> prescribe, filtered -> filter)
    if w.ends_with("ed") && len >= 5 {
        let stem = &w[..len - 2];
        if stem.ends_with('e') {
            return stem.to_string();
        }
        return stem.to_string();
    }

    w
}

/// Checks whether a token is a known grammatical, code, or conversational stopword.
pub fn is_stopword(word: &str) -> bool {
    let lower = word.to_lowercase();
    get_stopwords().contains(lower.as_str())
}

/// Checks whether a short token is a recognized technical or domain acronym.
pub fn is_recognized_acronym(word: &str) -> bool {
    let lower = word.to_lowercase();
    get_short_acronyms().contains(lower.as_str())
}

/// Validates whether a token is a legitimate natural language or technical term,
/// rejecting OCR gibberish, extreme consonant/vowel jams, and non-alphanumeric noise.
pub fn is_lexically_valid(word: &str) -> bool {
    let w = word.trim().to_lowercase();
    let len = w.chars().count();

    // Length boundary
    if len < 2 || len > 32 {
        return false;
    }

    // Acronyms pass directly
    if len <= 3 && is_recognized_acronym(&w) {
        return true;
    }

    // Must not be a pure number unless an ordinal (e.g. 1st, 2nd, 3rd, 5th)
    if w.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // Check for character repetition (3+ of same char in a row, e.g. "eee", "rrr")
    let chars: Vec<char> = w.chars().collect();
    for window in chars.windows(3) {
        if window[0] == window[1] && window[1] == window[2] {
            return false;
        }
    }

    // Must contain letters
    let letter_count = chars.iter().filter(|c| c.is_alphabetic()).count();
    if letter_count < 2 {
        return false;
    }

    // Check vowel-to-consonant plausibility for words length >= 5
    if len >= 5 {
        let vowels = chars.iter().filter(|c| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')).count();
        // Zero vowels in 5+ letters is almost certainly OCR gibberish or random hash (e.g. "dsdpsuy")
        if vowels == 0 {
            return false;
        }
        // Vowel ratio too extreme (e.g. 5 vowels out of 6 letters without common diphthongs)
        let vowel_ratio = vowels as f32 / len as f32;
        if vowel_ratio > 0.85 {
            return false;
        }

        // Check for 5 consecutive consonants (e.g. "dsdps")
        let mut consecutive_consonants = 0;
        for &c in &chars {
            if matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y') {
                consecutive_consonants = 0;
            } else if c.is_alphabetic() {
                consecutive_consonants += 1;
                if consecutive_consonants >= 5 {
                    return false;
                }
            }
        }
    }

    true
}

/// Dynamically formats a subtopic token:
/// Technical acronyms (<= 3 chars, e.g. CV, QPS, API, SQL, TAX, OS) are uppercased.
/// Standard words (e.g. Invoice, Resume, Prescription) are cleanly capitalized.
pub fn format_subtopic_token(term: &str) -> String {
    let clean = term.trim();
    if clean.is_empty() {
        return String::new();
    }
    if clean.chars().count() <= 3 {
        clean.to_uppercase()
    } else {
        capitalize(clean)
    }
}

/// Capitalizes the first character of each space-delimited word.
pub fn capitalize(s: &str) -> String {
    let mut result = Vec::new();
    for part in s.split_whitespace() {
        let mut chars = part.chars();
        let word = match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        };
        result.push(word);
    }
    result.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stemming() {
        assert_eq!(stem("invoices"), "invoice");
        assert_eq!(stem("lectures"), "lecture");
        assert_eq!(stem("notes"), "note");
        assert_eq!(stem("questions"), "question");
        assert_eq!(stem("summaries"), "summary");
        assert_eq!(stem("billing"), "bill");
        assert_eq!(stem("consulting"), "consult");
        assert_eq!(stem("systems"), "system");
    }

    #[test]
    fn test_ocr_gibberish_rejection() {
        assert!(!is_lexically_valid("aeeeth")); // 3 'e's in a row
        assert!(!is_lexically_valid("eeeeseseeete")); // 4 'e's in a row
        assert!(!is_lexically_valid("mrrrhtoo")); // 3 'r's in a row
        assert!(!is_lexically_valid("dsdpsuy")); // 5 consecutive consonants
        assert!(!is_lexically_valid("123456")); // numbers only

        // Legitimate terms
        assert!(is_lexically_valid("invoice"));
        assert!(is_lexically_valid("hospital"));
        assert!(is_lexically_valid("prescription"));
        assert!(is_lexically_valid("cv"));
        assert!(is_lexically_valid("qps"));
        assert!(is_lexically_valid("mapreduce"));
    }

    #[test]
    fn test_camel_case_tokenization() {
        let tokens = tokenize("DataScienceOperatingSystems");
        assert_eq!(tokens, vec!["data", "science", "operating", "systems"]);

        let ord_tokens = tokenize("5thSemQPs");
        assert_eq!(ord_tokens, vec!["5th", "sem", "qps"]);
    }
}
