import pikepdf

def write_pdf_tags(path, keywords_list, out_path=None):
    """Writes keywords to PDF metadata (Keywords and Subject)."""
    if out_path is None:
        out_path = path

    try:
        keywords_str = "; ".join(keywords_list)
        with pikepdf.open(path) as pdf:
            with pdf.open_metadata() as meta:
                meta['pdf:Keywords'] = keywords_str
                meta['dc:subject'] = keywords_str  # Adding to Subject equivalent
            
            if out_path == path:
                temp_path = path + ".tmp"
                pdf.save(temp_path)
            else:
                pdf.save(out_path)
                
        if out_path == path:
            import os
            os.replace(temp_path, path)
            
        # Verify
        with pikepdf.open(out_path) as pdf_verify:
            with pdf_verify.open_metadata() as meta:
                read_back = meta.get('pdf:Keywords', '')
                print(f"[INFO] Metadata successfully written:\n{read_back}")
                
    except Exception as e:
        print(f"[ERROR] Failed to write PDF metadata: {e}")
