pub mod chrome;
pub mod galaxy;
pub mod overlays;
pub mod panels;
pub mod theme;
pub mod workstation;

use crate::ai::AiEngine;
use crate::config::Config;
use crate::desktop::galaxy::{
    render_cluster_grid, render_galaxy_canvas, GalaxyAction, GalaxyCamera, GalaxyLayout,
};
use crate::desktop::panels::{
    merge_action, render_activity_sidebar, render_drop_dock_overlay, render_inspector_drawer,
    render_left_rail, render_properties_modal, render_semantic_search_overlay,
    render_telemetry_drawer, render_titlebar, render_topbar, render_workspace_switcher_popover,
    PanelAction, ViewMode,
};
use crate::desktop::theme::{
    active_theme, font_medium, font_sans, install_fonts_and_theme, palette, set_active_theme,
    with_alpha,
};
use crate::desktop::workstation::render_workstation;
use crate::engine::{
    analyze_and_build_item, build_document_detail, build_graph_snapshot, build_system_stats,
    delete_document_from_index, extract_file_properties, ingest_local_files, scan_folder_into_items,
    DocumentDetail, FileProperties, GraphSnapshot, ScanSummary, SystemStats,
};
use crate::index::{DocumentIndex, FileMetadataTag, IndexItem, SearchResult};
use crate::watcher::FolderWatcher;
use egui::{
    ColorImage, CornerRadius, Frame, Id, Key, Order, Pos2, Rect, Stroke, StrokeKind,
    ViewportCommand,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tokio::sync::mpsc as tokio_mpsc;

enum WorkerCommand {
    Scan { force: bool },
    IngestFiles(Vec<PathBuf>),
    SwitchWorkspace(PathBuf),
    LoadDetail(String),
    DeleteDocument { hash: String, delete_file: bool },
    TrashDuplicateFile { canonical_hash: String, trash_path: String },
    TrashAllDuplicates,
    TrashFile(String),
}

enum WorkerEvent {
    SnapshotUpdated {
        snapshot: GraphSnapshot,
        stats: SystemStats,
        items: Vec<IndexItem>,
        status: Option<String>,
    },
    DetailLoaded(DocumentDetail),
    Busy(Option<String>),
    Error(String),
}

pub struct AxelDesktopApp {
    cmd_tx: tokio_mpsc::UnboundedSender<WorkerCommand>,
    evt_rx: mpsc::Receiver<WorkerEvent>,

    snapshot: GraphSnapshot,
    stats: SystemStats,
    search_index: DocumentIndex,

    layout: GalaxyLayout,
    camera: GalaxyCamera,
    view_mode: ViewMode,

    show_activity: bool,
    show_telemetry: bool,
    is_maximized: bool,

    selected_hub: Option<String>,
    selected_node: Option<String>,
    active_detail: Option<DocumentDetail>,
    confirm_delete: bool,

    active_category_filter: Option<String>,
    search_query: String,
    focus_search_requested: bool,
    search_popover_open: bool,
    search_results: Vec<SearchResult>,
    search_highlight_hashes: HashSet<String>,

    dock_index: usize,
    is_busy: bool,
    status_text: Option<String>,
    error_banner: Option<(String, Instant)>,

    show_semantic_links: bool,
    workspace_popover_open: bool,
    workspace_path_input: String,
    workspace_path_error: Option<String>,
    workspace_quick_folders: Vec<(String, PathBuf)>,
    recent_workspaces: Vec<PathBuf>,
    top_k_results: usize,
    is_picking_folder: bool,
    topbar_anchor_rect: Rect,

    screenshot_path: Option<PathBuf>,
    screenshot_requested: bool,
    screenshot_saved: Arc<AtomicBool>,
    frames_rendered: u32,
    snapshot_ready: bool,
    started_at: Instant,
    properties_target: Option<FileProperties>,
}

impl AxelDesktopApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        screenshot_path: Option<PathBuf>,
    ) -> Self {
        install_fonts_and_theme(&cc.egui_ctx);

        let initial_stats = SystemStats {
            total_documents: 0,
            groq_enabled: config.groq_api_key.is_some(),
            tesseract_ready: crate::extractor::ocr::find_tesseract_binary().is_some(),
            active_model: config.model_name.clone(),
            input_folder: config.input_folder.display().to_string(),
            index_file: config.index_file.display().to_string(),
            categories: Default::default(),
        };

        let mut quick_folders = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            let docs = cwd.join("documents");
            if docs.exists() {
                quick_folders.push(("Axel Documents".to_string(), docs));
            }
        }
        if let Some(home) = dirs::home_dir() {
            let dl = home.join("Downloads");
            if dl.exists() {
                quick_folders.push(("Downloads".to_string(), dl));
            }
            let doc = home.join("Documents");
            if doc.exists() {
                quick_folders.push(("Home Documents".to_string(), doc));
            }
        }
        let mut recent_workspaces = crate::config::load_recent_workspaces();
        if !recent_workspaces.iter().any(|p| crate::config::same_dir(p, &config.input_folder)) {
            recent_workspaces.insert(0, config.input_folder.clone());
        }

        let (cmd_tx, cmd_rx) = tokio_mpsc::unbounded_channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        let egui_ctx = cc.egui_ctx.clone();

        spawn_background_worker(config.clone(), cmd_rx, evt_tx, egui_ctx);

        Self {
            cmd_tx,
            evt_rx,
            snapshot: GraphSnapshot::default(),
            stats: initial_stats,
            search_index: DocumentIndex::from_items(Vec::new()),
            layout: GalaxyLayout::default(),
            camera: GalaxyCamera::default(),
            view_mode: ViewMode::Workstation,
            show_activity: false,
            show_telemetry: false,
            is_maximized: false,
            selected_hub: None,
            selected_node: None,
            active_detail: None,
            confirm_delete: false,
            active_category_filter: None,
            search_query: String::new(),
            focus_search_requested: false,
            search_popover_open: false,
            search_results: Vec::new(),
            search_highlight_hashes: HashSet::new(),
            dock_index: 0,
            is_busy: true,
            status_text: Some("Loading workspace...".to_string()),
            error_banner: None,
            show_semantic_links: false,
            workspace_popover_open: false,
            workspace_path_input: config.input_folder.display().to_string(),
            workspace_path_error: None,
            workspace_quick_folders: quick_folders,
            recent_workspaces,
            top_k_results: config.top_k_results,
            is_picking_folder: false,
            topbar_anchor_rect: Rect::NOTHING,
            screenshot_path,
            screenshot_requested: false,
            screenshot_saved: Arc::new(AtomicBool::new(false)),
            frames_rendered: 0,
            snapshot_ready: false,
            started_at: Instant::now(),
            properties_target: None,
        }
    }

    fn workspace_display_name(&self) -> String {
        let p = Path::new(&self.stats.input_folder);
        p.file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty() && *s != ".")
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.stats.input_folder.clone())
    }

    fn drain_worker_events(&mut self) {
        while let Ok(evt) = self.evt_rx.try_recv() {
            match evt {
                WorkerEvent::SnapshotUpdated {
                    snapshot,
                    stats,
                    items,
                    status,
                } => {
                    self.snapshot = snapshot;
                    self.stats = stats;
                    self.search_index = DocumentIndex::from_items(items);
                    self.layout.rebuild(&self.snapshot);
                    self.recompute_search();
                    self.is_busy = false;
                    self.status_text = status;
                    self.snapshot_ready = true;

                    if let Some(ref sel) = self.selected_node {
                        if !self.snapshot.nodes.iter().any(|n| &n.hash == sel) {
                            self.selected_node = None;
                            self.active_detail = None;
                            self.confirm_delete = false;
                        }
                    }
                    if !self.snapshot.nodes.is_empty() {
                        self.dock_index %= self.snapshot.nodes.len();
                    } else {
                        self.dock_index = 0;
                    }
                }
                WorkerEvent::DetailLoaded(detail) => {
                    if self.selected_node.as_deref() == Some(detail.item.hash.as_str()) {
                        self.active_detail = Some(detail);
                    }
                }
                WorkerEvent::Busy(msg) => {
                    self.is_busy = msg.is_some();
                    self.status_text = msg;
                }
                WorkerEvent::Error(err) => {
                    self.is_busy = false;
                    self.status_text = None;
                    self.error_banner = Some((err, Instant::now()));
                }
            }
        }
    }

    fn recompute_search(&mut self) {
        if self.search_query.trim().is_empty() {
            self.search_results.clear();
            self.search_highlight_hashes.clear();
        } else {
            let limit = self.top_k_results.max(12);
            self.search_results = self.search_index.search(&self.search_query, limit);
            self.search_highlight_hashes = self
                .search_results
                .iter()
                .map(|r| r.item.hash.clone())
                .collect();
        }
    }

    fn select_node(&mut self, hash: String, center_camera: bool) {
        self.selected_node = Some(hash.clone());
        self.selected_hub = None;
        self.show_telemetry = false;
        self.confirm_delete = false;
        self.search_popover_open = false;

        if center_camera {
            if let Some(&idx) = self.layout.node_index.get(&hash) {
                let world_pos = self.layout.nodes[idx].pos;
                self.camera.target_pan = -world_pos.to_vec2() * self.camera.target_zoom;
            }
        }

        let _ = self.cmd_tx.send(WorkerCommand::LoadDetail(hash));
    }

    fn handle_panel_action(&mut self, ctx: &egui::Context, action: PanelAction) {
        match action {
            PanelAction::None => {}
            PanelAction::MinimizeWindow => {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            PanelAction::ToggleMaximizeWindow => {
                self.is_maximized = !self.is_maximized;
                ctx.send_viewport_cmd(ViewportCommand::Maximized(self.is_maximized));
            }
            PanelAction::CloseWindow => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            PanelAction::StartWindowDrag => {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            PanelAction::SetViewMode(mode) => {
                self.view_mode = mode;
            }
            PanelAction::FocusSearch => {
                self.focus_search_requested = true;
                if !self.search_query.trim().is_empty() {
                    self.search_popover_open = true;
                }
            }
            PanelAction::TriggerScan { force } => {
                self.is_busy = true;
                self.status_text = Some(if force {
                    "Re-indexing workspace...".to_string()
                } else {
                    "Scanning workspace...".to_string()
                });
                let _ = self.cmd_tx.send(WorkerCommand::Scan { force });
            }
            PanelAction::PickFilesToIngest => {
                let tx = self.cmd_tx.clone();
                let ctx_clone = ctx.clone();
                std::thread::spawn(move || {
                    if let Some(paths) = rfd::FileDialog::new()
                        .set_title("Ingest Files into AXEL")
                        .add_filter(
                            "All Supported Files",
                            &[
                                "pdf", "docx", "doc", "odt", "rtf", "txt", "md", "csv", "tsv", "json", "yaml", "yml", "toml", "xml",
                                "png", "jpg", "jpeg", "webp", "tiff", "bmp", "gif", "svg", "ico",
                                "mp4", "mkv", "avi", "mov", "webm", "flv", "m4v", "wmv",
                                "mp3", "wav", "flac", "ogg", "m4a", "aac", "opus", "wma",
                                "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "zst", "iso", "deb", "rpm",
                                "py", "rs", "go", "java", "c", "cpp", "h", "hpp", "js", "ts", "jsx", "tsx", "sh", "sql", "html", "css",
                            ],
                        )
                        .add_filter("All Files (*.*)", &["*"])
                        .pick_files()
                    {
                        if !paths.is_empty() {
                            let _ = tx.send(WorkerCommand::IngestFiles(paths));
                            ctx_clone.request_repaint();
                        }
                    }
                });
            }
            PanelAction::ToggleWorkspacePopover => {
                self.workspace_popover_open = !self.workspace_popover_open;
                self.workspace_path_error = None;
                if self.workspace_popover_open {
                    self.workspace_path_input = self.stats.input_folder.clone();
                }
            }
            PanelAction::CloseWorkspacePopover => {
                self.workspace_popover_open = false;
                self.workspace_path_error = None;
            }
            PanelAction::SwitchWorkspacePath(new_path) => {
                self.workspace_popover_open = false;
                self.workspace_path_error = None;
                if !self.recent_workspaces.contains(&new_path) {
                    self.recent_workspaces.insert(0, new_path.clone());
                    self.recent_workspaces.truncate(6);
                }
                self.is_busy = true;
                self.status_text = Some(format!("Switching to {}...", new_path.display()));
                let _ = self.cmd_tx.send(WorkerCommand::SwitchWorkspace(new_path));
            }
            PanelAction::BrowseWorkspaceFolder => {
                self.is_picking_folder = true;
                let tx = self.cmd_tx.clone();
                let ctx_clone = ctx.clone();
                let current_dir = PathBuf::from(&self.stats.input_folder);
                std::thread::spawn(move || {
                    let mut dialog = rfd::FileDialog::new().set_title("Select Workspace Folder");
                    if current_dir.exists() {
                        dialog = dialog.set_directory(&current_dir);
                    }
                    if let Some(folder) = dialog.pick_folder() {
                        let _ = tx.send(WorkerCommand::SwitchWorkspace(folder));
                    }
                    ctx_clone.request_repaint();
                });
            }
            PanelAction::ToggleSemanticLinks => {
                self.show_semantic_links = !self.show_semantic_links;
            }
            PanelAction::CycleTheme => {
                let next = active_theme().next();
                set_active_theme(ctx, next);
            }
            PanelAction::SetTheme(preset) => {
                set_active_theme(ctx, preset);
            }
            PanelAction::ToggleActivitySidebar => {
                self.show_activity = !self.show_activity;
            }
            PanelAction::ResetCamera => {
                self.camera.reset();
            }
            PanelAction::ToggleTelemetryDrawer => {
                self.show_telemetry = !self.show_telemetry;
                if self.show_telemetry {
                    self.selected_node = None;
                    self.active_detail = None;
                    self.confirm_delete = false;
                }
            }
            PanelAction::SelectNode(hash) => {
                self.select_node(hash, true);
            }
            PanelAction::SelectHub(hub_id) => {
                let cat = hub_id.strip_prefix("hub:").unwrap_or(&hub_id).to_string();
                if self.active_category_filter.as_deref() == Some(cat.as_str()) {
                    self.active_category_filter = None;
                    self.selected_hub = None;
                } else {
                    self.active_category_filter = Some(cat);
                    self.selected_hub = Some(hub_id);
                }
            }
            PanelAction::SetCategoryFilter(cat) => {
                self.selected_hub = cat.as_ref().map(|c| format!("hub:{c}"));
                self.active_category_filter = cat;
            }
            PanelAction::SearchQueryChanged(q) => {
                self.search_query = q;
                self.search_popover_open = !self.search_query.trim().is_empty();
                self.recompute_search();
            }
            PanelAction::CloseSearchPopover => {
                self.search_popover_open = false;
            }
            PanelAction::OpenFileExternal(path) => {
                std::thread::spawn(move || {
                    let _ = open::that_detached(&path);
                });
            }
            PanelAction::RevealFolderExternal(path) => {
                std::thread::spawn(move || {
                    let p = Path::new(&path);
                    let dir = p.parent().unwrap_or_else(|| Path::new("."));
                    let _ = open::that_detached(dir);
                });
            }
            PanelAction::DeleteDocument { hash, delete_file } => {
                self.selected_node = None;
                self.active_detail = None;
                self.confirm_delete = false;
                self.is_busy = true;
                self.status_text = Some("Removing document...".to_string());
                let _ = self
                    .cmd_tx
                    .send(WorkerCommand::DeleteDocument { hash, delete_file });
            }
            PanelAction::CloseInspector => {
                self.selected_node = None;
                self.active_detail = None;
                self.confirm_delete = false;
            }
            PanelAction::CloseTelemetry => {
                self.show_telemetry = false;
            }
            PanelAction::CycleDropDock(delta) => {
                let len = self.snapshot.nodes.len();
                if len > 0 {
                    let cur = self.dock_index as isize;
                    self.dock_index = (cur + delta).rem_euclid(len as isize) as usize;
                }
            }
            PanelAction::SelectDropDockIndex(idx) => {
                if !self.snapshot.nodes.is_empty() {
                    self.dock_index = idx % self.snapshot.nodes.len();
                }
            }
            PanelAction::AppendSearchToken(token) => {
                let trimmed = self.search_query.trim();
                if trimmed.is_empty() {
                    self.search_query = token;
                } else if !trimmed.split_whitespace().any(|t| t == token) {
                    self.search_query = format!("{} {}", trimmed, token);
                }
                self.search_popover_open = true;
                self.focus_search_requested = true;
                self.recompute_search();
            }
            PanelAction::TrashDuplicateFile {
                canonical_hash,
                trash_path,
            } => {
                self.is_busy = true;
                self.status_text = Some("Verifying duplicate hash before trashing...".to_string());
                let _ = self.cmd_tx.send(WorkerCommand::TrashDuplicateFile {
                    canonical_hash,
                    trash_path,
                });
            }
            PanelAction::TrashAllDuplicates => {
                self.is_busy = true;
                self.status_text = Some("Running batch deduplication...".to_string());
                let _ = self.cmd_tx.send(WorkerCommand::TrashAllDuplicates);
            }
            PanelAction::CopyPath(path) => {
                ctx.copy_text(path);
            }
            PanelAction::CopyName(name) => {
                ctx.copy_text(name);
            }
            PanelAction::CopyFileUri(path) => {
                ctx.copy_text(format!("file://{path}"));
            }
            PanelAction::OpenProperties(path) => {
                if let Some(props) = extract_file_properties(Path::new(&path)) {
                    self.properties_target = Some(props);
                }
            }
            PanelAction::CloseProperties => {
                self.properties_target = None;
            }
            PanelAction::TrashFile(path) => {
                self.properties_target = None;
                self.is_busy = true;
                self.status_text = Some("Moving file to OS trash...".to_string());
                let _ = self.cmd_tx.send(WorkerCommand::TrashFile(path));
            }
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let search_focused = ctx.memory(|m| m.has_focus(Id::unique("axel_semantic_search_input")));

        let (ctrl_k, slash, esc, ctrl_r, zero) = ctx.input(|i| {
            (
                (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::K),
                !search_focused && i.key_pressed(Key::Slash),
                i.key_pressed(Key::Escape),
                (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::R),
                !search_focused && i.key_pressed(Key::Num0),
            )
        });

        if ctrl_k || slash {
            self.focus_search_requested = true;
            if !self.search_query.trim().is_empty() {
                self.search_popover_open = true;
            }
        }
        if ctrl_r {
            self.is_busy = true;
            self.status_text = Some("Scanning workspace...".to_string());
            let _ = self.cmd_tx.send(WorkerCommand::Scan { force: false });
        }
        if zero {
            self.camera.reset();
        }
        if esc {
            if self.properties_target.is_some() {
                self.properties_target = None;
            } else if self.search_popover_open {
                self.search_popover_open = false;
            } else if self.selected_node.is_some() {
                self.selected_node = None;
                self.active_detail = None;
                self.confirm_delete = false;
            } else if self.show_telemetry {
                self.show_telemetry = false;
            } else if self.active_category_filter.is_some() || self.selected_hub.is_some() {
                self.active_category_filter = None;
                self.selected_hub = None;
            }
        }
    }

    fn handle_os_file_drop(&mut self, ctx: &egui::Context) -> bool {
        let is_hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });

        if !dropped.is_empty() {
            self.is_busy = true;
            self.status_text = Some(format!("Ingesting {} dropped file(s)...", dropped.len()));
            let _ = self.cmd_tx.send(WorkerCommand::IngestFiles(dropped));
        }

        is_hovering
    }

    fn maybe_capture_screenshot(&mut self, ctx: &egui::Context) {
        if self.screenshot_saved.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        let Some(ref out_path) = self.screenshot_path else {
            return;
        };

        if !self.screenshot_requested
            && self.frames_rendered >= 10
            && (self.snapshot_ready || self.started_at.elapsed() > Duration::from_secs(2))
        {
            self.screenshot_requested = true;
            let path = out_path.clone();
            let saved_flag = Arc::clone(&self.screenshot_saved);
            let ctx_clone = ctx.clone();
            ctx.request_screenshot(move |image| {
                if let Err(err) = save_color_image(&image, &path) {
                    eprintln!("Failed to save screenshot to {}: {err}", path.display());
                }
                saved_flag.store(true, Ordering::Relaxed);
                ctx_clone.request_repaint();
            });
        }
    }
}

