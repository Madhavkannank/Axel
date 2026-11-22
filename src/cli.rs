use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "axel",
    version = "0.2.0",
    about = "AXEL: Automated Document Extraction, Labeling & Semantic Intelligence in Rust",
    long_about = "A high-performance, single-binary document intelligence tool that automates text extraction (PDF, DOCX, Image OCR), AI-driven categorization, keyword extraction, and vector-precision document retrieval."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan a directory and index all documents (PDF, DOCX, images, text)
    Scan {
        /// Directory path to scan (defaults to configured input folder)
        #[arg(value_name = "DIR")]
        dir: Option<PathBuf>,

        /// Re-process and overwrite already indexed files
        #[arg(short, long)]
        force: bool,
    },

    /// Watch a directory continuously and index new/modified files in real time
    Watch {
        /// Directory path to watch (defaults to configured input folder)
        #[arg(value_name = "DIR")]
        dir: Option<PathBuf>,
    },

    /// Search indexed documents by keyword, category, file name, or topic
    Search {
        /// Search query
        query: String,

        /// Maximum number of search results to display
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    /// Process a single document and print its extracted metadata and category
    Process {
        /// File path to process
        file: PathBuf,
    },

    /// Launch the embedded interactive Web UI and REST API
    Ui {
        /// Port number to listen on
        #[arg(short, long)]
        port: Option<u16>,

        /// Do not automatically open default web browser
        #[arg(long)]
        no_open: bool,
    },

    /// Alias for UI: Start the local server and API
    Serve {
        /// Port number to listen on
        #[arg(short, long)]
        port: Option<u16>,

        /// Do not automatically open default web browser
        #[arg(long)]
        no_open: bool,
    },

    /// Inspect system configuration, OCR dependencies, and index health
    Check,
}
