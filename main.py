import sys
import os
import argparse
from utils.file_handler import validate_file, detect_file_type, preprocess_text, clean_keywords, has_existing_metadata
from extractor.ocr import extract_text_from_image
from extractor.pdf import extract_text_from_pdf
from extractor.docx_reader import extract_text_from_docx
from ai.groq_client import generate_keywords, generate_keywords_from_file_context
from metadata.image_meta import write_image_tags
from metadata.pdf_meta import write_pdf_tags
from metadata.docx_meta import write_docx_tags
from utils.indexer import update_index, search_index, semantic_search_index
from utils.watcher import start_watching
from ai.embedding import generate_embedding

from ml.classifier import train_model, predict_category
from ml.keywords import extract_keywords

def convert_keywords_to_windows_format(keywords_list):
    """Converts a list of keywords to Windows tag format."""
    return "; ".join(keywords_list)

def process_single_file(file_path):
    """Core AXEL pipeline for a single file."""
    if not validate_file(file_path):
        print(f"[ERROR] File not found or not accessible - {file_path}")
        return False
        
    file_type = detect_file_type(file_path)
    if file_type == "unsupported":
        print("[ERROR] Unsupported file format.")
        return False
        
    # Check for existing metadata to avoid duplicates
    if has_existing_metadata(file_path, file_type):
        print(f"[INFO] Skipped already processed file: {file_path}")
        return False
        
    print(f"\n[INFO] Processing file: {file_path}")
    print(f"[INFO] File type detected: {file_type.upper()}")
        
    text = ""
    try:
        if file_type == "image":
            text = extract_text_from_image(file_path)
        elif file_type == "pdf":
            text = extract_text_from_pdf(file_path)
        elif file_type == "docx":
            text = extract_text_from_docx(file_path)
    except Exception as e:
        print(f"[ERROR] Extraction failed due to corrupt file or internal error: {e}")
        return False
        
    if not text.strip():
        if file_type == "image":
            print("[WARNING] OCR text empty. Using LLM fallback based on file context.")
            result = generate_keywords_from_file_context(
                file_name=os.path.basename(file_path),
                file_type=file_type,
                reason="ocr_empty"
            )
            category = result.get("category", "Unknown")
            raw_keywords = result.get("keywords", [])
            target_kws = clean_keywords(raw_keywords, text=os.path.basename(file_path))[:5]
            if not target_kws:
                target_kws = ["image", "unreadable", "review"]
            print("[INFO] Groq context fallback response parsed successfully.")
        else:
            print("[ERROR] Extracted text is empty or extraction failed.")
            return False
    else:
        full_text = text
        print(f"[INFO] Extracted text length: {len(full_text)}")

        # HYBRID PIPELINE - privacy and ML run on complete untruncated text
        ml_category, confidence = predict_category(full_text)
        ml_keywords = extract_keywords(full_text, top_n=5)

        print(f"[INFO] ML Category: {ml_category}")
        print(f"[INFO] Confidence: {confidence:.2f}")
        print(f"[INFO] Keywords (TF-IDF): {', '.join(ml_keywords)}")

        prompt_text = preprocess_text(full_text)

        print("[INFO] Routing to LLM with ML hints...")
        result = generate_keywords(prompt_text, ml_category=ml_category, ml_keywords=ml_keywords, file_name=os.path.basename(file_path))
        category = result.get("category", "Unknown")
        raw_keywords = result.get("keywords", [])
        target_kws = clean_keywords(raw_keywords, text=full_text)[:5]
        print("[INFO] Groq response parsed successfully.")

    keywords = target_kws
    
    print(f"[INFO] Target Keywords: {', '.join(keywords)}")

    # Generate Embeddings for Semantic Search
    combined_text = category + " " + " ".join(keywords)
    print(f"[INFO] Generating embeddings...")
    embedding_vector = generate_embedding(combined_text)

    dir_name = os.path.dirname(os.path.abspath(file_path))
    base_name = os.path.basename(file_path)
    updated_file_path = file_path
    
    if file_type == "image":
        write_image_tags(file_path, keywords, updated_file_path)
    elif file_type == "pdf":
        write_pdf_tags(file_path, keywords, updated_file_path)
    elif file_type == "docx":
        write_docx_tags(file_path, keywords, updated_file_path)
        
    # Store results in Index along with vector
    update_index(base_name, category, keywords, file_path, embedding=embedding_vector)
        
    print("\n------------------------------")
    print("AXEL ANALYSIS RESULT")
    print("------------------------------")
    print(f"File: {base_name}")
    print(f"Category: {category}")
    print(f"Keywords: {', '.join(keywords)}")
    print("------------------------------\n")
    print(f"Saved updated file to: {updated_file_path}")
    return True

def print_search_results(query, results):
    """Outputs search results in the requested format."""
    print("\n------------------------------")
    print("AXEL SEARCH RESULTS")
    print("------------------------------")
    print(f"Keyword: {query}\n")
    
    if results:
        print("Found:")
        for res in results:
            print(f"- {res['file']} ({res['category']})")
    else:
        print("No matching files found.")
    print("------------------------------\n")

def print_semantic_search_results(query, results):
    """Outputs semantic search results in the requested format."""
    print("\n------------------------------")
    print("AXEL SEMANTIC SEARCH")
    print("------------------------------")
    print(f"Query: {query}\n")
    
    if results:
        print("Top Matches:")
        for i, res in enumerate(results, 1):
            # score is stored during semantic_search_index sorting
            score = res.get("score", 0)
            print(f"{i}. {res['file']} (Score: {score:.2f})")
    else:
        print("No matching files found.")
    print("------------------------------\n")

def main():
    print("[INFO] Starting AXEL... initializing ML modules.")
    train_model()
    
    parser = argparse.ArgumentParser(description="AXEL: Automated Extraction & Enhanced Labeling")
    parser.add_argument("file", nargs="?", help="Path to single file for processing")
    parser.add_argument("--watch", action="store_true", help="Monitor 'input_files' folder continuously")
    parser.add_argument("--search", type=str, help="Search the AXEL index for a keyword")
    parser.add_argument("--semantic", type=str, help="Semantically search the AXEL index via AI embeddings")
    
    args = parser.parse_args()
    
    if args.search:
        results = search_index(args.search)
        print_search_results(args.search, results)
        sys.exit(0)
        
    if args.semantic:
        query_emb = generate_embedding(args.semantic)
        results = semantic_search_index(query_emb, top_k=5)
        print_semantic_search_results(args.semantic, results)
        sys.exit(0)
        
    if args.watch:
        start_watching("input_files", process_single_file)
        sys.exit(0)
        
    target_file = args.file
    if not target_file:
        target_file = input("Enter file path, --watch, --search <keyword>, or --semantic <query>: ").strip()
        
    if target_file == "--watch":
        start_watching("input_files", process_single_file)
    elif target_file.startswith("--search "):
        query = target_file.replace("--search ", "").strip('"\' ')
        results = search_index(query)
        print_search_results(query, results)
    elif target_file.startswith("--semantic "):
        query = target_file.replace("--semantic ", "").strip('"\' ')
        query_emb = generate_embedding(query)
        results = semantic_search_index(query_emb, top_k=5)
        print_semantic_search_results(query, results)
    elif target_file:
        process_single_file(target_file)
    else:
        print("[ERROR] No valid action provided.")
        sys.exit(1)

if __name__ == "__main__":
    main()