impl eframe::App for AxelDesktopApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        palette().bg_void.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frames_rendered = self.frames_rendered.saturating_add(1);

        self.drain_worker_events();
        self.handle_keyboard_shortcuts(&ctx);
        let is_file_drag_hovering = self.handle_os_file_drop(&ctx);

        if let Some((_, ts)) = self.error_banner {
            if ts.elapsed() > Duration::from_secs(6) {
                self.error_banner = None;
            }
        }

        let dt = ctx.input(|i| i.stable_dt);
        let camera_animating = self.camera.step(dt);
        let time = ctx.input(|i| i.time);

        let mut pending_action = PanelAction::None;
        let workspace_name = self.workspace_display_name();

        // 1. Custom Frameless Titlebar
        merge_action(
            &mut pending_action,
            render_titlebar(ui, &workspace_name, self.is_maximized),
        );

        // 2. Top Workspace & Action Bar
        let (topbar_action, pill_anchor) = render_topbar(
            ui,
            &workspace_name,
            self.stats.total_documents,
            self.is_busy,
            self.status_text.as_deref(),
            self.workspace_popover_open,
            self.show_telemetry,
        );
        merge_action(&mut pending_action, topbar_action);
        self.topbar_anchor_rect = pill_anchor;

        // 3. Left Icon Rail
        merge_action(
            &mut pending_action,
            render_left_rail(
                ui,
                self.view_mode,
                self.show_activity,
                self.show_semantic_links,
                self.show_telemetry,
            ),
        );

        // 4. Right Activity Streams Sidebar (collapsible)
        if self.show_activity {
            merge_action(
                &mut pending_action,
                render_activity_sidebar(
                    ui,
                    &self.snapshot.activity,
                    self.selected_node.as_deref(),
                ),
            );
        }

        // 5. Central Galaxy / Cluster Matrix Canvas
        let mut canvas_rect = Rect::NOTHING;
        let mut galaxy_action = GalaxyAction::None;

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(palette().bg_void))
            .show(ui, |ui| {
                canvas_rect = ui.max_rect();
                galaxy_action = match self.view_mode {
                    ViewMode::Workstation => {
                        let (g_act, p_act) = render_workstation(
                            ui,
                            canvas_rect,
                            &self.snapshot,
                            &self.stats,
                            &self.search_index,
                            self.selected_node.as_deref(),
                            self.active_category_filter.as_deref(),
                            &self.search_highlight_hashes,
                            self.active_detail.as_ref(),
                            &mut self.confirm_delete,
                            &self.search_query,
                            self.focus_search_requested,
                        );
                        self.focus_search_requested = false;
                        merge_action(&mut pending_action, p_act);
                        g_act
                    }
                    ViewMode::Galaxy => render_galaxy_canvas(
                        ui,
                        canvas_rect,
                        &self.snapshot,
                        &mut self.layout,
                        &mut self.camera,
                        self.selected_hub.as_deref(),
                        self.selected_node.as_deref(),
                        self.active_category_filter.as_deref(),
                        &self.search_highlight_hashes,
                        self.show_semantic_links,
                        time,
                    ),
                    ViewMode::ClusterGrid => render_cluster_grid(
                        ui,
                        canvas_rect,
                        &self.snapshot,
                        self.selected_node.as_deref(),
                        self.active_category_filter.as_deref(),
                    ),
                };

                if is_file_drag_hovering {
                    let p = palette();
                    ui.painter().rect_stroke(
                        canvas_rect.shrink(6.0),
                        CornerRadius::same(14),
                        Stroke::new(2.0, with_alpha(p.copper_bright, 210)),
                        StrokeKind::Inside,
                    );
                }
            });

        match galaxy_action {
            GalaxyAction::None => {}
            GalaxyAction::SelectHub(hub_id) => {
                merge_action(&mut pending_action, PanelAction::SelectHub(hub_id));
            }
            GalaxyAction::SelectNode(hash) => {
                self.select_node(hash, false);
            }
            GalaxyAction::ClearSelection => {
                self.selected_node = None;
                self.active_detail = None;
                self.confirm_delete = false;
                self.search_popover_open = false;
            }
        }

        // 6. Floating Overlays over Canvas
        if canvas_rect.is_positive() {
            if self.workspace_popover_open && self.topbar_anchor_rect.is_positive() {
                merge_action(
                    &mut pending_action,
                    render_workspace_switcher_popover(
                        &ctx,
                        self.topbar_anchor_rect,
                        &self.stats.input_folder,
                        &mut self.workspace_path_input,
                        &mut self.workspace_path_error,
                        &self.workspace_quick_folders,
                        &self.recent_workspaces,
                        self.is_picking_folder,
                    ),
                );
            }

            if self.view_mode != ViewMode::Workstation {
                merge_action(
                    &mut pending_action,
                    render_semantic_search_overlay(
                        &ctx,
                        canvas_rect,
                        &mut self.search_query,
                        &mut self.focus_search_requested,
                        &mut self.search_popover_open,
                        &self.search_results,
                        &self.snapshot.hubs,
                        self.active_category_filter.as_deref(),
                    ),
                );

                merge_action(
                    &mut pending_action,
                    render_drop_dock_overlay(
                        &ctx,
                        canvas_rect,
                        &self.snapshot.nodes,
                        self.dock_index,
                        is_file_drag_hovering,
                    ),
                );
            }

            if let Some(ref detail) = self.active_detail {
                if self.view_mode != ViewMode::Workstation {
                    merge_action(
                        &mut pending_action,
                        render_inspector_drawer(&ctx, canvas_rect, detail, &mut self.confirm_delete),
                    );
                }
            } else if self.show_telemetry {
                merge_action(
                    &mut pending_action,
                    render_telemetry_drawer(&ctx, canvas_rect, &self.stats),
                );
            }

            if let Some((ref err_msg, _)) = self.error_banner {
                let p = palette();
                let toast_w = 380.0_f32.min((canvas_rect.width() - 40.0).max(220.0));
                let toast_pos = Pos2::new(canvas_rect.center().x - toast_w * 0.5, canvas_rect.top() + 14.0);
                egui::Area::new(Id::unique("axel_error_toast"))
                    .order(Order::Foreground)
                    .fixed_pos(toast_pos)
                    .show(&ctx, |ui| {
                        Frame::new()
                            .fill(with_alpha(p.bg_surface_elevated, 246))
                            .stroke(Stroke::new(1.0, p.danger))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(egui::Margin::symmetric(14, 9))
                            .show(ui, |ui| {
                                ui.set_width(toast_w - 28.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("Error")
                                            .font(font_medium(11.5))
                                            .color(p.danger),
                                    );
                                    ui.label(
                                        egui::RichText::new(err_msg)
                                            .font(font_sans(11.5))
                                            .color(p.ink_primary),
                                    );
                                });
                            });
                    });
            }
        }

        if let Some(ref props) = self.properties_target {
            let prop_act = render_properties_modal(&ctx, props);
            merge_action(&mut pending_action, prop_act);
        }

        self.handle_panel_action(&ctx, pending_action);
        self.maybe_capture_screenshot(&ctx);

        if camera_animating || self.is_busy || self.screenshot_path.is_some() {
            ctx.request_repaint();
        } else if self.view_mode == ViewMode::Galaxy && !self.snapshot.edges.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
}

