use axel::config::Config;
use axel::desktop::settings::DesktopSettingsState;
use axel::server::{create_router, AppState, SettingsResponse, UpdateSettingsRequest};
use axel::ai::AiEngine;
use axel::index::DocumentIndex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(prefix: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}_{nanos}"));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_desktop_settings_state_sync() {
    let mut config = Config::default();
    config.groq_api_key = Some("gsk_test_key_12345".to_string());
    config.model_name = "llama-3.3-70b-versatile".to_string();
    config.force_offline = false;
    config.enable_ocr = true;
    config.max_file_size_mb = 100;
    config.theme = "Nebula Obsidian".to_string();

    let mut state = DesktopSettingsState::from_config(&config);
    assert_eq!(state.api_key_input, "gsk_test_key_12345");
    assert_eq!(state.selected_model, "llama-3.3-70b-versatile");
    assert!(!state.force_offline);
    assert!(state.enable_ocr);
    assert_eq!(state.max_file_size_mb, 100);

    // Modify GUI fields
    state.api_key_input = "gsk_new_key_67890".to_string();
    state.selected_model = "mixtral-8x7b-32768".to_string();
    state.force_offline = true;
    state.enable_cloud_code = true;
    state.enable_ocr = false;
    state.max_file_size_mb = 250;
    state.selected_theme = "Cosmic Slate".to_string();

    config = state.to_config(&config);
    assert_eq!(config.groq_api_key.as_deref(), Some("gsk_new_key_67890"));
    assert_eq!(config.model_name, "mixtral-8x7b-32768");
    assert!(config.force_offline);
    assert!(config.enable_cloud_code);
    assert!(!config.enable_ocr);
    assert_eq!(config.max_file_size_mb, 250);
    assert_eq!(config.theme, "Cosmic Slate");
}

