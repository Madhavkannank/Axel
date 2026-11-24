use crate::desktop::galaxy::truncate_label;
use crate::desktop::theme::{
    accent_bright, accent_color, active_theme, draw_icon_activity, draw_icon_chevron_down,
    draw_icon_chevron_right, draw_icon_close, draw_icon_crosshair, draw_icon_file_type,
    draw_icon_folder, draw_icon_galaxy, draw_icon_gear, draw_icon_grid, draw_icon_links,
    draw_icon_maximize, draw_icon_minimize, draw_icon_palette, draw_icon_refresh,
    draw_icon_search, draw_icon_upload, draw_icon_workstation, font_medium, font_mono, font_sans,
    font_semibold, lerp_color, palette, with_alpha, ThemePreset,
};
use crate::engine::ActivityEntry;
use chrono::{DateTime, Utc};
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Frame, Layout, Margin, Pos2, Rect,
    ScrollArea, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Workstation,
    Galaxy,
    ClusterGrid,
}

#[derive(Debug, Clone)]
pub enum PanelAction {
    None,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
    StartWindowDrag,
    SetViewMode(ViewMode),
    FocusSearch,
    TriggerScan { force: bool },
    PickFilesToIngest,
    ToggleWorkspacePopover,
    CloseWorkspacePopover,
    SwitchWorkspacePath(PathBuf),
    BrowseWorkspaceFolder,
    ToggleActivitySidebar,
    ToggleSemanticLinks,
    CycleTheme,
    SetTheme(ThemePreset),
    ResetCamera,
    ToggleTelemetryDrawer,
    SelectNode(String),
    SelectHub(String),
    SetCategoryFilter(Option<String>),
    SearchQueryChanged(String),
    AppendSearchToken(String),
    CloseSearchPopover,
    OpenFileExternal(String),
    RevealFolderExternal(String),
    DeleteDocument { hash: String, delete_file: bool },
    TrashDuplicateFile { canonical_hash: String, trash_path: String },
    TrashAllDuplicates,
    CloseInspector,
    CloseTelemetry,
    CycleDropDock(isize),
    SelectDropDockIndex(usize),
    // File Manager capabilities
    CopyPath(String),
    CopyName(String),
    CopyFileUri(String),
    TrashFile(String),
    OpenProperties(String),
    CloseProperties,
}

pub fn merge_action(current: &mut PanelAction, next: PanelAction) {
    if !matches!(next, PanelAction::None) {
        *current = next;
    }
}

pub fn render_titlebar(ui: &mut Ui, workspace_name: &str, is_maximized: bool) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();

    egui::Panel::top("axel_titlebar")
        .exact_size(34.0)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(p.bg_chrome)
                .inner_margin(Margin::symmetric(12, 0)),
        )
        .show(ui, |ui| {
            let bar_rect = ui.max_rect();
            ui.painter().line_segment(
                [
                    Pos2::new(bar_rect.left(), bar_rect.bottom()),
                    Pos2::new(bar_rect.right(), bar_rect.bottom()),
                ],
                Stroke::new(1.0, p.border_subtle),
            );

            ui.horizontal_centered(|ui| {
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                ui.painter()
                    .circle_filled(dot_rect.center(), 4.5, p.copper);
                ui.painter().circle_stroke(
                    dot_rect.center(),
                    6.5,
                    Stroke::new(1.0, with_alpha(p.indigo_bright, 150)),
                );

                ui.label(
                    egui::RichText::new("AXEL")
                        .font(font_semibold(12.5))
                        .color(p.ink_primary),
                );
                ui.label(
                    egui::RichText::new(format!("·  {workspace_name}"))
                        .font(font_sans(11.5))
                        .color(p.ink_muted),
                );

                let avail_w = (ui.available_width() - 108.0).max(24.0);
                let (drag_rect, drag_resp) =
                    ui.allocate_exact_size(Vec2::new(avail_w, 30.0), Sense::click_and_drag());
                let _ = drag_rect;
                if drag_resp.drag_started() {
                    action = PanelAction::StartWindowDrag;
                } else if drag_resp.double_clicked() {
                    action = PanelAction::ToggleMaximizeWindow;
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    if window_ctrl_btn(ui, "Close", true, |painter, c, col| {
                        draw_icon_close(painter, c, 12.0, col);
                    }) {
                        action = PanelAction::CloseWindow;
                    }

                    if window_ctrl_btn(
                        ui,
                        if is_maximized { "Restore" } else { "Maximize" },
                        false,
                        |painter, c, col| {
                            draw_icon_maximize(painter, c, 12.0, col);
                        },
                    ) {
                        action = PanelAction::ToggleMaximizeWindow;
                    }

                    if window_ctrl_btn(ui, "Minimize", false, |painter, c, col| {
                        draw_icon_minimize(painter, c, 12.0, col);
                    }) {
                        action = PanelAction::MinimizeWindow;
                    }
                });
            });
        });

    action
}