fn spawn_background_worker(
    initial_config: Config,
    mut cmd_rx: tokio_mpsc::UnboundedReceiver<WorkerCommand>,
    evt_tx: mpsc::Sender<WorkerEvent>,
    egui_ctx: egui::Context,
) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = evt_tx.send(WorkerEvent::Error(format!(
                    "Failed to start async runtime: {e}"
                )));
                egui_ctx.request_repaint();
                return;
            }
        };

        rt.block_on(async move {
            let default_input_folder = initial_config.input_folder.clone();
            let default_index_file = initial_config.index_file.clone();

            let mut config = initial_config;
            let mut index = DocumentIndex::load(&config.index_file);
            let ai = Arc::new(AiEngine::new(&config));

            let emit_snapshot = |cfg: &Config, idx: &DocumentIndex, status: Option<String>| {
                let items = idx.all().to_vec();
                let snapshot = build_graph_snapshot(&items);
                let stats = build_system_stats(cfg, idx);
                let _ = evt_tx.send(WorkerEvent::SnapshotUpdated {
                    snapshot,
                    stats,
                    items,
                    status,
                });
                egui_ctx.request_repaint();
            };

            // Fast-load extended attributes metadata (xattr) if index is empty
            if index.count() == 0 && config.input_folder.exists() {
                if let Ok(entries) = std::fs::read_dir(&config.input_folder) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(item) = FileMetadataTag::read_from_file(&path) {
                                index.upsert(item);
                            }
                        }
                    }
                }
            }

            // Emit cached/xattr index immediately so UI renders in <1ms without waiting for scan
            let initial_count = index.count();
            let initial_msg = if initial_count > 0 {
                Some(format!("{initial_count} document(s) loaded"))
            } else {
                Some("Ready".to_string())
            };
            emit_snapshot(&config, &index, initial_msg);

            // Channel for background delta scan completions
            let (scan_result_tx, mut scan_result_rx) =
                tokio_mpsc::unbounded_channel::<(PathBuf, ScanSummary, Vec<IndexItem>)>();

            let mut active_scan: Option<tokio::task::JoinHandle<()>> = None;

            // Spawn initial delta scan in the background without blocking the loop
            if config.input_folder.exists() {
                let scan_tx = scan_result_tx.clone();
                let ai_clone = Arc::clone(&ai);
                let folder = config.input_folder.clone();
                let existing_hashes: HashSet<String> =
                    index.all().iter().map(|i| i.hash.clone()).collect();
                active_scan = Some(tokio::spawn(async move {
                    let (summary, new_items) =
                        scan_folder_into_items(&folder, &existing_hashes, &ai_clone, false).await;
                    let _ = scan_tx.send((folder, summary, new_items));
                }));
            }

            let mut watcher = FolderWatcher::new(&config.input_folder).ok();
            let mut tick = tokio::time::interval(Duration::from_millis(450));

            loop {
                tokio::select! {
                    maybe_cmd = cmd_rx.recv() => {
                        let Some(cmd) = maybe_cmd else {
                            break;
                        };
                        match cmd {
                            WorkerCommand::Scan { force } => {
                                let _ = evt_tx.send(WorkerEvent::Busy(Some(if force {
                                    "Re-indexing workspace...".to_string()
                                } else {
                                    "Scanning workspace...".to_string()
                                })));
                                egui_ctx.request_repaint();

                                // Abort any in-flight scan
                                if let Some(handle) = active_scan.take() {
                                    handle.abort();
                                }

                                let scan_tx = scan_result_tx.clone();
                                let ai_clone = Arc::clone(&ai);
                                let folder = config.input_folder.clone();
                                let existing_hashes: HashSet<String> = if force {
                                    HashSet::new()
                                } else {
                                    index.all().iter().map(|i| i.hash.clone()).collect()
                                };
                                active_scan = Some(tokio::spawn(async move {
                                    let (summary, new_items) =
                                        scan_folder_into_items(&folder, &existing_hashes, &ai_clone, force).await;
                                    let _ = scan_tx.send((folder, summary, new_items));
                                }));
                            }
                            WorkerCommand::IngestFiles(paths) => {
                                let _ = evt_tx.send(WorkerEvent::Busy(Some(format!(
                                    "Ingesting {} file(s)...",
                                    paths.len()
                                ))));
                                egui_ctx.request_repaint();

                                match ingest_local_files(&paths, &config.input_folder, &ai, true).await {
                                    Ok(items) => {
                                        let count = items.len();
                                        for item in items {
                                            index.upsert(item);
                                        }
                                        if let Err(e) = index.save() {
                                            let _ = evt_tx.send(WorkerEvent::Error(format!("Failed to save index: {e}")));
                                        } else {
                                            emit_snapshot(
                                                &config,
                                                &index,
                                                Some(format!("Ingested {count} document(s)")),
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        let _ = evt_tx.send(WorkerEvent::Error(e));
                                        egui_ctx.request_repaint();
                                    }
                                }
                            }
                            WorkerCommand::SwitchWorkspace(new_dir) => {
                                // 1. Abort any running scan immediately
                                if let Some(handle) = active_scan.take() {
                                    handle.abort();
                                }

                                config.input_folder = new_dir.clone();
                                config.index_file = crate::config::workspace_index_path(
                                    &new_dir,
                                    &default_input_folder,
                                    &default_index_file,
                                );

                                // 2. Phase 1: Load index and fast-load file metadata (xattr) in <1ms
                                index = DocumentIndex::load(&config.index_file);
                                if new_dir.exists() {
                                    let known_paths: HashSet<String> =
                                        index.all().iter().map(|i| i.path.clone()).collect();
                                    if let Ok(entries) = std::fs::read_dir(&new_dir) {
                                        for entry in entries.flatten() {
                                            let path = entry.path();
                                            if path.is_file() && !known_paths.contains(&path.to_string_lossy().to_string()) {
                                                if let Some(item) = FileMetadataTag::read_from_file(&path) {
                                                    index.upsert(item);
                                                }
                                            }
                                        }
                                    }
                                }

                                watcher = FolderWatcher::new(&config.input_folder).ok();

                                let cached_count = index.count();
                                let initial_msg = if cached_count > 0 {
                                    Some(format!("{cached_count} document(s) ready"))
                                } else {
                                    Some(format!("Watching {}", new_dir.display()))
                                };
                                emit_snapshot(&config, &index, initial_msg);

                                // 3. Phase 2: Spawn asynchronous background delta scan without blocking loop
                                if config.input_folder.exists() {
                                    let scan_tx = scan_result_tx.clone();
                                    let ai_clone = Arc::clone(&ai);
                                    let folder = config.input_folder.clone();
                                    let existing_hashes: HashSet<String> =
                                        index.all().iter().map(|i| i.hash.clone()).collect();
                                    active_scan = Some(tokio::spawn(async move {
                                        let (summary, new_items) =
                                            scan_folder_into_items(&folder, &existing_hashes, &ai_clone, false).await;
                                        let _ = scan_tx.send((folder, summary, new_items));
                                    }));
                                }
                            }
                            WorkerCommand::LoadDetail(hash) => {
                                if let Some(item) = index.find_by_hash(&hash).cloned() {
                                    let detail = tokio::task::spawn_blocking(move || {
                                        build_document_detail(&item)
                                    })
                                    .await;
                                    if let Ok(detail) = detail {
                                        let _ = evt_tx.send(WorkerEvent::DetailLoaded(detail));
                                        egui_ctx.request_repaint();
                                    }
                                }
                            }
                            WorkerCommand::DeleteDocument { hash, delete_file } => {
                                match delete_document_from_index(&hash, delete_file, false, &config, &mut index) {
                                    Ok(outcome) => {
                                        emit_snapshot(
                                            &config,
                                            &index,
                                            Some(format!("Removed {}", outcome.file)),
                                        );
                                    }
                                    Err(e) => {
                                        let _ = evt_tx.send(WorkerEvent::Error(e));
                                        egui_ctx.request_repaint();
                                    }
                                }
                            }
                            WorkerCommand::TrashDuplicateFile {
                                canonical_hash,
                                trash_path,
                            } => {
                                let mut error_msg = None;
                                let mut success_msg = None;

                                if let Some(item) = index.find_by_hash(&canonical_hash).cloned() {
                                    let canonical_path = Path::new(&item.path);
                                    let tpath = Path::new(&trash_path);

                                    if !canonical_path.exists() {
                                        error_msg = Some(
                                            "Security abort: Canonical primary file is missing on disk. Refusing to delete duplicate."
                                                .to_string(),
                                        );
                                    } else if !tpath.exists() {
                                        error_msg = Some(format!(
                                            "Duplicate file does not exist: {}",
                                            trash_path
                                        ));
                                    } else if !crate::engine::is_path_inside_workspace(canonical_path, &config.input_folder)
                                        || !crate::engine::is_path_inside_workspace(tpath, &config.input_folder)
                                    {
                                        error_msg = Some(
                                            "Security abort: Both canonical and duplicate files must be within workspace boundaries."
                                                .to_string(),
                                        );
                                    } else if tpath == canonical_path {
                                        error_msg = Some(
                                            "Security abort: Attempted to delete the canonical primary file. Refusing to delete."
                                                .to_string(),
                                        );
                                    } else if let (Ok(c1), Ok(c2)) = (std::fs::canonicalize(canonical_path), std::fs::canonicalize(tpath)) {
                                        if c1 == c2 {
                                            error_msg = Some(
                                                "Security abort: Duplicate path resolves to the same filesystem inode as canonical file."
                                                    .to_string(),
                                            );
                                        } else if !item.physical_paths.iter().any(|p| p == &trash_path || Path::new(p) == tpath) {
                                            error_msg = Some(
                                                "Security abort: Target file is not a registered physical copy of this document."
                                                    .to_string(),
                                            );
                                        } else {
                                            // Verify that canonical file currently on disk still matches canonical hash (H04)
                                            match DocumentIndex::compute_file_hash(canonical_path) {
                                                Some(ref chash) if chash == &canonical_hash => {
                                                    // Verify bit-for-bit identity between candidate and canonical copy (H03, H04)
                                                    match crate::engine::are_files_identical(canonical_path, tpath) {
                                                        Ok(true) => {
                                                            match trash::delete(tpath) {
                                                                Ok(_) => {
                                                                    let fname = tpath
                                                                        .file_name()
                                                                        .and_then(|n| n.to_str())
                                                                        .unwrap_or("duplicate");
                                                                    success_msg = Some(format!(
                                                                        "Verified and moved duplicate {} to OS trash",
                                                                        fname
                                                                    ));
                                                                    index.remove_physical_copy(
                                                                        &canonical_hash,
                                                                        &trash_path,
                                                                    );
                                                                    let _ = index.save();
                                                                }
                                                                Err(e) => {
                                                                    error_msg = Some(format!(
                                                                        "Failed to move duplicate to trash: {}",
                                                                        e
                                                                    ));
                                                                }
                                                            }
                                                        }
                                                        Ok(false) => {
                                                            error_msg = Some(
                                                                "Security check failed: Duplicate file content does not match canonical file! Aborting trash to prevent data loss."
                                                                    .to_string(),
                                                            );
                                                        }
                                                        Err(e) => {
                                                            error_msg = Some(format!(
                                                                "Failed to verify duplicate content against canonical file: {e}"
                                                            ));
                                                        }
                                                    }
                                                }
                                                Some(_) => {
                                                    error_msg = Some(
                                                        "Security abort: Canonical primary file has changed on disk since indexing. Aborting trash to prevent data loss."
                                                            .to_string(),
                                                    );
                                                }
                                                None => {
                                                    error_msg = Some(
                                                        "Failed to read canonical file for verification."
                                                            .to_string(),
                                                    );
                                                }
                                            }
                                        }
                                    } else {
                                        error_msg = Some("Failed to resolve filesystem paths for security verification.".to_string());
                                    }
                                } else {
                                    error_msg = Some("Canonical item not found in index.".to_string());
                                }

                                if let Some(err) = error_msg {
                                    let _ = evt_tx.send(WorkerEvent::Error(err));
                                    egui_ctx.request_repaint();
                                } else if let Some(succ) = success_msg {
                                    emit_snapshot(&config, &index, Some(succ));
                                }
                            }
                            WorkerCommand::TrashAllDuplicates => {
                                let mut trashed_count = 0;
                                let mut bytes_saved = 0u64;
                                let mut errors = Vec::new();

                                let dup_items: Vec<(String, String, Vec<String>)> = index
                                    .items()
                                    .iter()
                                    .filter(|i| i.is_duplicate || i.physical_paths.len() > 1)
                                    .filter(|i| {
                                        // Safety shield: never batch-trash sensitive identity files;
                                        // user must review them individually in the inspector
                                        !crate::desktop::workstation::is_sensitive_item(
                                            &i.category,
                                            &i.file,
                                            &i.keywords,
                                            None,
                                        )
                                    })
                                    .map(|i| {
                                        (
                                            i.hash.clone(),
                                            i.path.clone(),
                                            i.physical_paths.clone(),
                                        )
                                    })
                                    .collect();

                                for (hash, canonical_path_str, physical_paths) in dup_items {
                                    let canonical_path = Path::new(&canonical_path_str);
                                    if !canonical_path.exists() {
                                        errors.push(format!(
                                            "Canonical missing for {}",
                                            canonical_path_str
                                        ));
                                        continue;
                                    }
                                    if !crate::engine::is_path_inside_workspace(canonical_path, &config.input_folder) {
                                        continue;
                                    }
                                    // Verify canonical file has not changed on disk (H04)
                                    if DocumentIndex::compute_file_hash(canonical_path).as_deref() != Some(&hash) {
                                        errors.push(format!(
                                            "Canonical content changed on disk for {}; skipping duplicate cleanup",
                                            canonical_path_str
                                        ));
                                        continue;
                                    }

                                    let canonical_canon = match std::fs::canonicalize(canonical_path) {
                                        Ok(c) => c,
                                        Err(_) => continue,
                                    };

                                    // Iterate over all physical paths, explicitly preserving the canonical file regardless of alias order (H04)
                                    for redundant in &physical_paths {
                                        let rpath = Path::new(redundant);
                                        if rpath == canonical_path {
                                            continue;
                                        }
                                        if let Ok(rcanon) = std::fs::canonicalize(rpath) {
                                            if rcanon == canonical_canon {
                                                continue;
                                            }
                                        }
                                        if !rpath.exists() {
                                            continue;
                                        }
                                        if !crate::engine::is_path_inside_workspace(rpath, &config.input_folder) {
                                            continue;
                                        }

                                        // Verify byte-for-byte identity with canonical file before trashing (H03, H04)
                                        match crate::engine::are_files_identical(canonical_path, rpath) {
                                            Ok(true) => {
                                                let size = std::fs::metadata(rpath)
                                                    .map(|m| m.len())
                                                    .unwrap_or(0);
                                                if trash::delete(rpath).is_ok() {
                                                    trashed_count += 1;
                                                    bytes_saved += size;
                                                    index.remove_physical_copy(&hash, redundant);
                                                }
                                            }
                                            _ => continue,
                                        }
                                    }
                                }
                                let _ = index.save();
                                if trashed_count > 0 {
                                    let mb = (bytes_saved as f64) / (1024.0 * 1024.0);
                                    emit_snapshot(
                                        &config,
                                        &index,
                                        Some(format!(
                                            "Batch deduplication: moved {} duplicate(s) to trash (reclaimed {:.1} MB)",
                                            trashed_count, mb
                                        )),
                                    );
                                } else if !errors.is_empty() {
                                    let _ = evt_tx.send(WorkerEvent::Error(errors.join("; ")));
                                    egui_ctx.request_repaint();
                                } else {
                                    emit_snapshot(
                                        &config,
                                        &index,
                                        Some("No redundant duplicates found to trash".to_string()),
                                    );
                                }
                            }
                            WorkerCommand::TrashFile(path_str) => {
                                let p = Path::new(&path_str);
                                match crate::engine::trash_file_safely(p) {
                                    Ok(_) => {
                                        if let Some(item) = index.find_by_path(&path_str).cloned() {
                                            let _ = delete_document_from_index(&item.hash, false, false, &config, &mut index);
                                        }
                                        let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                                        emit_snapshot(
                                            &config,
                                            &index,
                                            Some(format!("Moved {fname} to OS trash")),
                                        );
                                    }
                                    Err(e) => {
                                        let _ = evt_tx.send(WorkerEvent::Error(e));
                                        egui_ctx.request_repaint();
                                    }
                                }
                            }
                        }
                    }
                    Some((scanned_folder, summary, new_items)) = scan_result_rx.recv() => {
                        if scanned_folder == config.input_folder {
                            let mut changed = false;
                            for item in new_items {
                                index.upsert(item);
                                changed = true;
                            }
                            if changed {
                                let _ = index.save();
                                let msg = if summary.indexed > 0 {
                                    Some(format!("Indexed {} document(s)", summary.indexed))
                                } else {
                                    None
                                };
                                emit_snapshot(&config, &index, msg);
                            } else {
                                let _ = evt_tx.send(WorkerEvent::Busy(None));
                                egui_ctx.request_repaint();
                            }
                        }
                    }
                    _ = tick.tick() => {
                        if let Some(ref w) = watcher {
                            let mut changed = false;
                            while let Some(path) = w.poll_next_file() {
                                if let Some(hash) = DocumentIndex::compute_file_hash(&path) {
                                    if index.find_by_hash(&hash).is_some() {
                                        continue;
                                    }
                                }
                                if let Some(item) = analyze_and_build_item(&path, &ai).await {
                                    index.upsert(item);
                                    changed = true;
                                }
                            }
                            if changed {
                                let _ = index.save();
                                emit_snapshot(&config, &index, Some("Live watcher indexed update".to_string()));
                            }
                        }
                    }
                }
            }
        });
    });
}

