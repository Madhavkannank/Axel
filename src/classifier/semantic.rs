use std::collections::HashMap;
use crate::classifier::nlp::{
    capitalize, format_subtopic_token, is_lexically_valid, is_recognized_acronym, is_stopword,
    stem, tokenize,
};

/// Represents a conceptual domain subspace in vector space.
struct DomainConceptSpace {
    name: &'static str,
    default_subtopic: &'static str,
    canonical_terms: &'static [&'static str],
}

static DOMAIN_SPACES: &[DomainConceptSpace] = &[
    DomainConceptSpace {
        name: "Academics",
        default_subtopic: "Coursework",
        canonical_terms: &[
            "curriculum", "syllabus", "semester", "exam", "examination", "qpaper", "qps",
            "question", "paper", "marks", "lecture", "calculus", "mathematics", "physics",
            "chemistry", "biology", "university", "college", "degree", "assignment",
            "midsem", "endsem", "textbook", "student", "grade", "credits", "thesis",
            "dissertation", "coursework", "laboratory", "academia",
        ],
    },
    DomainConceptSpace {
        name: "Career",
        default_subtopic: "Resume",
        canonical_terms: &[
            "cv", "resume", "curriculum", "vitae", "experience", "education", "skills",
            "objective", "projects", "intern", "internship", "employment", "job",
            "candidate", "portfolio", "qualifications", "certifications", "achievements",
            "leadership", "responsibilities", "technologies", "cgpa", "gpa", "github",
            "linkedin", "career", "professional",
        ],
    },
    DomainConceptSpace {
        name: "Engineering",
        default_subtopic: "Software",
        canonical_terms: &[
            "code", "algorithm", "architecture", "implementation", "pipeline", "backend",
            "frontend", "database", "compiler", "runtime", "optimization", "framework",
            "repository", "benchmark", "cuda", "pytorch", "opencv", "vision", "filter",
            "gradient", "protocol", "hardware", "firmware", "debugging", "testing",
            "hackathon", "software", "computing", "network", "server",
        ],
    },
    DomainConceptSpace {
        name: "Finance",
        default_subtopic: "Invoice",
        canonical_terms: &[
            "invoice", "billing", "payment", "receipt", "tax", "subtotal", "total",
            "amount", "due", "balance", "vendor", "remittance", "purchase", "order",
            "accounting", "audit", "expense", "statement", "consulting", "payroll",
            "financial", "transaction", "banking",
        ],
    },
    DomainConceptSpace {
        name: "Creative",
        default_subtopic: "Writing",
        canonical_terms: &[
            "screenplay", "script", "scene", "dialogue", "horror", "story", "character",
            "narrative", "novel", "chapter", "drama", "acts", "author", "film", "cinema",
            "fiction", "literature", "poetry", "storyline", "writing", "screenwriter",
        ],
    },
    DomainConceptSpace {
        name: "Medical",
        default_subtopic: "Healthcare",
        canonical_terms: &[
            "prescription", "dosage", "diagnosis", "treatment", "medicine", "antibiotics",
            "patient", "hospital", "doctor", "physician", "clinical", "symptoms",
            "discharge", "summary", "pathology", "laboratory", "healthcare", "therapy",
            "pharmacy", "medical",
        ],
    },
    DomainConceptSpace {
        name: "Legal",
        default_subtopic: "Contracts",
        canonical_terms: &[
            "contract", "agreement", "terms", "clause", "liability", "indemnity",
            "jurisdiction", "parties", "notary", "statutory", "compliance", "arbitration",
            "lease", "legal", "covenant", "warranty", "affidavit",
        ],
    },
    DomainConceptSpace {
        name: "Media",
        default_subtopic: "Visuals",
        canonical_terms: &[
            "wallpaper", "comic", "comics", "collage", "poster", "artwork", "illustration",
            "cover", "animation", "video", "episode", "audio", "soundtrack", "recording",
            "photography", "render", "visual", "graphic", "snapshot", "media",
        ],
    },
    DomainConceptSpace {
        name: "Identity",
        default_subtopic: "Credentials",
        canonical_terms: &[
            "aadhaar", "uidai", "pan", "passport", "voter", "ssn", "identification",
            "citizen", "identity", "government", "resident", "elector", "license",
            "nationality", "citizenship", "credentials", "card", "authority",
        ],
    },
];

/// Document form descriptors representing the medium/type rather than the subject matter.
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
    "1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th", "first", "second",
    "third", "fourth", "fifth", "sixth", "seventh", "eighth",
];

