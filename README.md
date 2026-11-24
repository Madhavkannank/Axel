# AXEL: Intelligent Desktop Document Management System

AXEL is a high-performance, single-binary desktop document understanding and management system engineered in pure Rust. Built to replace conventional operating system file managers, AXEL introduces dynamic virtual path constellations, automated semantic categorization, sub-millisecond hybrid search, and native GPU-accelerated visualization without byte duplication.

---

## Vision and Purpose

Traditional operating system file managers (such as Windows Explorer, macOS Finder, and Linux Nautilus) are bound to rigid, single-path hierarchical folder structures conceived in the early days of personal computing. Modern knowledge work requires documents to reside in multiple logical contexts simultaneously without consuming duplicate disk space.

AXEL re-architects personal document management from first principles:
- **Zero Byte Duplication**: Documents are organized into multi-pointer virtual hierarchies. A single file can belong to multiple categories (for example, simultaneously indexed under Academics, Research, and Career) without duplicating physical bytes.
- **Native Single-Binary Performance**: Built entirely in Rust using `egui` and `axum`. AXEL runs within a lean ~10 MB memory footprint with instantaneous sub-millisecond startup, completely eliminating heavy runtimes such as Electron, Node.js, or Python.
- **Zero-Terminal Configuration**: Users never need to configure API keys, ports, or model parameters via terminal commands or manual `.env` edits. An atomic, persistent in-app settings dialog manages all operational parameters.
- **Privacy Shield & Air-Gapped Operation**: Comprehensive local-first architecture featuring automated PII redaction prior to cloud dispatch, crash-safe atomic persistence, and a complete Force Offline mode that operates using mathematical vector centroids.

---

## Architectural Highlights

### 1. In-App Onboarding & Architecture Guide
On first launch, AXEL presents an onboarding modal introducing system architecture, security boundaries, and Groq cloud integration. This guide persists dismissal state atomically to disk and remains accessible at any time via the Settings modal.
- **How It Works**: Highlighting local indexing, semantic classification, virtual folder trees without byte duplication, and the interactive Galaxy canvas.
- **Native Permissions**: Documenting local filesystem access restricted strictly to configured workspaces, non-destructive OS recycle bin/trash routing, local loopback HTTP server binding (127.0.0.1), and zero telemetry or diagnostic beacons.
- **Privacy & Security**: Automated deterministic pre-flight redaction of secrets, passwords, credentials, credit cards, Aadhaar, PAN, SSN, and emails; 100% air-gapped Force Offline mode; atomic temporary file staging with crash-safe `.bak` backups.
- **Groq Cloud Integration**: Direct link to the Groq Console (`https://console.groq.com/keys`), latency benefits, large context windows, and runtime model enumeration.

### 2. Dynamic Groq Model Discovery & Latency Benchmarking
AXEL connects to live OpenAI-compatible endpoints on Groq's Language Processing Unit (LPU) cloud. Rather than relying on static lists or hardcoded models, AXEL:
- Dynamically queries `https://api.groq.com/openai/v1/models` on credential configuration.
- Automatically extracts and populates the available models associated with the user's account.
- Benchmarks true network round-trip latency in milliseconds.
- Provides automatic failover across user-selected primary and fallback models.

### 3. Why Groq Cloud Acceleration?
- **LPU Hardware Acceleration**: Groq's Language Processing Unit silicon delivers inference speeds exceeding 500 tokens per second with sub-200ms round-trip latency, enabling real-time categorization rather than sluggish batch queues.
- **Massive Context Windows**: Models such as LLaMA 3.3 70B offer up to 128,000-token context windows, allowing AXEL to process comprehensive academic textbooks, dense codebases, and multi-page legal contracts without lossy chunking.
- **Generous Free Tier & Open-Weights Freedom**: Users retain full control over their model choice, preventing vendor lock-in.

