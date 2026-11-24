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

        // Step 3: Privacy boundary check evaluated on complete document.
        // High-risk government identity documents (Aadhaar, PAN, SSN, Passport, etc.)
        // or documents containing verified sensitive IDs are strictly kept local
        // and NEVER transmitted to external cloud LLM APIs.
        let is_sensitive_identity = is_high_risk_identity_document(&effective_text, file_name)
            || redaction_summary.has_sensitive_id();

        if !is_sensitive_identity {
            if let Some(ref client) = self.groq {
                let prompt_text = if !sanitized_text.trim().is_empty() {
                    crate::extractor::preprocess_text(&sanitized_text)
                } else {
                    format!("File name: {file_name}. Infer category, keywords, and virtual_paths.")
                };

                if let Some(mut llm_res) = client.categorize(&prompt_text, &candidate_kws).await {
                    let ext = std::path::Path::new(file_name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");

                    // Dynamic fallback if LLM returned empty, generic, or invalid category
                    let is_generic_or_invalid = llm_res.category.trim().is_empty()
                        || llm_res.category.eq_ignore_ascii_case("general")
                        || llm_res.category.eq_ignore_ascii_case("video")
                        || llm_res.category.eq_ignore_ascii_case("media")
                        || llm_res.category.eq_ignore_ascii_case("unrecognized_text")
                        || llm_res.category.eq_ignore_ascii_case("unknown")
                        || llm_res.category.eq_ignore_ascii_case("unrecognized")
                        || llm_res.category.eq_ignore_ascii_case("none");

                    if is_generic_or_invalid {
                        let (dyn_cat, dyn_vpaths) = crate::classifier::DynamicSemanticClassifier::classify(
                            &sanitized_text,
                            file_name,
                            &candidate_kws,
                            ext,
                        );
                        llm_res.category = dyn_cat;
                        llm_res.virtual_paths = dyn_vpaths;
                    }

                    // Clean keywords strictly against sanitized text to prevent PII refill (H11)
                    llm_res.keywords = self.keywords.clean_keywords(llm_res.keywords, &sanitized_text, self.max_keywords);
                    return llm_res;
                }
            }
        }

        // Step 4: Dynamic categorization when Groq is offline, not configured, or document is sensitive identity.
        // Uses mathematical vector space cosine similarity against domain centroids - zero hardcoded static strings.
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

        CategoryResult {
            category: dyn_cat,
            keywords: self.keywords.clean_keywords(candidate_kws, &sanitized_text, self.max_keywords),
            virtual_paths: dyn_vpaths,
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
            let clean = kw.to_lowercase();
            if !clean.contains("redacted") && !candidate_kws.contains(&clean) {
                candidate_kws.push(clean);
            }
        }

        // 2. Extracted identifier tokens (split snake_case / camelCase)
        for tok in &symbols.tokens {
            let clean = tok.to_lowercase();
            if clean.len() >= 3 && !clean.contains("redacted") && !candidate_kws.contains(&clean) {
                candidate_kws.push(clean);
            }
        }

        // 3. Extracted symbol names (functions, classes, variables)
        for sym in &symbols.symbols {
            let clean = sym.to_lowercase();
            if clean.len() >= 3 && !clean.contains("redacted") && !candidate_kws.contains(&clean) {
                candidate_kws.push(clean);
            }
        }

        // Step 2: Privacy Shield for Code (H10).
        // 1. Cloud code analysis requires explicit opt-in (AXEL_ENABLE_CLOUD_CODE=1).
        //    Defaults to strictly local analysis to prevent unintentional proprietary code leakage.
        // 2. Outbound code snippets are redacted for API keys, AWS keys, passwords, JWTs, and PII.
        // 3. If any secrets or high-risk identifiers are present, cloud transmission is strictly aborted.
        let cloud_code_opt_in = std::env::var("AXEL_ENABLE_CLOUD_CODE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let (sanitized_code, has_secrets) = crate::classifier::privacy::redact_code_secrets_and_pii(code);

        if cloud_code_opt_in && !has_secrets {
            if let Some(ref client) = self.groq {
                let code_snippet: String = sanitized_code.lines().take(60).collect::<Vec<_>>().join("\n");
                let clean_symbols = crate::classifier::privacy::sanitize_code_symbols(&symbols.symbols);
                let clean_tokens = crate::classifier::privacy::sanitize_code_symbols(&symbols.tokens);

                if let Some(mut llm_res) = client
                    .categorize_code(
                        &code_snippet,
                        file_name,
                        &clean_symbols,
                        &clean_tokens,
                    )
                    .await
                {
                    // Blend Groq concepts with local tokens
                    let mut final_kws = Vec::new();
                    for kw in llm_res.keywords {
                        let clean = kw.trim().to_lowercase();
                        if !clean.is_empty() && !clean.contains("redacted") && !final_kws.contains(&clean) {
                            final_kws.push(clean);
                        }
                    }
                    for tok in &clean_tokens {
                        let clean = tok.to_lowercase();
                        if !clean.contains("redacted") && !final_kws.contains(&clean) && final_kws.len() < 10 {
                            final_kws.push(clean);
                        }
                    }
                    for kw in tfidf_kws {
                        let clean = kw.to_lowercase();
                        if !clean.contains("redacted") && !final_kws.contains(&clean) && final_kws.len() < 10 {
                            final_kws.push(clean);
                        }
                    }
                    llm_res.keywords = final_kws;
                    return (llm_res, symbols);
                }
            }
        }

        // Step 3: Dynamic local categorization without cloud transmission
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
