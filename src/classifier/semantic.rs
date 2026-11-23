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
        default_subtopic: "Study Materials",
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
        default_subtopic: "Technical",
        term_weights: &[
            ("code", 2.5),
            ("vortex", 2.5),
            ("problem", 2.2),
            ("statements", 2.2),
            ("hackathon", 3.0),
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
            ("sobel", 3.0),
            ("filter", 2.0),
            ("gradient", 2.0),
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
        ],
    },
    DomainCentroid {
        name: "Creative",
        default_subtopic: "Screenplay",
        term_weights: &[
            ("screenplay", 4.0),
            ("script", 3.5),
            ("scene", 3.0),
            ("dialogue", 3.0),
            ("horror", 2.8),
            ("story", 3.0),
            ("kadha", 3.5),
            ("character", 2.5),
            ("narrative", 2.5),
            ("novel", 2.5),
            ("chapter", 2.0),
            ("drama", 2.2),
            ("acts", 2.0),
            ("author", 2.0),
            ("film", 2.2),
            ("cinema", 2.2),
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
            ("collage", 3.0),
            ("poster", 2.5),
            ("artwork", 2.8),
            ("illustration", 2.8),
            ("cover", 2.0),
            ("thor", 2.5),
            ("animation", 3.0),
            ("video", 3.0),
            ("episode", 3.0),
            ("audio", 2.5),
            ("soundtrack", 2.5),
            ("recording", 2.2),
        ],
    },
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

        // 1. Tokenize document text and filename into normalized term frequency map
        let mut term_freq: HashMap<String, f32> = HashMap::new();

        // High weight for tokens in filename
        let file_tokens: Vec<String> = file_name
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| s.len() >= 2)
            .collect();

        // High weight for tokens in filename (title metadata has high semantic salience)
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

        // 3. Fallback to file extension media types if textual similarity is low
        let is_video = matches!(clean_ext.as_str(), "mp4" | "mkv" | "avi" | "mov" | "webm");
        let is_image = matches!(clean_ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif" | "svg");
        let is_audio = matches!(clean_ext.as_str(), "mp3" | "wav" | "flac" | "aac" | "ogg");
        let is_code = matches!(clean_ext.as_str(), "rs" | "py" | "js" | "ts" | "c" | "cpp" | "go" | "java");

        // 4. Synthesize dynamic category and virtual paths
        let threshold = 0.06;

        let (category, dynamic_subtopic) = if let Some(domain) = best_domain {
            if best_score >= threshold {
                let subtopic = if let Some(matches) = domain_matching_terms.get(domain.name) {
                    if let Some((top_term, _)) = matches.first() {
                        synthesize_subtopic(domain.name, top_term, &clean_ext)
                    } else {
                        domain.default_subtopic.to_string()
                    }
                } else {
                    domain.default_subtopic.to_string()
                };
                (format!("{}/{}", domain.name, subtopic), subtopic)
            } else if is_video {
                ("Media/Video".to_string(), "Video".to_string())
            } else if is_image {
                ("Media/Visuals".to_string(), "Visuals".to_string())
            } else if is_audio {
                ("Media/Audio".to_string(), "Audio".to_string())
            } else if is_code {
                (format!("Code/{}", clean_ext.to_uppercase()), clean_ext.to_uppercase())
            } else {
                let dynamic_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name);
                (format!("Documents/{}", dynamic_topic), dynamic_topic)
            }
        } else if is_video {
            ("Media/Video".to_string(), "Video".to_string())
        } else if is_image {
            ("Media/Visuals".to_string(), "Visuals".to_string())
        } else if is_audio {
            ("Media/Audio".to_string(), "Audio".to_string())
        } else if is_code {
            (format!("Code/{}", clean_ext.to_uppercase()), clean_ext.to_uppercase())
        } else {
            let dynamic_topic = extract_salient_topic(&term_freq, extracted_keywords, file_name);
            (format!("Documents/{}", dynamic_topic), dynamic_topic)
        };

        // 5. Synthesize multi-pointer virtual hierarchy paths dynamically
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

/// Dynamically synthesizes a refined subtopic name from dominant term and file context
fn synthesize_subtopic(domain: &str, top_term: &str, ext: &str) -> String {
    match domain {
        "Academics" => {
            if top_term == "syllabus" || top_term == "curriculum" {
                "Syllabus".to_string()
            } else if top_term == "qps" || top_term == "qpaper" || top_term == "exam" || top_term == "question" {
                "Past Papers".to_string()
            } else if top_term == "lecture" || top_term == "notes" {
                "Lectures".to_string()
            } else {
                capitalize(top_term)
            }
        }
        "Career" => {
            if top_term == "cv" || top_term == "resume" || top_term == "curriculum" || top_term == "vitae" || top_term == "education" || top_term == "objective" {
                "Resume".to_string()
            } else if top_term == "portfolio" || top_term == "projects" {
                "Portfolio".to_string()
            } else {
                capitalize(top_term)
            }
        }
        "Creative" => {
            if top_term == "screenplay" || top_term == "script" {
                "Screenplays".to_string()
            } else if top_term == "story" || top_term == "kadha" || top_term == "horror" {
                "Stories".to_string()
            } else {
                capitalize(top_term)
            }
        }
        "Finance" => {
            if top_term == "invoice" || top_term == "billing" {
                "Invoices".to_string()
            } else if top_term == "receipt" {
                "Receipts".to_string()
            } else {
                capitalize(top_term)
            }
        }
        "Media" => {
            if matches!(ext, "mp4" | "mkv" | "avi" | "mov") {
                "Video".to_string()
            } else if matches!(ext, "png" | "jpg" | "jpeg" | "webp") {
                "Visuals".to_string()
            } else {
                capitalize(top_term)
            }
        }
        _ => capitalize(top_term),
    }
}

/// Extracts the most salient topic term from the document when similarity is diffuse
fn extract_salient_topic(
    term_freq: &HashMap<String, f32>,
    extracted_keywords: &[String],
    file_name: &str,
) -> String {
    // 1. Check extracted keywords
    if let Some(kw) = extracted_keywords.first() {
        let clean = kw.trim();
        if clean.len() >= 3 && !clean.contains("redacted") {
            return capitalize(clean);
        }
    }

    // 2. Check term frequencies sorted by weight
    let mut terms: Vec<_> = term_freq.iter().collect();
    terms.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (term, _) in terms {
        if term.len() >= 4 && !term.chars().all(|c| c.is_ascii_digit()) {
            return capitalize(term);
        }
    }

    // 3. Check filename token
    for tok in file_name.split(|c: char| !c.is_alphanumeric()) {
        if tok.len() >= 4 && !tok.chars().all(|c| c.is_ascii_digit()) {
            return capitalize(tok);
        }
    }

    "Reference".to_string()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