#[tokio::test]
async fn test_api_settings_get_and_post() {
    let test_dir = TestDir::new("axel_settings_test");
    let index_file = test_dir.path.join("test_index.json");
    let input_folder = test_dir.path.join("docs");
    std::fs::create_dir_all(&input_folder).unwrap();

    let mut config = Config::default();
    config.input_folder = input_folder;
    config.index_file = index_file.clone();
    config.groq_api_key = Some("gsk_initial_key_secret".to_string());
    config.model_name = "llama-3.3-70b-versatile".to_string();
    config.force_offline = false;
    config.theme = "Nebula Obsidian".to_string();

    let session_token = "test_session_token_123".to_string();
    let index = Arc::new(Mutex::new(DocumentIndex::load(&index_file)));
    let ai = Arc::new(AiEngine::new(&config));
    let (events_tx, _) = broadcast::channel::<String>(16);

    let state = AppState::new(
        config.clone(),
        index,
        ai,
        events_tx,
        None,
        session_token.clone(),
        false,
    );

    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();

    // 1. GET /api/settings without auth must return 401 Unauthorized
    let unauth_res = client.get(&format!("{base_url}/api/settings")).send().await.unwrap();
    assert_eq!(unauth_res.status(), reqwest::StatusCode::UNAUTHORIZED);

    // 2. GET /api/settings with session token must return 200 OK with masked key
    let get_res = client
        .get(&format!("{base_url}/api/settings"))
        .header("Authorization", format!("Bearer {session_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(get_res.status(), reqwest::StatusCode::OK);
    let settings: SettingsResponse = get_res.json().await.unwrap();
    assert!(settings.groq_api_key_configured);
    assert_eq!(settings.model_name, "llama-3.3-70b-versatile");
    assert!(!settings.force_offline);
    assert!(settings.groq_api_key_masked.is_some());
    // Ensure raw secret is masked and never leaked in full
    assert_ne!(settings.groq_api_key_masked.unwrap(), "gsk_initial_key_secret");

    // 3. POST /api/settings to update configuration dynamically
    let update_req = UpdateSettingsRequest {
        groq_api_key: Some("gsk_updated_hot_key".to_string()),
        model_name: Some("mixtral-8x7b-32768".to_string()),
        model_fallbacks: None,
        force_offline: Some(true),
        enable_cloud_code: Some(true),
        enable_ocr: Some(false),
        max_file_size_mb: Some(300),
        excluded_patterns: Some(vec![".git".to_string(), "custom_build".to_string()]),
        theme: Some("Cosmic Slate".to_string()),
        has_seen_onboarding: Some(true),
    };

    let post_res = client
        .post(&format!("{base_url}/api/settings"))
        .header("Authorization", format!("Bearer {session_token}"))
        .json(&update_req)
        .send()
        .await
        .unwrap();
    assert_eq!(post_res.status(), reqwest::StatusCode::OK);
    let updated: SettingsResponse = post_res.json().await.unwrap();
    assert!(updated.groq_api_key_configured);
    assert_eq!(updated.model_name, "mixtral-8x7b-32768");
    assert!(updated.force_offline);
    assert!(updated.enable_cloud_code);
    assert!(!updated.enable_ocr);
    assert_eq!(updated.max_file_size_mb, 300);
    assert_eq!(updated.theme, "Cosmic Slate");
    assert!(updated.has_seen_onboarding);

    // 4. GET /api/settings again to verify runtime hot-reload persisted across state
    let get_res2 = client
        .get(&format!("{base_url}/api/settings"))
        .header("Authorization", format!("Bearer {session_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(get_res2.status(), reqwest::StatusCode::OK);
    let verify_cfg: SettingsResponse = get_res2.json().await.unwrap();
    assert_eq!(verify_cfg.model_name, "mixtral-8x7b-32768");
    assert!(verify_cfg.force_offline);
    assert_eq!(verify_cfg.theme, "Cosmic Slate");
    assert!(verify_cfg.has_seen_onboarding);
}

#[tokio::test]
async fn test_api_test_key_endpoint() {
    let test_dir = TestDir::new("axel_key_test");
    let index_file = test_dir.path.join("test_index.json");
    let input_folder = test_dir.path.join("docs");
    std::fs::create_dir_all(&input_folder).unwrap();

    let config = Config::default();
    let session_token = "test_session_token_key".to_string();
    let index = Arc::new(Mutex::new(DocumentIndex::load(&index_file)));
    let ai = Arc::new(AiEngine::new(&config));
    let (events_tx, _) = broadcast::channel::<String>(16);

    let state = AppState::new(
        config,
        index,
        ai,
        events_tx,
        None,
        session_token.clone(),
        false,
    );

    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();

    // 1. POST /api/settings/test-key with empty payload when no key configured
    let empty_res = client
        .post(&format!("{base_url}/api/settings/test-key"))
        .header("Authorization", format!("Bearer {session_token}"))
        .json(&serde_json::json!({ "api_key": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(empty_res.status(), reqwest::StatusCode::OK);
    let empty_data: axel::ai::GroqKeyVerification = empty_res.json().await.unwrap();
    assert!(!empty_data.valid);
    assert_eq!(empty_data.message, "No API key provided or configured");

    // 2. POST /api/settings/test-key with a dummy key returns structured failure instead of crashing
    let dummy_res = client
        .post(&format!("{base_url}/api/settings/test-key"))
        .header("Authorization", format!("Bearer {session_token}"))
        .json(&serde_json::json!({ "api_key": "gsk_invalid_test_key_nonexistent" }))
        .send()
        .await
        .unwrap();
    assert_eq!(dummy_res.status(), reqwest::StatusCode::OK);
    let dummy_data: axel::ai::GroqKeyVerification = dummy_res.json().await.unwrap();
    assert!(!dummy_data.valid);
    assert!(
        dummy_data.message.contains("Authentication failed")
            || dummy_data.message.contains("Network request failed")
            || dummy_data.message.contains("HTTP")
            || dummy_data.message.contains("Unauthorized")
    );
}
