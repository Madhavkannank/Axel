#!/usr/bin/env python3
"""
AXEL Desktop Application - Quick Verification
Tests all UI components and core imports
"""

import sys
import os

def main():
    """Run verification tests."""
    print("\n" + "╔" + "=" * 58 + "╗")
    print("║" + " AXEL Desktop Application - Verification ".center(58) + "║")
    print("╚" + "=" * 58 + "╝\n")
    
    os.chdir(os.path.dirname(os.path.abspath(__file__)) or '.')
    
    tests = [
        ("Project Files", verify_files),
        ("UI Components", verify_ui),
        ("Core Modules", verify_core),
    ]
    
    results = []
    for name, test_func in tests:
        try:
            status = test_func()
            results.append((name, status))
        except Exception as e:
            print(f"✗ {name}: {e}")
            results.append((name, False))
    
    # Summary
    print("\n" + "=" * 60)
    print("Summary")
    print("=" * 60)
    all_pass = True
    for name, status in results:
        symbol = "✓" if status else "✗"
        print(f"{symbol} {name}")
        all_pass = all_pass and status
    
    print("=" * 60)
    if all_pass:
        print("\n✅ Desktop application is ready!")
        print("\nTo launch the application:")
        print("  python launcher.py")
        return 0
    else:
        print("\n⚠️ Some checks failed. Review errors above.")
        return 1

def verify_files():
    """Verify required project files exist."""
    print("\nVerifying Project Files...")
    files = [
        'main.py', 'config.py', 'launcher.py', 'axel_desktop_app.py',
        '.env', 'requirements.txt', 'axel_index.json',
        'ui/__init__.py', 'ui/file_browser.py', 'ui/processor.py',
        'ui/results_display.py', 'ui/search_widget.py'
    ]
    
    missing = []
    for f in files:
        if not os.path.exists(f):
            missing.append(f)
            print(f"  ✗ {f}")
        else:
            print(f"  ✓ {f}")
    
    return len(missing) == 0

def verify_ui():
    """Verify UI components import correctly."""
    print("\nVerifying UI Components...")
    components = [
        ('AxelDesktopApp', 'axel_desktop_app', 'AxelDesktopApp'),
        ('FileBrowserWidget', 'ui.file_browser', 'FileBrowserWidget'),
        ('ProcessorThread', 'ui.processor', 'ProcessorThread'),
        ('ResultsDisplayWidget', 'ui.results_display', 'ResultsDisplayWidget'),
        ('SearchWidget', 'ui.search_widget', 'SearchWidget'),
    ]
    
    all_ok = True
    for display_name, module_name, class_name in components:
        try:
            module = __import__(module_name, fromlist=[class_name])
            getattr(module, class_name)
            print(f"  ✓ {display_name}")
        except Exception as e:
            print(f"  ✗ {display_name}: {e}")
            all_ok = False
    
    return all_ok

def verify_core():
    """Verify core AXEL modules import."""
    print("\nVerifying Core AXEL Modules...")
    
    modules = [
        ('.env file', lambda: os.path.exists('.env')),
        ('utils.indexer', lambda: __import__('utils.indexer', fromlist=['search_index'])),
        ('utils.file_handler', lambda: __import__('utils.file_handler', fromlist=['detect_file_type'])),
        ('ml.classifier', lambda: __import__('ml.classifier', fromlist=['predict_category'])),
    ]
    
    all_ok = True
    for name, test in modules:
        try:
            result = test()
            if result or result is None:
                print(f"  ✓ {name}")
            else:
                print(f"  ✗ {name}: returned False")
                all_ok = False
        except Exception as e:
            print(f"  ✗ {name}: {str(e)[:50]}")
            all_ok = False
    
    return all_ok

if __name__ == "__main__":
    sys.exit(main())
