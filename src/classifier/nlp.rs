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
/// Uses UTF-8 character-safe suffix stripping to prevent panics on multilingual inputs.
pub fn stem(word: &str) -> String {
    let w = word.trim().to_lowercase();
    let char_count = w.chars().count();
    if char_count <= 3 {
        return w;
    }

    // -ies -> -y (summaries -> summary, realities -> reality)
    if let Some(prefix) = w.strip_suffix("ies") {
        if prefix.chars().count() >= 2 {
            return format!("{prefix}y");
        }
    }

    // -sses -> -ss (classes -> class, processes -> process)
    if let Some(prefix) = w.strip_suffix("sses") {
        if prefix.chars().count() >= 2 {
            return format!("{prefix}ss");
        }
    }

    // -ves -> -f or -ve (leaves -> leaf, shelves -> shelf)
    if let Some(prefix) = w.strip_suffix("ves") {
        if prefix.chars().count() >= 2 {
            return format!("{prefix}f");
        }
    }

    // -es -> base for sibilants (boxes -> box, branches -> branch, invoices -> invoice)
    if let Some(prefix) = w.strip_suffix("es") {
        if prefix.chars().count() >= 3 {
            if prefix.ends_with("ch")
                || prefix.ends_with("sh")
                || prefix.ends_with('x')
                || prefix.ends_with('z')
                || prefix.ends_with('s')
            {
                return prefix.to_string();
            }
            // e.g. invoices -> invoice, slides -> slide: preserve 'e'
            return format!("{prefix}e");
        }
    }

    // -s -> base (notes -> note, lectures -> lecture, algorithms -> algorithm)
    if let Some(prefix) = w.strip_suffix('s') {
        if !w.ends_with("ss") && !w.ends_with("us") && !w.ends_with("is") && prefix.chars().count() >= 3 {
            return prefix.to_string();
        }
    }

    // -ing -> base (billing -> bill, recording -> record, consulting -> consult)
    if let Some(prefix) = w.strip_suffix("ing") {
        if prefix.chars().count() >= 3 {
            // Handle double consonant (e.g. running -> run, programming -> program)
            let last_two: Vec<char> = prefix.chars().rev().take(2).collect();
            if last_two.len() == 2 && last_two[0] == last_two[1] && !matches!(last_two[0], 's' | 'l') {
                let mut chars: Vec<char> = prefix.chars().collect();
                chars.pop();
                return chars.into_iter().collect();
            }
            return prefix.to_string();
        }
    }

    // -ed -> base (prescribed -> prescribe, filtered -> filter)
    if let Some(prefix) = w.strip_suffix("ed") {
        if prefix.chars().count() >= 3 {
            return prefix.to_string();
        }
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

fn is_vowel(c: char) -> bool {
    matches!(
        c,
        // Latin vowels (basic + accented)
        'a' | 'e' | 'i' | 'o' | 'u' | 'y'
        | 'á' | 'é' | 'í' | 'ó' | 'ú' | 'ý'
        | 'à' | 'è' | 'ì' | 'ò' | 'ù'
        | 'ä' | 'ë' | 'ï' | 'ö' | 'ü' | 'ÿ'
        | 'â' | 'ê' | 'î' | 'ô' | 'û'
        | 'ã' | 'õ' | 'å' | 'æ' | 'ø' | 'œ'
        // Cyrillic vowels (Russian, Ukrainian, Belarusian, Bulgarian, Serbian)
        | 'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я'
        | 'і' | 'ї' | 'є' | 'ў'
        // Greek vowels
        | 'α' | 'ε' | 'η' | 'ι' | 'ο' | 'υ' | 'ω'
        | 'ά' | 'έ' | 'ή' | 'ί' | 'ό' | 'ύ' | 'ώ'
    )
}

fn is_alphabetic_with_vowels(c: char) -> bool {
    matches!(
        c,
        'a'..='z'
        | 'A'..='Z'
        | '\u{00C0}'..='\u{024F}' // Latin Extended A/B
        | '\u{0400}'..='\u{04FF}' // Cyrillic
        | '\u{0370}'..='\u{03FF}' // Greek
    )
}

/// Validates whether a token is a legitimate natural language or technical term,
/// rejecting OCR gibberish, extreme consonant/vowel jams, and non-alphanumeric noise.
/// Uses Unicode script awareness so valid Cyrillic, Greek, Indic, CJK, and accented terms are accepted.
pub fn is_lexically_valid(word: &str) -> bool {
    let w = word.trim().to_lowercase();
    let len = w.chars().count();

    // Length boundary
    if len < 2 || len > 40 {
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

    // Script-aware vowel-to-consonant plausibility check for alphabetic languages (Latin, Cyrillic, Greek)
    let has_alphabetic_script = chars.iter().any(|c| is_alphabetic_with_vowels(*c));
    if has_alphabetic_script && len >= 5 {
        let vowels = chars.iter().filter(|c| is_vowel(**c)).count();
        // Zero vowels in 5+ letters is almost certainly OCR gibberish or random hash (e.g. "dsdpsuy")
        if vowels == 0 {
            return false;
        }
        // Vowel ratio check (rejects non-words that are almost entirely vowels without standard phonology)
        let vowel_ratio = vowels as f32 / len as f32;
        if vowel_ratio > 0.90 {
            return false;
        }

        // Check for 5 consecutive consonants (e.g. "dsdps")
        let mut consecutive_consonants = 0;
        for &c in &chars {
            if is_vowel(c) {
                consecutive_consonants = 0;
            } else if c.is_alphabetic() {
                consecutive_consonants += 1;
                if consecutive_consonants >= 5 {
                    // Allow legitimate natural English coda clusters: "-ngths" (strengths, lengths) and "-lfths" (twelfths)
                    if consecutive_consonants == 5 && (w.ends_with("ngths") || w.ends_with("lfths")) {
                        continue;
                    }
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
    fn test_multilingual_unicode_stemming_never_panics() {
        // German umlauts and compounds
        assert_eq!(stem("überflüssiges"), "überflüssige");
        assert_eq!(stem("verhältnisse"), "verhältnisse");
        assert_eq!(stem("schönes"), "schöne");
        // French accents
        assert_eq!(stem("résumés"), "résumé");
        assert_eq!(stem("données"), "donnée");
        // Spanish
        assert_eq!(stem("niños"), "niño");
        assert_eq!(stem("organizaciones"), "organizacione");
        // Indic / Devanagari / Tamil / CJK
        assert_eq!(stem("प्रणाली"), "प्रणाली");
        assert_eq!(stem("கோப்புகள்"), "கோப்புகள்");
        assert_eq!(stem("日本語データ"), "日本語データ");
        // Emojis and symbols
        assert_eq!(stem("🚀notes"), "🚀note");
        assert_eq!(stem("test🔥"), "test🔥");
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

        // Multilingual words across world scripts must pass cleanly
        assert!(is_lexically_valid("документ")); // Russian/Cyrillic: document
        assert!(is_lexically_valid("алгоритм")); // Russian/Cyrillic: algorithm
        assert!(is_lexically_valid("πρόγραμμα")); // Greek: program
        assert!(is_lexically_valid("अध्ययन")); // Hindi: study
        assert!(is_lexically_valid("நிரலாக்கம்")); // Tamil: programming
        assert!(is_lexically_valid("自然言語処理")); // Japanese: NLP
        assert!(is_lexically_valid("résumé")); // French accented
        assert!(is_lexically_valid("geschäftsbericht")); // German umlaut
    }

    #[test]
    fn test_camel_case_tokenization() {
        let tokens = tokenize("DataScienceOperatingSystems");
        assert_eq!(tokens, vec!["data", "science", "operating", "systems"]);

        let ord_tokens = tokenize("5thSemQPs");
        assert_eq!(ord_tokens, vec!["5th", "sem", "qps"]);
    }
}
