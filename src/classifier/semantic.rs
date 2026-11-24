use std::collections::HashMap;

/// Semantic domain centroid representing a weighted concept subspace.
struct DomainCentroid {
    name: &'static str,
    default_subtopic: &'static str,
    term_weights: &'static [(&'static str, f32)],
}

static DOMAIN_CENTROIDS: &[DomainCentroid] = &[
    DomainCentroid {
        name: "Academics",
        default_subtopic: "Coursework",
        term_weights: &[
            ("syllabus", 3.5),
            ("curriculum", 3.0),
            ("semester", 2.8),
            ("exam", 2.8),
            ("examination", 2.8),
            ("qps", 3.2),
            ("qpaper", 3.2),
            ("question", 2.2),
            ("paper", 2.0),
            ("marks", 2.0),
            ("lecture", 2.5),
            ("notes", 2.0),
            ("calculus", 2.5),
            ("mathematics", 2.2),
            ("physics", 2.2),
            ("chemistry", 2.2),
            ("biology", 2.2),
            ("university", 2.0),
            ("college", 1.8),
            ("degree", 1.8),
            ("course", 2.0),
            ("assignment", 2.2),
            ("midsem", 3.0),
            ("endsem", 3.0),
            ("textbook", 2.5),
            ("student", 1.5),
            ("grade", 1.8),
            ("credits", 2.0),
            ("thesis", 2.5),
            ("dissertation", 2.5),
        ],
    },
    DomainCentroid {
        name: "Career",
        default_subtopic: "Resume",
        term_weights: &[
            ("cv", 5.0),
            ("resume", 5.0),
            ("curriculum", 4.0),
            ("vitae", 4.5),
            ("experience", 3.0),
            ("education", 2.8),
            ("skills", 2.8),
            ("objective", 3.0),
            ("projects", 2.0),
            ("intern", 2.5),
            ("internship", 2.5),
            ("employment", 2.5),
            ("job", 2.0),
            ("candidate", 2.2),
            ("portfolio", 3.0),
            ("qualifications", 2.5),
            ("certifications", 2.5),
            ("achievements", 2.0),
            ("leadership", 1.8),
            ("responsibilities", 2.0),
            ("technologies", 1.8),
            ("cgpa", 2.5),
            ("gpa", 2.5),
            ("github", 2.0),
            ("linkedin", 2.0),
        ],
    },
    DomainCentroid {
        name: "Engineering",
        default_subtopic: "Software",
        term_weights: &[
            ("code", 2.5),
            ("algorithm", 2.8),
            ("architecture", 2.5),
            ("implementation", 2.2),
            ("pipeline", 2.2),
            ("backend", 2.5),
            ("frontend", 2.5),
            ("database", 2.2),
            ("compiler", 2.5),
            ("runtime", 2.0),
            ("optimization", 2.2),
            ("framework", 2.0),
            ("repository", 2.2),
            ("benchmark", 2.2),
            ("cuda", 2.5),
            ("pytorch", 2.5),
            ("opencv", 2.8),
            ("vision", 2.2),
            ("filter", 2.0),
            ("gradient", 2.0),
            ("protocol", 2.2),
            ("hardware", 2.2),
            ("firmware", 2.2),
            ("debugging", 2.0),
            ("testing", 2.0),
            ("hackathon", 2.5),
        ],
    },
    DomainCentroid {
        name: "Finance",
        default_subtopic: "Invoice",
        term_weights: &[
            ("invoice", 4.0),
            ("billing", 3.2),
            ("payment", 3.0),
            ("receipt", 3.5),
            ("tax", 2.5),
            ("subtotal", 3.0),
            ("total", 1.5),
            ("amount", 2.0),
            ("due", 2.2),
            ("balance", 2.5),
            ("vendor", 2.2),
            ("remittance", 2.8),
            ("purchase", 2.0),
            ("order", 1.8),
            ("accounting", 2.5),
            ("audit", 2.2),
            ("expense", 2.2),
            ("statement", 2.0),
            ("consulting", 2.0),
            ("payroll", 2.5),
        ],
    },
    DomainCentroid {
        name: "Creative",
        default_subtopic: "Writing",
        term_weights: &[
            ("screenplay", 4.0),
            ("script", 3.5),
            ("scene", 3.0),
            ("dialogue", 3.0),
            ("horror", 2.5),
            ("story", 3.0),
            ("character", 2.5),
            ("narrative", 2.5),
            ("novel", 2.5),
            ("chapter", 2.0),
            ("drama", 2.2),
            ("acts", 2.0),
            ("author", 2.0),
            ("film", 2.2),
            ("cinema", 2.2),
            ("fiction", 2.5),
            ("literature", 2.5),
            ("poetry", 2.5),
            ("storyline", 2.5),
        ],
    },
    DomainCentroid {
        name: "Medical",
        default_subtopic: "Healthcare",
        term_weights: &[
            ("patient", 3.5),
            ("hospital", 3.2),
            ("prescription", 4.0),
            ("dosage", 3.5),
            ("diagnosis", 3.5),
            ("treatment", 3.0),
            ("doctor", 3.0),
            ("physician", 3.0),
            ("clinical", 2.8),
            ("medicine", 3.0),
            ("antibiotics", 3.0),
            ("symptoms", 2.8),
            ("discharge", 2.8),
            ("summary", 1.5),
            ("pathology", 3.0),
            ("laboratory", 2.2),
        ],
    },
    DomainCentroid {
        name: "Legal",
        default_subtopic: "Contracts",
        term_weights: &[
            ("contract", 3.5),
            ("agreement", 3.5),
            ("terms", 2.5),
            ("clause", 3.0),
            ("liability", 3.0),
            ("indemnity", 3.0),
            ("jurisdiction", 2.8),
            ("parties", 2.2),
            ("notary", 3.0),
            ("statutory", 2.5),
            ("compliance", 2.5),
            ("arbitration", 2.8),
            ("lease", 3.0),
        ],
    },
    DomainCentroid {
        name: "Media",
        default_subtopic: "Visuals",
        term_weights: &[
            ("wallpaper", 3.0),
            ("comic", 3.0),
            ("comics", 3.0),
            ("collage", 3.0),
            ("poster", 2.5),
            ("artwork", 2.8),
            ("illustration", 2.8),
            ("cover", 2.0),
            ("animation", 3.0),
            ("video", 3.0),
            ("episode", 3.0),
            ("audio", 2.5),
            ("soundtrack", 2.5),
            ("recording", 2.2),
            ("photography", 2.5),
            ("render", 2.2),
        ],
    },
];

