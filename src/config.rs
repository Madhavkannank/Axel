use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
            input_folder: resolve_workspace_path(Path::new("./documents")),
            index_file: resolve_workspace_path(Path::new("axel_index.json")),
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
            .unwrap_or_else(|_| {
                vec![
                    "llama-3.1-8b-instant".to_string(),
                    "mixtral-8x7b-32768".to_string(),
                    "gemma2-9b-it".to_string(),
                ]
            });

        let raw_input_folder = std::env::var("AXEL_INPUT_FOLDER")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./documents"));
        let input_folder = resolve_workspace_path(&raw_input_folder);

        let raw_index_file = std::env::var("AXEL_INDEX_FILE")
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