### 4. Pure-Rust Multi-Format Extraction Engine
- **PDF Extraction**: Memory-safe parsing via `pdf-extract`.
- **Zero-Dependency DOCX Parsing**: Direct streaming XML decompression of `word/document.xml` with entity decoding.
- **Cross-Platform OCR**: Auto-discovery of local Tesseract OCR binaries with graceful fallback to filename and metadata signals.
- **Code & Text Deconstruction**: Semantic symbol analysis and domain keyword extraction across 40+ programming languages and markup formats.

### 5. Sub-Millisecond Hybrid Search & Galaxy Canvas
Search queries execute against memory-mapped indices in sub-millisecond time by blending three distinct scoring algorithms:
- Exact substring matching.
- Token intersection scoring.
- Jaro-Winkler string similarity (`strsim`).

The native desktop UI features an interactive, hardware-accelerated Galaxy Canvas (60 FPS) that maps document constellations, semantic cluster hubs, and keyword linkages.

---

## Quick Start

### Prerequisites
- Rust 1.80+ (recommended via `rustup`)
- Optional: `tesseract` for scanned image OCR

### Installation from Source
```bash
git clone https://github.com/madhavkannan/Axel.git
cd Axel
cargo build --release
```
The compiled standalone binary is placed in `target/release/axel`.

### Launching the Application

#### Native Desktop Workstation (Primary Interface)
```bash
./target/release/axel ui
```
Launches the native GPU-accelerated desktop workstation with the interactive Galaxy canvas, document inspector, search popover, and in-app settings dialog.

#### Embedded Web Interface & REST API
```bash
./target/release/axel serve --port 8765 --host 127.0.0.1
```
Spawns the local Axum HTTP server and serves the web workstation at `http://127.0.0.1:8765`.

#### Command-Line Operations
```bash
# Verify system dependencies and OCR availability
./target/release/axel check

# Recursively index a folder
./target/release/axel scan ~/Documents

# Extract metadata and classify a single file
./target/release/axel process contract.pdf

# Execute hybrid search from the terminal
./target/release/axel search "distributed systems architecture"

# Launch the filesystem watcher daemon
./target/release/axel watch ~/Documents
```

---

## Configuration & Persistence

AXEL adheres to zero-terminal configuration. All settings can be adjusted through the native desktop or web dialogs and are persisted atomically:

- **Config Path**: `~/.local/share/axel/settings.json` (or platform equivalent in `%LOCALAPPDATA%` on Windows and `~/Library/Application Support` on macOS).
- **Atomic Commits**: Settings and document indices are staged to `.tmp` files and committed via atomic renames, preventing index corruption on unexpected shutdown.
- **Bidirectional Sync**: Changes in the UI automatically sync with local `.env` files when present.

### Configuration Parameters
| Parameter | Default | Description |
|---|---|---|
| `groq_api_key` | None | Groq Cloud API authentication key |
| `model_name` | llama-3.3-70b-versatile | Primary LLM model for categorization |
| `model_fallbacks` | llama-3.1-8b-instant, mixtral-8x7b-32768 | Ordered fallback models for cloud requests |
| `force_offline` | false | Complete air-gapped isolation bypassing cloud calls |
| `enable_cloud_code` | false | Allows sanitized code files to be classified by cloud LLM |
| `enable_ocr` | true | Enables Tesseract OCR for scanned documents |
| `max_file_size_mb` | 100 | Maximum document file size to parse |
| `theme` | Nebula Obsidian | Desktop and Web visual theme |
| `port` | 8765 | Embedded HTTP server port |
| `host` | 127.0.0.1 | Embedded HTTP server bind address |

---

## REST API Reference

When running `axel ui` or `axel serve`, the embedded server exposes the following REST endpoints on `127.0.0.1:8765`:

| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/` | Web workstation dashboard |
| `GET` | `/api/documents` | List all indexed document nodes and metadata |
| `GET` | `/api/documents/:hash` | Retrieve detailed document metadata and text preview |
| `GET` | `/api/documents/:hash/raw` | Stream raw file content from disk |
| `DELETE` | `/api/documents/:hash` | Move document to OS trash or remove from index |
| `GET` | `/api/search?q={query}` | Execute hybrid search across indexed documents |
| `GET` | `/api/graph` | Full galaxy graph topology (nodes, hubs, links) |
| `GET` | `/api/stats` | Document counts, category breakdowns, and system health |
| `GET` | `/api/settings` | Read active runtime configuration |
| `POST` | `/api/settings` | Atomically update configuration and sync with disk |
| `POST` | `/api/settings/test-key` | Test Groq API key and fetch available models |
| `POST` | `/api/scan` | Trigger background indexing scan on workspace |
| `POST` | `/api/upload` | Ingest and index uploaded document files |
| `GET` | `/api/events` | Server-Sent Events (SSE) stream for real-time updates |

---

## Security & Privacy Model

1. **Workspace Sandboxing**: AXEL only interacts with directories explicitly selected by the user. It never scans root filesystems or unselected personal directories.
2. **Deterministic PII Sanitization**: Prior to any outbound LLM transmission, sensitive data patterns (API keys, private tokens, passwords, Aadhaar, PAN, SSN, credit cards, and emails) are detected and replaced with redaction markers.
3. **Non-Destructive Trashing**: File deletions route directly to the native operating system trash bin (via `trash` APIs on Linux, macOS, and Windows) rather than permanent deletion, unless explicitly confirmed.
4. **Local Loopback Binding**: The REST API and WebSocket/SSE interfaces default strictly to `127.0.0.1`, preventing exposure over local area networks or public interfaces.
5. **No Telemetry**: AXEL contains zero tracking libraries, third-party analytics, or crash-reporting beacons.

---

## Project Structure

```
Axel/
├── Cargo.toml            # Rust workspace definition and dependencies
├── src/
│   ├── main.rs           # CLI entry point and sub-command routing
│   ├── lib.rs            # Core Axel library crate
│   ├── config.rs         # Atomic settings persistence and path resolution
│   ├── ai/
│   │   └── mod.rs        # Groq LPU client, model discovery, and failover
│   ├── classifier/
│   │   ├── mod.rs        # Dual-engine classification (deterministic rules + LLM)
│   │   ├── rules.rs      # 10-category deterministic heuristic rules
│   │   └── keywords.rs   # Domain keyword extraction and TF-IDF scoring
│   ├── extractor/
│   │   ├── mod.rs        # File format routing and detection
│   │   ├── pdf.rs        # Pure-Rust PDF text extraction
│   │   ├── docx.rs       # Streaming DOCX XML document parser
│   │   └── ocr.rs        # Tesseract OCR discovery and fallback
│   ├── index/
│   │   └── mod.rs        # SHA-256 deduplication, atomic index, hybrid search
│   ├── watcher/
│   │   └── mod.rs        # OS kernel filesystem notification watcher (notify)
│   ├── server/
│   │   └── mod.rs        # Embedded Axum HTTP server, SSE, and REST API
│   └── desktop/
│       ├── mod.rs        # eframe/egui native desktop workstation
│       ├── onboarding.rs # Architecture, permissions, and Groq guide modal
│       ├── settings.rs   # Atomic in-app settings modal with live validation
│       ├── galaxy.rs     # GPU-accelerated interactive document galaxy
│       └── theme.rs      # Typography, tokens, and geometric icon painter
├── ui/                   # Embedded Web workstation assets (HTML, CSS, JS)
└── tests/
    ├── engine_tests.rs   # Core engine and extraction tests
    └── settings_tests.rs # Settings synchronization, authorization, and persistence tests
```

---

## Authors & Architectural Credits

**Madhav Kannan K**  
Founder & Chief Architect

---

## License

MIT License. Designed and engineered for high-performance, private, local-first computing.
