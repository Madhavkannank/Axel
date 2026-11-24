use notify::event::{ModifyKind, RenameMode};
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
                let mut new_events = Vec::new();
                match event.kind {
                    // A06: Explicitly handle rename modes
                    EventKind::Modify(ModifyKind::Name(rename_mode)) => match rename_mode {
                        RenameMode::Both => {
                            if event.paths.len() >= 2 {
                                let from = &event.paths[0];
                                let to = &event.paths[1];
                                if self.is_processable_removal(from) {
                                    new_events.push(WatchEvent::Removed(from.clone()));
                                }
                                if self.is_processable_file(to) {
                                    new_events.push(WatchEvent::CreatedOrModified(to.clone()));
                                }
                            } else if let Some(p) = event.paths.first() {
                                if p.exists() {
                                    if self.is_processable_file(p) {
                                        new_events.push(WatchEvent::CreatedOrModified(p.clone()));
                                    }
                                } else if self.is_processable_removal(p) {
                                    new_events.push(WatchEvent::Removed(p.clone()));
                                }
                            }
                        }
                        RenameMode::From => {
                            for path in event.paths {
                                if self.is_processable_removal(&path) {
                                    new_events.push(WatchEvent::Removed(path));
                                }
                            }
                        }
                        RenameMode::To => {
                            for path in event.paths {
                                if self.is_processable_file(&path) {
                                    new_events.push(WatchEvent::CreatedOrModified(path));
                                }
                            }
                        }
                        RenameMode::Any | RenameMode::Other => {
                            for path in event.paths {
                                if path.exists() {
                                    if self.is_processable_file(&path) {
                                        new_events.push(WatchEvent::CreatedOrModified(path));
                                    }
                                } else if self.is_processable_removal(&path) {
                                    new_events.push(WatchEvent::Removed(path));
                                }
                            }
                        }
                    },
                    EventKind::Create(_) => {
                        for path in event.paths {
                            if self.is_processable_file(&path) {
                                new_events.push(WatchEvent::CreatedOrModified(path));
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in event.paths {
                            if self.is_processable_removal(&path) {
                                new_events.push(WatchEvent::Removed(path));
                            }
                        }
                    }
                    EventKind::Modify(_) => {
                        for path in event.paths {
                            if path.exists() {
                                if self.is_processable_file(&path) {
                                    new_events.push(WatchEvent::CreatedOrModified(path));
                                }
                            } else if self.is_processable_removal(&path) {
                                new_events.push(WatchEvent::Removed(path));
                            }
                        }
                    }
                    _ => {}
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

    fn is_processable_removal(&self, path: &Path) -> bool {
        if !crate::engine::is_path_inside_workspace(path, &self.watched_dir) {
            return false;
        }

        let resolved_workspace = crate::config::resolve_workspace_path(&self.watched_dir);
        let resolved_path = crate::config::resolve_workspace_path(path);
        let rel_path = resolved_path
            .strip_prefix(&resolved_workspace)
            .unwrap_or(&resolved_path);
        if let Some(parent) = rel_path.parent() {
            for comp in parent.components() {
                if let Some(name) = comp.as_os_str().to_str() {
                    if crate::classifier::filter::should_skip_directory(name) || name == ".axel" {
                        return false;
                    }
                }
            }
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            if ext.ends_with("tmp") || ext.ends_with("bak") || ext.ends_with("lock") {
                return false;
            }
        }

        true
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
