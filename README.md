# ⚡ AXEL: High-Performance Automated Document Extraction & Intelligence

**AXEL** is a blazing-fast, single-binary document intelligence and search system written in **Rust** (with legacy Python reference components). It automates text extraction across diverse document formats (PDFs, DOCX, Images via OCR, and Plain Text), performs zero-overhead deterministic rule classification and LLM categorization (via Groq LLaMA-3.3-70B), extracts high-signal semantic keywords, indexes documents with SHA-256 deduplication, and provides hybrid search via CLI and an embedded real-time Web UI.

---

## 🚀 Key Advantages of the Rust Engine (v0.2.0)

- **Single Binary, Zero Heavy Runtimes**: Replaces multi-gigabyte PyTorch and desktop GUI dependencies with a lean, standalone binary.
- **Pure-Rust Cross-Platform TLS**: Built with `rustls-tls` — compiles and runs out of the box on Linux, macOS, and Windows without OpenSSL dev package headaches.
- **Embedded Web UI & REST API**: Native asynchronous Axum HTTP server serving a modern dark-mode dashboard with real-time search, stats, and folder scanner trigger.
- **Zero-Dependency DOCX Parsing**: Unzips and parses `word/document.xml` directly with XML entity decoding — no Microsoft Word or heavy office suite required.
- **Pure Rust PDF Text Extraction**: Uses `pdf-extract` for lightning-fast memory-safe parsing.
- **Intelligent OCR Engine**: Cross-platform auto-discovery of Tesseract OCR with resilient metadata and filename context fallback.
- **Groq LLM Acceleration with Fallback**: Async HTTP client targeting `llama-3.3-70b-versatile` with automatic multi-model failover chains (`llama-3.1-8b-instant`, `mixtral-8x7b-32768`), backed by local deterministic classification.
- **Hybrid Search Engine**: Exact substring matching, token intersection scoring, and Jaro-Winkler string distance (`strsim`) with sub-millisecond query latency.

---

## 📦 Quick Start with Rust

### 1. Build from Source
```bash
# Build optimized release binary
cargo build --release

# Run unit tests
cargo test
```

The compiled binary will be located at `./target/release/axel`.

### 2. Check System Health
```bash
./target/release/axel check
```

### 3. Launch Web Server or Native Desktop App

Start the secure local Web UI & REST API:
```bash
./target/release/axel serve
# Or specify a custom port, host binding, or token:
./target/release/axel serve --port 8080 --host 127.0.0.1
```
Open [http://localhost:8765](http://localhost:8765) in your browser to inspect document stats, search indexed files, and trigger folder scans.

To launch the native GPU-accelerated desktop workstation:
```bash
./target/release/axel ui
```

### 4. CLI Commands

#### Scan and Index a Folder:
```bash
./target/release/axel scan ./my_documents
```

#### Process a Single Document:
```bash
./target/release/axel process invoice.pdf
```

#### Search Documents:
```bash
./target/release/axel search "consulting invoice"
```

#### Real-Time Folder Watcher:
```bash
./target/release/axel watch ./incoming_files
```

---

## ⚙️ Configuration (.env)

AXEL looks for an optional `.env` file in the current directory. Copy `.env.example` to customize settings:

```env
GROQ_API_KEY=
AXEL_MODEL_NAME=qwen/qwen3.8-27b
AXEL_GROQ_FALLBACK_MODELS=openai/gpt-oss-120b,openai/gpt-oss-20b,llama-3.3-70b-versatile
AXEL_INPUT_FOLDER=./documents
AXEL_INDEX_FILE=axel_index.json
AXEL_MAX_KEYWORDS=10
AXEL_TOP_K_RESULTS=5
AXEL_HOST=127.0.0.1
AXEL_PORT=8765
```

*Note: If `GROQ_API_KEY` is omitted, AXEL runs 100% offline using its built-in rule classifier and keyword scoring engine.*

---

## 🌐 REST API Endpoints

When running `axel ui` or `axel serve`:

| Endpoint | Method | Description |
|---|---|---|
| `/` | `GET` | Interactive Dark-Mode Dashboard |
| `/api/documents` | `GET` | List all indexed documents |
| `/api/search?q={query}` | `GET` | Hybrid search indexed documents |
| `/api/stats` | `GET` | Total documents and category breakdown |
| `/api/scan` | `POST` | Trigger background indexing on a directory |

---

## 🏗️ Architecture & Project Structure

```
Axel/
├── Cargo.toml            # Rust package specification & feature flags
├── src/
│   ├── main.rs           # CLI entry point (clap CLI parser)
│   ├── lib.rs            # Core Axel library crate
│   ├── config.rs         # Resilient environment & .env loader
│   ├── ai/
│   │   └── mod.rs        # Async Groq OpenAI-compatible LLM client & failover
│   ├── classifier/
│   │   ├── mod.rs        # Classification engine (Rule + LLM fallback)
│   │   ├── rules.rs      # 10-category deterministic rule engine
│   │   └── keywords.rs   # Domain-boosted keyword extractor
│   ├── extractor/
│   │   ├── mod.rs        # Format detection & extractor router
│   │   ├── pdf.rs        # Pure-Rust PDF extractor
│   │   ├── docx.rs       # Pure-Rust DOCX XML parser
│   │   └── ocr.rs        # Cross-platform Tesseract OCR & fallback
│   ├── index/
│   │   └── mod.rs        # SHA-256 deduplication, atomic JSON, & hybrid search
│   ├── watcher/
│   │   └── mod.rs        # Native cross-platform directory watcher (notify)
│   └── server/
│       └── mod.rs        # Embedded Axum web server & REST API
├── tests/
│   └── engine_tests.rs   # Integration and unit tests
└── legacy/ (Python)      # Original PyQt5 and PyTorch reference code
```

---

## 📜 License
MIT License
