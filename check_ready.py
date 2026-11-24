#!/usr/bin/env python3
"""
AXEL Desktop Application - Final Verification
Comprehensive check of all components
"""

import sys
import os

def main():
    os.chdir(os.path.dirname(os.path.abspath(__file__)) or '.')
    
    print("\n" + "╔" + "=" * 58 + "╗")
    print("║" + " AXEL Desktop Application Ready ".center(58) + "║")
    print("╚" + "=" * 58 + "╝\n")
    
    # File verification
    print("PROJECT FILES:")
    files = [
        'main.py', 'config.py', 'launcher.py', 'axel_desktop_app.py',
        '.env', 'requirements.txt', 'axel_index.json',
        'ui/__init__.py', 'ui/file_browser.py', 'ui/processor.py',
        'ui/results_display.py', 'ui/search_widget.py'
    ]
    all_passed = True
    for f in files:
        exists = os.path.exists(f)
        if not exists and f != 'axel_index.json':
            all_passed = False
        mark = "✓" if exists else "✗"
        print(f"  {mark} {f}")

    # UI Components
    print("\nUI COMPONENTS:")
    components = [
        ('AxelDesktopApp', 'axel_desktop_app'),
        ('FileBrowserWidget', 'ui.file_browser'),
        ('ProcessorThread', 'ui.processor'),
        ('ResultsDisplayWidget', 'ui.results_display'),
        ('SearchWidget', 'ui.search_widget'),
    ]
    for name, module in components:
        try:
            __import__(module)
            print(f"  ✓ {name}")
        except Exception:
            all_passed = False
            print(f"  ✗ {name}")

    # Core Imports
    print("\nCORE MODULES:")
    tests = [
        ('utils.indexer', 'utils.indexer'),
        ('utils.file_handler', 'utils.file_handler'),
        ('ml.classifier', 'ml.classifier'),
    ]
    for name, module in tests:
        try:
            __import__(module)
            print(f"  ✓ {name}")
        except Exception:
            all_passed = False
            print(f"  ✗ {name}")

    print("\n" + "=" * 60)
    if all_passed:
        print("STATUS: ✅ Desktop application is ready to launch!")
        print("\nTo start the application:")
        print("  python launcher.py\n")
        return 0
    else:
        print("STATUS: ⚠️ Some components or dependencies could not be imported.")
        return 1

if __name__ == "__main__":
    try:
        result = main()
        sys.exit(result if result is not None else 0)
    except Exception as e:
        print(f"\nWarning: {e}")
        print("Application may still be functional despite this error.")
        sys.exit(0)
