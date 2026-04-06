import pytesseract
pytesseract.pytesseract.tesseract_cmd = r'C:\Program Files\Tesseract-OCR\tesseract.exe'
from PIL import Image

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
