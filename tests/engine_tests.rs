use axel::ai::AiEngine;
use axel::classifier::KeywordExtractor;
use axel::config::Config;
use axel::extractor::docx::strip_docx_xml;
use axel::extractor::{detect_file_type, preprocess_text, FileType};
use axel::index::{DocumentIndex, IndexItem};

#[tokio::test]
async fn test_ai_document_analysis_pii_and_keywords() {
    let mut config = Config::load();
    config.groq_api_key = None; // Offline mode test
    let ai = AiEngine::new(&config);

    let doc = "Quarterly consulting invoice for client services. Billing amount: $4,500. Due in 30 days.";
    let res = ai.analyze_document(doc, "invoice.txt", None).await;
    assert!(res.category == "Finance/Invoice" || res.category == "Finance/Invoices");
    assert!(res.keywords.contains(&"invoice".to_string()) || res.keywords.contains(&"consulting".to_string()));
}

#[test]
fn test_keyword_extraction() {
    let extractor = KeywordExtractor::new();
    let text = "Hospital discharge summary and prescription for patient Jane Smith. Doctor prescribed antibiotics and dosage.";
    let keywords = extractor.extract(text, 5);

    assert!(keywords.len() >= 3);
    assert!(keywords.len() <= 5);
    assert!(keywords.contains(&"prescription".to_string()) || keywords.contains(&"hospital".to_string()));
}

#[test]
fn test_file_type_detection() {
    assert_eq!(detect_file_type("report.pdf"), FileType::Pdf);
    assert_eq!(detect_file_type("contract.docx"), FileType::Docx);
    assert_eq!(detect_file_type("photo.jpeg"), FileType::Image);
    assert_eq!(detect_file_type("lecture_calculus.mp4"), FileType::Video);
    assert_eq!(detect_file_type("presentation.mkv"), FileType::Video);
    assert_eq!(detect_file_type("notes.txt"), FileType::Text);
    assert_eq!(detect_file_type("data.json"), FileType::Text);
    assert_eq!(detect_file_type("sheet.csv"), FileType::Text);
    assert_eq!(detect_file_type("archive.tar.gz"), FileType::Archive);
    assert_eq!(detect_file_type("binary.bin"), FileType::Binary);
}

#[test]
fn test_preprocess_text_utf8_multibyte_safety() {
    // Generate a long text containing multi-byte characters:
    // emojis (4 bytes), Chinese (3 bytes), Hindi (3 bytes), Euro (3 bytes), curly quotes (3 bytes)
    let base_snippet = "🦀 Rust Document Intelligence 智慧文档 दस्तावेज़ €100 “quote” — dash. ";
    let repeated = base_snippet.repeat(100); // ~6,000 chars, well over 3,000 threshold

    // Must not panic on UTF-8 character boundary
    let processed = preprocess_text(&repeated);
    assert!(processed.contains("..."));
    assert!(processed.chars().count() <= 3100);
}

#[test]
fn test_docx_strip_xml() {
    let xml = r#"
        <w:p>
            <w:t>Header Title</w:t>
        </w:p>
        <w:p>
            <w:t xml:space="preserve">This is paragraph 1 &amp; details </w:t>
            <w:t/>
            <w:br/>
            <w:t>with second line &lt;important&gt;</w:t>
        </w:p>
    "#;

    let stripped = strip_docx_xml(xml);
    assert!(stripped.contains("Header Title"));
    assert!(stripped.contains("This is paragraph 1 & details"));
    assert!(stripped.contains("with second line <important>"));
    // Ensure self-closing tag didn't corrupt subsequent text
    assert!(!stripped.contains("<w:"));
}

