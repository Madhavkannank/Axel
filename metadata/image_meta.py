import piexif
import os

def write_image_tags(path, keywords_list, out_path=None):
    """Writes keywords to image metadata (XPKeywords and XPComment)."""
    if out_path is None:
        out_path = path

    try:
        # piexif only supports JPEG and TIFF
        ext = os.path.splitext(path)[1].lower()
        if ext not in ['.jpg', '.jpeg']:
            print("[WARNING] Metadata embedding primarily supported for .jpg/.jpeg via piexif.")
            return

        exif_dict = piexif.load(path)
        
        # Convert keywords list -> string using semicolon (;)
        keywords_str = "; ".join(keywords_list)
        
        # Encode using UTF-16 LE and add null terminator: b"\x00\x00"
        xp_keywords = keywords_str.encode('utf-16le') + b'\x00\x00'
        
        exif_dict['0th'][piexif.ImageIFD.XPKeywords] = xp_keywords
        exif_dict['0th'][piexif.ImageIFD.XPComment] = xp_keywords  # Also save to XPComment
        
        exif_bytes = piexif.dump(exif_dict)
        piexif.insert(exif_bytes, path, out_path)
        
        # Verify
        verify_dict = piexif.load(out_path)
        raw_keywords = verify_dict['0th'].get(piexif.ImageIFD.XPKeywords, b'')
        if isinstance(raw_keywords, tuple):
            raw_keywords = bytes(raw_keywords)
        if isinstance(raw_keywords, (bytes, bytearray)):
            read_back = raw_keywords.decode('utf-16le', errors='ignore').rstrip('\x00')
        else:
            read_back = str(raw_keywords)
        print(f"[INFO] Metadata successfully written:\n{read_back}")
        
    except Exception as e:
        print(f"[ERROR] Failed to write image metadata: {e}")
