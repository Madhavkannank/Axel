use axel::ai::AiEngine;
use axel::cli::{Cli, Commands};
use axel::config::Config;
use axel::desktop;
use axel::engine::scan_folder_with_progress;
use axel::extractor::{self, detect_file_type, extract_text};
use axel::index::DocumentIndex;
use axel::server;
use axel::watcher::FolderWatcher;
use clap::Parser;
use colored::*;
use std::collections::HashSet;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load();
    let cli = Cli::parse();

    match cli.command {
        None => {
            desktop::run_desktop_app(config, cli.screenshot)?;
        }
        Some(Commands::Ui { dir, screenshot }) => {
            if let Some(d) = dir {
                config.input_folder = d;
            }
            desktop::run_desktop_app(config, screenshot.or(cli.screenshot))?;
        }
        Some(cmd) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(async move {
                match cmd {
                    Commands::Scan { dir, force } => {
                        let target_dir = dir.unwrap_or(config.input_folder.clone());
                        run_scan(&target_dir, force, &config).await?;
                    }
                    Commands::Watch { dir } => {
                        let target_dir = dir.unwrap_or(config.input_folder.clone());
                        run_watch(&target_dir, &config).await?;
                    }
                    Commands::Search { query, limit } => {
                        run_search(&query, limit, &config)?;
                    }
                    Commands::Process { file } => {
                        run_process(&file, &config).await?;
                    }
                    Commands::Serve { port, no_open } => {
                        if let Some(p) = port {
                            config.port = p;
                        }
                        server::run_server(config, !no_open).await?;
                    }
                    Commands::Check => {
                        run_check(&config).await;
                    }
                    Commands::Ui { .. } => unreachable!(),
                }
                Ok::<(), Box<dyn std::error::Error>>(())
            })?;
        }
    }

    Ok(())
}

async fn run_scan(
    target_dir: &Path,
    force: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        format!("\n📂 Scanning directory: {}", target_dir.display())
            .bold()
            .blue()
    );

    if !target_dir.exists() {
        println!(
            "{}",
            format!("Directory not found: {}", target_dir.display()).red()
        );
        return Ok(());
    }

    let mut index = DocumentIndex::load(&config.index_file);
    let ai = AiEngine::new(config);

    let existing_hashes: HashSet<String> = if force {
        HashSet::new()
    } else {
        index.all().iter().map(|i| i.hash.clone()).collect()
    };

    println!("Scanning and indexing with smart filtering and TF-IDF extraction...");

    let (summary, new_items) = scan_folder_with_progress(
        target_dir,
        &existing_hashes,
        &ai,
        force,
        |current, total, file_name| {
            println!(
                "[{}/{}] Processing {} ...",
                current.to_string().cyan(),
                total.to_string().cyan(),
                file_name.bold()
            );
        },
    )
    .await;

    for item in new_items {
        println!(
            "  {} [{}] {} (keywords: {}, symbols: {})",
            "✓".green(),
            item.category.yellow(),
            item.file.bold(),
            item.keywords.len().to_string().cyan(),
            item.symbols.len().to_string().magenta()
        );
        index.upsert(item);
    }

    index.save()?;

    println!("\n{}", "═══════════════════════════════════════════".dimmed());
    println!("Scan Complete!");
    println!("  Indexed: {}", summary.indexed.to_string().green().bold());
    println!("  Skipped: {}", summary.skipped.to_string().dimmed());
    println!(
        "  Total in Index: {}",
        index.count().to_string().cyan().bold()
    );
    println!("═══════════════════════════════════════════\n");

    Ok(())
}