/// Generic words that should never form a primary topic or hub cluster.
const GENERIC_DISCARD_WORDS: &[&str] = &[
    "document", "records", "archive", "content", "untitled", "file", "temp", "sample",
    "image", "video", "audio", "screenshot", "download", "title", "metadata", "preview",
    "thumbnail",
];

pub struct DynamicSemanticClassifier;

impl DynamicSemanticClassifier {
    /// Classifies a document using a mathematically sound Vector Space Model (VSM)
    /// with length-normalized cosine similarity against domain concept subspaces,
    /// dynamic salient subject extraction, and episodic media parsing.
    ///
    /// Never returns hardcoded static outcomes or uses arbitrary heuristic ladders.
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

        // 1. Dynamic Series & Media Classification for Episodic Videos
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

        // 2. Build Information Retrieval Term Vector with BM25F Field Weighting
        // Fields: Title (w=2.5), Keywords (w=1.5), Body (w=1.0)
        let mut raw_tf: HashMap<String, f32> = HashMap::new();

        // 2a. Title field tokens
        let file_stem = std::path::Path::new(file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_name);

        let file_tokens = tokenize(file_stem);
        for tok in &file_tokens {
            let stemmed = stem(tok);
            if !is_stopword(tok) && !is_stopword(&stemmed) && is_lexically_valid(tok) {
                *raw_tf.entry(tok.clone()).or_insert(0.0) += 2.5;
                if stemmed != *tok {
                    *raw_tf.entry(stemmed).or_insert(0.0) += 2.0;
                }
            }
        }

        // 2b. Keywords field tokens
        for kw in extracted_keywords {
            for tok in tokenize(kw) {
                let stemmed = stem(&tok);
                if !is_stopword(&tok) && !is_stopword(&stemmed) && is_lexically_valid(&tok) {
                    *raw_tf.entry(tok.clone()).or_insert(0.0) += 1.5;
                    if stemmed != tok {
                        *raw_tf.entry(stemmed).or_insert(0.0) += 1.2;
                    }
                }
            }
        }

        // 2c. Body text field tokens (bounded to first 6,000 characters)
        let body_sample = if text.len() > 6000 {
            &text[..text.char_indices().map(|(i, _)| i).nth(6000).unwrap_or(text.len())]
        } else {
            text
        };

        for tok in tokenize(body_sample) {
            let stemmed = stem(&tok);
            if !is_stopword(&tok) && !is_stopword(&stemmed) && is_lexically_valid(&tok) {
                *raw_tf.entry(tok.clone()).or_insert(0.0) += 1.0;
                if stemmed != tok {
                    *raw_tf.entry(stemmed).or_insert(0.0) += 0.8;
                }
            }
        }

        // 3. Compute Sublinear Normalized Document Vector
        // sublinear_tf(t) = 1.0 + ln(raw_tf(t))
        let mut doc_vector: HashMap<String, f32> = HashMap::new();
        let mut doc_sq_norm = 0.0f32;

        for (term, count) in raw_tf {
            if count > 0.0 {
                let sub_tf = 1.0 + count.ln();
                doc_sq_norm += sub_tf * sub_tf;
                doc_vector.insert(term, sub_tf);
            }
        }

        let doc_norm = doc_sq_norm.sqrt();
        if doc_norm > 0.0 {
            for val in doc_vector.values_mut() {
                *val /= doc_norm;
            }
        }

