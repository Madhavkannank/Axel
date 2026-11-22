mod ai;
mod classifier;
mod cli;
mod config;
mod extractor;
mod index;
mod server;
mod watcher;

use ai::AiEngine;
use clap::Parser;
use cli::{Cli, Commands};
use colored::*;
use config::Config;
use extractor::{detect_file_type, extract_text, FileType};
use index::{DocumentIndex, IndexItem};
use std::path::{Path, PathBuf};
use watcher::FolderWatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Scan { dir, force }) => {
            let target_dir = dir.unwrap_or(config.input_folder.clone());
            run_scan(&target_dir, force, &config).await?;
        }
        Some(Commands::Watch { dir }) => {
            let target_dir = dir.unwrap_or(config.input_folder.clone());
            run_watch(&target_dir, &config).await?;
        }
        Some(Commands::Search { query, limit }) => {
            run_search(&query, limit, &config)?;
        }
        Some(Commands::Process { file }) => {
            run_process(&file, &config).await?;
        }
        Some(Commands::Ui { port, no_open }) | Some(Commands::Serve { port, no_open }) => {
            if let Some(p) = port {
                config.port = p;
            }
            server::run_server(config, !no_open).await?;
        }
        Some(Commands::Check) => {
            run_check(&config).await;
        }
        None => {
            // Default behavior when run with no arguments: start interactive UI
            println!("{}", "No command provided. Launching AXEL Web UI...".cyan());
            server::run_server(config, true).await?;
        }
    }

    Ok(())
}

async fn run_scan(target_dir: &Path, force: bool, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", format!("\n📂 Scanning directory: {}", target_dir.display()).bold().blue());

    if !target_dir.exists() {
        println!("{}", format!("Directory not found: {}", target_dir.display()).red());
        return Ok(());
    }

    let mut index = DocumentIndex::load(&config.index_file);
    let ai = AiEngine::new(config);

    let mut files = Vec::new();
    collect_files(target_dir, &mut files)?;

    println!("Found {} candidate files.", files.len());
    let mut indexed_count = 0;
    let mut skipped_count = 0;

    for path in files {
        let file_type = detect_file_type(&path);
        if file_type == FileType::Unsupported {
            skipped_count += 1;
            continue;
        }

        let hash = match DocumentIndex::compute_file_hash(&path) {
            Some(h) => h,
            None => {
                skipped_count += 1;
                continue;
            }
        };

        if !force && index.find_by_hash(&hash).is_some() {
            skipped_count += 1;
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("document")
            .to_string();

        print!("Processing [{}] {} ... ", file_type.as_str().cyan(), file_name.bold());

        let (text, _) = extract_text(&path);
        let ai_res = ai.analyze_document(&text, &file_name).await;

        println!("{} [{}]", "✓".green(), ai_res.category.yellow());

        let item = IndexItem {
            file: file_name,
            category: ai_res.category,
            keywords: ai_res.keywords,
            path: path.display().to_string(),
            hash,
            indexed_at: Some(chrono::Utc::now().to_rfc3339()),
        };

        index.upsert(item);
        indexed_count += 1;
    }

    index.save()?;

    println!("\n{}", "═══════════════════════════════════════════".dimmed());
    println!("Scan Complete!");
    println!("  Indexed: {}", indexed_count.to_string().green().bold());
    println!("  Skipped: {}", skipped_count.to_string().dimmed());
    println!("  Total in Index: {}", index.count().to_string().cyan().bold());
    println!("═══════════════════════════════════════════\n");

    Ok(())
}

async fn run_watch(target_dir: &Path, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", format!("\n👁️  Starting file watcher on: {}", target_dir.display()).bold().green());
    println!("{}", "Drop documents (PDF, DOCX, Images, Text) here to auto-process.".dimmed());
    println!("Press Ctrl+C to stop.\n");

    let watcher = FolderWatcher::new(target_dir).map_err(|e| e.to_string())?;
    let mut index = DocumentIndex::load(&config.index_file);
    let ai = AiEngine::new(config);

    loop {
        if let Some(path) = watcher.poll_next_file() {
            // Give file writes a brief moment to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("document")
                .to_string();

            if let Some(hash) = DocumentIndex::compute_file_hash(&path) {
                if index.find_by_hash(&hash).is_none() {
                    println!("\n[Watcher] Detected new document: {}", file_name.cyan().bold());

                    let (text, _) = extract_text(&path);
                    let ai_res = ai.analyze_document(&text, &file_name).await;

                    println!("  Category: {}", ai_res.category.green().bold());
                    println!("  Keywords: {}", ai_res.keywords.join(", ").dimmed());

                    let item = IndexItem {
                        file: file_name,
                        category: ai_res.category,
                        keywords: ai_res.keywords,
                        path: path.display().to_string(),
                        hash,
                        indexed_at: Some(chrono::Utc::now().to_rfc3339()),
                    };

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

fn run_search(query: &str, limit: usize, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let index = DocumentIndex::load(&config.index_file);
    println!("{}", format!("\n🔍 Searching for: \"{}\"", query).bold().cyan());

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
    println!("{}", format!("\n⚙️  Processing document: {}", file.display()).bold());

    if !file.exists() {
        println!("{}", format!("File not found: {}", file.display()).red());
        return Ok(());
    }

    let file_type = detect_file_type(file);
    let (text, _) = extract_text(file);

    println!("Detected format: {}", file_type.as_str().cyan());
    println!("Extracted text length: {} characters", text.len());

    let ai = AiEngine::new(config);
    let file_name = file.file_name().and_then(|s| s.to_str()).unwrap_or("document");
    let ai_res = ai.analyze_document(&text, file_name).await;

    println!("\n{}", "Analysis Result:".bold().green());
    println!("  Category: {}", ai_res.category.bold().yellow());
    println!("  Keywords: {}", ai_res.keywords.join(", ").cyan());

    if let Some(hash) = DocumentIndex::compute_file_hash(file) {
        println!("  SHA-256:  {}", hash.dimmed());
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
        println!("{}", "Configured (LLaMA cloud acceleration enabled)".green().bold());
    } else {
        println!("{}", "Not set (Will use local Rule Classifier & Keywords)".yellow());
    }

    print!("\nOCR Engine:       ");
    if let Some(bin) = extractor::ocr::find_tesseract_binary() {
        println!("{} (found at {})", "Tesseract ready".green().bold(), bin);
    } else {
        println!("{}", "Tesseract not in PATH (Image OCR will use filename/metadata context)".yellow());
    }

    let index = DocumentIndex::load(&config.index_file);
    println!("\nIndex Status:");
    println!("  Indexed Docs:   {}", index.count().to_string().cyan().bold());

    println!("\n{}", "Status: ✅ AXEL Rust Engine is ready for production!".green().bold());
    println!("To launch UI:     cargo run -- ui");
    println!("To scan folder:   cargo run -- scan\n");
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, files)?;
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    Ok(())
}
