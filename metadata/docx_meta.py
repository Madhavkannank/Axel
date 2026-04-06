import docx

def write_docx_tags(path, keywords_list, out_path=None):
    """Writes keywords to DOCX metadata."""
    if out_path is None:
        out_path = path

    try:
        doc = docx.Document(path)
        keywords_str = "; ".join(keywords_list)
        doc.core_properties.keywords = keywords_str
        doc.save(out_path)
        
        # Verify
        verify_doc = docx.Document(out_path)
        read_back = verify_doc.core_properties.keywords
        print(f"[INFO] Metadata successfully written:\n{read_back}")
        
    except Exception as e:
        print(f"[ERROR] Failed to write DOCX metadata: {e}")
