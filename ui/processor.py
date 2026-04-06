"""Background processor thread for file processing."""

import os
import sys
from PyQt5.QtCore import QThread, pyqtSignal
from PyQt5.QtWidgets import QMessageBox


class ProcessorThread(QThread):
    """Background thread for processing files with AXEL pipeline."""
    
    progress = pyqtSignal(str)  # Status message
    result_ready = pyqtSignal(dict)  # Processing result
    error = pyqtSignal(str)  # Error message
    
    def __init__(self, file_path):
        super().__init__()
        self.file_path = file_path
    
    def run(self):
        """Run file processing in background."""
        try:
            self.progress.emit(f"Loading pipeline...")
            
            # Import AXEL modules
            from main import process_single_file
            from utils.file_handler import detect_file_type
            
            self.progress.emit(f"Detecting file type...")
            file_type = detect_file_type(self.file_path)
            
            if file_type == "unsupported":
                self.error.emit("Unsupported file format")
                return
            
            self.progress.emit(f"Processing {file_type.upper()} file...")
            
            # Process the file
            success = process_single_file(self.file_path)
            
            if success:
                # Read the result from the index
                result = {
                    'file': os.path.basename(self.file_path),
                    'path': self.file_path,
                    'type': file_type,
                    'status': 'success',
                    'message': 'File processed successfully'
                }
                self.result_ready.emit(result)
            else:
                self.error.emit(f"Failed to process file: {os.path.basename(self.file_path)}")
        
        except Exception as e:
            self.error.emit(f"Processing error: {str(e)}")