fn window_ctrl_btn(
    ui: &mut Ui,
    tooltip: &str,
    is_close: bool,
    draw_icon: impl FnOnce(&egui::Painter, Pos2, Color32),
) -> bool {
    let p = palette();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(30.0, 24.0), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        let fill = if is_close {
            with_alpha(p.danger, 55)
        } else {
            p.bg_surface_hover
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    }
    let icon_col = if resp.hovered() && is_close {
        if p.is_light {
            p.danger
        } else {
            Color32::from_rgb(255, 180, 186)
        }
    } else if resp.hovered() {
        p.ink_primary
    } else {
        p.ink_muted
    };
    draw_icon(ui.painter(), rect.center(), icon_col);
    resp.on_hover_text(tooltip).clicked()
}

pub fn render_topbar(
    ui: &mut Ui,
    workspace_name: &str,
    doc_count: usize,
    is_busy: bool,
    status_text: Option<&str>,
    workspace_popover_open: bool,
    show_telemetry: bool,
) -> (PanelAction, Rect) {
    let mut action = PanelAction::None;
    let mut pill_anchor = Rect::NOTHING;
    let p = palette();

    egui::Panel::top("axel_topbar")
        .exact_size(48.0)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(p.bg_chrome)
                .inner_margin(Margin::symmetric(12, 7)),
        )
        .show(ui, |ui| {
            let bar_rect = ui.max_rect();
            ui.painter().line_segment(
                [
                    Pos2::new(bar_rect.left(), bar_rect.bottom()),
                    Pos2::new(bar_rect.right(), bar_rect.bottom()),
                ],
                Stroke::new(1.0, p.border_subtle),
            );

            ui.horizontal_centered(|ui| {
                // Workspace selector pill (opens Workspace Switcher Popover)
                let pill_w = 254.0;
                let (pill_rect, pill_resp) =
                    ui.allocate_exact_size(Vec2::new(pill_w, 32.0), Sense::click());
                pill_anchor = pill_rect;

                let pill_fill = if workspace_popover_open {
                    lerp_color(p.bg_surface_elevated, p.copper, 0.14)
                } else if pill_resp.hovered() {
                    p.bg_surface_hover
                } else {
                    p.bg_surface
                };
                let pill_stroke = if workspace_popover_open {
                    p.copper
                } else if pill_resp.hovered() {
                    p.border_strong
                } else {
                    p.border_subtle
                };
                ui.painter().rect_filled(pill_rect, CornerRadius::same(9), pill_fill);
                ui.painter().rect_stroke(
                    pill_rect,
                    CornerRadius::same(9),
                    Stroke::new(1.0, pill_stroke),
                    StrokeKind::Inside,
                );
                draw_icon_folder(
                    ui.painter(),
                    Pos2::new(pill_rect.left() + 16.0, pill_rect.center().y),
                    14.0,
                    p.copper_bright,
                );
                ui.painter().text(
                    Pos2::new(pill_rect.left() + 31.0, pill_rect.center().y),
                    Align2::LEFT_CENTER,
                    truncate_label(workspace_name, 23),
                    font_medium(12.0),
                    p.ink_primary,
                );
                draw_icon_chevron_down(
                    ui.painter(),
                    Pos2::new(pill_rect.right() - 14.0, pill_rect.center().y),
                    12.0,
                    if workspace_popover_open {
                        p.copper_bright
                    } else {
                        p.ink_muted
                    },
                );
                if pill_resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if pill_resp
                    .on_hover_text("Switch workspace directory")
                    .clicked()
                {
                    action = PanelAction::ToggleWorkspacePopover;
                }

                ui.add_space(10.0);

                let status_label = if is_busy {
                    status_text.unwrap_or("Indexing documents...").to_string()
                } else if let Some(msg) = status_text {
                    msg.to_string()
                } else {
                    format!("{doc_count} indexed")
                };
                let dot_col = if is_busy { p.copper_bright } else { p.emerald };
                let (st_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(st_rect.center(), 3.5, dot_col);
                ui.label(
                    egui::RichText::new(status_label)
                        .font(font_mono(11.0))
                        .color(if is_busy { p.copper_bright } else { p.ink_muted }),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if top_icon_btn(
                        ui,
                        "System Telemetry & Settings",
                        show_telemetry,
                        |painter, c, col| draw_icon_gear(painter, c, 16.0, col),
                    ) {
                        action = PanelAction::ToggleTelemetryDrawer;
                    }

                    let theme_tip = format!("Theme: {} (click to switch)", active_theme().label());
                    if top_icon_btn(ui, &theme_tip, false, |painter, c, col| {
                        draw_icon_palette(painter, c, 16.0, col);
                    }) {
                        action = PanelAction::CycleTheme;
                    }

                    // Ingest / Upload button
                    let (up_rect, up_resp) =
                        ui.allocate_exact_size(Vec2::new(90.0, 30.0), Sense::click());
                    let up_fill = if up_resp.hovered() {
                        with_alpha(p.indigo, 52)
                    } else {
                        p.bg_surface
                    };
                    let up_stroke = if up_resp.hovered() {
                        p.indigo_bright
                    } else {
                        p.border_subtle
                    };
                    ui.painter()
                        .rect_filled(up_rect, CornerRadius::same(8), up_fill);
                    ui.painter().rect_stroke(
                        up_rect,
                        CornerRadius::same(8),
                        Stroke::new(1.0, up_stroke),
                        StrokeKind::Inside,
                    );
                    draw_icon_upload(
                        ui.painter(),
                        Pos2::new(up_rect.left() + 16.0, up_rect.center().y),
                        14.0,
                        p.indigo_bright,
                    );
                    ui.painter().text(
                        Pos2::new(up_rect.left() + 31.0, up_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Ingest",
                        font_medium(12.0),
                        p.ink_primary,
                    );
                    if up_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if up_resp
                        .on_hover_text("Add PDF, DOCX, image, code, or text files")
                        .clicked()
                    {
                        action = PanelAction::PickFilesToIngest;
                    }
                });
            });
        });

    (action, pill_anchor)
}

