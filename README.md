# AXEL: Automated Extraction & Enhanced Labeling

AXEL is an AI-powered document organization and search tool. It automatically extracts text from various file formats (Images, PDFs, Word Documents), generates intelligent categories and keywords using Large Language Models (LLMs), and embeds these tags directly into the file's metadata. Furthermore, AXEL builds a local vector index, enabling Lightning-fast keyword searches and powerful AI-driven semantic searches.

## 🚀 Key Features

- **Automated Text Extraction**: Supports Images (via Tesseract OCR), PDFs (`pdfplumber`), and DOCX files.
- **AI-Powered Tagging**: Utilizes the Groq API (LLaMA3-70b) to intelligently categorize and extract keywords from your documents.
- **Embedded Metadata**: Injects generated keywords directly into file attributes (EXIF for images, core properties for DOCX, metadata for PDFs) creating an `updated_` version of the file.
- **Semantic Search**: Uses local `sentence-transformers` embeddings to allow natural language querying against your document index.
- **Folder Watcher**: Run AXEL in watch mode to continuously monitor a dedicated `input_files` directory and tag incoming documents in real-time.

## 📋 Prerequisites

Before running AXEL, ensure you have the following installed:
1. **Python 3.8+**
2. **Tesseract OCR**: Required for image text extraction. You must have Tesseract installed on your system.
   - *Windows*: Download from [UB-Mannheim](https://github.com/UB-Mannheim/tesseract/wiki).
   - *Linux*: `sudo apt install tesseract-ocr`
   - *Mac*: `brew install tesseract`
3. **Groq API Key**: Needed for LLM keyword generation.

## 🛠️ Installation & Setup

1. **Clone or Navigate to the Project Directory**
   ```bash
   cd axel
   ```

2. **Install Python Dependencies**
   Install the required libraries listed in `requirements.txt`:
   ```bash
   pip install -r requirements.txt
   ```

3. **Configure Environment Variables (.env file)**
   AXEL requires a `.env` file in the project root with the following keys:
   ```
   GROQ_API_KEY=your-api-key-here
   MODEL_NAME=llama-3.3-70b-versatile
   AXEL_INPUT_FOLDER=./input_files
   AXEL_MAX_KEYWORDS=10
   AXEL_TOP_K_RESULTS=5
   AXEL_GROQ_FALLBACK_MODELS=llama-3.1-8b-instant,gemma2-9b-it
   ```
   Create a `.env` file in the project root and populate with your configuration.

## 💻 Usage

### CLI Mode
Run the main pipeline using the CLI interface provided by `main.py`.

#### 1. Process a Single File
Analyze and tag a specific file manually:
```bash
python main.py path/to/your/document.pdf
```

#### 2. Run the Folder Watcher
Automatically process any new files dropped into the `input_files` directory:
```bash
python main.py --watch
```

#### 3. Keyword Search
Search your processed documents index by a specific keyword or category:
```bash
python main.py --search "invoice"
```

#### 4. Semantic Search
Perform a natural language AI search to find documents based on the meaning of your query:
```bash
python main.py --semantic "tax documents from last year"
```

### Desktop GUI Mode
Launch the PyQt5-based desktop file manager application:
```bash
python launcher.py
```

The AXEL Desktop Application provides:
- **File Browser**: Navigate and select documents for processing
- **Single-Click Processing**: Process files with visual feedback
- **Results Display**: View category, keywords, confidence scores, and metadata
- **Keyword Search**: Search the document index by keywords
- **Semantic Search**: AI-powered natural language search across your documents
- **Progress Indicators**: Real-time processing status and file count
- **Statistics View**: Overview of processed documents and index contents

## 📂 Project Structure

- `main.py` - CLI entry point orchestrating the AXEL pipeline.
- `config.py` - Configuration loading and environment variable management.
- `launcher.py` - PyQt5 desktop application launcher.
- `axel_desktop_app.py` - Main desktop application window and event orchestration.
- `ai/` - Core AI logic:
  - `groq_client.py`: Interacts with the Groq API for keyword extraction with model fallback.
  - `embedding.py`: Manages local `sentence-transformers` for semantic vectors.
- `extractor/` - Text extraction modules:
  - `ocr.py`: Image OCR using Tesseract with LLM fallback.
  - `pdf.py`: PDF text extraction.
  - `docx_reader.py`: Word document parsing.
- `metadata/` - File writers that embed generated tags directly into file metadata:
  - `image_meta.py` (EXIF properties via `piexif`)
  - `pdf_meta.py` (PDF properties via `pikepdf`)
  - `docx_meta.py` (DOCX properties)
- `ml/` - Machine Learning classification:
  - `classifier.py`: Hybrid rule-based + ML classifier with 95.83% accuracy
  - `keywords.py`: Keyword extraction utilities
  - `data.json`: Processed document metadata and index
- `ui/` - PyQt5 desktop GUI components:
  - `file_browser.py`: File/folder navigation widget with tree view
  - `processor.py`: Background processing thread with progress signals
  - `results_display.py`: Results visualization and search results display
  - `search_widget.py`: Keyword and semantic search interface
- `utils/` - Helpers and infrastructure:
  - `indexer.py`: Manages the local JSON index (`axel_index.json`) and performs search/similarity matching.
  - `watcher.py`: File system watcher using `watchdog`.
  - `file_handler.py`: Validation, type detection, and text preprocessing.

## 📜 License
*Specify your specific license terms here.*
