pub mod groq;

use crate::classifier::{KeywordExtractor, RuleClassifier};
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

    pub async fn analyze_document(&self, text: &str, file_name: &str) -> CategoryResult {
        // Step 1: Run local rule classifier as initial baseline
        let (rule_cat, _) = self.rules.classify(text);
        let rule_kws = self.keywords.extract(text, self.max_keywords);

        // Step 2: If Groq is configured, attempt LLM reasoning with model fallbacks
        if let Some(ref client) = self.groq {
            if !text.is_empty() {
                if let Some(mut llm_res) = client.categorize(text, Some(&rule_cat), &rule_kws).await {
                    llm_res.keywords = self.keywords.clean_keywords(llm_res.keywords, text, self.max_keywords);
                    return llm_res;
                }
            } else {
                // If OCR/text extraction was empty, infer from file name
                let context_prompt = format!("File name: {file_name}. Infer category and up to 5 keywords.");
                if let Some(mut llm_res) = client.categorize(&context_prompt, Some("Unknown"), &[]).await {
                    llm_res.keywords = self.keywords.clean_keywords(llm_res.keywords, file_name, self.max_keywords);
                    return llm_res;
                }
            }
        }

        // Step 3: Fallback to local rule engine
        CategoryResult {
            category: if rule_cat.is_empty() { "General".to_string() } else { rule_cat },
            keywords: self.keywords.clean_keywords(rule_kws, text, self.max_keywords),
        }
    }
}
