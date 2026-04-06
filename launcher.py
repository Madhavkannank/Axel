#!/usr/bin/env python3
"""
AXEL Desktop Application Launcher
Starts the PyQt5-based file manager and document processing UI
"""

import sys
import os
from PyQt5.QtWidgets import QApplication
from axel_desktop_app import AxelDesktopApp

def main():
    """Launch the AXEL desktop application."""
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    app = QApplication(sys.argv)
    app.setApplicationName("AXEL Document Processor")
    app.setApplicationVersion("1.0.0")
    
    window = AxelDesktopApp()
    window.setWindowTitle("AXEL - Document Classification & Indexing System")
    window.show()
    
    sys.exit(app.exec_())

if __name__ == "__main__":
    main()
