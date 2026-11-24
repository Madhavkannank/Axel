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

    // 2. Attempt deletion with delete_file: true. It MUST refuse to delete the changed file on disk and abort!
    let res = axel::engine::delete_document_from_index(&file_hash, true, false, &config, &mut index);
    assert!(res.is_err(), "Must refuse to delete modified file on disk");
    assert!(res.unwrap_err().contains("modified"));
    assert!(test_file.exists(), "Modified file must be preserved on disk");
    assert!(index.find_by_hash(&file_hash).is_some(), "Document must still remain in index when deletion was aborted");

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

#[test]
fn test_delete_document_verified_alias_promotion() {
    let temp_dir = std::env::temp_dir().join(format!("axel_alias_promo_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let file_a = temp_dir.join("copy_a.txt");
    let file_b = temp_dir.join("copy_b.txt");
    let content = b"Identical content across two aliases for promotion test";
    std::fs::write(&file_a, content).unwrap();
    std::fs::write(&file_b, content).unwrap();

    let mut config = Config::default();
    config.input_folder = temp_dir.clone();
    config.index_file = temp_dir.join("index.json");

    let mut index = DocumentIndex::load(config.index_file.clone());
    let hash = DocumentIndex::compute_file_hash(&file_a).unwrap();

    let mut item = IndexItem::new(
        "copy_a.txt".to_string(),
        "Document".to_string(),
        vec!["copy_a.txt".to_string()],
        Vec::new(),
        file_a.display().to_string(),
        hash.clone(),
        Vec::new(),
    );
    item.physical_paths = vec![file_a.display().to_string(), file_b.display().to_string()];
    item.is_duplicate = true;
    index.upsert(item);
    index.save().unwrap();

    // Delete primary file_a permanently
    let outcome = axel::engine::delete_document_from_index(&hash, true, true, &config, &mut index).unwrap();
    assert!(outcome.deleted);
    assert!(!outcome.index_deleted, "Index record should NOT be deleted because copy_b still exists");
    assert!(!file_a.exists(), "Primary file_a should be deleted from disk");
    assert!(file_b.exists(), "Alias file_b must still exist on disk");

    // The index item's primary path should now be promoted to file_b!
    let item_after = index.find_by_hash(&hash).expect("Item must still exist");
    assert_eq!(item_after.path, file_b.display().to_string(), "Primary path must be promoted to file_b");
    assert_eq!(item_after.file, "copy_b.txt", "File name must be updated to copy_b.txt");
    assert_eq!(item_after.physical_paths.len(), 1);
    assert_eq!(item_after.physical_paths[0], file_b.display().to_string());

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

    let state = axel::server::AppState::new(
        config.clone(),
        index.clone(),
        ai,
        events_tx,
        None,
        session_token.clone(),
        false,
    );

    let app = axel::server::create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();

    // 2. GET / (UI) must serve HTML with HttpOnly SameSite=Strict session cookie without leaking token in DOM
    let ui_res = client.get(&format!("{base_url}/")).send().await.unwrap();
    assert_eq!(ui_res.status(), reqwest::StatusCode::OK);
    let set_cookie = ui_res
        .headers()
        .get("set-cookie")
        .expect("Must set session cookie")
        .to_str()
        .unwrap();
    assert!(set_cookie.contains("axel_session="));
    assert!(set_cookie.contains("HttpOnly"));

    // 3. Unauthenticated requests to mutating / sensitive endpoints MUST be rejected with 401
    let unauth_scan = client.post(&format!("{base_url}/api/scan")).send().await.unwrap();
    assert_eq!(unauth_scan.status(), reqwest::StatusCode::UNAUTHORIZED);

    let unauth_upload = client.post(&format!("{base_url}/api/upload")).send().await.unwrap();
    assert_eq!(unauth_upload.status(), reqwest::StatusCode::UNAUTHORIZED);

    let unauth_delete = client.delete(&format!("{base_url}/api/documents/somehash")).send().await.unwrap();
    assert_eq!(unauth_delete.status(), reqwest::StatusCode::UNAUTHORIZED);

    let unauth_raw = client.get(&format!("{base_url}/api/documents/somehash/raw")).send().await.unwrap();
    assert_eq!(unauth_raw.status(), reqwest::StatusCode::UNAUTHORIZED);

    // Query string token must be rejected (H01)
    let query_auth = client.post(&format!("{base_url}/api/scan?token=secure-test-token-999")).send().await.unwrap();
    assert_eq!(query_auth.status(), reqwest::StatusCode::UNAUTHORIZED);

    // 4. Authenticated requests with Bearer or X-API-Key succeed
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

    // 5a. Authenticated download for valid file inside workspace succeeds
    let raw_res = client
        .get(&format!("{base_url}/api/documents/{valid_hash}/raw"))
        .header("Authorization", "Bearer secure-test-token-999")
        .send()
        .await
        .unwrap();
    assert_eq!(raw_res.status(), reqwest::StatusCode::OK);
    let bytes = raw_res.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"Inside workspace content bytes");

    // 5b. File OUTSIDE workspace must return 404 (not found / security abort) even with valid token (H02, M03)
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
        .get(&format!("{base_url}/api/documents/{outside_hash}/raw"))
        .header("Authorization", "Bearer secure-test-token-999")
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden_res.status(), reqwest::StatusCode::NOT_FOUND, "Accessing file outside workspace must be rejected");

    // 5c. Tampered/modified file on disk must be rejected (M03)
    std::fs::write(&valid_file, b"Content modified on disk without indexing").unwrap();
    let tampered_res = client
        .get(&format!("{base_url}/api/documents/{valid_hash}/raw"))
        .header("Authorization", "Bearer secure-test-token-999")
        .send()
        .await
        .unwrap();
    assert_eq!(tampered_res.status(), reqwest::StatusCode::NOT_FOUND, "Serving file with mismatched hash must be rejected");

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

    let state = axel::server::AppState::new(
        config.clone(),
        index,
        ai,
        events_tx,
        Some("test-token".to_string()),
        "test-token".to_string(),
        false,
    );

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
        .get(&format!("{base_url}/api/search?q=invoice"))
        .header("Authorization", "Bearer test-token")
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

#[test]
fn test_m04_same_hash_refresh_updates_category_and_virtual_paths() {
    let mut index = DocumentIndex::new();
    let initial = IndexItem::new(
        "doc.txt".to_string(),
        "Work/Old".to_string(),
        vec!["old_kw".to_string()],
        vec!["old_sym".to_string()],
        "/workspace/doc.txt".to_string(),
        "hash_123".to_string(),
        vec!["virtual_old".to_string()],
    );
    index.upsert(initial.clone());

    let doc = index.find_by_hash("hash_123").unwrap();
    assert_eq!(doc.category, "Work/Old");
    assert_eq!(doc.virtual_paths, initial.virtual_paths);
    assert_eq!(doc.keywords, vec!["old_kw".to_string()]);

    // Re-scan / refresh of same document with new category
    let refreshed = IndexItem::new(
        "doc.txt".to_string(),
        "Work/New".to_string(),
        vec!["new_kw".to_string()],
        vec!["new_sym".to_string()],
        "/workspace/doc.txt".to_string(),
        "hash_123".to_string(),
        vec!["virtual_new".to_string()],
    );
    index.upsert(refreshed.clone());

    let doc_after = index.find_by_hash("hash_123").unwrap();
    assert_eq!(doc_after.category, "Work/New", "M04: Category must be replaced on refresh");
    assert_eq!(doc_after.virtual_paths, refreshed.virtual_paths, "M04: Virtual paths must be replaced on refresh");
    assert_eq!(doc_after.keywords, vec!["new_kw".to_string()]);

    // Duplicate registration at a different path
    let duplicate = IndexItem::new(
        "doc_copy.txt".to_string(),
        "IgnoreCategory".to_string(),
        vec!["ignore_kw".to_string()],
        Vec::new(),
        "/workspace/doc_copy.txt".to_string(),
        "hash_123".to_string(),
        vec!["ignore_vpath".to_string()],
    );
    index.upsert(duplicate);

    let doc_dup = index.find_by_hash("hash_123").unwrap();
    assert_eq!(doc_dup.category, "Work/New", "Duplicate registration must not overwrite primary metadata");
    assert_eq!(doc_dup.virtual_paths, refreshed.virtual_paths);
    assert!(doc_dup.is_duplicate);
    assert_eq!(doc_dup.physical_paths.len(), 2);
}

#[tokio::test]
async fn test_m03_raw_download_tries_physical_aliases_when_primary_missing() {
    let temp_dir = std::env::temp_dir().join(format!("axel_m03_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let workspace = &temp_dir;

    let primary_path = workspace.join("orig.txt");
    let alias_path = workspace.join("alias.txt");

    let content = b"Identical stream content across multiple physical aliases";
    std::fs::write(&primary_path, content).unwrap();
    std::fs::write(&alias_path, content).unwrap();

    let hash = DocumentIndex::compute_file_hash(&primary_path).unwrap();

    let mut item = IndexItem::new(
        "orig.txt".to_string(),
        "Documents".to_string(),
        vec!["stream".to_string()],
        Vec::new(),
        primary_path.display().to_string(),
        hash.clone(),
        Vec::new(),
    );
    item.physical_paths.push(alias_path.display().to_string());
    item.is_duplicate = true;

    // Delete primary file from disk so only alias exists
    std::fs::remove_file(&primary_path).unwrap();
    assert!(!primary_path.exists());
    assert!(alias_path.exists());

    // M03: resolve_verified_document_path must not fail canonicalization on missing primary
    let resolved = axel::engine::resolve_verified_document_path(&item, workspace);
    assert!(resolved.is_some(), "M03: Must resolve live physical alias when primary is missing");
    let resolved_path = resolved.unwrap();
    assert_eq!(
        std::fs::canonicalize(resolved_path).unwrap(),
        std::fs::canonicalize(&alias_path).unwrap()
    );

    // Verify raw download endpoint streams the alias content
    let mut config = Config::default();
    config.input_folder = workspace.to_path_buf();

    let mut index_data = DocumentIndex::new();
    index_data.upsert(item);

    let index = std::sync::Arc::new(tokio::sync::Mutex::new(index_data));
    let ai = std::sync::Arc::new(axel::ai::AiEngine::new(&config));
    let (events_tx, _) = tokio::sync::broadcast::channel::<String>(16);

    let state = axel::server::AppState::new(
        config.clone(),
        index,
        ai,
        events_tx,
        Some("test-token".to_string()),
        "test-token".to_string(),
        false,
    );

    let app = axel::server::create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let res = client
        .get(&format!("{base_url}/api/documents/{hash}/raw"))
        .header("Authorization", "Bearer test-token")
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let downloaded_bytes = res.bytes().await.unwrap();
    assert_eq!(downloaded_bytes.as_ref(), content);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_m02_upload_3_18_mib_file_succeeds() {
    let temp_dir = std::env::temp_dir().join(format!("axel_m02_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let workspace = &temp_dir;

    let mut config = Config::default();
    config.input_folder = workspace.to_path_buf();
    config.index_file = workspace.join("index.json");

    let index = std::sync::Arc::new(tokio::sync::Mutex::new(DocumentIndex::load(&config.index_file)));
    let ai = std::sync::Arc::new(axel::ai::AiEngine::new(&config));
    let (events_tx, _) = tokio::sync::broadcast::channel::<String>(16);

    let state = axel::server::AppState::new(
        config.clone(),
        index,
        ai,
        events_tx,
        Some("test-token".to_string()),
        "test-token".to_string(),
        false,
    );

    let app = axel::server::create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Generate a 3.5 MB file (which fails default Axum 2MB body limit)
    let payload_size = (3.5 * 1024.0 * 1024.0) as usize;
    let large_payload = vec![b'A'; payload_size];

    let boundary = "------------------------boundary1234567890";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"large_upload.txt\"\r\n");
    body.extend_from_slice(b"Content-Type: text/plain\r\n\r\n");
    body.extend_from_slice(&large_payload);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let client = reqwest::Client::new();
    let res = client
        .post(&format!("{base_url}/api/upload"))
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(body)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        reqwest::StatusCode::OK,
        "M02: 3.5 MB upload must succeed without hitting Axum 2MB default body limit"
    );

    let res_body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(res_body["count"].as_u64().unwrap(), 1);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_h07_large_file_legacy_hash_migration() {
    let temp_dir = std::env::temp_dir().join(format!("axel_h07_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("large_legacy.bin");

    // Create a 17 MB file
    let size = 17 * 1024 * 1024;
    let file = std::fs::File::create(&file_path).unwrap();
    file.set_len(size as u64).unwrap();

    let full_sha256 = DocumentIndex::compute_file_hash(&file_path).unwrap();
    let legacy_sparse_hash = "legacy_sparse_hash_v2_dummy".to_string();

    // Stamp with real legacy version 2 xattrs lacking user.axel.hash_algo (A04)
    let _ = xattr::set(&file_path, axel::index::metadata::XATTR_CATEGORY, b"Documents/Large");
    let _ = xattr::set(&file_path, axel::index::metadata::XATTR_HASH, legacy_sparse_hash.as_bytes());
    let _ = xattr::set(&file_path, axel::index::metadata::XATTR_VERSION, b"2");

    // A04: read_from_file must reject cached xattr for files > 16MB when user.axel.hash_algo != "sha256"
    let read_tagged = axel::index::metadata::FileMetadataTag::read_from_file(&file_path);
    assert!(read_tagged.is_none(), "A04: Must reject legacy sparse hash on files > 16MB lacking sha256 algo tag");

    // Create index JSON referencing legacy hash
    let index_file = temp_dir.join("index.json");
    let initial_items = vec![IndexItem::new(
        "large_legacy.bin".to_string(),
        "Documents/Large".to_string(),
        vec!["large".to_string()],
        Vec::new(),
        file_path.display().to_string(),
        legacy_sparse_hash,
        Vec::new(),
    )];
    std::fs::write(&index_file, serde_json::to_string(&initial_items).unwrap()).unwrap();

    // Load index - must auto-migrate legacy large file to full SHA-256
    let mut loaded_index = DocumentIndex::load(&index_file);
    let migrated_item = loaded_index.find_by_hash(&full_sha256);
    assert!(migrated_item.is_some(), "A04: DocumentIndex::load must migrate large legacy file to full SHA-256");

    // Save index - durably commits index.json and stamps version 3 and sha256 algo tag (A04)
    loaded_index.save().unwrap();

    let v_tag = xattr::get(&file_path, axel::index::metadata::XATTR_VERSION).ok().flatten();
    assert_eq!(v_tag.as_deref(), Some(&b"3"[..]), "A04: Migration must stamp user.axel.version = 3");
    let a_tag = xattr::get(&file_path, axel::index::metadata::XATTR_HASH_ALGO).ok().flatten();
    assert_eq!(a_tag.as_deref(), Some(&b"sha256"[..]), "A04: Migration must stamp user.axel.hash_algo = sha256");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_a02_a03_a09_index_persistence_locking_and_backup_schema() {
    let temp_dir = std::env::temp_dir().join(format!("axel_a09_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let index_file = temp_dir.join("index.json");
    let bak_file = temp_dir.join("index.json.bak");

    // 1. Create a valid index with 1 item and save it
    let mut index = DocumentIndex::load(&index_file);
    let item1 = IndexItem::new(
        "doc1.txt".to_string(),
        "Engineering".to_string(),
        vec!["rust".to_string()],
        Vec::new(),
        "documents/doc1.txt".to_string(),
        "hash_doc_1".to_string(),
        Vec::new(),
    );
    index.upsert(item1);
    index.save().unwrap();

    // 2. Perform a second save: backup rotation should copy the valid primary index to index.json.bak
    let item2 = IndexItem::new(
        "doc2.txt".to_string(),
        "Finance".to_string(),
        vec!["invoice".to_string()],
        Vec::new(),
        "documents/doc2.txt".to_string(),
        "hash_doc_2".to_string(),
        Vec::new(),
    );
    index.upsert(item2);
    index.save().unwrap();
    assert!(bak_file.exists(), "A valid primary index must be rotated to index.json.bak");

    // Verify .bak has valid Vec<IndexItem> schema
    let bak_content = std::fs::read_to_string(&bak_file).unwrap();
    let bak_items: Vec<IndexItem> = serde_json::from_str(&bak_content).unwrap();
    assert_eq!(bak_items.len(), 1, "Backup must contain valid 1-item generation");

    // 3. A09: Now corrupt primary with an arbitrary JSON value that is NOT a Vec<IndexItem> (e.g. `{ "invalid": true }`)
    std::fs::write(&index_file, b"{\"invalid\": \"not an array of items\"}").unwrap();

    // Create a new handle attempting to save. The primary validation must REJECT the object JSON
    // and NOT overwrite index.json.bak!
    let mut index_handle = DocumentIndex::load(&index_file);
    // Loading from corrupt primary should recover from valid index.json.bak!
    assert_eq!(index_handle.items().len(), 1, "Must recover valid items from backup");
    assert_eq!(index_handle.items()[0].hash, "hash_doc_1");

    // Verify corrupt primary was preserved to .corrupt.<timestamp>
    let mut found_corrupt_file = false;
    for entry in std::fs::read_dir(&temp_dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().contains("index.json.corrupt") {
            found_corrupt_file = true;
            break;
        }
    }
    assert!(found_corrupt_file, "A09: Corrupt primary index must be preserved to .corrupt.<timestamp>");

    // 4. A02: Verify removed physical alias does not reappear on concurrent merge
    let mut item_with_alias = IndexItem::new(
        "primary.txt".to_string(),
        "Legal".to_string(),
        vec!["contract".to_string()],
        Vec::new(),
        "documents/primary.txt".to_string(),
        "hash_contract".to_string(),
        Vec::new(),
    );
    item_with_alias.physical_paths = vec![
        "documents/primary.txt".to_string(),
        "documents/alias.txt".to_string(),
    ];
    item_with_alias.is_duplicate = true;
    index_handle.upsert(item_with_alias);
    index_handle.save().unwrap();

    // Now remove alias.txt
    index_handle.remove_physical_copy("hash_contract", "documents/alias.txt");
    assert_eq!(index_handle.find_by_hash("hash_contract").unwrap().physical_paths.len(), 1);

    // Save: on-disk index.json currently has both primary.txt and alias.txt
    // With explicit mutation deltas (A02), alias.txt must NOT be revived!
    index_handle.save().unwrap();

    let reloaded = DocumentIndex::load(&index_file);
    let contract_item = reloaded.find_by_hash("hash_contract").unwrap();
    assert_eq!(
        contract_item.physical_paths.len(),
        1,
        "A02: Explicitly removed physical alias must not reappear after save"
    );
    assert_eq!(contract_item.physical_paths[0], "documents/primary.txt");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_m05_duplicate_tracking_and_deletion_reconciliation() {
    let mut index = DocumentIndex::new();
    let hash = "hash_m05_test";
    let p1 = "/workspace/folder_a/report.pdf";
    let p2 = "/workspace/folder_b/report_copy.pdf";

    let mut item = IndexItem::new(
        "report.pdf".to_string(),
        "Finance".to_string(),
        vec!["financial".to_string()],
        Vec::new(),
        p1.to_string(),
        hash.to_string(),
        Vec::new(),
    );
    item.physical_paths = vec![p1.to_string(), p2.to_string()];
    item.is_duplicate = true;
    index.upsert(item);

    assert_eq!(index.count(), 1);
    let entry = index.find_by_hash(hash).unwrap();
    assert_eq!(entry.path, p1);
    assert_eq!(entry.physical_paths.len(), 2);
    assert!(entry.is_duplicate);

    // Reconcile deletion of primary path p1
    let modified = index.remove_path(p1);
    assert!(modified, "M05: remove_path must return true when matching physical path is removed");

    let entry_after = index.find_by_hash(hash).unwrap();
    assert_eq!(entry_after.path, p2, "M05: Surviving alias must be promoted to primary path");
    assert_eq!(entry_after.file, "report_copy.pdf");
    assert_eq!(entry_after.physical_paths.len(), 1);
    assert!(!entry_after.is_duplicate, "M05: Single remaining path is no longer duplicate");

    // Reconcile deletion of remaining path p2
    let modified_last = index.remove_path(p2);
    assert!(modified_last, "M05: remove_path must succeed on last copy");
    assert_eq!(index.count(), 0, "M05: Document must be dropped when all copies are deleted");
    assert!(index.find_by_hash(hash).is_none());
}

#[tokio::test]
async fn test_m06_search_limit_honors_down_to_one() {
    let mut config = Config::default();
    config.top_k_results = 10;

    let mut index_data = DocumentIndex::new();
    for i in 0..5 {
        let item = IndexItem::new(
            format!("doc_{i}.txt"),
            "Docs".to_string(),
            vec!["target".to_string()],
            Vec::new(),
            format!("/workspace/doc_{i}.txt"),
            format!("hash_m06_{i}"),
            Vec::new(),
        );
        index_data.upsert(item);
    }

    let index = std::sync::Arc::new(tokio::sync::Mutex::new(index_data));
    let ai = std::sync::Arc::new(axel::ai::AiEngine::new(&config));
    let (events_tx, _) = tokio::sync::broadcast::channel::<String>(16);

    let state = axel::server::AppState::new(
        config.clone(),
        index,
        ai,
        events_tx,
        Some("test-token".to_string()),
        "test-token".to_string(),
        false,
    );

    let app = axel::server::create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let res = client
        .get(&format!("{base_url}/api/search?q=target&limit=1"))
        .header("Authorization", "Bearer test-token")
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = res.json().await.unwrap();
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "M06: Limit=1 must be strictly honored without floor of 12");
}

#[test]
fn test_m07_dot_directory_include_overrides() {
    std::env::set_var("AXEL_INCLUDE_DIRS", ".github,.notes,.config");

    // Overridden dot-directories should NOT be skipped
    assert!(!axel::classifier::filter::should_skip_directory(".github"));
    assert!(!axel::classifier::filter::should_skip_directory(".notes"));
    assert!(!axel::classifier::filter::should_skip_directory(".config"));

    // Standard ignored dot-directories and build folders should still be skipped
    assert!(axel::classifier::filter::should_skip_directory(".git"));
    assert!(axel::classifier::filter::should_skip_directory(".cache"));
    assert!(axel::classifier::filter::should_skip_directory("node_modules"));

    std::env::remove_var("AXEL_INCLUDE_DIRS");
}

#[test]
fn test_m10_multilingual_unicode_combining_marks_and_keyword_limit() {
    use axel::classifier::nlp::{stem, tokenize};
    use axel::classifier::keywords::KeywordExtractor;

    // Test Devanagari script combining marks: नमस्ते (na + ma + sa + virama + te)
    let devanagari = "नमस्ते भारत सरकार";
    let tokens = tokenize(devanagari);
    assert!(!tokens.is_empty(), "M10: Devanagari tokens must not be stripped");

    // Test European accented marks
    let accented = "résumé café naïve";
    let tokens_acc = tokenize(accented);
    assert!(!tokens_acc.is_empty(), "M10: Accented characters must be preserved");

    // Test stem function on European input does not panic
    let stemmed = stem("cafés");
    assert!(!stemmed.is_empty());

    // Test keyword extraction respecting custom max_keywords
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron";
    let kws = KeywordExtractor::new().extract(text, 3);
    assert!(kws.len() <= 3, "M10: Keyword extraction must strictly respect max_keywords limit");
}

#[test]
fn test_m11_modern_script_types_and_docx_entities() {
    use axel::extractor::{detect_file_type, FileType};
    use axel::extractor::docx::decode_xml_entities;
    use std::path::Path;

    // Modern script detection
    assert_eq!(detect_file_type(Path::new("index.mjs")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("bundle.cjs")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("script.lua")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("app.rb")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("index.php")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("analysis.r")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("MainActivity.kt")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("App.swift")), FileType::Text);
    assert_eq!(detect_file_type(Path::new("tool.pl")), FileType::Text);

    // XML entity decoding
    let raw_xml = "Tom &amp; Jerry &quot;Show&quot; &lt;Episode 1&gt; &#169; 2026 &#x21; &nbsp;";
    let decoded = decode_xml_entities(raw_xml);
    assert!(decoded.contains("Tom & Jerry"));
    assert!(decoded.contains("\"Show\""));
    assert!(decoded.contains("<Episode 1>"));
    assert!(decoded.contains("©"));
    assert!(decoded.contains("!"));
}

#[test]
fn test_m12_isolated_node_layout() {
    use axel::desktop::galaxy::GalaxyLayout;
    use axel::engine::{GraphHub, GraphNode, GraphSnapshot};

    let mut snapshot = GraphSnapshot::default();
    // Hub exists for "Code"
    snapshot.hubs.push(GraphHub {
        id: "Code".to_string(),
        category: "Code".to_string(),
        count: 1,
        top_keywords: vec!["rust".to_string()],
        accent: "#6366f1".to_string(),
    });

    // Node 1 is in "Code"
    snapshot.nodes.push(GraphNode {
        hash: "hash_code".to_string(),
        file: "main.rs".to_string(),
        category: "Code".to_string(),
        file_type: "code".to_string(),
        keywords: vec!["rust".to_string()],
        path: "/workspace/main.rs".to_string(),
        indexed_at: None,
        size_bytes: Some(1024),
        file_exists: true,
    });

    // Node 2 has unmapped/isolated category "UnknownOrphan"
    snapshot.nodes.push(GraphNode {
        hash: "hash_orphan".to_string(),
        file: "isolated.xyz".to_string(),
        category: "UnknownOrphan".to_string(),
        file_type: "unknown".to_string(),
        keywords: vec!["orphan".to_string()],
        path: "/workspace/isolated.xyz".to_string(),
        indexed_at: None,
        size_bytes: Some(512),
        file_exists: true,
    });

    let mut layout = GalaxyLayout::default();
    layout.rebuild(&snapshot);

    assert_eq!(layout.nodes.len(), 2, "M12: Isolated nodes must be included in galaxy layout");
    assert!(layout.node_index.contains_key("hash_orphan"));
    assert!(layout.node_index.contains_key("hash_code"));
}

#[test]
fn test_ui_index_html_supported_upload_types() {
    let html = include_str!("../ui/index.html");
    assert!(html.contains(".mjs"), "ui/index.html must accept modern script formats");
    assert!(html.contains(".lua"), "ui/index.html must accept lua");
    assert!(html.contains(".odt"), "ui/index.html must accept odt");

    let css = include_str!("../ui/styles.css");
    assert!(css.contains(".guide-modal"), "ui/styles.css must include guide modal styling");
}

#[tokio::test]
async fn test_ai_categorization_guardrails_media_and_ordinals() {
    let mut config = Config::load();
    config.groq_api_key = None;
    let ai = AiEngine::new(&config);

    // 1. Video animation must never be categorized as Code
    let video_res = ai
        .analyze_document(
            "format: mp4, title: Title Animation, resolution: 1920x1080",
            "madhav_title_animation.mp4",
            None,
        )
        .await;
    assert_ne!(video_res.category, "Code", "Video must not be Code");
    assert_ne!(video_res.category, "Engineering", "Video must not be Engineering");
    assert!(video_res.category.starts_with("Media/"), "Video must be Media, got: {}", video_res.category);
    for kw in &video_res.keywords {
        assert_ne!(kw, "encoder");
        assert_ne!(kw, "isom");
        assert_ne!(kw, "compatible_brands");
    }

    // 2. 5thSemQPs.pdf must never be categorized as "5th"
    let qps_res = ai
        .analyze_document(
            "",
            "5thSemQPs.pdf",
            None,
        )
        .await;
    assert_ne!(qps_res.category, "5th", "Category must not be raw ordinal '5th'");
    assert!(
        qps_res.category.starts_with("Academics/"),
        "5thSemQPs.pdf must be under Academics, got: {}",
        qps_res.category
    );
    for vp in &qps_res.virtual_paths {
        assert!(!vp.contains(".pdf/"), "Virtual path must not treat .pdf as folder: {vp}");
        assert!(vp.ends_with("5thSemQPs.pdf"));
    }
}

#[test]
fn test_a06_vanished_path_containment_and_matching() {
    let temp_dir = std::env::temp_dir().join(format!("axel_a06_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let workspace = &temp_dir;

    let sub_folder = workspace.join("sub_folder");
    std::fs::create_dir_all(&sub_folder).unwrap();

    let non_existent_file = sub_folder.join("deleted_doc.txt");
    assert!(!non_existent_file.exists());

    // A06: is_path_inside_workspace must return true for vanished paths whose existing ancestors are inside workspace
    assert!(
        axel::engine::is_path_inside_workspace(&non_existent_file, workspace),
        "A06: Vanished path inside workspace ancestor must evaluate to true"
    );

    // Outside path that doesn't exist
    let outside_non_existent = std::env::temp_dir().join(format!("outside_{}", std::process::id())).join("vanished.txt");
    assert!(
        !axel::engine::is_path_inside_workspace(&outside_non_existent, workspace),
        "A06: Vanished path outside workspace must evaluate to false"
    );

    // Test DocumentIndex::remove_path for vanished paths using both relative and absolute representations
    let mut index = DocumentIndex::new();
    let item = IndexItem::new(
        "deleted_doc.txt".to_string(),
        "Docs".to_string(),
        vec!["test".to_string()],
        Vec::new(),
        non_existent_file.display().to_string(),
        "hash_vanished_123".to_string(),
        Vec::new(),
    );
    index.upsert(item);
    assert_eq!(index.count(), 1);

    // Remove using path - must match vanished path accurately
    let removed = index.remove_path(&non_existent_file);
    assert!(removed, "A06: remove_path must match and remove vanished path");
    assert_eq!(index.count(), 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_a11_a17_docx_namespace_decoupling() {
    use axel::extractor::docx::strip_docx_xml;

    // Test default or non-standard namespace prefix like 'w14:', 'ns2:', or no prefix
    let xml_nonstandard = r#"
        <ns2:p>
            <ns2:t>Custom prefix text</ns2:t>
        </ns2:p>
        <p>
            <t>No prefix text</t>
            <br/>
            <t>After break</t>
        </p>
    "#;

    let stripped = strip_docx_xml(xml_nonstandard);
    assert!(stripped.contains("Custom prefix text"), "Must extract text from non-w: namespaced tags");
    assert!(stripped.contains("No prefix text"), "Must extract text from unprefixed tags");
    assert!(stripped.contains("After break"), "Must extract text after line breaks");
}

#[test]
fn test_a08_staging_file_creation_and_atomic_publish() {
    use std::io::Write;
    let temp_dir = std::env::temp_dir().join(format!("axel_a08_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let (mut staging_file, staging_path) =
        axel::engine::create_workspace_staging_file(&temp_dir).expect("Failed to create staging file");
    assert!(staging_path.exists());
    assert!(staging_path.file_name().unwrap().to_str().unwrap().starts_with(".axel_stage_"));

    staging_file.write_all(b"Hello Staged World").unwrap();
    staging_file.sync_all().unwrap();
    drop(staging_file);

    let pub_res = axel::engine::publish_staged_workspace_file(
        &staging_path,
        &temp_dir,
        "sample.txt",
        None,
        &temp_dir,
    )
    .expect("Publish failed");

    assert!(!staging_path.exists(), "Staging path must be cleaned up");
    assert!(pub_res.path.exists());
    assert_eq!(pub_res.file_name, "sample.txt");
    assert_eq!(std::fs::read_to_string(&pub_res.path).unwrap(), "Hello Staged World");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_a10_entropy_token_generation() {
    let t1 = axel::server::generate_secure_token();
    let t2 = axel::server::generate_secure_token();
    assert_eq!(t1.len(), 64);
    assert_eq!(t2.len(), 64);
    assert_ne!(t1, t2);
    assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_a15_virtual_path_dotted_folder_preservation() {
    use axel::index::virtual_tree::compute_virtual_paths;

    let suggested = vec![
        "Research.v2/Drafts".to_string(),
        "Project.2026/Reports".to_string(),
        "Reports/Annual.pdf".to_string(),
    ];

    let paths = compute_virtual_paths("thesis.pdf", "Research.v2", &suggested, "pdf");
    assert!(paths.contains(&"Research.v2/Drafts/thesis.pdf".to_string()));
    assert!(paths.contains(&"Project.2026/Reports/thesis.pdf".to_string()));
    assert!(paths.contains(&"Reports/thesis.pdf".to_string()));
    assert!(!paths.iter().any(|p| p.contains(".pdf/")));

    // Fallback category normalization
    let fb_paths = compute_virtual_paths("paper.docx", "Project.v3/Summary.docx", &[], "docx");
    assert_eq!(fb_paths, vec!["Project.v3/paper.docx".to_string()]);
}







