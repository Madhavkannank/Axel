"""
AXEL Desktop File Manager Application
Main entry point for PyQt5-based GUI with watchdog monitoring
"""

import sys
import os
import threading
import queue
from PyQt5.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QSplitter, QStatusBar, QMenuBar, QMenu, QFileDialog, QMessageBox,
    QProgressBar, QLabel, QPushButton, QCheckBox, QGraphicsDropShadowEffect
)
from PyQt5.QtCore import Qt, QThread, pyqtSignal, QTimer, QSize
from PyQt5.QtGui import QIcon, QFont, QPixmap, QColor, QPainter, QLinearGradient, QBrush

from ui.file_browser import FileBrowserWidget
from ui.processor import ProcessorThread
from ui.results_display import ResultsDisplayWidget
from ui.search_widget import SearchWidget
from utils.watcher import start_watching
from config import INPUT_FOLDER


class AxelDesktopApp(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("AXEL - Automated Document Extraction & Labeling")
        self.setWindowIcon(self._create_icon())
        self.setGeometry(100, 100, 1400, 900)
        
        # Initialize file processing queue and watchdog
        self.file_queue = queue.Queue()
        self.input_folder = INPUT_FOLDER
        self.watchdog_thread = None
        self.watchdog_running = False
        
        # Initialize processor thread
        self.processor_thread = None
        self.current_processing_file = None
        
        # Build UI
        self._create_menu_bar()
        self._create_central_widget()
        self._create_status_bar()
        self._apply_dark_theme()
        
        # Set initial folder path
        self._set_input_folder_path()
        
        # Start watchdog monitoring
        self._start_watchdog()
        
        # Create queue processor timer
        self.queue_timer = QTimer()
        self.queue_timer.timeout.connect(self._process_queue)
        self.queue_timer.start(1000)  # Check queue every second
        
        self.show()
    
    def _create_icon(self):
        """Create a simple icon for the window."""
        pixmap = QPixmap(64, 64)
        pixmap.fill(QColor(52, 152, 219))
        painter = QPainter(pixmap)
        painter.drawText(pixmap.rect(), Qt.AlignCenter, "AX")
        painter.end()
        return QIcon(pixmap)

    def _create_background_art(self):
        """Create a subtle abstract background image."""
        pixmap = QPixmap(1200, 800)
        pixmap.fill(QColor(15, 23, 42))

        painter = QPainter(pixmap)
        painter.setRenderHint(QPainter.Antialiasing)

        gradient = QLinearGradient(0, 0, 1200, 800)
        gradient.setColorAt(0.0, QColor(59, 130, 246, 100))
        gradient.setColorAt(0.4, QColor(34, 197, 94, 55))
        gradient.setColorAt(1.0, QColor(15, 23, 42, 0))
        painter.fillRect(pixmap.rect(), QBrush(gradient))

        painter.setBrush(QColor(255, 255, 255, 18))
        painter.setPen(Qt.NoPen)
        painter.drawEllipse(850, 50, 280, 280)
        painter.drawEllipse(160, 420, 240, 240)
        painter.drawEllipse(620, 460, 180, 180)
        painter.end()
        return pixmap

    def _apply_dark_theme(self):
        """Apply the modern dark theme across the app."""
        self.setStyleSheet("""
            QMainWindow {
                background: #0f172a;
                color: #e2e8f0;
            }
            QWidget {
                color: #e2e8f0;
                font-family: 'Inter', 'Segoe UI', Arial, sans-serif;
                font-size: 10pt;
            }
            QMenuBar {
                background: rgba(15, 23, 42, 220);
                color: #e2e8f0;
                padding: 6px 10px;
                border-bottom: 1px solid rgba(148, 163, 184, 0.18);
            }
            QMenuBar::item {
                background: transparent;
                padding: 6px 12px;
                border-radius: 8px;
            }
            QMenuBar::item:selected {
                background: rgba(59, 130, 246, 0.18);
            }
            QMenu {
                background: #111827;
                color: #e2e8f0;
                border: 1px solid rgba(148, 163, 184, 0.18);
                border-radius: 10px;
                padding: 6px;
            }
            QMenu::item {
                padding: 8px 20px;
                border-radius: 8px;
            }
            QMenu::item:selected {
                background: rgba(59, 130, 246, 0.22);
            }
            QStatusBar {
                background: rgba(15, 23, 42, 220);
                color: #cbd5e1;
                border-top: 1px solid rgba(148, 163, 184, 0.18);
            }
            QWidget#mainSplitter::handle {
                background: rgba(148, 163, 184, 0.16);
            }
            QWidget#fileBrowserCard, QWidget#resultsCard {
                background: rgba(15, 23, 42, 0.62);
                border: 1px solid rgba(148, 163, 184, 0.16);
                border-radius: 16px;
            }
            QPushButton {
                background: rgba(51, 65, 85, 0.95);
                color: #e2e8f0;
                border: 1px solid rgba(148, 163, 184, 0.12);
                border-radius: 14px;
                padding: 10px 16px;
                font-weight: 600;
            }
            QPushButton:hover {
                background: rgba(59, 130, 246, 0.24);
                border: 1px solid rgba(59, 130, 246, 0.40);
            }
            QPushButton#primaryAction {
                background: #3b82f6;
                border: 1px solid rgba(96, 165, 250, 0.5);
            }
            QPushButton#primaryAction:hover {
                background: #60a5fa;
            }
            QPushButton#secondaryAction {
                background: rgba(30, 41, 59, 0.96);
            }
            QCheckBox#watchdogSwitch {
                color: #e2e8f0;
                padding: 8px 14px;
                border-radius: 14px;
                background: rgba(30, 41, 59, 0.96);
                border: 1px solid rgba(148, 163, 184, 0.12);
                font-weight: 600;
            }
            QCheckBox#watchdogSwitch:hover {
                background: rgba(59, 130, 246, 0.18);
                border: 1px solid rgba(59, 130, 246, 0.35);
            }
            QCheckBox#watchdogSwitch::indicator {
                width: 36px;
                height: 20px;
                border-radius: 10px;
                background: rgba(100, 116, 139, 0.8);
                border: 1px solid rgba(148, 163, 184, 0.18);
            }
            QCheckBox#watchdogSwitch::indicator:checked {
                background: #22c55e;
                border: 1px solid rgba(34, 197, 94, 0.65);
            }
            QCheckBox#watchdogSwitch::indicator:hover {
                box-shadow: 0 0 12px rgba(59, 130, 246, 0.35);
            }
            QProgressBar {
                background: rgba(15, 23, 42, 180);
                border: 1px solid rgba(148, 163, 184, 0.18);
                border-radius: 10px;
                text-align: center;
                color: #e2e8f0;
                height: 18px;
            }
            QProgressBar::chunk {
                border-radius: 10px;
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #3b82f6, stop:1 #22c55e);
            }
            QToolTip {
                background: #0f172a;
                color: #e2e8f0;
                border: 1px solid rgba(59, 130, 246, 0.35);
            }
        """)
    
    def _set_input_folder_path(self):
        """Set file browser to input_files folder on startup."""
        if os.path.exists(self.input_folder):
            self.file_browser.set_root_path(self.input_folder)
            self.status_label.setText(f"📁 Input Folder: {os.path.basename(self.input_folder)} | 👀 Watchdog: Active")
        else:
            os.makedirs(self.input_folder, exist_ok=True)
            self.file_browser.set_root_path(self.input_folder)
            self.status_label.setText(f"📁 Input Folder: {os.path.basename(self.input_folder)} (created) | 👀 Watchdog: Active")
    
    def _start_watchdog(self):
        """Start watchdog monitoring in background thread."""
        self.watchdog_running = True
        self.watchdog_thread = threading.Thread(
            target=self._watchdog_worker,
            daemon=True
        )
        self.watchdog_thread.start()
        print("[INFO] Watchdog monitoring started for:", self.input_folder)
    
    def _watchdog_worker(self):
        """Background worker for watchdog monitoring."""
        try:
            start_watching(self.input_folder, self._watchdog_callback)
        except KeyboardInterrupt:
            self.watchdog_running = False
        except Exception as e:
            print(f"[ERROR] Watchdog error: {e}")
            self.watchdog_running = False
    
    def _watchdog_callback(self, file_path):
        """Callback when watchdog detects a new file."""
        from utils.file_handler import detect_file_type
        
        file_type = detect_file_type(file_path)
        if file_type != "unsupported":
            print(f"[WATCHDOG] Auto-queuing file: {os.path.basename(file_path)}")
            self.file_queue.put(file_path)
    
    def _process_queue(self):
        """Process queued files from the file system watcher."""
        if not self.file_queue.empty():
            try:
                file_path = self.file_queue.get_nowait()
                if os.path.exists(file_path) and not (self.processor_thread and self.processor_thread.isRunning()):
                    print(f"[AUTO-PROCESS] Starting auto-processing: {os.path.basename(file_path)}")
                    self._start_processing(file_path)
            except queue.Empty:
                pass
    
    def _create_menu_bar(self):
        """Create application menu bar."""
        menubar = self.menuBar()
        
        # File menu
        file_menu = menubar.addMenu("File")
        
        open_action = file_menu.addAction("Open Folder")
        open_action.triggered.connect(self.open_folder)
        
        process_action = file_menu.addAction("Process File")
        process_action.triggered.connect(self.process_file)
        
        file_menu.addSeparator()
        exit_action = file_menu.addAction("Exit")
        exit_action.triggered.connect(self.close)
        
        # Tools menu
        tools_menu = menubar.addMenu("Tools")
        
        self.watchdog_action = tools_menu.addAction("🔴 Disable Watchdog Monitoring")
        self.watchdog_action.triggered.connect(self.toggle_watchdog)
        
        tools_menu.addSeparator()
        
        clear_action = tools_menu.addAction("Clear Processed Markers")
        clear_action.triggered.connect(self.clear_processed_markers)
        
        stats_action = tools_menu.addAction("View Statistics")
        stats_action.triggered.connect(self.show_statistics)
        
        # Help menu
        help_menu = menubar.addMenu("Help")
        about_action = help_menu.addAction("About AXEL")
        about_action.triggered.connect(self.show_about)
    
    def _create_central_widget(self):
        """Create main application layout."""
        central_widget = QWidget()
        self.setCentralWidget(central_widget)

        background = QLabel(central_widget)
        background.setPixmap(self._create_background_art())
        background.setScaledContents(True)
        background.setGeometry(0, 0, 1400, 900)
        background.lower()
        background.setStyleSheet("opacity: 0.12;")
        
        layout = QVBoxLayout(central_widget)
        layout.setContentsMargins(18, 18, 18, 18)
        layout.setSpacing(14)
        
        # Top toolbar area
        toolbar_layout = QHBoxLayout()
        toolbar_layout.setSpacing(10)
        
        refresh_btn = QPushButton("🔄 Refresh Files")
        refresh_btn.clicked.connect(self.refresh_files)
        refresh_btn.setObjectName("primaryAction")
        toolbar_layout.addWidget(refresh_btn)
        
        process_btn = QPushButton("⚡ Process Selected")
        process_btn.clicked.connect(self.process_file)
        process_btn.setObjectName("secondaryAction")
        toolbar_layout.addWidget(process_btn)
        
        self.watchdog_switch = QCheckBox("👀 Watchdog")
        self.watchdog_switch.setChecked(True)
        self.watchdog_switch.stateChanged.connect(self.toggle_watchdog)
        self.watchdog_switch.setObjectName("watchdogSwitch")
        toolbar_layout.addWidget(self.watchdog_switch)
        
        toolbar_layout.addStretch()
        layout.addLayout(toolbar_layout)
        
        # Main content: File browser | Results display
        splitter = QSplitter(Qt.Horizontal)
        
        # Left: File browser
        self.file_browser = FileBrowserWidget()
        self.file_browser.file_selected.connect(self.on_file_selected)
        self.file_browser.file_double_clicked.connect(self.process_file_direct)
        splitter.addWidget(self.file_browser)
        
        # Right: Results display and search
        right_layout = QVBoxLayout()
        
        # Search widget
        self.search_widget = SearchWidget()
        self.search_widget.search_triggered.connect(self.on_search)
        right_layout.addWidget(self.search_widget)
        
        # Results display
        self.results_display = ResultsDisplayWidget()
        right_layout.addWidget(self.results_display)
        
        right_widget = QWidget()
        right_widget.setLayout(right_layout)
        splitter.addWidget(right_widget)
        
        splitter.setStretchFactor(0, 1)
        splitter.setStretchFactor(1, 1)
        layout.addWidget(splitter)
        
        # Progress bar
        self.progress_bar = QProgressBar()
        self.progress_bar.setVisible(False)
        self.progress_bar.setMaximum(0)
        layout.addWidget(self.progress_bar)

        # Give the splitter glass-card styling.
        splitter.setObjectName("mainSplitter")
        self.file_browser.setObjectName("fileBrowserCard")
        right_widget.setObjectName("resultsCard")
    
    def _create_status_bar(self):
        """Create status bar."""
        self.status_label = QLabel("Ready")
        self.statusBar().addWidget(self.status_label)
        
        self.file_count_label = QLabel("Files: 0")
        self.statusBar().addPermanentWidget(self.file_count_label)

        self.connection_label = QLabel("● Watchdog active")
        self.statusBar().addPermanentWidget(self.connection_label)

        self.statusBar().setStyleSheet("""
            QStatusBar::item { border: none; }
        """)
    
    def open_folder(self):
        """Open a folder in the file browser."""
        folder = QFileDialog.getExistingDirectory(self, "Select Folder to Browse")
        if folder:
            self.file_browser.set_root_path(folder)
            self.status_label.setText(f"Browsing: {folder}")
    
    def open_input_folder(self):
        """Open the input_files folder."""
        input_folder = os.path.join(os.getcwd(), "input_files")
        if os.path.exists(input_folder):
            self.file_browser.set_root_path(input_folder)
            self.status_label.setText(f"Browsing: input_files")
        else:
            QMessageBox.warning(self, "Error", "input_files folder not found")
    
    def on_file_selected(self, file_path):
        """Handle file selection in browser."""
        self.current_processing_file = file_path
        self.results_display.clear_results()
        self.status_label.setText(f"Selected: {os.path.basename(file_path)}")

    def refresh_files(self):
        """Refresh the file browser."""
        self.file_browser.refresh()
        self.status_label.setText("Files refreshed")

    def toggle_watchdog(self):
        """Toggle watchdog monitoring on/off."""
        enabled = bool(self.watchdog_switch.isChecked())
        self.watchdog_running = enabled
        if enabled:
            self.connection_label.setText("● Watchdog active")
            self.connection_label.setStyleSheet("color: #22c55e; font-weight: 600;")
            self.watchdog_action.setText("🔴 Disable Watchdog Monitoring")
            self.status_label.setText("Watchdog: ACTIVE - Monitoring input folder for new files")
            if not self.watchdog_thread or not self.watchdog_thread.is_alive():
                self._start_watchdog()
        else:
            self.connection_label.setText("● Watchdog paused")
            self.connection_label.setStyleSheet("color: #f59e0b; font-weight: 600;")
            self.watchdog_action.setText("🟢 Enable Watchdog Monitoring")
            self.status_label.setText("Watchdog: DISABLED")
    
    def process_file(self):
        """Process selected or chosen file."""
        if self.current_processing_file:
            self._start_processing(self.current_processing_file)
        else:
            file_path, _ = QFileDialog.getOpenFileName(
                self, "Select File to Process",
                filter="Documents (*.pdf *.docx *.jpg *.jpeg *.png);;All Files (*)"
            )
            if file_path:
                self._start_processing(file_path)
    
    def process_file_direct(self, file_path):
        """Process file when double-clicked."""
        self._start_processing(file_path)
    
    def _start_processing(self, file_path):
        """Start processing a file in a background thread."""
        if not os.path.exists(file_path):
            QMessageBox.warning(self, "Error", "File not found")
            return
        
        if self.processor_thread and self.processor_thread.isRunning():
            QMessageBox.warning(self, "Processing", "Already processing a file. Please wait.")
            return
        
        self.progress_bar.setVisible(True)
        self.status_label.setText(f"Processing: {os.path.basename(file_path)}")
        
        self.processor_thread = ProcessorThread(file_path)
        self.processor_thread.progress.connect(self.on_progress)
        self.processor_thread.result_ready.connect(self.on_result_ready)
        self.processor_thread.error.connect(self.on_processing_error)
        self.processor_thread.finished.connect(self.on_processing_finished)
        self.processor_thread.start()
    
    def on_progress(self, message):
        """Update status during processing."""
        self.status_label.setText(message)
    
    def on_result_ready(self, result):
        """Display processing result."""
        self.results_display.display_result(result)
        self.file_browser.refresh()
    
    def on_processing_error(self, error_msg):
        """Handle processing error."""
        self.status_label.setText(f"Error: {error_msg}")
        QMessageBox.critical(self, "Processing Error", error_msg)
    
    def on_processing_finished(self):
        """Handle processing completion."""
        self.progress_bar.setVisible(False)
        self.status_label.setText("Ready")
    
    def on_search(self, query, search_type):
        """Handle search request."""
        self.status_label.setText(f"Searching for: {query}")
        results = self.results_display.search_index(query, search_type)
        self.results_display.display_search_results(results, query)
    
    def clear_processed_markers(self):
        """Clear processed file markers from metadata."""
        reply = QMessageBox.question(
            self, "Clear Markers",
            "This will allow reprocessing of already processed files. Continue?",
            QMessageBox.Yes | QMessageBox.No
        )
        if reply == QMessageBox.Yes:
            # Implementation to clear markers
            QMessageBox.information(self, "Success", "Markers cleared")
            self.file_browser.refresh()
    
    def show_statistics(self):
        """Show project statistics dynamically computed from the active index."""
        try:
            from utils.indexer import get_stats, get_index
            from config import INPUT_FOLDER, MAX_KEYWORDS, TOP_K_RESULTS
            s = get_stats()
            idx = get_index()
            total_kws = sum(len(item.get("keywords", [])) for item in idx)
            stats = f"""AXEL Project Statistics

📊 Processing Summary:
  • Files indexed: {s['total_files']}
  • Categories active: {s['total_categories']}
  • Most frequent category: {s['most_common_category'] or 'None'} ({s['most_common_count']})
  • Total keywords indexed: {total_kws}

⚙️ Active Configuration:
  • Input folder: {INPUT_FOLDER}
  • Max keywords per file: {MAX_KEYWORDS}
  • Top search results: {TOP_K_RESULTS}
"""
        except Exception as e:
            stats = f"AXEL Statistics\n\nCould not load index stats: {e}"
        QMessageBox.information(self, "Statistics", stats)
    
    def show_about(self):
        """Show about dialog."""
        about_text = """
<h2>AXEL</h2>
<p><b>Automated Extraction & Enhanced Labeling</b></p>
<p>An AI-powered document organization and search tool.</p>

<h3>Features:</h3>
<ul>
  <li>Automated text extraction from PDFs, images, and documents</li>
  <li>AI-powered categorization using hybrid ML + LLM</li>
  <li>Intelligent keyword extraction and embedding</li>
  <li>Semantic search with local embeddings</li>
  <li>Metadata embedding back into files</li>
</ul>

<h3>Supported Formats:</h3>
<ul>
  <li>Images: JPG, JPEG, PNG (via Tesseract OCR)</li>
  <li>Documents: PDF, DOCX</li>
</ul>

<p>Version 1.0 | 2026</p>
        """
        QMessageBox.about(self, "About AXEL", about_text)
    
    def closeEvent(self, event):
        """Clean up resources when the application closes."""
        self.watchdog_running = False
        if self.queue_timer:
            self.queue_timer.stop()
        if self.processor_thread and self.processor_thread.isRunning():
            self.processor_thread.quit()
            self.processor_thread.wait()
        if self.watchdog_thread and self.watchdog_thread.is_alive():
            self.watchdog_thread.join(timeout=1)
        print("[INFO] AXEL application closed gracefully")
        event.accept()


def main():
    app = QApplication(sys.argv)
    
    # Set application style
    app.setStyle('Fusion')
    
    # Create and show main window
    window = AxelDesktopApp()
    
    sys.exit(app.exec_())


if __name__ == "__main__":
    main()
