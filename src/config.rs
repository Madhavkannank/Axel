use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

fn default_max_file_size_mb() -> usize {
    100
}

fn default_excluded_patterns() -> Vec<String> {
    vec![
        ".git".to_string(),
        "node_modules".to_string(),
        "target".to_string(),
        "tmp".to_string(),
        ".DS_Store".to_string(),
    ]
}

fn default_theme() -> String {
    "Nebula Obsidian".to_string()
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub groq_api_key: Option<String>,
    pub model_name: String,
    pub model_fallbacks: Vec<String>,
    pub input_folder: PathBuf,
    pub index_file: PathBuf,
    pub max_keywords: usize,
    pub top_k_results: usize,
    pub port: u16,
    pub host: String,
    pub api_token: Option<String>,
    #[serde(default)]
    pub allowed_origins: Vec<String>,

    // Real App Settings
    #[serde(default)]
    pub force_offline: bool,
    #[serde(default)]
    pub enable_cloud_code: bool,
    #[serde(default = "default_true")]
    pub enable_ocr: bool,
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: usize,
    #[serde(default = "default_excluded_patterns")]
    pub excluded_patterns: Vec<String>,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub has_seen_onboarding: bool,
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
            input_folder: resolve_workspace_path(Path::new("./documents")),
            index_file: resolve_workspace_path(Path::new("axel_index.json")),
            max_keywords: 10,
            top_k_results: 5,
            port: 8765,
            host: "127.0.0.1".to_string(),
            api_token: None,
            allowed_origins: Vec::new(),
            force_offline: false,
            enable_cloud_code: false,
            enable_ocr: true,
            max_file_size_mb: 100,
            excluded_patterns: default_excluded_patterns(),
            theme: default_theme(),
            has_seen_onboarding: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        // Attempt to load .env, but never panic if missing
        let _ = dotenvy::dotenv();

        let is_placeholder = |val: &str| -> bool {
            let lower = val.to_lowercase();
            lower.contains("your-")
                || lower.contains("placeholder")
                || lower.contains("gsk_your_groq_api_key_here")
                || lower.contains("example")
        };

        let groq_api_key = std::env::var("GROQ_API_KEY")
            .or_else(|_| std::env::var("AXEL_GROQ_API_KEY"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !is_placeholder(s));

        let model_name = std::env::var("AXEL_MODEL_NAME")
            .or_else(|_| std::env::var("MODEL_NAME"))
            .unwrap_or_else(|_| "qwen/qwen3.8-27b".to_string());

        let model_fallbacks = std::env::var("AXEL_GROQ_FALLBACK_MODELS")
            .map(|val| {
                val.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| {
                vec![
                    "openai/gpt-oss-120b".to_string(),
                    "openai/gpt-oss-20b".to_string(),
                    "llama-3.3-70b-versatile".to_string(),
                ]
            });

        let raw_input_folder = std::env::var("AXEL_INPUT_FOLDER")
            .or_else(|_| std::env::var("AXEL_WATCH_DIR"))
            .or_else(|_| std::env::var("AXEL_INPUT_DIR"))
            .or_else(|_| std::env::var("INPUT_FOLDER"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./documents"));
        let input_folder = resolve_workspace_path(&raw_input_folder);

        let raw_index_file = std::env::var("AXEL_INDEX_FILE")
            .or_else(|_| std::env::var("AXEL_INDEX_PATH"))
            .or_else(|_| std::env::var("INDEX_FILE"))
            .or_else(|_| std::env::var("INDEX_PATH"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("axel_index.json"));
        let index_file = resolve_workspace_path(&raw_index_file);

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
            .or_else(|_| std::env::var("AXEL_SERVER_PORT"))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8765);

        let host = std::env::var("AXEL_HOST")
            .or_else(|_| std::env::var("HOST"))
            .unwrap_or_else(|_| "127.0.0.1".to_string());

        let api_token = std::env::var("AXEL_API_KEY")
            .or_else(|_| std::env::var("AXEL_TOKEN"))
            .or_else(|_| std::env::var("AXEL_API_TOKEN"))
            .or_else(|_| std::env::var("API_TOKEN"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !is_placeholder(s));

        let allowed_origins = std::env::var("AXEL_ALLOWED_ORIGINS")
            .map(|val| {
                val.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let force_offline = std::env::var("AXEL_FORCE_OFFLINE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let enable_cloud_code = std::env::var("AXEL_ENABLE_CLOUD_CODE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let enable_ocr = std::env::var("AXEL_ENABLE_OCR")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true);

        let max_file_size_mb = std::env::var("AXEL_MAX_FILE_SIZE_MB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);

        let excluded_patterns = std::env::var("AXEL_EXCLUDED_PATTERNS")
            .map(|val| {
                val.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| default_excluded_patterns());

        let theme = std::env::var("AXEL_THEME").unwrap_or_else(|_| default_theme());

        let mut config = Self {
            groq_api_key,
            model_name,
            model_fallbacks,
            input_folder,
            index_file,
            max_keywords,
            top_k_results,
            port,
            host,
            api_token,
            allowed_origins,
            force_offline,
            enable_cloud_code,
            enable_ocr,
            max_file_size_mb,
            excluded_patterns,
            theme,
            has_seen_onboarding: false,
        };

        // If saved settings exist on disk, load and merge them (with env vars taking precedence if set)
        let settings_path = Self::settings_file_path();
        let local_path = PathBuf::from("axel_settings.json");
        let active_path = if local_path.exists() {
            Some(local_path)
        } else if settings_path.exists() {
            Some(settings_path)
        } else {
            None
        };

        if let Some(path) = active_path {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(saved) = serde_json::from_str::<Config>(&content) {
                    config.has_seen_onboarding = saved.has_seen_onboarding;
                    // Merge saved values if not explicitly overridden by environment
                    if config.groq_api_key.is_none() && saved.groq_api_key.is_some() {
                        config.groq_api_key = saved.groq_api_key;
                    }
                    if std::env::var("AXEL_MODEL_NAME").is_err() && std::env::var("MODEL_NAME").is_err() {
                        config.model_name = saved.model_name;
                    }
                    if std::env::var("AXEL_GROQ_FALLBACK_MODELS").is_err() && !saved.model_fallbacks.is_empty() {
                        config.model_fallbacks = saved.model_fallbacks;
                    }
                    if std::env::var("AXEL_INPUT_FOLDER").is_err() && std::env::var("INPUT_FOLDER").is_err() {
                        config.input_folder = saved.input_folder;
                    }
                    if std::env::var("AXEL_INDEX_FILE").is_err() && std::env::var("INDEX_FILE").is_err() {
                        config.index_file = saved.index_file;
                    }
                    if std::env::var("AXEL_MAX_KEYWORDS").is_err() {
                        config.max_keywords = saved.max_keywords;
                    }
                    if std::env::var("AXEL_TOP_K_RESULTS").is_err() {
                        config.top_k_results = saved.top_k_results;
                    }
                    if std::env::var("PORT").is_err() && std::env::var("AXEL_PORT").is_err() {
                        config.port = saved.port;
                    }
                    if std::env::var("AXEL_HOST").is_err() && std::env::var("HOST").is_err() {
                        config.host = saved.host;
                    }
                    if std::env::var("AXEL_FORCE_OFFLINE").is_err() {
                        config.force_offline = saved.force_offline;
                    }
                    if std::env::var("AXEL_ENABLE_CLOUD_CODE").is_err() {
                        config.enable_cloud_code = saved.enable_cloud_code;
                    }
                    if std::env::var("AXEL_ENABLE_OCR").is_err() {
                        config.enable_ocr = saved.enable_ocr;
                    }
                    if std::env::var("AXEL_MAX_FILE_SIZE_MB").is_err() {
                        config.max_file_size_mb = saved.max_file_size_mb;
                    }
                    if std::env::var("AXEL_EXCLUDED_PATTERNS").is_err() {
                        config.excluded_patterns = saved.excluded_patterns;
                    }
                    if std::env::var("AXEL_THEME").is_err() {
                        config.theme = saved.theme;
                    }
                }
            }
        }

        config
    }

    pub fn settings_file_path() -> PathBuf {
        app_data_dir().join("settings.json")
    }

    /// Atomically persists settings to disk with bidirectional .env synchronization.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::settings_file_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;

        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, &json)
            .map_err(|e| format!("Failed to write temporary settings file: {e}"))?;
        std::fs::rename(&tmp_path, &path)
            .map_err(|e| format!("Failed to commit settings file atomically: {e}"))?;

        // Also update local axel_settings.json if it exists
        let local_settings = PathBuf::from("axel_settings.json");
        if local_settings.exists() {
            let tmp_local = PathBuf::from("axel_settings.json.tmp");
            if std::fs::write(&tmp_local, &json).is_ok() {
                let _ = std::fs::rename(&tmp_local, &local_settings);
            }
        }

        // Bidirectional synchronization with .env if present
        Self::sync_to_dotenv(self);

        Ok(())
    }

    fn sync_to_dotenv(cfg: &Config) {
        let env_path = PathBuf::from(".env");
        let content = std::fs::read_to_string(&env_path).unwrap_or_default();
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        let set_or_replace = |lines: &mut Vec<String>, key: &str, val: &str| {
            let prefix = format!("{key}=");
            let mut found = false;
            for line in lines.iter_mut() {
                if line.starts_with(&prefix) {
                    *line = format!("{key}={val}");
                    found = true;
                    break;
                }
            }
            if !found {
                lines.push(format!("{key}={val}"));
            }
        };

        if let Some(ref k) = cfg.groq_api_key {
            set_or_replace(&mut lines, "GROQ_API_KEY", k);
        }
        set_or_replace(&mut lines, "AXEL_MODEL_NAME", &cfg.model_name);
        set_or_replace(&mut lines, "AXEL_FORCE_OFFLINE", if cfg.force_offline { "true" } else { "false" });
        set_or_replace(&mut lines, "AXEL_ENABLE_CLOUD_CODE", if cfg.enable_cloud_code { "true" } else { "false" });
        set_or_replace(&mut lines, "AXEL_ENABLE_OCR", if cfg.enable_ocr { "true" } else { "false" });
        set_or_replace(&mut lines, "AXEL_MAX_FILE_SIZE_MB", &cfg.max_file_size_mb.to_string());
        set_or_replace(&mut lines, "AXEL_THEME", &cfg.theme);

        let new_content = lines.join("\n") + "\n";
        let _ = std::fs::write(&env_path, new_content);
    }
}

/// Expand leading `~` and resolve relative paths to clean absolute paths.
/// Never returns a relative path when `current_dir()` is available, preventing
/// xdg-desktop-portal / GTK file chooser errors on relative `current_folder`.
pub fn resolve_workspace_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    let expanded = if raw == "~" {
        dirs::home_dir().unwrap_or_else(|| path.to_path_buf())
    } else if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(rest)
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    let abs = if expanded.is_absolute() {
        expanded
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(expanded)
    } else {
        expanded
    };

    if let Ok(canon) = std::fs::canonicalize(&abs) {
        canon
    } else {
        normalize_lexical(&abs)
    }
}

fn normalize_lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn same_dir(a: &Path, b: &Path) -> bool {
    resolve_workspace_path(a) == resolve_workspace_path(b)
}

pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("axel")
}

/// Compute a writable index file path for any workspace directory.
/// Keeps `default_index` for the default workspace, and stores other workspaces
/// under `~/.local/share/axel/workspaces/<sha256>.json` so read-only or user
/// folders (like ~/Downloads) are never polluted or blocked by permissions.
pub fn workspace_index_path(
    workspace_dir: &Path,
    default_dir: &Path,
    default_index: &Path,
) -> PathBuf {
    let resolved = resolve_workspace_path(workspace_dir);
    if same_dir(&resolved, default_dir) {
        return default_index.to_path_buf();
    }

    let mut hasher = Sha256::new();
    hasher.update(resolved.to_string_lossy().as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let slug = &digest[..16];

    let workspaces_dir = app_data_dir().join("workspaces");
    let _ = std::fs::create_dir_all(&workspaces_dir);
    let target = workspaces_dir.join(format!("{slug}.json"));

    // Migrate legacy `.axel_index.json` if present and target doesn't exist yet
    let legacy = resolved.join(".axel_index.json");
    if !target.exists() && legacy.exists() {
        let _ = std::fs::copy(&legacy, &target);
    }

    target
}

pub fn load_recent_workspaces() -> Vec<PathBuf> {
    let path = app_data_dir().join("recent_workspaces.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(list) = serde_json::from_str::<Vec<String>>(&raw) else {
        return Vec::new();
    };
    list.into_iter()
        .map(PathBuf::from)
        .filter(|p| p.exists() && p.is_dir())
        .collect()
}

pub fn record_recent_workspace(dir: &Path) -> Vec<PathBuf> {
    let resolved = resolve_workspace_path(dir);
    let mut list = load_recent_workspaces();
    list.retain(|p| !same_dir(p, &resolved));
    list.insert(0, resolved);
    list.truncate(8);

    let data_dir = app_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    let strings: Vec<String> = list.iter().map(|p| p.display().to_string()).collect();
    if let Ok(json) = serde_json::to_string_pretty(&strings) {
        let _ = std::fs::write(data_dir.join("recent_workspaces.json"), json);
    }
    list
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkstationLayoutState {
    pub left_w: f32,
    pub right_w: f32,
}

pub fn load_workstation_layout() -> Option<WorkstationLayoutState> {
    let path = app_data_dir().join("workstation_layout.json");
    if let Ok(content) = std::fs::read_to_string(path) {
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

pub fn save_workstation_layout(state: &WorkstationLayoutState) {
    let data_dir = app_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    if let Ok(json) = serde_json::to_string(state) {
        let _ = std::fs::write(data_dir.join("workstation_layout.json"), json);
    }
}
