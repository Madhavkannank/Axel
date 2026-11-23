pub mod groq;

use crate::classifier::{
    extract_code_symbols, is_high_risk_identity_document, redact_sensitive_pii, CodeSymbols,
    KeywordExtractor,
};
use crate::config::Config;
pub use groq::{CategoryResult, GroqClient};

pub struct AiEngine {
    groq: Option<GroqClient>,
    keywords: KeywordExtractor,
    max_keywords: usize,
}

impl AiEngine {
    pub fn new(config: &Config) -> Self {
        let groq = config.groq_api_key.as_ref().map(|key| {
            GroqClient::new(
                key.clone(),
                config.model_name.clone(),
                config.model_fallbacks.clone(),
            )
        });

        Self {
            groq,
            keywords: KeywordExtractor::new(),
            max_keywords: config.max_keywords,
        }
    }

    pub async fn analyze_document(
        &self,
        text: &str,
        file_name: &str,
        suggested_tfidf_kws: Option<&[String]>,
    ) -> CategoryResult {
        let effective_text = if text.trim().is_empty() {
            file_name.replace(['_', '-', '.'], " ")
        } else {
            text.to_string()
        };

        // Step 1: Privacy Shield - redact sensitive personal identifiers
        // (Aadhaar, PAN, SSN, Credit Cards, personal names, DOB)
        let (sanitized_text, redaction_summary) = redact_sensitive_pii(&effective_text);

        // Step 2: Statistical keywords via KeywordExtractor & TF-IDF on sanitized text
        let mut candidate_kws = self.keywords.extract(&sanitized_text, self.max_keywords);
        if let Some(tfidf_kws) = suggested_tfidf_kws {
            for kw in tfidf_kws {
                let clean = kw.to_lowercase();
                if !clean.contains("redacted") && !candidate_kws.contains(&clean) {
                    candidate_kws.push(clean);
                }
            }
        }

        // Step 3: Privacy boundary check. High-risk government identity documents
        // (Aadhaar, PAN, SSN, Passport, etc.) are strictly kept local and NEVER
        // transmitted to external cloud LLM APIs.
        let is_sensitive_identity = is_high_risk_identity_document(&effective_text, file_name);

        if !is_sensitive_identity {
            if let Some(ref client) = self.groq {
                let prompt_text = if !sanitized_text.trim().is_empty() {
                    sanitized_text.clone()
                } else {
                    format!("File name: {file_name}. Infer category, keywords, and virtual_paths.")
                };

                if let Some(mut llm_res) = client.categorize(&prompt_text, &candidate_kws).await {
                    llm_res.keywords = self.keywords.clean_keywords(llm_res.keywords, &sanitized_text, self.max_keywords);
                    return llm_res;
                }
            }
        }

        // Step 4: Dynamic categorization when Groq is offline, not configured, or document is sensitive identity.
        // Uses mathematical vector space cosine similarity against domain centroids - zero hardcoded static strings.
        let (category, fallback_kws, virtual_paths) = if is_sensitive_identity {
            let mut kws = Vec::new();
            if redaction_summary.aadhaar_count > 0 {
                kws.push("aadhaar".to_string());
            }
            if redaction_summary.pan_count > 0 {
                kws.push("pan".to_string());
            }
            if redaction_summary.ssn_count > 0 {
                kws.push("ssn".to_string());
            }
            if !kws.contains(&"government id".to_string()) {
                kws.push("government id".to_string());
            }
            for kw in candidate_kws {
                if !kws.contains(&kw) {
                    kws.push(kw);
                }
            }
            (
                "Government ID".to_string(),
                kws,
                vec![format!("Government ID/{file_name}")],
            )
        } else {
            let ext = std::path::Path::new(file_name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let (dyn_cat, dyn_vpaths) = crate::classifier::DynamicSemanticClassifier::classify(
                &sanitized_text,
                file_name,
                &candidate_kws,
                ext,
            );
            (dyn_cat, candidate_kws, dyn_vpaths)
        };

        CategoryResult {
            category,
            keywords: self.keywords.clean_keywords(fallback_kws, &effective_text, self.max_keywords),
            virtual_paths,
        }
    }

    pub async fn analyze_code_script(
        &self,
        code: &str,
        file_name: &str,
        ext: &str,
        tfidf_kws: &[String],
    ) -> (CategoryResult, CodeSymbols) {
        let symbols = extract_code_symbols(code, ext);
        let mut candidate_kws = Vec::new();

        // 1. TF-IDF extracted keywords
        for kw in tfidf_kws {
            if !candidate_kws.contains(kw) {
                candidate_kws.push(kw.clone());
            }
        }

        // 2. Extracted identifier tokens (split snake_case / camelCase)
        for tok in &symbols.tokens {
            if tok.len() >= 3 && !candidate_kws.contains(tok) {
                candidate_kws.push(tok.clone());
            }
        }

        // 3. Extracted symbol names (functions, classes, variables)
        for sym in &symbols.symbols {
            if sym.len() >= 3 && !candidate_kws.contains(sym) {
                candidate_kws.push(sym.clone());
            }
        }

        // Step 2: Groq Semantic Discovery Layer
        if let Some(ref client) = self.groq {
            let code_snippet: String = code.lines().take(60).collect::<Vec<_>>().join("\n");
            if let Some(mut llm_res) = client
                .categorize_code(
                    &code_snippet,
                    file_name,
                    &symbols.symbols,
                    &symbols.tokens,
                )
                .await
            {
                // Blend Groq concepts with local tokens
                let mut final_kws = Vec::new();
                for kw in llm_res.keywords {
                    let clean = kw.trim().to_lowercase();
                    if !clean.is_empty() && !final_kws.contains(&clean) {
                        final_kws.push(clean);
                    }
                }
                for tok in &symbols.tokens {
                    let clean = tok.to_lowercase();
                    if !final_kws.contains(&clean) && final_kws.len() < 10 {
                        final_kws.push(clean);
                    }
                }
                for kw in tfidf_kws {
                    let clean = kw.to_lowercase();
                    if !final_kws.contains(&clean) && final_kws.len() < 10 {
                        final_kws.push(clean);
                    }
                }
                llm_res.keywords = final_kws;
                return (llm_res, symbols);
            }
        }

        // Step 3: Offline dynamic fallback without Groq
        let category = format!("Code/{}", ext.to_uppercase());
        candidate_kws.truncate(self.max_keywords.max(6));

        let vpaths = vec![
            format!("{category}/{file_name}"),
            format!("Language/{}/{file_name}", ext.to_uppercase()),
        ];

        (
            CategoryResult {
                category,
                keywords: candidate_kws,
                virtual_paths: vpaths,
            },
            symbols,
        )
    }
}