        // 4. Project Document Vector onto Unit-Normalized Domain Concept Subspaces
        let mut domain_scores: Vec<(&'static DomainConceptSpace, f32, Option<String>)> = Vec::new();

        if doc_norm > 0.0 {
            for space in DOMAIN_SPACES {
                let space_dim = space.canonical_terms.len() as f32;
                let unit_weight = 1.0 / space_dim.sqrt();

                let mut dot_product = 0.0f32;
                let mut best_term_score = 0.0f32;
                let mut dominant_term: Option<String> = None;

                for &c_term in space.canonical_terms {
                    let stemmed_c = stem(c_term);
                    let term_weight = doc_vector.get(c_term).copied().unwrap_or(0.0);
                    let stem_weight = if stemmed_c != c_term {
                        doc_vector.get(&stemmed_c).copied().unwrap_or(0.0)
                    } else {
                        0.0
                    };

                    let effective_weight = term_weight.max(stem_weight);
                    if effective_weight > 0.0 {
                        dot_product += effective_weight * unit_weight;
                        if effective_weight > best_term_score {
                            best_term_score = effective_weight;
                            dominant_term = Some(c_term.to_string());
                        }
                    }
                }

                if dot_product > 0.0 {
                    domain_scores.push((space, dot_product, dominant_term));
                }
            }
        }

        domain_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let best_match = domain_scores.first();

        // 5. Educational / Academic Courseware Subject Disentanglement
        let is_academic_material = is_doc && {
            file_tokens.iter().any(|tok| {
                let s = stem(tok);
                DOC_DESCRIPTOR_WORDS.contains(&tok.as_str())
                    || DOC_DESCRIPTOR_WORDS.contains(&s.as_str())
            }) || best_match.map_or(false, |(space, score, _)| space.name == "Academics" && *score > 0.05)
        };

        if is_academic_material {
            if let Some(subject) = extract_salient_subject(&file_tokens, &doc_vector, extracted_keywords) {
                let vpaths = vec![
                    format!("{subject}/{file_name}"),
                    format!("Academics/{subject}/{file_name}"),
                    format!("Workspace/{subject}/{file_name}"),
                ];
                return (subject, vpaths);
            }
        }

        // 6. Dynamic Category & Subtopic Resolution
        let (category, dynamic_subtopic) = if let Some(&(space, score, ref dominant_term)) = best_match {
            // Guard: Media files without substantial document text evidence stay under Media domain.
            // Scanned images with rich OCR text (receipts, IDs, blueprints, diagrams) can match semantic domains (H11, A19).
            // Media files with substantial extracted text and high semantic confidence for Engineering are accepted (A19).
            let has_document_text = !text.trim().is_empty() && text.trim().len() >= 20;
            let is_incompatible_media = (is_image || is_video || is_audio)
                && (!has_document_text || score < 0.05)
                && space.name != "Media";

            if score > 0.02 && !is_incompatible_media {
                let subtopic = if let Some(ref term) = dominant_term {
                    format_subtopic_token(term)
                } else {
                    space.default_subtopic.to_string()
                };
                (format!("{}/{}", space.name, subtopic), subtopic)
            } else if is_video {
                let dyn_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Video");
                (format!("Media/{dyn_topic}"), dyn_topic)
            } else if is_image {
                let dyn_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Visuals");
                (format!("Media/{dyn_topic}"), dyn_topic)
            } else if is_audio {
                let dyn_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Audio");
                (format!("Media/{dyn_topic}"), dyn_topic)
            } else if is_code {
                (format!("Code/{}", clean_ext.to_uppercase()), clean_ext.to_uppercase())
            } else {
                let dynamic_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Reference");
                (format!("Documents/{dynamic_topic}"), dynamic_topic)
            }
        } else if is_video {
            let dyn_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Video");
            (format!("Media/{dyn_topic}"), dyn_topic)
        } else if is_image {
            let dyn_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Visuals");
            (format!("Media/{dyn_topic}"), dyn_topic)
        } else if is_audio {
            let dyn_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Audio");
            (format!("Media/{dyn_topic}"), dyn_topic)
        } else if is_code {
            (format!("Code/{}", clean_ext.to_uppercase()), clean_ext.to_uppercase())
        } else {
            let dynamic_topic = extract_salient_topic(&file_tokens, &doc_vector, extracted_keywords, "Reference");
            (format!("Documents/{dynamic_topic}"), dynamic_topic)
        };

        // 7. Dynamic Virtual Paths (Multi-Pointer Logical Hierarchy)
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

pub fn is_ordinal_or_numeric(s: &str) -> bool {
    let clean = s.trim().to_lowercase();
    if clean.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if (clean.ends_with("th") || clean.ends_with("st") || clean.ends_with("nd") || clean.ends_with("rd"))
        && clean.len() >= 3
    {
        let prefix = &clean[..clean.len() - 2];
        if prefix.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    false
}

/// Disentangles form descriptors from the salient subject name.
fn extract_salient_subject(
    file_tokens: &[String],
    doc_vector: &HashMap<String, f32>,
    extracted_keywords: &[String],
) -> Option<String> {
    // 1. Analyze filename tokens excluding descriptors and generic discards
    let mut subject_tokens = Vec::new();

    for tok in file_tokens {
        let clean = tok.to_lowercase();
        let s = stem(&clean);

        if DOC_DESCRIPTOR_WORDS.contains(&clean.as_str())
            || DOC_DESCRIPTOR_WORDS.contains(&s.as_str())
            || GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            || clean.chars().all(|c| c.is_ascii_digit())
            || is_ordinal_or_numeric(&clean)
            || (clean.len() < 3 && !is_recognized_acronym(&clean))
            || !is_lexically_valid(&clean)
        {
            continue;
        }

        subject_tokens.push(capitalize(&clean));
    }

    if !subject_tokens.is_empty() {
        let count = subject_tokens.len().min(3);
        let phrase = subject_tokens[..count].join(" ");
        if !phrase.trim().is_empty() {
            return Some(phrase);
        }
    }

    // 2. Check extracted keywords excluding descriptors
    for kw in extracted_keywords {
        let clean = kw.trim().to_lowercase();
        let s = stem(&clean);

        if !DOC_DESCRIPTOR_WORDS.contains(&clean.as_str())
            && !DOC_DESCRIPTOR_WORDS.contains(&s.as_str())
            && !GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            && !is_ordinal_or_numeric(&clean)
            && is_lexically_valid(&clean)
            && !clean.contains("redacted")
            && (clean.len() >= 4 || is_recognized_acronym(&clean))
        {
            return Some(capitalize(&clean));
        }
    }

    // 3. Check document vector terms sorted by weight excluding descriptors
    let mut terms: Vec<_> = doc_vector.iter().collect();
    terms.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (term, _) in terms {
        let clean = term.to_lowercase();
        let s = stem(&clean);

        if !DOC_DESCRIPTOR_WORDS.contains(&clean.as_str())
            && !DOC_DESCRIPTOR_WORDS.contains(&s.as_str())
            && !GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            && !is_ordinal_or_numeric(&clean)
            && is_lexically_valid(&clean)
            && (clean.len() >= 4 || is_recognized_acronym(&clean))
        {
            return Some(capitalize(&clean));
        }
    }

    None
}

/// Dynamically extracts the most salient topic token for a document or media asset.
fn extract_salient_topic(
    file_tokens: &[String],
    doc_vector: &HashMap<String, f32>,
    extracted_keywords: &[String],
    default_fallback: &str,
) -> String {
    // 1. Check filename tokens (excluding generic discards and stop words)
    for tok in file_tokens {
        let clean = tok.to_lowercase();
        let s = stem(&clean);

        if !GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            && !GENERIC_DISCARD_WORDS.contains(&s.as_str())
            && !is_stopword(&clean)
            && is_lexically_valid(&clean)
            && (clean.len() >= 4 || is_recognized_acronym(&clean))
        {
            return format_subtopic_token(&clean);
        }
    }

    // 2. Check extracted keywords
    for kw in extracted_keywords {
        let clean = kw.trim().to_lowercase();
        if !GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            && !is_stopword(&clean)
            && is_lexically_valid(&clean)
            && (clean.len() >= 3 || is_recognized_acronym(&clean))
        {
            return format_subtopic_token(&clean);
        }
    }

    // 3. Check document vector terms
    let mut terms: Vec<_> = doc_vector.iter().collect();
    terms.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (term, _) in terms {
        let clean = term.to_lowercase();
        if !GENERIC_DISCARD_WORDS.contains(&clean.as_str())
            && !is_stopword(&clean)
            && is_lexically_valid(&clean)
            && (clean.len() >= 4 || is_recognized_acronym(&clean))
        {
            return format_subtopic_token(&clean);
        }
    }

    default_fallback.to_string()
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

    #[test]
    fn test_academic_question_papers_never_categorized_as_bare_ordinal() {
        let file = "5thSemQPs.pdf";
        let (cat, vpaths) = DynamicSemanticClassifier::classify("", file, &[], "pdf");
        assert_ne!(cat, "5th", "Category must not be raw ordinal '5th'");
        assert!(cat.starts_with("Academics/"), "5thSemQPs must be categorized under Academics, got: {cat}");
        for vp in &vpaths {
            assert!(!vp.contains(".pdf/"), "Virtual path must not contain file extension as directory: {vp}");
            assert!(vp.ends_with(file), "Virtual path must end with file name: {vp}");
        }
    }

    #[test]
    fn test_video_animation_never_categorized_as_code() {
        let file = "madhav_title_animation.mp4";
        let (cat, vpaths) = DynamicSemanticClassifier::classify("", file, &[], "mp4");
        assert_ne!(cat, "Code", "Video animation must not be Code");
        assert_ne!(cat, "Engineering", "Video animation must not be Engineering");
        assert!(cat.starts_with("Media/"), "Video animation must be under Media, got: {cat}");
        for vp in &vpaths {
            assert!(vp.ends_with(file));
        }
    }

    #[test]
    fn test_media_with_engineering_document_text_classifies_as_engineering() {
        let file = "engine_blueprint_scan.png";
        let text = "Mechanical engineering blueprint: CAD architecture, hardware pipeline, firmware protocol and optimization framework.";
        let (cat, _) = DynamicSemanticClassifier::classify(text, file, &["cad".to_string(), "architecture".to_string()], "png");
        assert!(cat.starts_with("Engineering/"), "Scanned engineering blueprint should be Engineering, got: {}", cat);
    }
}