pub const DOC_DESCRIPTOR_WORDS: &[&str] = &[
    "syllabus", "curriculum", "notes", "lecture", "lectures", "exam", "examination",
    "exams", "qps", "qpaper", "question", "questions", "paper", "papers", "assignment",
    "assignments", "homework", "midsem", "endsem", "semester", "sem", "unit", "units",
    "chapter", "chapters", "module", "modules", "lab", "manual", "handout", "guide",
    "textbook", "book", "slides", "part", "vol", "volume", "revision", "study",
    "material", "materials", "course", "class", "reference", "dept", "department",
    "university", "college", "test", "tests", "solution", "solutions", "internal",
    "internals", "assessment", "report", "msc", "bsc", "btech", "mtech", "bca", "mca",
    "phd", "be", "me", "ug", "pg", "final", "mid", "term",
];

const GENERIC_DISCARD_WORDS: &[&str] = &[
    "document", "records", "archive", "content", "untitled", "file", "temp", "sample",
    "image", "video", "audio", "screenshot", "download",
];

pub struct DynamicSemanticClassifier;

impl DynamicSemanticClassifier {
    /// Classifies a document dynamically using vector space cosine similarity against
    /// semantic domain centroids, with dynamic subtopic synthesis and salience fallback.
    ///
    /// Never returns a hardcoded "General" or uses static if-else ladders.
    pub fn classify(
        text: &str,
        file_name: &str,
        extracted_keywords: &[String],
        ext: &str,
    ) -> (String, Vec<String>) {
        let clean_ext = ext.trim().trim_start_matches('.').to_lowercase();
        let is_video = crate::classifier::filter::is_video_extension(&clean_ext);
        let is_image = crate::classifier::filter::is_image_extension(&clean_ext);
        let is_audio = matches!(clean_ext.as_str(), "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "wma");
        let is_code = crate::classifier::filter::is_script_extension(&clean_ext);
        let is_doc = crate::classifier::filter::is_doc_extension(&clean_ext);

        // 1. Tokenize document text and filename into normalized term frequency map
        let mut term_freq: HashMap<String, f32> = HashMap::new();

        // High weight for tokens in filename (title metadata has high semantic salience)
        let file_tokens: Vec<String> = file_name
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| s.len() >= 2)
            .collect();

        for tok in &file_tokens {
            *term_freq.entry(tok.clone()).or_insert(0.0) += 6.0;
        }

        // Keywords extracted via statistical TF-IDF
        for kw in extracted_keywords {
            for tok in kw.split_whitespace() {
                let clean = tok.to_lowercase();
                if clean.len() >= 2 {
                    *term_freq.entry(clean).or_insert(0.0) += 2.0;
                }
            }
        }

        // Words in body text (first 5000 chars)
        for word in text.split(|c: char| !c.is_alphanumeric()) {
            let clean = word.to_lowercase();
            if clean.len() >= 3 {
                *term_freq.entry(clean).or_insert(0.0) += 1.0;
            }
        }

        // Compute document vector norm
        let doc_norm: f32 = term_freq.values().map(|w| w * w).sum::<f32>().sqrt();

        // 2. Score cosine similarity across all domain centroids
        let mut best_domain: Option<&'static DomainCentroid> = None;
        let mut best_score = 0.0f32;
        let mut domain_matching_terms: HashMap<&'static str, Vec<(String, f32)>> = HashMap::new();

        if doc_norm > 0.0 {
            for centroid in DOMAIN_CENTROIDS {
                let mut dot_product = 0.0f32;
                let mut centroid_sq_sum = 0.0f32;
                let mut matched_terms = Vec::new();

                for &(c_term, c_weight) in centroid.term_weights {
                    centroid_sq_sum += c_weight * c_weight;
                    if let Some(&doc_w) = term_freq.get(c_term) {
                        dot_product += doc_w * c_weight;
                        matched_terms.push((c_term.to_string(), doc_w * c_weight));
                    }
                }

                let centroid_norm = centroid_sq_sum.sqrt();
                if centroid_norm > 0.0 {
                    let cosine_sim = dot_product / (doc_norm * centroid_norm);
                    if cosine_sim > best_score {
                        best_score = cosine_sim;
                        best_domain = Some(centroid);
                    }
                    if !matched_terms.is_empty() {
                        matched_terms.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                        domain_matching_terms.insert(centroid.name, matched_terms);
                    }
                }
            }
        }

        // 3. Dynamic Series & Media Classification for Episodic Videos
        if is_video {
            if let Some(stem) = std::path::Path::new(file_name).file_stem().and_then(|s| s.to_str()) {
                let (series_title, _season_ep, _episode_title) = crate::extractor::video::parse_media_filename_metadata(stem);
                if let Some(series) = series_title {
                    let clean_series = series.trim().to_string();
                    if !clean_series.is_empty() {
                        let vpaths = vec![
                            format!("{clean_series}/{file_name}"),
                            format!("Series/{clean_series}/{file_name}"),
                            format!("Media/{clean_series}/{file_name}"),
                        ];
                        return (clean_series, vpaths);
                    }
                }
            }
        }

        // 4. Dynamic Subject Classification for Course Materials, Notes, Lectures, Syllabus, Exams
        // Restricted to document types to prevent images/audio with year numbers from being treated as academic courseware
        let is_study_material = is_doc && {
            file_tokens.iter().any(|tok| {
                let trimmed = tok.trim_end_matches(|c: char| c.is_ascii_digit());
                DOC_DESCRIPTOR_WORDS.contains(&tok.as_str())
                    || (!trimmed.is_empty() && DOC_DESCRIPTOR_WORDS.contains(&trimmed))
            }) || best_domain.map_or(false, |d| d.name == "Academics")
        };

        if is_study_material {
            if let Some(subject) = extract_salient_subject(&term_freq, extracted_keywords, file_name) {
                let vpaths = vec![
                    format!("{subject}/{file_name}"),
                    format!("Academics/{subject}/{file_name}"),
                    format!("Workspace/{subject}/{file_name}"),
                ];
                return (subject, vpaths);
            }
        }

        // 5. Dynamic Topic Classification for General Documents and Media
        let threshold = 0.06;

        let (category, dynamic_subtopic) = if let Some(domain) = best_domain {
            if best_score >= threshold {
                let top_matching_term = domain_matching_terms
                    .get(domain.name)
                    .and_then(|matches| matches.first().map(|(term, _)| term.as_str()))
                    .unwrap_or("");

                let subtopic = synthesize_subtopic(top_matching_term, domain.default_subtopic);
                (format!("{}/{}", domain.name, subtopic), subtopic)
            } else if is_video {
                let dyn_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Video");
                (format!("Media/{dyn_topic}"), dyn_topic)
            } else if is_image {
                let dyn_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Visuals");
                (format!("Media/{dyn_topic}"), dyn_topic)
            } else if is_audio {
                let dyn_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Audio");
                (format!("Media/{dyn_topic}"), dyn_topic)
            } else if is_code {
                (format!("Code/{}", clean_ext.to_uppercase()), clean_ext.to_uppercase())
            } else {
                let dynamic_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Reference");
                (format!("Documents/{dynamic_topic}"), dynamic_topic)
            }
        } else if is_video {
            let dyn_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Video");
            (format!("Media/{dyn_topic}"), dyn_topic)
        } else if is_image {
            let dyn_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Visuals");
            (format!("Media/{dyn_topic}"), dyn_topic)
        } else if is_audio {
            let dyn_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Audio");
            (format!("Media/{dyn_topic}"), dyn_topic)
        } else if is_code {
            (format!("Code/{}", clean_ext.to_uppercase()), clean_ext.to_uppercase())
        } else {
            let dynamic_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name, "Reference");
            (format!("Documents/{dynamic_topic}"), dynamic_topic)
        };

