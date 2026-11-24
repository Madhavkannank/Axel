use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryResult {
    pub category: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub virtual_paths: Vec<String>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

pub struct GroqClient {
    api_key: String,
    primary_model: String,
    fallbacks: Vec<String>,
    client: reqwest::Client,
}

impl GroqClient {
    pub fn new(api_key: String, primary_model: String, fallbacks: Vec<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        Self {
            api_key,
            primary_model,
            fallbacks,
            client,
        }
    }

    pub async fn categorize(&self, text: &str, candidate_kws: &[String]) -> Option<CategoryResult> {
        let (sanitized_text, _) = crate::classifier::redact_sensitive_pii(text);
        let text = sanitized_text.as_str();

        let filtered_kws: Vec<String> = candidate_kws
            .iter()
            .filter(|k| !k.to_lowercase().contains("redacted"))
            .map(|k| crate::classifier::redact_sensitive_pii(k).0)
            .filter(|k| !k.trim().is_empty())
            .collect();

        let kws_context = if !filtered_kws.is_empty() {
            format!("\nExtracted candidate keywords: {}\n", filtered_kws.join(", "))
        } else {
            String::new()
        };

        let prompt = format!(
            "You are AXEL's semantic file intelligence engine. Analyze the following document text and metadata to determine:\n\
            1. Constellation category: If it's an episodic web series or television video, output the Series Name (e.g. \"Dark\"). If it's course/study material, notes, or syllabus, output the Subject Name (e.g. \"Data Science\", \"Operating Systems\"). If it's a resume or CV, output \"Career/CV\" or \"Career/Resume\". Otherwise, output the specific domain topic (e.g. \"Finance\", \"Legal\", \"Code\", etc.). NEVER output generic tags like \"General\", \"Video\", or \"unrecognized_text\".\n\
            2. Up to 5 descriptive semantic keywords (exclude common dialogue words or stop words)\n\
            3. Dynamic virtual folder hierarchy paths where this file logically belongs. A single file can appear in multiple virtual folder paths simultaneously without duplicating bytes (for example: a series episode belongs to \"Dark/Dark S03E01.mkv\" and \"Series/Dark/Dark S03E01.mkv\"; a study notes document belongs to \"Data Science/Syllabus.pdf\" and \"Academics/Data Science/Syllabus.pdf\").\n\
            {}\n\
            You MUST output STRICT JSON in the following format ONLY:\n\
            {{\n\
              \"category\": \"...\",\n\
              \"keywords\": [\"...\", \"...\"],\n\
              \"virtual_paths\": [\"Folder/Subfolder/Item\", \"Constellation/Item\"]\n\
            }}\n\
            Do NOT output markdown backticks, explanations, or any other text.\n\n\
            Text to analyze:\n{}",
            kws_context, text
        );

        let stricter_prompt = format!(
            "Your previous output failed JSON parsing. Return ONLY raw valid JSON with keys \"category\", \"keywords\", and \"virtual_paths\".\nText: {}",
            text
        );

        let mut models = vec![self.primary_model.clone()];
        for m in &self.fallbacks {
            if m != &self.primary_model {
                models.push(m.clone());
            }
        }

        for model in models {
            for attempt in 0..2 {
                let current_prompt = if attempt == 0 { &prompt } else { &stricter_prompt };
                let req = ChatRequest {
                    model: model.clone(),
                    messages: vec![ChatMessage {
                        role: "user".to_string(),
                        content: current_prompt.to_string(),
                    }],
                    temperature: 0.2,
                    max_tokens: 512,
                };

                let resp = match self
                    .client
                    .post("https://api.groq.com/openai/v1/chat/completions")
                    .bearer_auth(&self.api_key)
                    .json(&req)
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("[WARN] Groq request failed for model {model}: {e}");
                        break;
                    }
                };

                if !resp.status().is_success() {
                    let status = resp.status();
                    eprintln!("[WARN] Groq API returned status {status} for model {model}");
                    if status.as_u16() == 401 || status.as_u16() == 403 {
                        eprintln!("[WARN] Invalid or unauthorized Groq API key.");
                        return None;
                    }
                    break;
                }

                if let Ok(body) = resp.json::<ChatResponse>().await {
                    if let Some(choice) = body.choices.first() {
                        if let Some(parsed) = Self::parse_json_response(&choice.message.content) {
                            return Some(parsed);
                        }
                    }
                }
            }
        }

        None
    }

    pub async fn categorize_code(
        &self,
        code_snippet: &str,
        file_name: &str,
        symbols: &[String],
        tokens: &[String],
    ) -> Option<CategoryResult> {
        let symbols_str = symbols.join(", ");
        let tokens_str = tokens.join(", ");

        let prompt = format!(
            "You are an expert code analyst for AXEL. Analyze this user-written script to determine its true functional purpose, semantic concepts, and virtual hierarchy.\n\
            File name: {}\n\
            Extracted code symbols (functions, variables, classes): {}\n\
            Deconstructed tokens: {}\n\n\
            Code snippet:\n{}\n\n\
            Identify the high-level semantic concept and intent of this code (for example: if it defines math functions like add/subtract/multiply, identify it as \"calculator\" and \"arithmetic\" even if the word \"calculator\" is never explicitly written in the code; or if it uses Sobel filters/OpenCV, identify it as \"computer vision\" and \"edge detection\").\n\
            Assign natural virtual folder paths such as [\"Code/<Domain>/<Language>\"].\n\
            You MUST output STRICT JSON in the following format ONLY:\n\
            {{\n\
              \"category\": \"...\",\n\
              \"keywords\": [\"...\", \"...\"],\n\
              \"virtual_paths\": [\"Code/<Domain>/<Language>\"]\n\
            }}\n\
            Do NOT output any other text, explanation, or markdown backticks.",
            file_name, symbols_str, tokens_str, code_snippet
        );

        let mut models = vec![self.primary_model.clone()];
        for m in &self.fallbacks {
            if m != &self.primary_model {
                models.push(m.clone());
            }
        }

        for model in models {
            let req = ChatRequest {
                model: model.clone(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: prompt.clone(),
                }],
                temperature: 0.1,
                max_tokens: 512,
            };

            let resp = match self
                .client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .bearer_auth(&self.api_key)
                .json(&req)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[WARN] Groq code categorization request failed for model {model}: {e}");
                    break;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                eprintln!("[WARN] Groq API returned status {status} for model {model}");
                if status.as_u16() == 401 || status.as_u16() == 403 {
                    return None;
                }
                break;
            }

            if let Ok(body) = resp.json::<ChatResponse>().await {
                if let Some(choice) = body.choices.first() {
                    if let Some(parsed) = Self::parse_json_response(&choice.message.content) {
                        return Some(parsed);
                    }
                }
            }
        }

        None
    }

    pub fn parse_json_response(content: &str) -> Option<CategoryResult> {
        let trimmed = content.trim();
        // Look for { ... }
        if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
            if start < end {
                let json_slice = &trimmed[start..=end];
                if let Ok(res) = serde_json::from_str::<CategoryResult>(json_slice) {
                    if !res.category.is_empty() {
                        return Some(res);
                    }
                }
            }
        }
        None
    }
}
