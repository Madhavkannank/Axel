import os
import shutil
import pytesseract
from PIL import Image

# Cross-platform Tesseract executable discovery
tesseract_env = os.environ.get("TESSERACT_CMD") or os.environ.get("AXEL_TESSERACT_PATH")
if tesseract_env and os.path.exists(tesseract_env):
    pytesseract.pytesseract.tesseract_cmd = tesseract_env
elif shutil.which("tesseract"):
    pytesseract.pytesseract.tesseract_cmd = "tesseract"
elif os.name == "nt":
    win_path = r"C:\Program Files\Tesseract-OCR\tesseract.exe"
    if os.path.exists(win_path):
        pytesseract.pytesseract.tesseract_cmd = win_path

def extract_text_from_image(path):
    """Extracts text from an image using pytesseract."""
    try:
        img = Image.open(path)
        # Convert image to grayscale before OCR
        img = img.convert('L')
        # Apply basic binarization/thresholding for OCR improvement
        img = img.point(lambda p: 255 if p > 128 else 0)
        
        text = pytesseract.image_to_string(img)
        return text.strip()
    except Exception as e:
        print(f"[ERROR] OCR failure: {e}")
        return ""