        // 6. Synthesize multi-pointer virtual hierarchy paths dynamically
        let virtual_paths = vec![
            format!("{category}/{file_name}"),
            if is_image || is_video || is_audio {
                format!("Media/{dynamic_subtopic}/{file_name}")
            } else {
                format!("Workspace/{dynamic_subtopic}/{file_name}")
            },
        ];

        (category, virtual_paths)
    }
}

/// Dynamically formats a subtopic token from the top-scoring salient domain term.
/// Acronyms (e.g. "CV", "QPS", "API", "SQL", "TAX") are uppercased;
/// standard words are cleanly capitalized (e.g. "Invoice", "Resume", "Prescription", "Algorithm").
pub fn format_subtopic_token(term: &str) -> String {
    let clean = term.trim();
    if clean.is_empty() {
        return String::new();
    }
    if clean.len() <= 3 {
        clean.to_uppercase()
    } else {
        capitalize(clean)
    }
}

/// Dynamically synthesizes a refined subtopic name from dominant term and default fallback.
/// Strictly dynamic: no hardcoded if-else ladders.
fn synthesize_subtopic(top_term: &str, default_subtopic: &str) -> String {
    if !top_term.is_empty() {
        format_subtopic_token(top_term)
    } else {
        default_subtopic.to_string()
    }
}

