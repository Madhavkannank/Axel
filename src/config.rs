use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Config {
    pub groq_api_key: Option<String>,
    pub model_name: String,
    pub model_fallbacks: Vec<String>,
    pub input_folder: PathBuf,
    pub index_file: PathBuf,
    pub max_keywords: usize,
    pub top_k_results: usize,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            groq_api_key: None,
            model_name: "llama-3.3-70b-versatile".to_string(),
            model_fallbacks: vec![
                "llama-3.1-8b-instant".to_string(),
                "mixtral-8x7b-32768".to_string(),
                "gemma2-9b-it".to_string(),
            ],
            input_folder: PathBuf::from("./documents"),
            index_file: PathBuf::from("axel_index.json"),
            max_keywords: 10,
            top_k_results: 5,
            port: 8765,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        // Attempt to load .env, but never panic if missing
        let _ = dotenvy::dotenv();

        let groq_api_key = std::env::var("GROQ_API_KEY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "your-groq-api-key-here");

        let model_name = std::env::var("MODEL_NAME")
            .unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string());

        let model_fallbacks = std::env::var("AXEL_GROQ_FALLBACK_MODELS")
            .map(|val| {
                val.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| vec![
                "llama-3.1-8b-instant".to_string(),
                "mixtral-8x7b-32768".to_string(),
                "gemma2-9b-it".to_string(),
            ]);

        let input_folder = std::env::var("AXEL_INPUT_FOLDER")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./documents"));

        let index_file = std::env::var("AXEL_INDEX_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("axel_index.json"));

        let max_keywords = std::env::var("AXEL_MAX_KEYWORDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let top_k_results = std::env::var("AXEL_TOP_K_RESULTS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let port = std::env::var("PORT")
            .or_else(|_| std::env::var("AXEL_PORT"))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8765);

        Self {
            groq_api_key,
            model_name,
            model_fallbacks,
            input_folder,
            index_file,
            max_keywords,
            top_k_results,
            port,
        }
    }
}
