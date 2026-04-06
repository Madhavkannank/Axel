"""Search widget for keyword and semantic search."""

from PyQt5.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLineEdit, QPushButton,
    QComboBox, QLabel, QFrame
)
from PyQt5.QtCore import pyqtSignal


class SearchWidget(QWidget):
    """Widget for searching the document index."""
    
    search_triggered = pyqtSignal(str, str)  # query, search_type
    
    def __init__(self):
        super().__init__()
        self._create_ui()
    
    def _create_ui(self):
        """Create search UI."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(10)

        self.setStyleSheet("""
            QWidget#searchCard, SearchWidget {
                background: rgba(15, 23, 42, 0.78);
                border: 1px solid rgba(148, 163, 184, 0.16);
                border-radius: 14px;
            }
            QLabel {
                color: #cbd5e1;
            }
            QLineEdit {
                background: rgba(15, 23, 42, 0.92);
                color: #e2e8f0;
                border: 1px solid rgba(148, 163, 184, 0.18);
                border-radius: 14px;
                padding: 11px 14px;
            }
            QLineEdit:focus {
                border: 1px solid #3b82f6;
            }
            QComboBox {
                background: rgba(30, 41, 59, 0.95);
                color: #e2e8f0;
                border: 1px solid rgba(148, 163, 184, 0.18);
                border-radius: 12px;
                padding: 9px 12px;
            }
            QPushButton {
                background: #3b82f6;
                color: white;
                border: none;
                border-radius: 12px;
                padding: 10px 14px;
                font-weight: 600;
            }
            QPushButton:hover {
                background: #60a5fa;
            }
        """)

        card = QFrame()
        card.setObjectName("searchCard")
        card_layout = QVBoxLayout(card)
        card_layout.setContentsMargins(16, 16, 16, 16)
        card_layout.setSpacing(10)
        
        # Search bar
        search_layout = QHBoxLayout()
        search_icon = QLabel("🔎")
        search_icon.setStyleSheet("font-size: 14px; color: #93c5fd; font-weight: 700;")
        search_layout.addWidget(search_icon)
        
        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("Enter keywords or phrase...")
        self.search_input.returnPressed.connect(self._perform_search)
        search_layout.addWidget(self.search_input)
        
        # Search type selector
        self.search_type = QComboBox()
        self.search_type.addItems(["Keyword", "Semantic"])
        search_layout.addWidget(self.search_type)
        
        search_btn = QPushButton("🔍 Search")
        search_btn.clicked.connect(self._perform_search)
        search_layout.addWidget(search_btn)
        
        card_layout.addLayout(search_layout)
        card_layout.addStretch()
        layout.addWidget(card)
    
    def _perform_search(self):
        """Trigger search."""
        query = self.search_input.text().strip()
        if query:
            search_type = self.search_type.currentText()
            self.search_triggered.emit(query, search_type)
            self.search_input.clear()