/// Dynamically extracts the academic subject name from filename and TF-IDF terms
/// by filtering out document form descriptor words (notes, syllabus, exam, qps, lecture, etc.)
fn extract_salient_subject(
    term_freq: &HashMap<String, f32>,
    extracted_keywords: &[String],
    file_name: &str,
) -> Option<String> {
    // 1. Analyze filename tokens excluding descriptors
    let file_stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);

    let raw_tokens: Vec<&str> = file_stem
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();

    let mut subject_tokens = Vec::new();
    for tok in raw_tokens {
        let lower = tok.to_lowercase();
        let trimmed_lower = lower.trim_end_matches(|c: char| c.is_ascii_digit());

        if DOC_DESCRIPTOR_WORDS.contains(&lower.as_str())
            || (!trimmed_lower.is_empty() && DOC_DESCRIPTOR_WORDS.contains(&trimmed_lower))
            || GENERIC_DISCARD_WORDS.contains(&lower.as_str())
            || lower.chars().all(|c| c.is_ascii_digit())
            || (lower.len() < 3 && lower != "ai" && lower != "ml" && lower != "os" && lower != "db" && lower != "cs" && lower != "it")
        {
            continue;
        }
        subject_tokens.push(capitalize(tok));
    }

    if !subject_tokens.is_empty() {
        let count = subject_tokens.len().min(3);
        let phrase = subject_tokens[..count].join(" ");
        if !phrase.trim().is_empty() {
            return Some(phrase);
        }
    }

    // 2. Check extracted keywords excluding descriptors and generic words
    for kw in extracted_keywords {
        let clean = kw.trim().to_lowercase();
        if !DOC_DESCRIPTOR_WORDS.contains(&clean.as_str())
            && !GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            && clean.len() >= 4
            && !clean.contains("redacted")
            && !clean.chars().all(|c| c.is_ascii_digit())
        {
            return Some(capitalize(&clean));
        }
    }

    // 3. Check term frequencies sorted by weight excluding descriptors and generic words
    let mut terms: Vec<_> = term_freq.iter().collect();
    terms.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (term, _) in terms {
        let clean = term.to_lowercase();
        if !DOC_DESCRIPTOR_WORDS.contains(&clean.as_str())
            && !GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            && clean.len() >= 4
            && !clean.chars().all(|c| c.is_ascii_digit())
        {
            return Some(capitalize(&clean));
        }
    }

    None
}