fn top_icon_btn(
    ui: &mut Ui,
    tooltip: &str,
    active: bool,
    draw_icon: impl FnOnce(&egui::Painter, Pos2, Color32),
) -> bool {
    let p = palette();
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let fill = if active {
        with_alpha(p.copper, 34)
    } else if resp.hovered() {
        p.bg_surface_hover
    } else {
        p.bg_surface
    };
    let stroke = if active {
        p.copper
    } else if resp.hovered() {
        p.border_strong
    } else {
        p.border_subtle
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    let col = if active {
        p.copper_bright
    } else if resp.hovered() {
        p.ink_primary
    } else {
        p.ink_secondary
    };
    draw_icon(ui.painter(), rect.center(), col);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.on_hover_text(tooltip).clicked()
}

pub fn render_left_rail(
    ui: &mut Ui,
    view_mode: ViewMode,
    show_activity: bool,
    show_semantic_links: bool,
    _show_telemetry: bool,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();

    egui::Panel::left("axel_left_rail")
        .exact_size(54.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(p.bg_chrome)
                .inner_margin(Margin::symmetric(8, 12)),
        )
        .show(ui, |ui| {
            let rail_rect = ui.max_rect();
            ui.painter().line_segment(
                [
                    Pos2::new(rail_rect.right(), rail_rect.top()),
                    Pos2::new(rail_rect.right(), rail_rect.bottom()),
                ],
                Stroke::new(1.0, p.border_subtle),
            );

            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 10.0;

                if rail_btn(
                    ui,
                    "Workstation (3-Column Ledger)",
                    view_mode == ViewMode::Workstation,
                    true,
                    |painter, c, col| draw_icon_workstation(painter, c, 18.0, col),
                ) {
                    action = PanelAction::SetViewMode(ViewMode::Workstation);
                }

                if rail_btn(
                    ui,
                    "Galaxy Constellation",
                    view_mode == ViewMode::Galaxy,
                    true,
                    |painter, c, col| draw_icon_galaxy(painter, c, 18.0, col),
                ) {
                    action = PanelAction::SetViewMode(ViewMode::Galaxy);
                }

                if rail_btn(ui, "Semantic Search (Ctrl+K)", false, false, |painter, c, col| {
                    draw_icon_search(painter, c, 17.0, col);
                }) {
                    action = PanelAction::FocusSearch;
                }

                if rail_btn(
                    ui,
                    "Cluster Matrix",
                    view_mode == ViewMode::ClusterGrid,
                    true,
                    |painter, c, col| draw_icon_grid(painter, c, 16.0, col),
                ) {
                    action = PanelAction::SetViewMode(ViewMode::ClusterGrid);
                }

                // Show semantic link toggle only in 3D Galaxy view mode where it takes visual effect
                if view_mode == ViewMode::Galaxy {
                    if rail_btn(
                        ui,
                        if show_semantic_links {
                            "Hide Cross-File Semantic Links"
                        } else {
                            "Show Cross-File Semantic Links"
                        },
                        show_semantic_links,
                        false,
                        |painter, c, col| draw_icon_links(painter, c, 16.0, col),
                    ) {
                        action = PanelAction::ToggleSemanticLinks;
                    }
                }

                if rail_btn(ui, "Rescan Workspace (Ctrl+R)", false, false, |painter, c, col| {
                    draw_icon_refresh(painter, c, 16.0, col);
                }) {
                    action = PanelAction::TriggerScan { force: false };
                }

                let bottom_reserve = if view_mode == ViewMode::Galaxy { 96.0 } else { 54.0 };
                let spacer = (ui.available_height() - bottom_reserve).max(12.0);
                ui.add_space(spacer);

                // 3D Galaxy camera reset only visible in Galaxy mode
                if view_mode == ViewMode::Galaxy {
                    if rail_btn(ui, "Reset Camera (0)", false, false, |painter, c, col| {
                        draw_icon_crosshair(painter, c, 16.0, col);
                    }) {
                        action = PanelAction::ResetCamera;
                    }
                }

                if rail_btn(
                    ui,
                    "Activity Streams",
                    show_activity,
                    false,
                    |painter, c, col| draw_icon_activity(painter, c, 16.0, col),
                ) {
                    action = PanelAction::ToggleActivitySidebar;
                }
            });
        });

    action
}

