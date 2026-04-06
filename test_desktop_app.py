#!/usr/bin/env python3
"""
AXEL Desktop Application Component Test
Tests all UI components and integration with core pipeline
"""

import sys
import os

def test_imports():
    """Test all imports work correctly."""
    print("=" * 60)
    print("Testing Component Imports...")
    print("=" * 60)
    
    try:
        from axel_desktop_app import AxelDesktopApp
        print("✓ AxelDesktopApp imported successfully")
    except ImportError as e:
        print(f"✗ AxelDesktopApp import failed: {e}")
        return False
    
    try:
        from ui.file_browser import FileBrowserWidget
        print("✓ FileBrowserWidget imported successfully")
    except ImportError as e:
        print(f"✗ FileBrowserWidget import failed: {e}")
        return False
    
    try:
        from ui.processor import ProcessorThread
        print("✓ ProcessorThread imported successfully")
    except ImportError as e:
        print(f"✗ ProcessorThread import failed: {e}")
        return False
    
    try:
        from ui.results_display import ResultsDisplayWidget
        print("✓ ResultsDisplayWidget imported successfully")
    except ImportError as e:
        print(f"✗ ResultsDisplayWidget import failed: {e}")
        return False
    
    try:
        from ui.search_widget import SearchWidget
        print("✓ SearchWidget imported successfully")
    except ImportError as e:
        print(f"✗ SearchWidget import failed: {e}")
        return False
    
    return True

def test_core_pipeline():
    """Test core AXEL pipeline modules."""
    print("\n" + "=" * 60)
    print("Testing Core AXEL Pipeline...")
    print("=" * 60)
    
    try:
        import os
        if os.path.exists('.env'):
            print("✓ .env file exists")
        else:
            print("✗ .env file not found")
            return False
    except Exception as e:
        print(f"✗ .env check failed: {e}")
        return False
    
    try:
        from utils.indexer import search_index
        print("✓ utils.indexer imported successfully")
    except ImportError as e:
        print(f"✗ utils.indexer import failed: {e}")
        return False
    
    try:
        from utils.file_handler import detect_file_type
        print("✓ utils.file_handler imported successfully")
    except ImportError as e:
        print(f"✗ utils.file_handler import failed: {e}")
        return False
    
    try:
        from ml.classifier import train_model, predict_category
        print("✓ ml.classifier imported successfully")
    except ImportError as e:
        print(f"✗ ml.classifier import failed: {e}")
        return False
    
    print("✓ Core AXEL modules available (main module imports deferred)")
    return True

def test_pyqt_components():
    """Test PyQt5 component instantiation."""
    print("\n" + "=" * 60)
    print("Testing PyQt5 Component Instantiation...")
    print("=" * 60)
    
    try:
        from PyQt5.QtWidgets import QApplication
        app = QApplication.instance() or QApplication(sys.argv)
        print("✓ QApplication initialized")
    except Exception as e:
        print(f"✗ QApplication init failed: {e}")
        return False
    
    try:
        from ui.file_browser import FileBrowserWidget
        widget = FileBrowserWidget()
        print("✓ FileBrowserWidget instantiated")
    except Exception as e:
        print(f"✗ FileBrowserWidget instantiation failed: {e}")
        return False
    
    try:
        from ui.processor import ProcessorThread
        # Don't actually run, just instantiate
        thread = ProcessorThread("dummy_path.pdf")
        print("✓ ProcessorThread instantiated")
    except Exception as e:
        print(f"✗ ProcessorThread instantiation failed: {e}")
        return False
    
    try:
        from ui.results_display import ResultsDisplayWidget
        widget = ResultsDisplayWidget()
        print("✓ ResultsDisplayWidget instantiated")
    except Exception as e:
        print(f"✗ ResultsDisplayWidget instantiation failed: {e}")
        return False
    
    try:
        from ui.search_widget import SearchWidget
        widget = SearchWidget()
        print("✓ SearchWidget instantiated")
    except Exception as e:
        print(f"✗ SearchWidget instantiation failed: {e}")
        return False
    
    return True

def test_project_files():
    """Verify key project files exist."""
    print("\n" + "=" * 60)
    print("Verifying Project Files...")
    print("=" * 60)
    
    required_files = [
        'main.py',
        'config.py',
        'launcher.py',
        'axel_desktop_app.py',
        '.env',
        'requirements.txt',
        'axel_index.json',
        'ui/__init__.py',
        'ui/file_browser.py',
        'ui/processor.py',
        'ui/results_display.py',
        'ui/search_widget.py',
    ]
    
    missing = []
    for file_path in required_files:
        full_path = os.path.join(os.getcwd(), file_path)
        if os.path.exists(full_path):
            print(f"✓ {file_path}")
        else:
            print(f"✗ {file_path} (MISSING)")
            missing.append(file_path)
    
    return len(missing) == 0

def main():
    """Run all tests."""
    print("\n")
    print("╔" + "=" * 58 + "╗")
    print("║" + " " * 58 + "║")
    print("║" + "  AXEL Desktop Application - Component Test Suite".center(58) + "║")
    print("║" + " " * 58 + "║")
    print("╚" + "=" * 58 + "╝")
    
    # Change to script directory
    os.chdir(os.path.dirname(os.path.abspath(__file__)) or '.')
    
    results = []
    results.append(("Project Files", test_project_files()))
    results.append(("Component Imports", test_imports()))
    results.append(("Core Pipeline", test_core_pipeline()))
    results.append(("PyQt5 Components", test_pyqt_components()))
    
    # Summary
    print("\n" + "=" * 60)
    print("Test Summary")
    print("=" * 60)
    
    all_passed = True
    for test_name, passed in results:
        status = "✓ PASS" if passed else "✗ FAIL"
        print(f"{test_name}: {status}")
        all_passed = all_passed and passed
    
    print("=" * 60)
    if all_passed:
        print("\n✓ All tests passed! Desktop application is ready to launch.")
        print("\nTo start the application, run:")
        print("  python launcher.py")
    else:
        print("\n✗ Some tests failed. Please review errors above.")
    
    return 0 if all_passed else 1

if __name__ == "__main__":
    sys.exit(main())