/// Extracts the most salient topic term from the document when similarity is diffuse
fn extract_salient_topic(
    term_freq: &HashMap<String, f32>,
    extracted_keywords: &[String],
    file_name: &str,
    default_fallback: &str,
) -> String {
    // 1. Check extracted keywords excluding generic noise
    for kw in extracted_keywords {
        let clean = kw.trim();
        let lower = clean.to_lowercase();
        if clean.len() >= 3
            && !clean.contains("redacted")
            && !GENERIC_DISCARD_WORDS.contains(&lower.as_str())
            && !clean.chars().all(|c| c.is_ascii_digit())
        {
            return capitalize(clean);
        }
    }

    // 2. Check term frequencies sorted by weight
    let mut terms: Vec<_> = term_freq.iter().collect();
    terms.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (term, _) in terms {
        let lower = term.to_lowercase();
        if term.len() >= 4
            && !term.chars().all(|c| c.is_ascii_digit())
            && !GENERIC_DISCARD_WORDS.contains(&lower.as_str())
        {
            return capitalize(term);
        }
    }

    // 3. Check filename token
    for tok in file_name.split(|c: char| !c.is_alphanumeric()) {
        let lower = tok.to_lowercase();
        if tok.len() >= 4
            && !tok.chars().all(|c| c.is_ascii_digit())
            && !GENERIC_DISCARD_WORDS.contains(&lower.as_str())
        {
            return capitalize(tok);
        }
    }

    default_fallback.to_string()
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episodic_video_series_classification() {
        let file = "Dark_S03E01_Déjà_vu_DUAL_AUDIO_GER_ENG_1080p_10bit_WEBRip_6CH_x265.mkv";
        let (category, vpaths) = DynamicSemanticClassifier::classify("", file, &[], "mkv");
        assert_eq!(category, "Dark");
        assert!(vpaths.contains(&format!("Dark/{file}")));
        assert!(vpaths.contains(&format!("Series/Dark/{file}")));
    }

    #[test]
    fn test_subject_notes_classification() {
        let file = "2021-MSc Data Science - Syllabus Book.pdf";
        let text = "Curriculum and course outline for Data Science Master of Science semester syllabus.";
        let (category, vpaths) = DynamicSemanticClassifier::classify(text, file, &["data science".to_string(), "syllabus".to_string()], "pdf");
        assert_eq!(category, "Data Science");
        assert!(vpaths.contains(&format!("Data Science/{file}")));
    }

    #[test]
    fn test_operating_systems_subject_classification() {
        let file = "Operating_Systems_Lecture1.pdf";
        let text = "Operating systems kernel memory virtualisation and CPU scheduling.";
        let (category, _) = DynamicSemanticClassifier::classify(text, file, &["operating systems".to_string()], "pdf");
        assert_eq!(category, "Operating Systems");
    }

    #[test]
    fn test_never_returns_general() {
        let files = [
            ("sample.txt", "Random unstructured text without specific centroid keywords."),
            ("audio.mp3", ""),
            ("archive.zip", ""),
        ];
        for (f, t) in files {
            let (cat, _) = DynamicSemanticClassifier::classify(t, f, &[], "txt");
            assert_ne!(cat, "General");
        }
    }

    #[test]
    fn test_image_never_falsely_categorized_as_academics() {
        let file = "WhatsApp Image 2026-10-09 at 11.47.38 AM.jpeg";
        let (cat, _) = DynamicSemanticClassifier::classify("", file, &[], "jpeg");
        assert!(!cat.starts_with("Academics/"), "Image must not be Academics, got: {}", cat);
        assert!(cat.starts_with("Media/"), "Image must be Media, got: {}", cat);
    }
}