fn rail_btn(
    ui: &mut Ui,
    tooltip: &str,
    active: bool,
    warm_active: bool,
    draw_icon: impl FnOnce(&egui::Painter, Pos2, Color32),
) -> bool {
    let p = palette();
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
    let accent = if warm_active { p.copper } else { p.indigo };
    let bright = if warm_active {
        p.copper_bright
    } else {
        p.indigo_bright
    };

    if active {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(10), with_alpha(accent, 34));
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(1.2, with_alpha(accent, 220)),
            StrokeKind::Inside,
        );
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(10), p.bg_surface_hover);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(1.0, p.border_strong),
            StrokeKind::Inside,
        );
    }

    let col = if active {
        bright
    } else if resp.hovered() {
        p.ink_primary
    } else {
        p.ink_muted
    };
    draw_icon(ui.painter(), rect.center(), col);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.on_hover_text(tooltip).clicked()
}

pub fn render_activity_sidebar(
    ui: &mut Ui,
    activity: &[ActivityEntry],
    selected_node: Option<&str>,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();

    egui::Panel::right("axel_activity_sidebar")
        .exact_size(272.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(p.bg_chrome)
                .inner_margin(Margin::symmetric(14, 14)),
        )
        .show(ui, |ui| {
            let side_rect = ui.max_rect();
            ui.painter().line_segment(
                [
                    Pos2::new(side_rect.left(), side_rect.top()),
                    Pos2::new(side_rect.left(), side_rect.bottom()),
                ],
                Stroke::new(1.0, p.border_subtle),
            );

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Activity streams")
                        .font(font_semibold(13.5))
                        .color(p.ink_primary),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (btn_rect, btn_resp) =
                        ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                    if btn_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        ui.painter()
                            .rect_filled(btn_rect, CornerRadius::same(6), p.bg_surface_hover);
                    }
                    draw_icon_chevron_right(
                        ui.painter(),
                        btn_rect.center(),
                        12.0,
                        if btn_resp.hovered() {
                            p.ink_primary
                        } else {
                            p.ink_muted
                        },
                    );
                    if btn_resp.on_hover_text("Collapse sidebar").clicked() {
                        action = PanelAction::ToggleActivitySidebar;
                    }
                });
            });

            ui.add_space(12.0);

            if activity.is_empty() {
                ui.label(
                    egui::RichText::new("No indexed activity yet.")
                        .font(font_sans(12.0))
                        .color(p.ink_muted),
                );
                return;
            }

            let now = Utc::now();
            let mut today_entries = Vec::new();
            let mut earlier_entries = Vec::new();
            for entry in activity {
                if is_recent_24h(entry.indexed_at.as_deref(), now) {
                    today_entries.push(entry);
                } else {
                    earlier_entries.push(entry);
                }
            }

            ScrollArea::vertical()
                .id_salt("activity_stream_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if !today_entries.is_empty() {
                        timeline_section_header(ui, "Today", p.copper_bright);
                        ui.add_space(6.0);
                        for (idx, entry) in today_entries.iter().enumerate() {
                            let is_sel = selected_node == Some(entry.hash.as_str())
                                || (selected_node.is_none() && idx == 0);
                            if activity_card(ui, entry, is_sel, now) {
                                action = PanelAction::SelectNode(entry.hash.clone());
                            }
                            ui.add_space(8.0);
                        }
                    }

                    if !earlier_entries.is_empty() {
                        if !today_entries.is_empty() {
                            ui.add_space(6.0);
                        }
                        timeline_section_header(ui, "Past timelines", p.indigo_bright);
                        ui.add_space(6.0);
                        for entry in earlier_entries {
                            let is_sel = selected_node == Some(entry.hash.as_str());
                            if activity_card(ui, entry, is_sel, now) {
                                action = PanelAction::SelectNode(entry.hash.clone());
                            }
                            ui.add_space(8.0);
                        }
                    }
                });
        });

    action
}