async fn run_watch(target_dir: &Path, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        format!("\n👁️  Starting file watcher on: {}", target_dir.display())
            .bold()
            .green()
    );
    println!(
        "{}",
        "Drop documents (PDF, DOCX, Images, Text) here to auto-process.".dimmed()
    );
    println!("Press Ctrl+C to stop.\n");

    let watcher = FolderWatcher::new(target_dir).map_err(|e| e.to_string())?;
    let mut index = DocumentIndex::load(&config.index_file);
    let ai = AiEngine::new(config);

    loop {
        if let Some(path) = watcher.poll_next_file() {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("document")
                .to_string();

            if let Some(hash) = DocumentIndex::compute_file_hash(&path) {
                let path_str = path.display().to_string();
                if let Some(existing) = index.find_by_hash(&hash) {
                    if !existing.physical_paths.contains(&path_str) {
                        println!(
                            "\n[Watcher] Duplicate download detected: {} -> matches existing canonical: {}",
                            file_name.yellow().bold(),
                            existing.file.cyan().bold()
                        );
                        let mut dup_item = existing.clone();
                        dup_item.path = path_str;
                        index.upsert(dup_item);
                        if let Err(e) = index.save() {
                            eprintln!("[Error] Failed to save index: {e}");
                        }
                    }
                    continue;
                }

                println!(
                    "\n[Watcher] Detected new document: {}",
                    file_name.cyan().bold()
                );

                if let Some(item) = axel::engine::analyze_and_build_item(&path, &ai).await {
                    println!("  Category: {}", item.category.green().bold());
                    println!("  Keywords: {}", item.keywords.join(", ").dimmed());
                    if !item.symbols.is_empty() {
                        println!("  Symbols:  {}", item.symbols.join(", ").magenta());
                    }

                    index.upsert(item);
                    if let Err(e) = index.save() {
                        eprintln!("[Error] Failed to save index: {e}");
                    }
                }
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

fn run_search(
    query: &str,
    limit: usize,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let index = DocumentIndex::load(&config.index_file);
    println!(
        "{}",
        format!("\n🔍 Searching for: \"{}\"", query).bold().cyan()
    );

    let results = index.search(query, limit);

    if results.is_empty() {
        println!("{}", "No matching documents found.".yellow());
        return Ok(());
    }

    println!("\nFound {} results:\n", results.len().to_string().bold());

    for (idx, res) in results.iter().enumerate() {
        let match_badge = match res.match_type.as_str() {
            "exact" => "EXACT".green().bold(),
            "suggested" => "FUZZY".yellow(),
            _ => "PARTIAL".cyan(),
        };

        println!(
            "{}. [{}] {} ({}) - Score: {:.0}%",
            idx + 1,
            match_badge,
            res.item.file.bold(),
            res.item.category.magenta(),
            res.score * 100.0
        );
        println!("   Path: {}", res.item.path.dimmed());
        println!("   Keywords: {}\n", res.item.keywords.join(", ").blue());
    }

    Ok(())
}

async fn run_process(file: &Path, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        format!("\n⚙️  Processing document: {}", file.display()).bold()
    );

    if !file.exists() {
        println!(
            "{}",
            format!("File not found: {}", file.display()).red()
        );
        return Ok(());
    }

    let file_type = detect_file_type(file);
    let (text, _) = extract_text(file);

    println!("Detected format: {}", file_type.as_str().cyan());
    println!("Extracted text length: {} characters", text.len());

    let ai = AiEngine::new(config);
    if let Some(item) = axel::engine::analyze_and_build_item(file, &ai).await {
        println!("\n{}", "Analysis Result:".bold().green());
        println!("  Category:      {}", item.category.bold().yellow());
        println!("  Keywords:      {}", item.keywords.join(", ").cyan());
        if !item.symbols.is_empty() {
            println!("  Symbols:       {}", item.symbols.join(", ").magenta());
        }
        if !item.virtual_paths.is_empty() {
            println!("  Virtual Paths: {}", item.virtual_paths.join(" | ").blue());
        }
        println!("  SHA-256:       {}", item.hash.dimmed());
    } else {
        println!("{}", "Skipped or unsupported file type.".yellow());
    }

    Ok(())
}

async fn run_check(config: &Config) {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║             AXEL System Health Check                 ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    println!("Configuration:");
    println!("  Active Model:   {}", config.model_name.cyan());
    println!("  Input Folder:   {}", config.input_folder.display());
    println!("  Index File:     {}", config.index_file.display());
    println!("  Web UI Port:    {}", config.port);

    print!("  Groq API Key:   ");
    if config.groq_api_key.is_some() {
        println!(
            "{}",
            "Configured (LLaMA cloud acceleration enabled)"
                .green()
                .bold()
        );
    } else {
        println!(
            "{}",
            "Not set (Will use local Rule Classifier & Keywords)".yellow()
        );
    }

    let ocr_status = extractor::ocr::detect_ocr_backend();
    print!("\nOCR Engine:       ");
    match ocr_status.backend {
        extractor::ocr::OcrBackendKind::Tesseract => {
            println!("{}", ocr_status.description.green().bold());
        }
        extractor::ocr::OcrBackendKind::CliWorker | extractor::ocr::OcrBackendKind::LightweightOcrsModel => {
            println!("{}", format!("{} [Lightweight < 20MB Models]", ocr_status.description).green().bold());
        }
        extractor::ocr::OcrBackendKind::MetadataFallback => {
            println!("{}", ocr_status.description.yellow());
        }
    }

    let index = DocumentIndex::load(&config.index_file);
    println!("\nIndex Status:");
    println!(
        "  Indexed Docs:   {}",
        index.count().to_string().cyan().bold()
    );

    println!(
        "\n{}",
        "Status: ✅ AXEL Rust Engine is ready for production!"
            .green()
            .bold()
    );
    println!("To launch Desktop App: cargo run");
    println!("To launch Web API:     cargo run -- serve");
    println!("To scan folder:        cargo run -- scan\n");
}
