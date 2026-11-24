use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    CreatedOrModified(PathBuf),
    Removed(PathBuf),
}

pub struct FolderWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
    pending_events: Mutex<VecDeque<WatchEvent>>,
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
            pending_events: Mutex::new(VecDeque::new()),
            watched_dir,
        })
    }

    #[allow(dead_code)]
    pub fn watched_dir(&self) -> &Path {
        &self.watched_dir
    }

    /// Polls for the next file system event (creation, modification, or removal)
    pub fn poll_next_event(&self) -> Option<WatchEvent> {
        if let Ok(mut queue) = self.pending_events.lock() {
            if let Some(event) = queue.pop_front() {
                return Some(event);
            }
        }

        while let Ok(event_result) = self.rx.try_recv() {
            if let Ok(event) = event_result {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        let mut new_events = Vec::new();
                        for path in event.paths {
                            if self.is_processable_file(&path) {
                                new_events.push(WatchEvent::CreatedOrModified(path));
                            }
                        }

                        if !new_events.is_empty() {
                            if let Ok(mut queue) = self.pending_events.lock() {
                                for ev in new_events {
                                    queue.push_back(ev);
                                }
                                return queue.pop_front();
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        let mut removed_events = Vec::new();
                        for path in event.paths {
                            if crate::engine::is_path_inside_workspace(&path, &self.watched_dir) {
                                removed_events.push(WatchEvent::Removed(path));
                            }
                        }

                        if !removed_events.is_empty() {
                            if let Ok(mut queue) = self.pending_events.lock() {
                                for ev in removed_events {
                                    queue.push_back(ev);
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

    /// Backwards-compatible convenience helper returning created/modified paths
    pub fn poll_next_file(&self) -> Option<PathBuf> {
        while let Some(ev) = self.poll_next_event() {
            if let WatchEvent::CreatedOrModified(path) = ev {
                return Some(path);
            }
        }
        None
    }

    fn is_processable_file(&self, path: &Path) -> bool {
        // H02: Reject symlinks that target files outside workspace boundary
        let canonical = match path.canonicalize() {
            Ok(c) => c,
            Err(_) => return false,
        };

        if !crate::engine::is_path_inside_workspace(&canonical, &self.watched_dir) {
            return false;
        }

        if !canonical.is_file() {
            return false;
        }

        // Only inspect directory components UNDER the watched root directory.
        // Parent components above watched_dir must never cause files inside the workspace to be skipped.
        let rel_path = canonical.strip_prefix(&self.watched_dir).unwrap_or(&canonical);
        if let Some(parent) = rel_path.parent() {
            for comp in parent.components() {
                if let Some(name) = comp.as_os_str().to_str() {
                    if crate::classifier::filter::should_skip_directory(name) {
                        return false;
                    }
                }
            }
        }

        let file_size = canonical.metadata().ok().map(|m| m.len());
        if crate::classifier::filter::should_skip_file(&canonical, file_size) {
            return false;
        }

        true
    }
}
