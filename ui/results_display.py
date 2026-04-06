"""Results display widget showing processing results and search outcomes."""

import json
import os
from PyQt5.QtWidgets import (
    QWidget, QVBoxLayout, QScrollArea, QLabel, QFrame,
    QGridLayout, QProgressBar, QTableWidget, QTableWidgetItem
)
from PyQt5.QtCore import Qt
from PyQt5.QtGui import QFont, QColor, QDesktopServices
from PyQt5.QtCore import QUrl


class ResultsDisplayWidget(QWidget):
    """Widget for displaying file processing results and search results."""
    
    def __init__(self):
        super().__init__()
        self._last_search_results = []
        self._create_ui()
    
    def _create_ui(self):
        """Create results display UI."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(10)

        self.setStyleSheet("""
            QWidget#resultsCard, ResultsDisplayWidget {
                background: rgba(15, 23, 42, 0.78);
                border: 1px solid rgba(148, 163, 184, 0.16);
                border-radius: 14px;
            }
            QLabel {
                color: #e2e8f0;
            }
            QScrollArea {
                background: transparent;
                border: none;
            }
            QFrame[class="resultFrame"] {
                background: rgba(15, 23, 42, 0.9);
                border: 1px solid rgba(148, 163, 184, 0.16);
                border-radius: 14px;
            }
            QTableWidget {
                background: rgba(15, 23, 42, 0.9);
                color: #e2e8f0;
                border: 1px solid rgba(148, 163, 184, 0.16);
                border-radius: 14px;
                gridline-color: rgba(148, 163, 184, 0.12);
                alternate-background-color: rgba(30, 41, 59, 0.68);
            }
            QTableWidget::item {
                padding: 10px 8px;
                border-bottom: 1px solid rgba(148, 163, 184, 0.08);
            }
            QTableWidget::item:selected {
                background: rgba(59, 130, 246, 0.28);
            }
            QHeaderView::section {
                background: rgba(30, 41, 59, 0.96);
                color: #cbd5e1;
                border: none;
                padding: 10px;
                border-bottom: 1px solid rgba(148, 163, 184, 0.12);
                font-weight: 600;
            }
        """)
        
        # Scroll area for results
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        
        self.results_container = QWidget()
        self.results_layout = QVBoxLayout(self.results_container)
        self.results_layout.addStretch()
        
        scroll.setWidget(self.results_container)
        layout.addWidget(scroll)
    
    def clear_results(self):
        """Clear displayed results."""
        while self.results_layout.count() > 1:
            item = self.results_layout.takeAt(0)
            if item.widget():
                item.widget().deleteLater()
    
    def display_result(self, result):
        """Display file processing result."""
        self.clear_results()
        
        if result.get('status') == 'success':
            self._show_success_result(result)
        else:
            self._show_error_result(result)
    
    def _show_success_result(self, result):
        """Display successful processing result."""
        frame = QFrame()
        frame.setProperty("class", "resultFrame")
        frame.setStyleSheet("background: rgba(15, 23, 42, 0.9); border: 1px solid rgba(148, 163, 184, 0.16); border-radius: 14px;")
        frame_layout = QGridLayout(frame)
        frame_layout.setContentsMargins(16, 16, 16, 16)
        frame_layout.setHorizontalSpacing(14)
        frame_layout.setVerticalSpacing(10)
        
        # File info
        file_label = QLabel("📄 File Information")
        file_label.setFont(QFont("Segoe UI", 11, QFont.Bold))
        frame_layout.addWidget(file_label, 0, 0, 1, 2)
        
        frame_layout.addWidget(QLabel("File:"), 1, 0)
        frame_layout.addWidget(QLabel(result.get('file', 'Unknown')), 1, 1)
        
        frame_layout.addWidget(QLabel("Type:"), 2, 0)
        frame_layout.addWidget(QLabel(result.get('type', 'Unknown').upper()), 2, 1)
        
        frame_layout.addWidget(QLabel("Status:"), 3, 0)
        status_label = QLabel("✓ Processed Successfully")
        status_label.setStyleSheet("color: #22c55e; font-weight: 700;")
        frame_layout.addWidget(status_label, 3, 1)
        
        frame_layout.addWidget(QLabel(""), 4, 0)  # Spacer
        
        # Category and keywords (would come from actual processing)
        cat_label = QLabel("🏷️ Classification & Keywords")
        cat_label.setFont(QFont("Segoe UI", 11, QFont.Bold))
        frame_layout.addWidget(cat_label, 5, 0, 1, 2)
        
        frame_layout.addWidget(QLabel("Category:"), 6, 0)
        category_value = QLabel("Document")
        category_value.setStyleSheet("background: rgba(34, 197, 94, 0.18); color: #bbf7d0; padding: 6px 10px; border-radius: 999px; font-weight: 700;")
        frame_layout.addWidget(category_value, 6, 1)
        
        frame_layout.addWidget(QLabel("Keywords:"), 7, 0)
        keyword_value = QLabel("processing, data, ai")
        keyword_value.setStyleSheet("color: #cbd5e1;")
        frame_layout.addWidget(keyword_value, 7, 1)
        
        frame_layout.addWidget(QLabel("Confidence:"), 8, 0)
        
        conf_bar = QProgressBar()
        conf_bar.setValue(85)
        conf_bar.setFormat("85%")
        frame_layout.addWidget(conf_bar, 8, 1)
        
        frame_layout.setRowStretch(9, 1)
        
        self.results_layout.insertWidget(0, frame)
    
    def _show_error_result(self, result):
        """Display error during processing."""
        frame = QFrame()
        frame.setStyleSheet("background: rgba(127, 29, 29, 0.22); border: 1px solid rgba(248, 113, 113, 0.26); border-radius: 14px;")
        frame_layout = QVBoxLayout(frame)
        frame_layout.setContentsMargins(16, 16, 16, 16)
        
        error_label = QLabel("⚠️ Processing Error")
        error_label.setFont(QFont("Segoe UI", 11, QFont.Bold))
        error_label.setStyleSheet("color: #fca5a5;")
        frame_layout.addWidget(error_label)
        
        message = QLabel(result.get('message', 'Unknown error occurred'))
        frame_layout.addWidget(message)
        frame_layout.addStretch()
        
        self.results_layout.insertWidget(0, frame)
    
    def display_search_results(self, results, query):
        """Display search results."""
        self.clear_results()
        self._last_search_results = list(results or [])
        
        frame = QFrame()
        frame.setStyleSheet("background: rgba(15, 23, 42, 0.9); border: 1px solid rgba(148, 163, 184, 0.16); border-radius: 14px;")
        frame_layout = QVBoxLayout(frame)
        frame_layout.setContentsMargins(16, 16, 16, 16)
        frame_layout.setSpacing(10)
        
        title = QLabel(f"🔍 Search Results for: '{query}'")
        title.setFont(QFont("Segoe UI", 11, QFont.Bold))
        frame_layout.addWidget(title)
        
        if results:
            if any(result.get('match_type') == 'suggested' for result in results):
                hint = QLabel("Showing closest matches from the index")
                hint.setStyleSheet("color: #8a6d3b; font-style: italic;")
                frame_layout.addWidget(hint)

            # Results table
            table = QTableWidget()
            table.setColumnCount(5)
            table.setHorizontalHeaderLabels(["File", "Category", "Score", "Match", "Path"])
            table.setRowCount(len(results))
            table.setAlternatingRowColors(True)
            table.setShowGrid(False)
            table.cellClicked.connect(self._open_search_result)
            
            for row, result in enumerate(results):
                table.setItem(row, 0, QTableWidgetItem(result.get('file', 'Unknown')))
                table.setItem(row, 1, QTableWidgetItem(result.get('category', 'N/A')))
                
                score = QTableWidgetItem(f"{result.get('score', 0):.2f}")
                score.setTextAlignment(Qt.AlignCenter)
                table.setItem(row, 2, score)

                match_type = result.get('match_type', 'exact')
                match_label = 'Exact' if match_type == 'exact' else 'Suggested'
                match_item = QTableWidgetItem(match_label)
                if match_type == 'exact':
                    match_item.setForeground(QColor("#22c55e"))
                else:
                    match_item.setForeground(QColor("#f59e0b"))
                table.setItem(row, 3, match_item)
                
                table.setItem(row, 4, QTableWidgetItem(result.get('path', '')))
            
            table.resizeColumnsToContents()
            frame_layout.addWidget(table)
        else:
            no_results = QLabel("No results found")
            no_results.setStyleSheet("color: #94a3b8; font-style: italic;")
            frame_layout.addWidget(no_results)
        
        frame_layout.addStretch()
        self.results_layout.insertWidget(0, frame)

    def _open_search_result(self, row, column):
        """Open the clicked search result in the default file handler."""
        if row < 0 or row >= len(self._last_search_results):
            return

        result = self._last_search_results[row]
        file_path = str(result.get('path', '')).strip()
        if not file_path:
            return

        if os.path.exists(file_path):
            QDesktopServices.openUrl(QUrl.fromLocalFile(os.path.abspath(file_path)))
    
    def search_index(self, query, search_type):
        """Search the AXEL index."""
        try:
            from utils.indexer import search_index, semantic_search_index
            
            if search_type == "Keyword":
                results = search_index(query)
            else:  # Semantic
                from ai.embedding import generate_embedding
                embedding = generate_embedding(query)
                results = semantic_search_index(embedding, query_text=query, top_k=5)
            
            return results
        except Exception as e:
            print(f"Search error: {e}")
            return []
