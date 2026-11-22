use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryResult {
    pub category: String,
    pub keywords: Vec<String>,
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

    pub async fn categorize(&self, text: &str, suggested_cat: Option<&str>, suggested_kws: &[String]) -> Option<CategoryResult> {
        let ml_context = if let Some(cat) = suggested_cat {
            format!(
                "\nMachine Learning suggestion:\nSuggested Category: {}\nSuggested Keywords: {}\nUse these suggestions if accurate, otherwise refine.\n",
                cat,
                suggested_kws.join(", ")
            )
        } else {
            String::new()
        };

        let prompt = format!(
            "Analyze the following text and provide a single category and a list of MAXIMUM 5 keywords.{}\
            You MUST output STRICT JSON in the following format ONLY:\n\
            {{\n  \"category\": \"...\",\n  \"keywords\": [\"...\", \"...\"]\n}}\n\
            Do NOT output any other text, explanation, or markdown backticks.\n\n\
            Text to analyze:\n{}",
            ml_context, text
        );

        let stricter_prompt = format!(
            "Your previous output failed JSON parsing. Return ONLY raw valid JSON with keys \"category\" and \"keywords\".\nText: {}",
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