fn timeline_section_header(ui: &mut Ui, title: &str, dot_color: Color32) {
    let p = palette();
    ui.horizontal(|ui| {
        let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        ui.painter()
            .circle_filled(dot_rect.center(), 3.5, dot_color);
        ui.painter().circle_stroke(
            dot_rect.center(),
            5.5,
            Stroke::new(1.0, with_alpha(dot_color, 95)),
        );
        ui.label(
            egui::RichText::new(title)
                .font(font_medium(11.5))
                .color(p.ink_secondary),
        );
    });
}

fn activity_card(
    ui: &mut Ui,
    entry: &ActivityEntry,
    highlighted: bool,
    now: DateTime<Utc>,
) -> bool {
    let p = palette();
    let w = ui.available_width();
    let h = 58.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());

    let accent_name = crate::engine::category_accent(&entry.category);
    let accent = accent_color(accent_name);
    let bright = accent_bright(accent_name);

    if highlighted || resp.hovered() {
        let left_col = with_alpha(lerp_color(p.bg_surface_elevated, p.indigo, 0.30), 245);
        let right_col = with_alpha(lerp_color(p.bg_surface_elevated, p.copper, 0.42), 245);
        paint_horizontal_gradient_rounded_rect(ui.painter(), rect, 10.0, left_col, right_col);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(
                1.15,
                with_alpha(p.copper_bright, if highlighted { 210 } else { 150 }),
            ),
            StrokeKind::Inside,
        );
    } else {
        ui.painter().rect_filled(rect, CornerRadius::same(10), p.bg_surface);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(1.0, p.border_subtle),
            StrokeKind::Inside,
        );
    }

    let badge_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 10.0, rect.center().y - 15.0),
        Vec2::splat(30.0),
    );
    ui.painter().rect_filled(
        badge_rect,
        CornerRadius::same(8),
        with_alpha(accent, if highlighted { 68 } else { 40 }),
    );
    draw_icon_file_type(
        ui.painter(),
        badge_rect.center(),
        16.0,
        &entry.file_type,
        bright,
    );

    let text_x = badge_rect.right() + 10.0;
    ui.painter().text(
        Pos2::new(text_x, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        truncate_label(&entry.file, 21),
        font_medium(12.0),
        p.ink_primary,
    );

    let rel = format_relative_time(entry.indexed_at.as_deref(), now);
    ui.painter().text(
        Pos2::new(text_x, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        format!("{rel} · {}", entry.category),
        font_sans(10.5),
        if highlighted {
            p.ink_secondary
        } else {
            p.ink_muted
        },
    );

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn is_recent_24h(ts: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(raw) = ts else {
        return false;
    };
    let Ok(dt) = DateTime::parse_from_rfc3339(raw) else {
        return false;
    };
    let secs = (now - dt.with_timezone(&Utc)).num_seconds();
    secs <= 86_400
}

pub fn format_relative_time(ts: Option<&str>, now: DateTime<Utc>) -> String {
    let Some(raw) = ts else {
        return "Indexed".to_string();
    };
    let Ok(dt) = DateTime::parse_from_rfc3339(raw) else {
        return "Indexed".to_string();
    };
    let secs = (now - dt.with_timezone(&Utc)).num_seconds().max(0);
    if secs < 60 {
        "Just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

fn paint_horizontal_gradient_rounded_rect(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    left: Color32,
    right: Color32,
) {
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5);
    let w = rect.width().max(1.0);
    let mut mesh = Mesh::default();

    let center = rect.center();
    mesh.vertices.push(Vertex {
        pos: center,
        uv: WHITE_UV,
        color: lerp_color(left, right, 0.5),
    });

    let corners = [
        (
            Pos2::new(rect.right() - r, rect.bottom() - r),
            0.0_f32,
            std::f32::consts::FRAC_PI_2,
        ),
        (
            Pos2::new(rect.left() + r, rect.bottom() - r),
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ),
        (
            Pos2::new(rect.left() + r, rect.top() + r),
            std::f32::consts::PI,
            std::f32::consts::PI * 1.5,
        ),
        (
            Pos2::new(rect.right() - r, rect.top() + r),
            std::f32::consts::PI * 1.5,
            std::f32::consts::TAU,
        ),
    ];

    let segs = 6;
    for (c, a0, a1) in corners {
        for s in 0..=segs {
            let a = a0 + (a1 - a0) * (s as f32 / segs as f32);
            let pos = Pos2::new(c.x + a.cos() * r, c.y + a.sin() * r);
            let t = ((pos.x - rect.left()) / w).clamp(0.0, 1.0);
            mesh.vertices.push(Vertex {
                pos,
                uv: WHITE_UV,
                color: lerp_color(left, right, t),
            });
        }
    }

    let ring_len = (mesh.vertices.len() - 1) as u32;
    for i in 0..ring_len {
        let next = (i + 1) % ring_len;
        mesh.indices.extend_from_slice(&[0, 1 + i, 1 + next]);
    }

    painter.add(Shape::mesh(mesh));
}
