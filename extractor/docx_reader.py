import docx

def extract_text_from_docx(path):
    """Extracts text from a docx file using python-docx."""
    try:
        doc = docx.Document(path)
        text = "\n".join([para.text for para in doc.paragraphs])
        return text.strip()
    except Exception as e:
        print(f"DOCX extraction failure: {e}")
        return ""
