"""File browser widget for exploring and selecting files."""

import os
from PyQt5.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QTreeWidget, QTreeWidgetItem,
    QLabel, QLineEdit, QPushButton, QFileIconProvider, QFrame
)
from PyQt5.QtCore import pyqtSignal, Qt
from PyQt5.QtGui import QIcon


class FileBrowserWidget(QWidget):
    """Widget for browsing files and folders."""
    
    file_selected = pyqtSignal(str)  # File path selected
    file_double_clicked = pyqtSignal(str)  # File double-clicked (process)
    
    SUPPORTED_EXTENSIONS = {'.pdf', '.docx', '.jpg', '.jpeg', '.png'}
    
    def __init__(self):
        super().__init__()
        self.current_root = os.getcwd()
        self.icon_provider = QFileIconProvider()
        self._create_ui()
    
    def _create_ui(self):
        """Create file browser UI."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(12)

        self.setStyleSheet("""
            QWidget#fileBrowserCard, FileBrowserWidget {
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
                border-radius: 12px;
                padding: 10px 12px;
            }
            QLineEdit:focus {
                border: 1px solid #3b82f6;
            }
            QPushButton {
                background: rgba(51, 65, 85, 0.92);
                color: #e2e8f0;
                border: 1px solid rgba(148, 163, 184, 0.12);
                border-radius: 12px;
                padding: 9px 14px;
            }
            QPushButton:hover {
                background: rgba(59, 130, 246, 0.2);
                border: 1px solid rgba(59, 130, 246, 0.45);
            }
            QTreeWidget {
                background: rgba(15, 23, 42, 0.90);
                color: #e2e8f0;
                border: 1px solid rgba(148, 163, 184, 0.14);
                border-radius: 14px;
                alternate-background-color: rgba(30, 41, 59, 0.65);
                padding: 6px;
                outline: none;
            }
            QTreeWidget::item {
                padding: 8px 6px;
                border-bottom: 1px solid rgba(148, 163, 184, 0.08);
            }
            QTreeWidget::item:hover {
                background: rgba(59, 130, 246, 0.18);
            }
            QTreeWidget::item:selected {
                background: rgba(59, 130, 246, 0.28);
                color: #f8fafc;
            }
            QHeaderView::section {
                background: rgba(30, 41, 59, 0.95);
                color: #cbd5e1;
                border: none;
                padding: 8px;
                border-bottom: 1px solid rgba(148, 163, 184, 0.12);
            }
        """)

        card = QFrame()
        card_layout = QVBoxLayout(card)
        card_layout.setContentsMargins(16, 16, 16, 16)
        card_layout.setSpacing(12)
        card.setObjectName("fileBrowserCard")
        layout.addWidget(card)
        
        # Path bar
        path_layout = QHBoxLayout()
        path_label = QLabel("📁 Path")
        path_label.setStyleSheet("font-weight: 600; color: #94a3b8;")
        path_layout.addWidget(path_label)
        self.path_input = QLineEdit()
        self.path_input.setText(self.current_root)
        self.path_input.setPlaceholderText("Browse input files folder")
        self.path_input.returnPressed.connect(self._navigate_to_path)
        path_layout.addWidget(self.path_input)
        
        go_btn = QPushButton("➡ Go")
        go_btn.clicked.connect(self._navigate_to_path)
        path_layout.addWidget(go_btn)
        card_layout.addLayout(path_layout)
        
        # File tree
        self.tree = QTreeWidget()
        self.tree.setHeaderLabels(["Name", "Type", "Size"])
        self.tree.setColumnCount(3)
        self.tree.setAlternatingRowColors(True)
        self.tree.itemSelectionChanged.connect(self._on_selection_changed)
        self.tree.itemDoubleClicked.connect(self._on_item_double_clicked)
        card_layout.addWidget(self.tree)
        
        self.refresh()
    
    def set_root_path(self, path):
        """Set the root path for browsing."""
        if os.path.exists(path) and os.path.isdir(path):
            self.current_root = path
            self.path_input.setText(path)
            self.refresh()
    
    def _navigate_to_path(self):
        """Navigate to the path in the input field."""
        path = self.path_input.text()
        self.set_root_path(path)
    
    def refresh(self):
        """Refresh the file tree."""
        self.tree.clear()
        self._populate_tree(self.current_root, self.tree.invisibleRootItem())
        self.tree.expandAll()
    
    def _populate_tree(self, path, parent_item):
        """Recursively populate tree with files and folders."""
        try:
            items = sorted(os.listdir(path))
        except PermissionError:
            return
        
        for item_name in items:
            if item_name.startswith('.'):
                continue
            
            item_path = os.path.join(path, item_name)
            
            try:
                if os.path.isdir(item_path):
                    folder_item = QTreeWidgetItem(parent_item)
                    folder_item.setText(0, item_name)
                    folder_item.setText(1, "Folder")
                    folder_item.setData(0, Qt.UserRole, item_path)
                    folder_item.setIcon(0, self.icon_provider.icon(QFileIconProvider.Folder))
                    
                    # Recursively add subfolder contents
                    self._populate_tree(item_path, folder_item)
                
                elif os.path.isfile(item_path):
                    ext = os.path.splitext(item_name)[1].lower()
                    if ext in self.SUPPORTED_EXTENSIONS:
                        file_item = QTreeWidgetItem(parent_item)
                        file_item.setText(0, item_name)
                        file_item.setText(1, ext[1:].upper())
                        
                        size = os.path.getsize(item_path)
                        file_item.setText(2, self._format_size(size))
                        file_item.setData(0, Qt.UserRole, item_path)
                        file_item.setIcon(0, self.icon_provider.icon(QFileIconProvider.File))
            except (PermissionError, OSError):
                continue
    
    def _on_selection_changed(self):
        """Handle file selection."""
        selected_items = self.tree.selectedItems()
        if selected_items:
            item = selected_items[0]
            file_path = item.data(0, Qt.UserRole)
            if file_path and os.path.isfile(file_path):
                self.file_selected.emit(file_path)
    
    def _on_item_double_clicked(self, item, column):
        """Handle double-click to process file."""
        file_path = item.data(0, Qt.UserRole)
        if file_path and os.path.isfile(file_path):
            self.file_double_clicked.emit(file_path)
        elif file_path and os.path.isdir(file_path):
            self.set_root_path(file_path)
    
    @staticmethod
    def _format_size(size_bytes):
        """Format file size for display."""
        for unit in ['B', 'KB', 'MB', 'GB']:
            if size_bytes < 1024:
                return f"{size_bytes:.1f} {unit}"
            size_bytes /= 1024
        return f"{size_bytes:.1f} TB"
