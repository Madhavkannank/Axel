import os
import re
from collections import Counter
import piexif
import pikepdf
import docx

def validate_file(file_path):
    """Checks if the file exists and is accessible."""
    if not os.path.exists(file_path):
        return False
    if not os.path.isfile(file_path):
        return False
    return True

def detect_file_type(file_path):
    """Detects the file type based on extension. returns: 'image', 'pdf', 'docx', 'unsupported'"""
    ext = os.path.splitext(file_path)[1].lower()
    if ext in ['.jpg', '.jpeg', '.png']:
        return "image"
    elif ext == '.pdf':
        return "pdf"
    elif ext == '.docx':
        return "docx"
    else:
        return "unsupported"

def has_existing_metadata(file_path, file_type):
    """Checks if the file already contains metadata (Tags/Keywords)."""
    try:
        if file_type == "image":
            ext = os.path.splitext(file_path)[1].lower()
            if ext in ['.jpg', '.jpeg']:
                exif_dict = piexif.load(file_path)
                if piexif.ImageIFD.XPKeywords in exif_dict.get('0th', {}):
                    return True
        elif file_type == "pdf":
            with pikepdf.open(file_path) as pdf:
                with pdf.open_metadata() as meta:
                    if meta.get('pdf:Keywords'):
                        return True
        elif file_type == "docx":
            doc = docx.Document(file_path)
            if doc.core_properties.keywords:
                return True
    except Exception:
        pass
    return False

def preprocess_text(text):
    """Removes excessive whitespace and limits length to ~3000 chars."""
    text = re.sub(r'\s+', ' ', text).strip()
    if len(text) > 3000:
        text = text[:1500] + " ... " + text[-1500:]
    return text

def clean_keywords(keywords_list, text=""):
    """Cleans, formats, and enforces keyword counts (min 3, max 10)."""
    cleaned = []
    for k in keywords_list:
        k = k.strip().lower()
        if k and k not in cleaned:
            cleaned.append(k)
    
    # Enforce maximum 10 keywords
    if len(cleaned) > 10:
        cleaned = cleaned[:10]
        
    # Enforce minimum 3 keywords by padding with relevant text words
    if len(cleaned) < 3 and text:
        words = re.findall(r'\b[a-zA-Z]{4,}\b', text.lower())
        freq = Counter(words)
        for word, _ in freq.most_common():
            if word not in cleaned:
                cleaned.append(word)
            if len(cleaned) >= 3:
                break
                
    return cleaned
