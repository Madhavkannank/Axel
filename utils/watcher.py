import time
import os
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

class AxelEventHandler(FileSystemEventHandler):
    def __init__(self, callback, processed_cache):
        self.callback = callback
        self.processed_cache = processed_cache

    def process_event(self, event):
        if event.is_directory:
            return
            
        path = event.src_path
        ext = os.path.splitext(path)[1].lower()
        base_name = os.path.basename(path)
        
        # Ignore temp files, partial writes, or files created by AXEL
        if ext in ['.tmp', '.part', '.crdownload'] or base_name.startswith('updated_'):
            return
            
        # Avoid processing the same file repeatedly in one session
        if path in self.processed_cache:
            return
            
        print(f"\n[WATCHER] New file detected: {path}")
        # Wait for file write completion to avoid "file in use" errors
        time.sleep(2)
        
        if os.path.exists(path) and os.path.getsize(path) > 0:
            self.processed_cache.add(path)
            self.callback(path)

    def on_created(self, event):
        self.process_event(event)

    def on_modified(self, event):
        self.process_event(event)

def start_watching(folder_path, callback, gui_mode=True):
    """Monitors a folder and triggers the callback when new files arrive.
    
    Args:
        folder_path: Path to monitor
        callback: Function to call when file is detected
        gui_mode: If True, runs indefinitely (for GUI/daemon threads)
                 If False, runs with keyboard interrupt support (CLI mode)
    """
    if not os.path.exists(folder_path):
        os.makedirs(folder_path)
        print(f"[INFO] Created directory: {folder_path}")
        
    processed_cache = set()
    event_handler = AxelEventHandler(callback, processed_cache)
    observer = Observer()
    observer.schedule(event_handler, folder_path, recursive=False)
    observer.start()
    
    print(f"[INFO] Starting to watch folder: {folder_path}")
    
    if gui_mode:
        # For GUI mode: run indefinitely without keyboard interrupt handling
        try:
            while True:
                time.sleep(1)
        except Exception as e:
            print(f"[WARNING] Watcher error: {e}")
        finally:
            observer.stop()
            observer.join()
    else:
        # For CLI mode: support Ctrl+C to stop
        print("[INFO] Press Ctrl+C to stop.")
        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            observer.stop()
            print("\n[INFO] Stopping watcher...")
        observer.join()