pub fn run_desktop_app(
    config: Config,
    screenshot_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AXEL — Semantic Document Intelligence")
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([920.0, 620.0])
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "AXEL",
        native_options,
        Box::new(move |cc| Ok(Box::new(AxelDesktopApp::new(cc, config, screenshot_path)))),
    )
    .map_err(|e| format!("Desktop UI error: {e}").into())
}

fn save_color_image(image: &ColorImage, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create screenshot directory: {e}"))?;
        }
    }

    let width = image.size[0] as u32;
    let height = image.size[1] as u32;
    let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
    for px in &image.pixels {
        rgba.extend_from_slice(&px.to_srgba_unmultiplied());
    }

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext == "ppm" {
        let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
        for chunk in rgba.chunks_exact(4) {
            out.extend_from_slice(&chunk[..3]);
        }
        std::fs::write(path, out).map_err(|e| format!("Failed to write PPM: {e}"))
    } else {
        let png = encode_uncompressed_rgba_png(width, height, &rgba);
        std::fs::write(path, png).map_err(|e| format!("Failed to write PNG: {e}"))
    }
}

fn encode_uncompressed_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row_bytes = (width as usize) * 4 + 1;
    let mut raw = Vec::with_capacity(row_bytes * (height as usize));
    for y in 0..(height as usize) {
        raw.push(0); // Filter type 0 (None)
        let start = y * (width as usize) * 4;
        let end = start + (width as usize) * 4;
        raw.extend_from_slice(&rgba[start..end]);
    }

    // Zlib stream with uncompressed DEFLATE stored blocks (max 65535 bytes per block)
    let mut zlib = Vec::with_capacity(raw.len() + 64);
    zlib.push(0x78);
    zlib.push(0x01);
    let mut offset = 0;
    while offset < raw.len() {
        let chunk_len = (raw.len() - offset).min(65535);
        let is_final = offset + chunk_len >= raw.len();
        zlib.push(if is_final { 0x01 } else { 0x00 });
        let len_u16 = chunk_len as u16;
        let nlen_u16 = !len_u16;
        zlib.extend_from_slice(&len_u16.to_le_bytes());
        zlib.extend_from_slice(&nlen_u16.to_le_bytes());
        zlib.extend_from_slice(&raw[offset..offset + chunk_len]);
        offset += chunk_len;
    }
    let adler = adler32(&raw);
    zlib.extend_from_slice(&adler.to_be_bytes());

    let mut png = Vec::with_capacity(zlib.len() + 64);
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &zlib);
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}
