use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

pub struct FolderWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
    pending_queue: Mutex<VecDeque<PathBuf>>,
    #[allow(dead_code)]
    watched_dir: PathBuf,
}

impl FolderWatcher {
    pub fn new<P: AsRef<Path>>(dir: P) -> Result<Self, String> {
        let watched_dir = dir.as_ref().to_path_buf();
        if !watched_dir.exists() {
            std::fs::create_dir_all(&watched_dir)
                .map_err(|e| format!("Failed to create watched directory {}: {e}", watched_dir.display()))?;
        }

        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            tx,
            NotifyConfig::default().with_poll_interval(Duration::from_secs(1)),
        )
        .map_err(|e| format!("Failed to initialize file watcher: {e}"))?;

        // Try recursive watch first; if inotify watch limit or deep tree fails, fall back to non-recursive
        if let Err(rec_err) = watcher.watch(&watched_dir, RecursiveMode::Recursive) {
            watcher
                .watch(&watched_dir, RecursiveMode::NonRecursive)
                .map_err(|e| {
                    format!(
                        "Failed to watch directory {}: {rec_err} / {e}",
                        watched_dir.display()
                    )
                })?;
        }

        Ok(Self {
            _watcher: watcher,
            rx,
            pending_queue: Mutex::new(VecDeque::new()),
            watched_dir,
        })
    }

    #[allow(dead_code)]
    pub fn watched_dir(&self) -> &Path {
        &self.watched_dir
    }

    pub fn poll_next_file(&self) -> Option<PathBuf> {
        if let Ok(mut queue) = self.pending_queue.lock() {
            if let Some(path) = queue.pop_front() {
                return Some(path);
            }
        }

        while let Ok(event_result) = self.rx.try_recv() {
            if let Ok(event) = event_result {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        let mut new_paths = Vec::new();
                        for path in event.paths {
                            if self.is_processable_file(&path) {
                                new_paths.push(path);
                            }
                        }

                        if !new_paths.is_empty() {
                            if let Ok(mut queue) = self.pending_queue.lock() {
                                for p in new_paths {
                                    queue.push_back(p);
                                }
                                return queue.pop_front();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        None
    }

    fn is_processable_file(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        // Only inspect directory components UNDER the watched root directory.
        // Parent components above watched_dir must never cause files inside the workspace to be skipped.
        let rel_path = path.strip_prefix(&self.watched_dir).unwrap_or(path);
        if let Some(parent) = rel_path.parent() {
            for comp in parent.components() {
                if let Some(name) = comp.as_os_str().to_str() {
                    if crate::classifier::filter::should_skip_directory(name) {
                        return false;
                    }
                }
            }
        }

        let file_size = path.metadata().ok().map(|m| m.len());
        if crate::classifier::filter::should_skip_file(path, file_size) {
            return false;
        }

        path.is_file()
    }
}
