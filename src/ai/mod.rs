pub mod groq;

use crate::classifier::{
    extract_code_symbols, is_high_risk_identity_document, redact_sensitive_pii, CodeSymbols,
    KeywordExtractor, RuleClassifier,
};
use crate::config::Config;
pub use groq::{CategoryResult, GroqClient};

pub struct AiEngine {
    groq: Option<GroqClient>,
    rules: RuleClassifier,
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
            rules: RuleClassifier::new(),
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

        // Step 1: Run local rule classifier with filename + extension context
        let (rule_cat, _) = self.rules.classify_with_filename(&effective_text, file_name);
        let mut rule_kws = self.keywords.extract(&effective_text, self.max_keywords);

        // Merge TF-IDF keywords if provided
        if let Some(tfidf_kws) = suggested_tfidf_kws {
            for kw in tfidf_kws {
                let clean = kw.to_lowercase();
                if !rule_kws.contains(&clean) {
                    rule_kws.push(clean);
                }
            }
        }

        // Step 2: Privacy Shield + Groq Semantic Discovery Layer
        // All sensitive personal identifiers (Aadhaar, PAN, SSN, Credit Cards, names, DOB)
        // are thoroughly redacted prior to sending context to the cloud LLM.
        if let Some(ref client) = self.groq {
            let (sanitized_text, _) = redact_sensitive_pii(&effective_text);
            if !sanitized_text.trim().is_empty() {
                if let Some(mut llm_res) = client.categorize(&sanitized_text, Some(&rule_cat), &rule_kws).await {
                    llm_res.keywords = self.keywords.clean_keywords(llm_res.keywords, text, self.max_keywords);
                    return llm_res;
                }
            } else {
                let context_prompt = format!("File name: {file_name}. Infer category, keywords, and virtual_paths.");
                if let Some(mut llm_res) = client.categorize(&context_prompt, Some(&rule_cat), &rule_kws).await {
                    llm_res.keywords = self.keywords.clean_keywords(llm_res.keywords, file_name, self.max_keywords);
                    return llm_res;
                }
            }
        }

        // Step 3: Offline fallback when Groq is not configured or offline
        let is_sensitive_id = is_high_risk_identity_document(&effective_text, file_name);
        let category = if is_sensitive_id {
            "Government ID".to_string()
        } else if rule_cat.is_empty() {
            "General".to_string()
        } else {
            rule_cat
        };

        if is_sensitive_id {
            let lower_eff = effective_text.to_lowercase();
            let lower_fname = file_name.to_lowercase();
            if (lower_eff.contains("aadhaar") || lower_eff.contains("aadhar") || lower_eff.contains("uidai")
                || lower_eff.contains("unique identification")
                || lower_fname.contains("aadhaar") || lower_fname.contains("aadhar"))
                && !rule_kws.contains(&"aadhaar".to_string())
            {
                rule_kws.insert(0, "aadhaar".to_string());
                rule_kws.insert(1, "government id".to_string());
            } else if (lower_eff.contains("pan") || lower_eff.contains("income tax") || lower_eff.contains("permanent account")
                || lower_fname.contains("pan"))
                && !rule_kws.contains(&"pan".to_string())
            {
                rule_kws.insert(0, "pan".to_string());
                rule_kws.insert(1, "government id".to_string());
            }
        }

        CategoryResult {
            category,
            keywords: self.keywords.clean_keywords(rule_kws, &effective_text, self.max_keywords),
            virtual_paths: Vec::new(),
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

        // 1. Domain hints (e.g. calculator, arithmetic, computer vision)
        for hint in &symbols.domain_hints {
            candidate_kws.push(hint.clone());
        }

        // 2. TF-IDF extracted keywords
        for kw in tfidf_kws {
            if !candidate_kws.contains(kw) {
                candidate_kws.push(kw.clone());
            }
        }

        // 3. Extracted identifier tokens (split snake_case / camelCase)
        for tok in &symbols.tokens {
            if tok.len() >= 3 && !candidate_kws.contains(tok) {
                candidate_kws.push(tok.clone());
            }
        }

        // Step 2: Try Groq API for conceptual intent synthesis
        if let Some(ref client) = self.groq {
            let code_snippet: String = code.lines().take(60).collect::<Vec<_>>().join("\n");
            if let Some(mut llm_res) = client
                .categorize_code(
                    &code_snippet,
                    file_name,
                    &symbols.symbols,
                    &symbols.tokens,
                    &symbols.domain_hints,
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
                for hint in &symbols.domain_hints {
                    let clean = hint.to_lowercase();
                    if !final_kws.contains(&clean) {
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

        // Step 3: Offline heuristic category and keywords
        let category = if symbols.domain_hints.iter().any(|h| h == "calculator" || h == "arithmetic" || h == "math") {
            "Code/Math".to_string()
        } else if symbols.domain_hints.iter().any(|h| h.contains("vision") || h.contains("opencv") || h.contains("image")) {
            "Code/Vision".to_string()
        } else if symbols.domain_hints.iter().any(|h| h.contains("machine learning") || h.contains("data science")) {
            "Code/ML".to_string()
        } else if symbols.domain_hints.iter().any(|h| h.contains("web") || h.contains("backend")) {
            "Code/Web".to_string()
        } else {
            format!("Code/{}", ext.to_uppercase())
        };

        candidate_kws.truncate(self.max_keywords.max(6));

        (
            CategoryResult {
                category,
                keywords: candidate_kws,
                virtual_paths: Vec::new(),
            },
            symbols,
        )
    }
}
