import os
import sys
import io
sys.stdout.reconfigure(encoding='utf-8')
import io
from contextlib import redirect_stdout
from main import process_single_file
from ml.classifier import train_model

def run_batch(directory="input_files"):
    print(f"==================================================")
    print(f"🚀 AXEL BATCH PROCESSING: {directory}")
    print(f"==================================================")
    
    if not os.path.exists(directory):
        print(f"[ERROR] Directory '{directory}' does not exist.")
        sys.exit(1)
        
    print("[INFO] Starting ML Training Initialization...")
    with redirect_stdout(io.StringIO()):
        train_model()
    
    files = [f for f in os.listdir(directory) if os.path.isfile(os.path.join(directory, f))]
    target_files = [f for f in files if not f.startswith("updated_")]
    
    if not target_files:
        print(f"[INFO] No fresh files found in {directory} to process.")
        sys.exit(0)
        
    print(f"\n[INFO] Found {len(target_files)} file(s) to process.\n")
    print(f"{'FILE NAME':<35} | {'CATEGORY':<15} | {'KEYWORDS'}")
    print("-" * 100)
    
    for file in target_files:
        file_path = os.path.join(directory, file)
        
        f = io.StringIO()
        try:
            with redirect_stdout(f):
                success = process_single_file(file_path)
            
            output = f.getvalue()
            
            category = "Unknown"
            keywords = "None"
            
            if "Category: " in output:
                category = output.split("Category: ")[1].split("\n")[0].strip()
            if "Keywords: " in output:
                keywords = output.split("Keywords: ")[1].split("\n")[0].strip()
            
            if success or category != "Unknown":
                print(f"{file:<35} | {category:<15} | {keywords}")
            else:
                print(f"{file:<35} | {'FAILED':<15} | Check file format or OCR dependencies")
                
        except Exception as e:
            print(f"{file:<35} | {'ERROR':<15} | Extraction crashed")
            
    print("-" * 100)
    print(f"✅ BATCH COMPLETED")
    print(f"==================================================\n")

if __name__ == "__main__":
    target_dir = sys.argv[1] if len(sys.argv) > 1 else "input_files"
    run_batch(target_dir)