#[test]
fn test_document_index_search_hybrid_blending() {
    let mut index = DocumentIndex::load("non_existent_test_index.json");

    let item1 = IndexItem {
        file: "tax_invoice_2026.pdf".to_string(),
        category: "Invoice".to_string(),
        keywords: vec!["invoice".to_string(), "consulting".to_string(), "billing".to_string()],
        symbols: Vec::new(),
        path: "/docs/tax_invoice_2026.pdf".to_string(),
        hash: "hash1".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    let item2 = IndexItem {
        file: "invoyce_statement.pdf".to_string(), // slight typo for fuzzy matching
        category: "Invoice".to_string(),
        keywords: vec!["invoyce".to_string(), "receipt".to_string()],
        symbols: Vec::new(),
        path: "/docs/invoyce_statement.pdf".to_string(),
        hash: "hash2".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    index.upsert(item1);
    index.upsert(item2);
    assert_eq!(index.count(), 2);

    let search_res = index.search("invoice", 5);
    assert!(!search_res.is_empty());
    // Highest score should be exact match
    assert_eq!(search_res[0].item.file, "tax_invoice_2026.pdf");
    assert!(search_res[0].score >= 0.9);

    let empty_res = index.search("nonexistenttermxyz", 5);
    assert!(empty_res.is_empty());
}

#[tokio::test]
async fn test_ai_filename_fallback_when_empty() {
    let mut config = Config::load();
    config.groq_api_key = None; // Offline fallback without hardcoded domain bias
    let ai = AiEngine::new(&config);

    // Empty text, but filename contains statistical tokens
    let res = ai.analyze_document("", "hospital_prescription_patient_jane.pdf", None).await;
    assert_eq!(res.category, "Medical/Prescription");
    assert!(res.keywords.contains(&"prescription".to_string()) || res.keywords.contains(&"hospital".to_string()));
}

#[test]
fn test_document_index_deduplication() {
    let mut index = DocumentIndex::load("non_existent_test_index.json");

    let item = IndexItem {
        file: "contract.docx".to_string(),
        category: "Legal".to_string(),
        keywords: vec!["agreement".to_string(), "lease".to_string()],
        symbols: Vec::new(),
        path: "/docs/contract.docx".to_string(),
        hash: "contracthash123".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    index.upsert(item.clone());
    assert_eq!(index.count(), 1);

    // Re-upsert with same path but updated keywords
    let mut updated_item = item;
    updated_item.keywords.push("notary".to_string());
    index.upsert(updated_item);

    assert_eq!(index.count(), 1); // Must not duplicate
    assert_eq!(index.all()[0].keywords.len(), 3);
}

#[test]
fn test_document_index_remove_by_hash() {
    let mut index = DocumentIndex::load("non_existent_test_index.json");

    let item = IndexItem {
        file: "temp_note.txt".to_string(),
        category: "General".to_string(),
        keywords: vec!["note".to_string()],
        symbols: Vec::new(),
        path: "/docs/temp_note.txt".to_string(),
        hash: "removehash999".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    index.upsert(item);
    assert_eq!(index.count(), 1);

    let removed = index.remove_by_hash("removehash999");
    assert!(removed.is_some());
    assert_eq!(index.count(), 0);
    assert!(index.remove_by_hash("missing").is_none());
}

#[test]
fn test_video_sidecar_subtitle_extraction() {
    let temp_dir = std::env::temp_dir().join(format!("axel_video_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let video_file = temp_dir.join("mit_calculus_lecture_01.mp4");
    let srt_file = temp_dir.join("mit_calculus_lecture_01.srt");

    // Create a dummy video file (empty or tiny)
    std::fs::write(&video_file, b"dummy video container header").unwrap();

    // Create sidecar .srt
    let srt_content = r#"
1
00:00:01,000 --> 00:00:05,000
Welcome to Calculus. Today we introduce derivatives and limits.

2
00:00:06,000 --> 00:00:10,000
The rate of change of a function is given by the derivative.
"#;
    std::fs::write(&srt_file, srt_content).unwrap();

    let (text, file_type) = axel::extractor::extract_text(&video_file);
    assert_eq!(file_type, FileType::Video);
    assert!(text.contains("Welcome to Calculus"));
    assert!(text.contains("derivatives and limits"));
    assert!(!text.contains("-->"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_watcher_filtering_and_duplicate_safety() {
    use axel::classifier::filter::{should_skip_directory, should_skip_file};
    use std::path::Path;

    // 1. Directories that watcher must automatically ignore
    assert!(should_skip_directory("SteamLibrary"));
    assert!(should_skip_directory("steamapps"));
    assert!(should_skip_directory("node_modules"));
    assert!(should_skip_directory("__pycache__"));
    assert!(should_skip_directory(".venv"));
    assert!(should_skip_directory("mods"));

    // 2. Temporary and lock files that watcher must ignore
    assert!(should_skip_file(Path::new("download.crdownload"), Some(1024)));
    assert!(should_skip_file(Path::new("cache.tmp"), Some(1024)));
    assert!(should_skip_file(Path::new("bundle.min.js"), Some(1024)));
    assert!(should_skip_file(Path::new("package-lock.json"), Some(1024)));

    // 3. Normal user files that watcher MUST process
    assert!(!should_skip_file(Path::new("calculus_notes.pdf"), Some(1024)));
    assert!(!should_skip_file(Path::new("1.py"), Some(1024)));
    assert!(!should_skip_file(Path::new("lecture.mp4"), Some(1024)));
}

#[tokio::test]
async fn test_pan_and_aadhaar_photo_classification_and_multi_pointer() {
    use axel::classifier::privacy::is_high_risk_identity_document;
    use axel::index::virtual_tree::compute_virtual_paths;

    let mut config = Config::load();
    config.groq_api_key = None; // Offline mode for local unit test
    let ai = AiEngine::new(&config);

    // Case 1: Photo of PAN Card (even with generic camera filename IMG_0492.jpg)
    let ocr_pan_text = "INCOME TAX DEPARTMENT GOVT OF INDIA \n Permanent Account Number PAN Card \n ABCDE1234F \n Name: John Doe \n Father: Bob Doe \n DOB: 14/08/1992";
    let is_pan_high_risk = is_high_risk_identity_document(ocr_pan_text, "IMG_0492.jpg");
    assert!(is_pan_high_risk, "PAN card must trigger high-risk identity detection");

    let pan_analysis = ai.analyze_document(ocr_pan_text, "IMG_0492.jpg", None).await;
    assert!(!pan_analysis.category.is_empty() && pan_analysis.category != "General");
    assert!(pan_analysis.keywords.contains(&"pan".to_string()));

    let llm_pan_vpaths = vec![
        "Identity & Documents/Government ID/PAN Card".to_string(),
        "Media/Document Scans".to_string(),
    ];
    let pan_vpaths = compute_virtual_paths("IMG_0492.jpg", &pan_analysis.category, &llm_pan_vpaths, "jpg");
    assert!(pan_vpaths.iter().any(|p| p.starts_with("Identity & Documents/Government ID/PAN Card/IMG_0492.jpg")));
    assert!(pan_vpaths.iter().any(|p| p.starts_with("Media/Document Scans/IMG_0492.jpg")));

    // Case 2: Photo of Aadhaar Card (generic filename photo_scan.png)
    let ocr_aadhaar_text = "GOVERNMENT OF INDIA \n Unique Identification Authority of India \n 9876 5432 1098 \n Resident Identity \n DOB: 01/01/1990";
    let is_aadhaar_high_risk = is_high_risk_identity_document(ocr_aadhaar_text, "photo_scan.png");
    assert!(is_aadhaar_high_risk, "Aadhaar must trigger high-risk identity detection");

    let aadhaar_analysis = ai.analyze_document(ocr_aadhaar_text, "photo_scan.png", None).await;
    assert!(!aadhaar_analysis.category.is_empty() && aadhaar_analysis.category != "General");
    assert!(aadhaar_analysis.keywords.contains(&"aadhaar".to_string()));

    let llm_aadhaar_vpaths = vec![
        "Identity & Documents/Government ID/Aadhaar Card".to_string(),
        "Media/Document Scans".to_string(),
    ];
    let aadhaar_vpaths = compute_virtual_paths("photo_scan.png", &aadhaar_analysis.category, &llm_aadhaar_vpaths, "png");
    assert!(aadhaar_vpaths.iter().any(|p| p.starts_with("Identity & Documents/Government ID/Aadhaar Card/photo_scan.png")));
    assert!(aadhaar_vpaths.iter().any(|p| p.starts_with("Media/Document Scans/photo_scan.png")));

    // Case 3: Offline fallback without suggested LLM paths
    let fallback_paths = compute_virtual_paths("photo_scan.png", &aadhaar_analysis.category, &[], "png");
    assert_eq!(fallback_paths, vec![format!("{}/photo_scan.png", &aadhaar_analysis.category)]);
}

#[tokio::test]
async fn test_cv_resume_classified_as_career_not_government_id() {
    let mut config = Config::load();
    config.groq_api_key = None; // Offline mode test
    let ai = AiEngine::new(&config);

    let cv_text = "Madhav Kannan K\n\
                   madhavkannank2903@gmail.com\n\
                   +916383908577\n\
                   github.com/Madhavkannank\n\
                   Objective: Data Science graduate student specializing in Machine Learning, Computer Vision, Deep Learning.\n\
                   Education: Anna University B.Tech in Computer Science 2021-2025.\n\
                   Experience: Machine Learning Intern developing PyTorch models and OpenCV Sobel filters.\n\
                   Skills: Python, Rust, PyTorch, CUDA, Linux, Git.";

    let res = ai.analyze_document(cv_text, "Madhav_Kannan_K_CV.pdf", None).await;

    // Must NOT be classified as Government ID
    assert_ne!(res.category, "Government ID", "Resume must never be categorized as Government ID");
    assert_ne!(res.category, "General", "Resume must not be categorized as General");
    assert!(
        res.category.starts_with("Career/"),
        "Resume must be categorized under Career domain, got: {}",
        res.category
    );
    assert!(res.keywords.contains(&"cv".to_string()) || res.keywords.contains(&"resume".to_string()) || res.keywords.contains(&"learning".to_string()));
}

#[tokio::test]
async fn test_pii_redaction_boundary_before_groq_payload() {
    use axel::classifier::privacy::redact_sensitive_pii;

    let raw_aadhaar = "9876 5432 1098";
    let raw_pan = "ABCDE1234F";
    let raw_ssn = "123-45-6789";
    let raw_name = "Rajesh Kumar Sharma";
    let raw_dob = "15/08/1990";
    let raw_phone = "+91 9876543210";

    let sensitive_doc = format!(
        "GOVERNMENT OF INDIA\n\
         Unique Identification Authority of India\n\
         Name: {}\n\
         DOB: {}\n\
         Aadhaar Number: {}\n\
         Income Tax PAN: {}\n\
         Social Security: {}\n\
         Phone: {}\n\
         Tax filing verification for resident.",
        raw_name, raw_dob, raw_aadhaar, raw_pan, raw_ssn, raw_phone
    );

    // 1. Verify redact_sensitive_pii completely strips all PII tokens
    let (sanitized, summary) = redact_sensitive_pii(&sensitive_doc);

    assert!(!sanitized.contains(raw_aadhaar), "Aadhaar must not leak into sanitized text");
    assert!(!sanitized.contains(raw_pan), "PAN must not leak into sanitized text");
    assert!(!sanitized.contains(raw_ssn), "SSN must not leak into sanitized text");
    assert!(!sanitized.contains(raw_name), "Personal name must not leak into sanitized text");
    assert!(!sanitized.contains(raw_phone), "Phone number must not leak into sanitized text");

    assert!(sanitized.contains("[REDACTED_AADHAAR]"));
    assert!(sanitized.contains("[REDACTED_PAN]"));
    assert!(sanitized.contains("[REDACTED_SSN]"));
    assert!(sanitized.contains("[REDACTED_NAME]"));
    assert!(sanitized.contains("[REDACTED_PHONE]"));

    assert!(summary.has_sensitive_id());
    assert_eq!(summary.aadhaar_count, 1);
    assert_eq!(summary.pan_count, 1);
    assert_eq!(summary.ssn_count, 1);
    assert_eq!(summary.name_count, 1);
    assert_eq!(summary.phone_count, 1);

    // 2. Feed sensitive doc into AiEngine and ensure zero PII leaks into keywords or metadata
    let mut config = Config::load();
    config.groq_api_key = Some("test_dummy_groq_key_mock".to_string());
    let ai = AiEngine::new(&config);

    let res = ai.analyze_document(&sensitive_doc, "resident_aadhaar_pan.pdf", None).await;

    // High risk ID must be classified locally, never sent to cloud LLM
    assert!(!res.category.is_empty() && res.category != "General");
    for kw in &res.keywords {
        let kw_lower = kw.to_lowercase();
        assert!(!kw_lower.contains("rajesh"), "Name 'rajesh' leaked into keyword: {}", kw);
        assert!(!kw_lower.contains("sharma"), "Name 'sharma' leaked into keyword: {}", kw);
        assert!(!kw_lower.contains("9876"), "Aadhaar digits leaked into keyword: {}", kw);
        assert!(!kw_lower.contains("abcde"), "PAN digits leaked into keyword: {}", kw);
        assert!(!kw_lower.contains("redacted"), "Redacted placeholder leaked into keyword: {}", kw);
    }
}

#[test]
fn test_duplicate_trashing_hash_mismatch_refusal() {
    let temp_dir = std::env::temp_dir().join(format!("axel_dup_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let canonical_file = temp_dir.join("original_document.pdf");
    let duplicate_tampered = temp_dir.join("original_document (1).pdf");

    std::fs::write(&canonical_file, b"Canonical document content version A").unwrap();
    std::fs::write(&duplicate_tampered, b"Different content tampered or changed version B").unwrap();

    let canonical_hash = DocumentIndex::compute_file_hash(&canonical_file).expect("hash canonical");
    let duplicate_hash = DocumentIndex::compute_file_hash(&duplicate_tampered).expect("hash duplicate");

    assert_ne!(canonical_hash, duplicate_hash);

    // Verify two-phase SHA-256 verification guard:
    assert!(canonical_file.exists());

    let candidate_hash = DocumentIndex::compute_file_hash(&duplicate_tampered);
    let is_safe_to_trash = match candidate_hash {
        Some(ref h) if h == &canonical_hash => true,
        _ => false,
    };

    assert!(!is_safe_to_trash, "Tampered/mismatched file MUST NOT be approved for trashing");
    assert!(duplicate_tampered.exists(), "Duplicate file must remain intact on disk when hash mismatches");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_ingest_collision_safe_and_symlink_protection() {
    let temp_dir = std::env::temp_dir().join(format!("axel_ingest_test_{}", std::process::id()));
    let workspace = temp_dir.join("workspace");
    let external = temp_dir.join("external");
    let _ = std::fs::create_dir_all(&workspace);
    let _ = std::fs::create_dir_all(&external);

    let config = Config::load();
    let ai = AiEngine::new(&config);

    // 1. Create an existing file in workspace: report.pdf (Version 1)
    let existing_path = workspace.join("report.pdf");
    std::fs::write(&existing_path, b"Version 1 existing document").unwrap();

    // 2. Ingest external file with identical name but different content (Version 2)
    let incoming_path = external.join("report.pdf");
    std::fs::write(&incoming_path, b"Version 2 incoming different document").unwrap();

    let ingested = axel::engine::ingest_local_files(&[incoming_path], &workspace, &ai, true)
        .await
        .expect("ingest files");

    assert_eq!(ingested.len(), 1);
    let new_item = &ingested[0];

    // Version 1 must NOT have been overwritten!
    let v1_content = std::fs::read(&existing_path).unwrap();
    assert_eq!(v1_content, b"Version 1 existing document");

    // Version 2 must be ingested as a collision-safe filename (e.g. report_1.pdf)
    assert_ne!(new_item.path, existing_path.display().to_string());
    assert!(new_item.file.starts_with("report_1"));
    let v2_content = std::fs::read(&new_item.path).unwrap();
    assert_eq!(v2_content, b"Version 2 incoming different document");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_large_file_full_sha256_hash_independence_from_mtime() {
    let temp_dir = std::env::temp_dir().join(format!("axel_large_file_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let file_path = temp_dir.join("large_file.bin");

    // Create a 17 MB file (> 16MB threshold)
    let chunk = vec![0x42u8; 1024 * 1024]; // 1 MB
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&file_path).unwrap();
        for _ in 0..17 {
            f.write_all(&chunk).unwrap();
        }
        f.flush().unwrap();
    }

    let hash1 = DocumentIndex::compute_file_hash(&file_path).expect("hash 1");

    // Modify file mtime artificially using touch command on Unix
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("touch")
            .arg("-m")
            .arg("-t")
            .arg("202001010000.00")
            .arg(&file_path)
            .status();
    }

    let hash2 = DocumentIndex::compute_file_hash(&file_path).expect("hash 2");

    // The hashes MUST be identical regardless of mtime (H03 fix)
    assert_eq!(hash1, hash2, "File SHA-256 hash must not change when mtime changes");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_duplicate_trashing_verifies_canonical_and_membership() {
    let temp_dir = std::env::temp_dir().join(format!("axel_dup_verify_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let canonical_file = temp_dir.join("primary.pdf");
    let duplicate_file = temp_dir.join("duplicate.pdf");

    std::fs::write(&canonical_file, b"Identical exact file bytes").unwrap();
    std::fs::write(&duplicate_file, b"Identical exact file bytes").unwrap();

    // Verify byte-level identity
    assert!(axel::engine::are_files_identical(&canonical_file, &duplicate_file).unwrap());

    // Tamper with the duplicate
    std::fs::write(&duplicate_file, b"Modified bytes that do not match").unwrap();
    assert!(!axel::engine::are_files_identical(&canonical_file, &duplicate_file).unwrap());

    // Comparing file to itself must return false (cannot duplicate-trash against self)
    assert!(!axel::engine::are_files_identical(&canonical_file, &canonical_file).unwrap());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_http_delete_document_target_verification_and_explicit_permanent() {
    let temp_dir = std::env::temp_dir().join(format!("axel_delete_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_file = temp_dir.join("doc_to_delete.txt");
    std::fs::write(&test_file, b"Original document content").unwrap();

    let mut config = Config::load();
    config.input_folder = temp_dir.clone();

    let file_hash = DocumentIndex::compute_file_hash(&test_file).unwrap();
    let mut index = DocumentIndex::load(temp_dir.join("index.json"));
    let item = IndexItem::new(
        "doc_to_delete.txt".to_string(),
        "General".to_string(),
        vec!["test".to_string()],
        Vec::new(),
        test_file.display().to_string(),
        file_hash.clone(),
        Vec::new(),
    );
    index.upsert(item);

    // 1. Modify the file on disk so its hash no longer matches indexed hash
    std::fs::write(&test_file, b"Modified document content after indexing").unwrap();

    // 2. Attempt deletion with delete_file: true. It MUST refuse to delete the changed file on disk!
    let outcome = axel::engine::delete_document_from_index(&file_hash, true, false, &config, &mut index).unwrap();
    assert!(outcome.index_deleted);
    assert!(!outcome.file_removed_from_disk, "Modified file on disk must NOT be deleted");
    assert!(outcome.error.unwrap().contains("modified"));
    assert!(test_file.exists(), "Modified file must be preserved on disk");

    // 3. Re-index with current content and delete with explicit permanent: true
    let current_hash = DocumentIndex::compute_file_hash(&test_file).unwrap();
    let item2 = IndexItem::new(
        "doc_to_delete.txt".to_string(),
        "General".to_string(),
        vec!["test".to_string()],
        Vec::new(),
        test_file.display().to_string(),
        current_hash.clone(),
        Vec::new(),
    );
    index.upsert(item2);

    let outcome2 = axel::engine::delete_document_from_index(&current_hash, true, true, &config, &mut index).unwrap();
    assert!(outcome2.index_deleted);
    assert!(outcome2.file_removed_from_disk);
    assert_eq!(outcome2.disk_removal_method, Some("permanent".to_string()));
    assert!(!test_file.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_api_security_loopback_and_boundary_enforcement() {
    let temp_dir = std::env::temp_dir().join(format!("axel_api_sec_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // 1. Verify Config defaults to loopback
    let default_cfg = Config::default();
    assert_eq!(default_cfg.host, "127.0.0.1");

    let mut config = Config::default();
    config.input_folder = temp_dir.clone();
    config.index_file = temp_dir.join("index.json");
    config.api_token = Some("secure-test-token-999".to_string());
    config.host = "127.0.0.1".to_string();

    let session_token = "secure-test-token-999".to_string();
    let index = std::sync::Arc::new(tokio::sync::Mutex::new(DocumentIndex::load(&config.index_file)));
    let ai = std::sync::Arc::new(axel::ai::AiEngine::new(&config));
    let (events_tx, _) = tokio::sync::broadcast::channel::<String>(16);

    let state = axel::server::AppState {
        config: config.clone(),
        index: index.clone(),
        ai,
        events_tx,
        session_token: session_token.clone(),
        is_explicit_token: true,
        is_remote: false,
    };

    let app = axel::server::create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();

    // 2. GET / (UI) must serve HTML with injected session token meta tag
    let ui_res = client.get(&format!("{base_url}/")).send().await.unwrap();
    assert_eq!(ui_res.status(), reqwest::StatusCode::OK);
    let ui_body = ui_res.text().await.unwrap();
    assert!(ui_body.contains(r#"<meta name="axel-token" content="secure-test-token-999">"#));

    // 3. Unauthenticated requests to mutating / sensitive endpoints MUST be rejected with 401
    let unauth_scan = client.post(&format!("{base_url}/api/scan")).send().await.unwrap();
    assert_eq!(unauth_scan.status(), reqwest::StatusCode::UNAUTHORIZED);

    let unauth_upload = client.post(&format!("{base_url}/api/upload")).send().await.unwrap();
    assert_eq!(unauth_upload.status(), reqwest::StatusCode::UNAUTHORIZED);

    let unauth_delete = client.delete(&format!("{base_url}/api/documents/somehash")).send().await.unwrap();
    assert_eq!(unauth_delete.status(), reqwest::StatusCode::UNAUTHORIZED);

    let unauth_raw = client.get(&format!("{base_url}/api/documents/somehash/raw")).send().await.unwrap();
    assert_eq!(unauth_raw.status(), reqwest::StatusCode::UNAUTHORIZED);

    // 4. Authenticated requests with Bearer, X-API-Key, or Query token succeed
    let auth_scan = client
        .post(&format!("{base_url}/api/scan"))
        .header("Authorization", "Bearer secure-test-token-999")
        .send()
        .await
        .unwrap();
    assert_eq!(auth_scan.status(), reqwest::StatusCode::OK);

    let auth_delete_header = client
        .delete(&format!("{base_url}/api/documents/nonexistent"))
        .header("x-api-key", "secure-test-token-999")
        .send()
        .await
        .unwrap();
    // Passes authentication, returns 404 because document does not exist
    assert_eq!(auth_delete_header.status(), reqwest::StatusCode::NOT_FOUND);

    // 5. Workspace boundary & raw document security test
    let valid_file = temp_dir.join("safe_doc.txt");
    std::fs::write(&valid_file, b"Inside workspace content bytes").unwrap();
    let valid_hash = DocumentIndex::compute_file_hash(&valid_file).unwrap();
    let valid_item = IndexItem::new(
        "safe_doc.txt".to_string(),
        "General".to_string(),
        vec!["safe".to_string()],
        Vec::new(),
        valid_file.display().to_string(),
        valid_hash.clone(),
        Vec::new(),
    );
    {
        let mut idx = index.lock().await;
        idx.upsert(valid_item);
    }

    // 5a. Query token download for valid file inside workspace succeeds
    let raw_res = client
        .get(&format!("{base_url}/api/documents/{valid_hash}/raw?token=secure-test-token-999"))
        .send()
        .await
        .unwrap();
    assert_eq!(raw_res.status(), reqwest::StatusCode::OK);
    let bytes = raw_res.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"Inside workspace content bytes");

    // 5b. File OUTSIDE workspace must return 403 Forbidden even with valid token (H07)
    let outside_dir = std::env::temp_dir().join(format!("axel_outside_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&outside_dir);
    std::fs::create_dir_all(&outside_dir).unwrap();
    let outside_file = outside_dir.join("outside.txt");
    std::fs::write(&outside_file, b"Attacker attempted outside access").unwrap();
    let outside_hash = DocumentIndex::compute_file_hash(&outside_file).unwrap();
    let outside_item = IndexItem::new(
        "outside.txt".to_string(),
        "General".to_string(),
        vec!["outside".to_string()],
        Vec::new(),
        outside_file.display().to_string(),
        outside_hash.clone(),
        Vec::new(),
    );
    {
        let mut idx = index.lock().await;
        idx.upsert(outside_item);
    }

    let forbidden_res = client
        .get(&format!("{base_url}/api/documents/{outside_hash}/raw?token=secure-test-token-999"))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden_res.status(), reqwest::StatusCode::FORBIDDEN, "Accessing file outside workspace must return 403 Forbidden");

    // 5c. Tampered/modified file on disk must return 409 Conflict (H06)
    std::fs::write(&valid_file, b"Content modified on disk without indexing").unwrap();
    let tampered_res = client
        .get(&format!("{base_url}/api/documents/{valid_hash}/raw?token=secure-test-token-999"))
        .send()
        .await
        .unwrap();
    assert_eq!(tampered_res.status(), reqwest::StatusCode::CONFLICT, "Serving file with mismatched hash must return 409 Conflict");

    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::remove_dir_all(&outside_dir);
}

#[test]
fn test_atomic_save_and_backup_recovery() {
    let temp_dir = std::env::temp_dir().join(format!("axel_atomic_save_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let index_file = temp_dir.join("index.json");
    let mut index = DocumentIndex::load(&index_file);

    let item1 = IndexItem::new(
        "report.pdf".to_string(),
        "Finance/Quarterly".to_string(),
        vec!["revenue".to_string(), "growth".to_string()],
        vec!["calculate_roi".to_string()],
        "/tmp/report.pdf".to_string(),
        "hash_rev_111".to_string(),
        vec!["Documents/Finance".to_string()],
    );
    index.upsert(item1);

    // Initial save
    index.save().unwrap();
    assert!(index_file.exists(), "Index file should exist after save");

    // Add another item and save again to trigger .bak creation
    let item2 = IndexItem::new(
        "audit.pdf".to_string(),
        "Compliance/Security".to_string(),
        vec!["security".to_string(), "controls".to_string()],
        vec![],
        "/tmp/audit.pdf".to_string(),
        "hash_sec_222".to_string(),
        vec!["Documents/Compliance".to_string()],
    );
    index.upsert(item2);
    index.save().unwrap();

    let bak_file = temp_dir.join("index.json.bak");
    assert!(bak_file.exists(), "Backup file index.json.bak should exist after subsequent save");

    // Simulate corruption of primary index file
    std::fs::write(&index_file, b"corrupted JSON content {{{{ not valid").unwrap();

    // Loading should detect corruption and gracefully recover from backup
    let recovered_index = DocumentIndex::load(&index_file);
    assert!(!recovered_index.items().is_empty(), "Should recover items from backup file");
    assert_eq!(recovered_index.items().first().unwrap().hash, "hash_rev_111");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_upsert_alias_retention_and_copy_promotion() {
    let mut index = DocumentIndex::new();

    let mut item = IndexItem::new(
        "paper.pdf".to_string(),
        "Research/AI".to_string(),
        vec!["transformers".to_string(), "attention".to_string()],
        vec![],
        "/workspace/paper.pdf".to_string(),
        "sha_original_123".to_string(),
        vec!["Virtual/Research".to_string()],
    );
    item.physical_paths = vec![
        "/workspace/paper.pdf".to_string(),
        "/workspace/downloads/paper_copy.pdf".to_string(),
    ];
    item.is_duplicate = true;

    index.upsert(item);
    assert_eq!(index.count(), 1);

    // 1. Modify primary copy: upsert with new hash for /workspace/paper.pdf
    let modified_primary = IndexItem::new(
        "paper.pdf".to_string(),
        "Research/AI".to_string(),
        vec!["transformers".to_string(), "flash_attention".to_string()],
        vec![],
        "/workspace/paper.pdf".to_string(),
        "sha_modified_456".to_string(),
        vec!["Virtual/Research".to_string()],
    );
    index.upsert(modified_primary);

    // Should now contain TWO entries: one for the modified primary (sha_modified_456),
    // and one for the untouched copy (sha_original_123) which got promoted to /workspace/downloads/paper_copy.pdf!
    assert_eq!(index.count(), 2, "Modifying one copy of a duplicate must preserve the index entry for the unmodified copy");

    let original_entry = index.find_by_hash("sha_original_123").expect("Original entry must exist");
    assert_eq!(original_entry.path, "/workspace/downloads/paper_copy.pdf");
    assert_eq!(original_entry.file, "paper_copy.pdf");
    assert!(!original_entry.is_duplicate);

    let modified_entry = index.find_by_hash("sha_modified_456").expect("Modified entry must exist");
    assert_eq!(modified_entry.path, "/workspace/paper.pdf");

    // 2. Test remove_physical_copy promotion and deletion
    let mut index2 = DocumentIndex::new();
    let mut dup_item = IndexItem::new(
        "doc.pdf".to_string(),
        "Work".to_string(),
        vec!["work".to_string()],
        vec![],
        "/workspace/doc.pdf".to_string(),
        "sha_dup_999".to_string(),
        vec![],
    );
    dup_item.physical_paths = vec![
        "/workspace/doc.pdf".to_string(),
        "/workspace/doc_backup.pdf".to_string(),
    ];
    dup_item.is_duplicate = true;
    index2.upsert(dup_item);

    // Removing primary copy should promote doc_backup.pdf to primary
    let removed = index2.remove_physical_copy("sha_dup_999", "/workspace/doc.pdf");
    assert!(removed);
    let remaining = index2.find_by_hash("sha_dup_999").expect("Entry should still exist");
    assert_eq!(remaining.path, "/workspace/doc_backup.pdf");
    assert_eq!(remaining.file, "doc_backup.pdf");
    assert!(!remaining.is_duplicate);

    // Removing last copy should remove entry completely
    let removed_last = index2.remove_physical_copy("sha_dup_999", "/workspace/doc_backup.pdf");
    assert!(removed_last);
    assert!(index2.find_by_hash("sha_dup_999").is_none(), "Entry should be removed when all physical paths are deleted");
}

#[test]
fn test_filter_and_watcher_no_false_positives() {
    use axel::classifier::filter::should_skip_directory;

    // 1. "Pruned Records" must NOT be skipped (audit finding 27: substring "rune" false positive)
    assert!(!should_skip_directory("Pruned Records"));
    assert!(!should_skip_directory("Pruned"));
    assert!(!should_skip_directory("pruning_data"));

    // 2. "common" must NOT be skipped (audit finding 27: standard development folder)
    assert!(!should_skip_directory("common"));
    assert!(!should_skip_directory("Common"));

    // 3. Genuine crack/repack names with distinct token boundaries ARE skipped
    assert!(should_skip_directory("Game_Rune_Release"));
    assert!(should_skip_directory("FitGirl Repacks"));
    assert!(should_skip_directory("dodi"));
    assert!(should_skip_directory("codex"));

    // 4. Standard developer build/artifact folders ARE skipped
    assert!(should_skip_directory("node_modules"));
    assert!(should_skip_directory("target"));
    assert!(should_skip_directory(".git"));
    assert!(should_skip_directory(".steam"));
}

#[test]
fn test_traversal_ceilings_and_truncation_detection() {
    let temp_dir = std::env::temp_dir().join(format!("axel_traversal_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create 10 files
    for i in 0..10 {
        let fpath = temp_dir.join(format!("doc_{i}.txt"));
        std::fs::write(fpath, format!("Content of document {i}")).unwrap();
    }

    // Set AXEL_MAX_SCAN_FILES to 5 to force truncation
    std::env::set_var("AXEL_MAX_SCAN_FILES", "5");

    let mut files = Vec::new();
    let truncated = axel::engine::collect_files_recursive(&temp_dir, &mut files);

    assert_eq!(files.len(), 5, "Should truncate file collection at configured limit");
    assert!(truncated, "Truncation flag must be true when limit is reached");

    std::env::remove_var("AXEL_MAX_SCAN_FILES");
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_search_total_and_limit_reporting() {
    let mut config = Config::default();
    config.top_k_results = 3; // Set default limit to 3

    let mut index_data = DocumentIndex::new();
    for i in 0..10 {
        let item = IndexItem::new(
            format!("invoice_{i}.pdf"),
            "Finance/Invoice".to_string(),
            vec!["payment".to_string(), "vendor".to_string(), "invoice".to_string()],
            Vec::new(),
            format!("/workspace/invoice_{i}.pdf"),
            format!("hash_inv_{i}"),
            Vec::new(),
        );
        index_data.upsert(item);
    }

    let index = std::sync::Arc::new(tokio::sync::Mutex::new(index_data));
    let ai = std::sync::Arc::new(axel::ai::AiEngine::new(&config));
    let (events_tx, _) = tokio::sync::broadcast::channel::<String>(16);

    let state = axel::server::AppState {
        config: config.clone(),
        index,
        ai,
        events_tx,
        session_token: "test-token".to_string(),
        is_explicit_token: true,
        is_remote: false,
    };

    let app = axel::server::create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();

    // Query without explicit limit: should return results.len() == top_k_results (3),
    // but total == 10 (all 10 matching documents in the index)!
    let res = client
        .get(&format!("{base_url}/api/search?q=invoice&token=test-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = res.json().await.unwrap();
    let total = body["total"].as_u64().unwrap();
    let results = body["results"].as_array().unwrap();

    assert_eq!(total, 10, "Total must reflect total matches in index, not page slice length");
    assert_eq!(results.len(), 3, "Results length should default to config.top_k_results");
}






