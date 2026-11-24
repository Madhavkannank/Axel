use crate::config::resolve_workspace_path;
use crate::desktop::chrome::{format_relative_time, merge_action, PanelAction};
use crate::desktop::galaxy::{format_bytes, truncate_label};
use crate::desktop::theme::{
    accent_bright, accent_color, active_theme, draw_icon_chevron_left, draw_icon_chevron_right,
    draw_icon_close, draw_icon_file_type, draw_icon_folder, draw_icon_refresh, draw_icon_search,
    draw_icon_upload, font_medium, font_mono, font_sans, font_semibold, lerp_color,
    paint_soft_shadow, palette, with_alpha, ThemePreset,
};
use crate::engine::{category_accent, DocumentDetail, GraphHub, GraphNode, SystemStats};
use crate::index::SearchResult;
use chrono::Utc;
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Frame, Id, Key, Layout, Margin, Order, Pos2,
    Rect, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub fn render_workspace_switcher_popover(
    ctx: &egui::Context,
    anchor_rect: Rect,
    current_folder: &str,
    path_input: &mut String,
    path_error: &mut Option<String>,
    quick_folders: &[(String, PathBuf)],
    recent_folders: &[PathBuf],
    is_picking_folder: bool,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();

    let pop_w = 410.0;
    let pos = Pos2::new(
        anchor_rect.left().max(12.0),
        (anchor_rect.bottom() + 8.0).max(48.0),
    );

    egui::Area::new(Id::unique("axel_workspace_switcher_popover"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let frame_resp = Frame::new()
                .fill(with_alpha(p.bg_chrome, 250))
                .stroke(Stroke::new(1.0, p.border_strong))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(pop_w - 28.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Switch Workspace Directory")
                                .font(font_semibold(13.5))
                                .color(p.ink_primary),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (c_rect, c_resp) =
                                ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                            if c_resp.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                ui.painter().rect_filled(
                                    c_rect,
                                    CornerRadius::same(6),
                                    p.bg_surface_hover,
                                );
                            }
                            draw_icon_close(
                                ui.painter(),
                                c_rect.center(),
                                10.0,
                                if c_resp.hovered() {
                                    p.ink_primary
                                } else {
                                    p.ink_muted
                                },
                            );
                            if c_resp.clicked() {
                                action = PanelAction::CloseWorkspacePopover;
                            }
                        });
                    });

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(truncate_label(current_folder, 52))
                            .font(font_mono(10.5))
                            .color(p.ink_muted),
                    );

                    ui.add_space(10.0);

                    // Direct path input + Open button
                    ui.horizontal(|ui| {
                        let input_w = (ui.available_width() - 74.0).max(160.0);
                        let edit_resp = ui.add_sized(
                            Vec2::new(input_w, 30.0),
                            TextEdit::singleline(path_input)
                                .hint_text("Enter folder path (e.g. ~/Downloads)")
                                .font(font_mono(11.5))
                                .text_color(p.ink_primary),
                        );
                        if edit_resp.changed() {
                            *path_error = None;
                        }
                        let enter_pressed =
                            edit_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

                        if action_pill_btn(ui, "Open", p.copper, true) || enter_pressed {
                            let trimmed = path_input.trim();
                            if trimmed.is_empty() {
                                *path_error = Some("Enter a folder path or pick one below.".to_string());
                            } else {
                                let resolved = resolve_workspace_path(Path::new(trimmed));
                                if !resolved.exists() {
                                    *path_error = Some(format!(
                                        "Folder not found: {}",
                                        resolved.display()
                                    ));
                                } else if !resolved.is_dir() {
                                    *path_error = Some(format!(
                                        "Path is a file, not a folder: {}",
                                        resolved.display()
                                    ));
                                } else {
                                    *path_error = None;
                                    action = PanelAction::SwitchWorkspacePath(resolved);
                                }
                            }
                        }
                    });

                    if let Some(ref err) = path_error {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(err)
                                .font(font_sans(11.0))
                                .color(p.danger),
                        );
                    }

                    ui.add_space(10.0);

                    // Quick Folders
                    if !quick_folders.is_empty() {
                        ui.label(
                            egui::RichText::new("Quick Folders")
                                .font(font_medium(11.0))
                                .color(p.ink_muted),
                        );
                        ui.add_space(5.0);
                        for (label, folder) in quick_folders {
                            let is_cur = folder.display().to_string() == current_folder;
                            if workspace_row_btn(ui, label, &folder.display().to_string(), is_cur) {
                                action = PanelAction::SwitchWorkspacePath(folder.clone());
                            }
                            ui.add_space(4.0);
                        }
                    }

                    // Recent Workspaces
                    let extra_recent: Vec<&PathBuf> = recent_folders
                        .iter()
                        .filter(|rf| !quick_folders.iter().any(|(_, qf)| qf == *rf))
                        .take(4)
                        .collect();
                    if !extra_recent.is_empty() {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Recent Workspaces")
                                .font(font_medium(11.0))
                                .color(p.ink_muted),
                        );
                        ui.add_space(5.0);
                        for folder in extra_recent {
                            let name = folder
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Workspace");
                            let is_cur = folder.display().to_string() == current_folder;
                            if workspace_row_btn(ui, name, &folder.display().to_string(), is_cur) {
                                action = PanelAction::SwitchWorkspacePath(folder.clone());
                            }
                            ui.add_space(4.0);
                        }
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let browse_label = if is_picking_folder {
                            "Opening Folder Picker..."
                        } else {
                            "Browse System Folder..."
                        };
                        if action_pill_btn(ui, browse_label, p.indigo, !is_picking_folder) {
                            action = PanelAction::BrowseWorkspaceFolder;
                        }
                    });
                });

            let _ = frame_resp;
        });

    action
}

fn workspace_row_btn(ui: &mut Ui, title: &str, path_str: &str, active: bool) -> bool {
    let p = palette();
    let w = ui.available_width();
    let h = 38.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());

    let fill = if active {
        with_alpha(p.copper, 32)
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

    draw_icon_folder(
        ui.painter(),
        Pos2::new(rect.left() + 16.0, rect.center().y),
        14.0,
        if active { p.copper_bright } else { p.indigo_bright },
    );
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y - 6.5),
        Align2::LEFT_CENTER,
        truncate_label(title, 28),
        font_medium(11.5),
        p.ink_primary,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y + 7.5),
        Align2::LEFT_CENTER,
        truncate_label(path_str, 44),
        font_mono(9.5),
        p.ink_muted,
    );

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

#[allow(clippy::too_many_arguments)]
pub fn render_semantic_search_overlay(
    ctx: &egui::Context,
    canvas_rect: Rect,
    search_query: &mut String,
    focus_requested: &mut bool,
    search_popover_open: &mut bool,
    search_results: &[SearchResult],
    hubs: &[GraphHub],
    active_category_filter: Option<&str>,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();
    if canvas_rect.width() < 320.0 || canvas_rect.height() < 180.0 {
        return action;
    }

    let dock_reserved_w = if canvas_rect.width() >= 720.0 {
        308.0
    } else {
        0.0
    };
    let max_bar_w = (canvas_rect.width() - dock_reserved_w - 40.0).clamp(280.0, 500.0);
    let bar_h = 42.0;
    let chips_h = 30.0;
    let total_h = bar_h + 10.0 + chips_h;

    let usable_center_x = canvas_rect.left() + (canvas_rect.width() - dock_reserved_w) * 0.5;
    let max_right = canvas_rect.right() - dock_reserved_w - 18.0;
    let bar_left = (usable_center_x - max_bar_w * 0.5)
        .min(max_right - max_bar_w)
        .max(canvas_rect.left() + 18.0);
    let bar_top = canvas_rect.bottom() - total_h - 16.0;

    if *search_popover_open && !search_query.trim().is_empty() {
        let pop_h = if search_results.is_empty() {
            74.0
        } else {
            (search_results.len() as f32 * 54.0 + 42.0).clamp(96.0, 250.0)
        };
        let pop_pos = Pos2::new(bar_left, (bar_top - pop_h - 10.0).max(canvas_rect.top() + 12.0));

        egui::Area::new(Id::unique("axel_search_popover"))
            .order(Order::Foreground)
            .fixed_pos(pop_pos)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(with_alpha(p.bg_surface_elevated, 248))
                    .stroke(Stroke::new(1.0, p.border_strong))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(max_bar_w - 24.0);
                        ui.set_max_height(pop_h - 24.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} semantic matches",
                                    search_results.len()
                                ))
                                .font(font_medium(11.5))
                                .color(p.ink_secondary),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (c_rect, c_resp) =
                                    ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                                if c_resp.hovered() {
                                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                }
                                draw_icon_close(
                                    ui.painter(),
                                    c_rect.center(),
                                    10.0,
                                    if c_resp.hovered() {
                                        p.ink_primary
                                    } else {
                                        p.ink_muted
                                    },
                                );
                                if c_resp.clicked() {
                                    *search_popover_open = false;
                                    action = PanelAction::CloseSearchPopover;
                                }
                            });
                        });
                        ui.add_space(6.0);

                        if search_results.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "No matching documents found. Try a keyword, filename, or category.",
                                )
                                .font(font_sans(12.0))
                                .color(p.ink_muted),
                            );
                        } else {
                            ScrollArea::vertical()
                                .id_salt("search_results_scroll")
                                .max_height(pop_h - 56.0)
                                .show(ui, |ui| {
                                    for res in search_results {
                                        let accent_name = category_accent(&res.item.category);
                                        let accent = accent_color(accent_name);
                                        let bright = accent_bright(accent_name);
                                        let row_w = ui.available_width();
                                        let (r_rect, r_resp) = ui.allocate_exact_size(
                                            Vec2::new(row_w, 46.0),
                                            Sense::click(),
                                        );
                                        if r_resp.hovered() {
                                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                            ui.painter().rect_filled(
                                                r_rect,
                                                CornerRadius::same(9),
                                                p.bg_surface_hover,
                                            );
                                        } else {
                                            ui.painter().rect_filled(
                                                r_rect,
                                                CornerRadius::same(9),
                                                with_alpha(p.bg_surface, 200),
                                            );
                                        }

                                        let badge_col = match res.match_type.as_str() {
                                            "exact" => p.emerald,
                                            "suggested" => p.copper_bright,
                                            _ => p.indigo_bright,
                                        };
                                        ui.painter().circle_filled(
                                            Pos2::new(r_rect.left() + 14.0, r_rect.center().y),
                                            4.0,
                                            accent,
                                        );
                                        ui.painter().text(
                                            Pos2::new(r_rect.left() + 26.0, r_rect.center().y - 7.5),
                                            Align2::LEFT_CENTER,
                                            truncate_label(&res.item.file, 30),
                                            font_medium(12.0),
                                            p.ink_primary,
                                        );
                                        let kws = res
                                            .item
                                            .keywords
                                            .iter()
                                            .take(3)
                                            .cloned()
                                            .collect::<Vec<_>>()
                                            .join(" · ");
                                        ui.painter().text(
                                            Pos2::new(r_rect.left() + 26.0, r_rect.center().y + 8.5),
                                            Align2::LEFT_CENTER,
                                            format!("{} · {}", res.item.category, kws),
                                            font_sans(10.5),
                                            p.ink_muted,
                                        );
                                        ui.painter().text(
                                            Pos2::new(r_rect.right() - 12.0, r_rect.center().y),
                                            Align2::RIGHT_CENTER,
                                            format!(
                                                "{} {:.0}%",
                                                res.match_type.to_uppercase(),
                                                res.score * 100.0
                                            ),
                                            font_mono(10.0),
                                            if res.match_type == "exact" {
                                                badge_col
                                            } else {
                                                bright
                                            },
                                        );

                                        if r_resp.clicked() {
                                            action = PanelAction::SelectNode(res.item.hash.clone());
                                        }
                                        ui.add_space(4.0);
                                    }
                                });
                        }
                    });
            });
    }

    // Curved Copper-to-Indigo Semantic Search Bar + Filter Chips
    egui::Area::new(Id::unique("axel_semantic_search_bar"))
        .order(Order::Foreground)
        .fixed_pos(Pos2::new(bar_left, bar_top))
        .show(ctx, |ui| {
            ui.set_width(max_bar_w);

            let (pill_rect, _) = ui.allocate_exact_size(Vec2::new(max_bar_w, bar_h), Sense::hover());

            paint_soft_shadow(ui.painter(), pill_rect, 21);
            ui.painter().rect_filled(
                pill_rect,
                CornerRadius::same(21),
                with_alpha(p.bg_surface, 246),
            );

            paint_gradient_pill_border(ui.painter(), pill_rect, p.copper, p.indigo);

            let input_rect = Rect::from_min_max(
                Pos2::new(pill_rect.left() + 18.0, pill_rect.center().y - 10.0),
                Pos2::new(pill_rect.right() - 48.0, pill_rect.center().y + 10.0),
            );
            let mut input_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(input_rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let edit_id = Id::unique("axel_semantic_search_input");
            let edit_resp = input_ui.add_sized(
                Vec2::new(input_rect.width() - 22.0, 20.0),
                TextEdit::singleline(search_query)
                    .id(edit_id)
                    .hint_text("Search semantic search...")
                    .frame(Frame::NONE)
                    .margin(Margin::ZERO)
                    .font(font_sans(13.0))
                    .text_color(p.ink_primary),
            );
            if *focus_requested {
                edit_resp.request_focus();
                *focus_requested = false;
            }
            if edit_resp.changed() {
                *search_popover_open = !search_query.trim().is_empty();
                action = PanelAction::SearchQueryChanged(search_query.clone());
            }

            if !search_query.is_empty() {
                let (clr_rect, clr_resp) =
                    input_ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                if clr_resp.hovered() {
                    input_ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                draw_icon_close(
                    input_ui.painter(),
                    clr_rect.center(),
                    10.0,
                    if clr_resp.hovered() {
                        p.ink_primary
                    } else {
                        p.ink_muted
                    },
                );
                if clr_resp.clicked() {
                    search_query.clear();
                    *search_popover_open = false;
                    action = PanelAction::SearchQueryChanged(String::new());
                }
            }

            let btn_center = Pos2::new(pill_rect.right() - 22.0, pill_rect.center().y);
            let btn_rect = Rect::from_center_size(btn_center, Vec2::splat(30.0));
            let btn_resp = ui.interact(btn_rect, Id::unique("axel_search_go_btn"), Sense::click());
            ui.painter().circle_filled(
                btn_center,
                15.0,
                if btn_resp.hovered() {
                    p.indigo_bright
                } else {
                    p.indigo
                },
            );
            ui.painter().circle_stroke(
                btn_center,
                16.5,
                Stroke::new(1.0, with_alpha(p.indigo_bright, 120)),
            );
            draw_icon_search(ui.painter(), btn_center, 14.0, Color32::WHITE);
            if btn_resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if btn_resp.clicked() {
                if search_query.trim().is_empty() {
                    *focus_requested = true;
                } else {
                    *search_popover_open = true;
                }
            }

            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                let all_active = active_category_filter.is_none();
                if filter_chip(ui, "All clusters", all_active, p.copper) {
                    action = PanelAction::SetCategoryFilter(None);
                }
                for hub in hubs.iter().take(6) {
                    let is_active = active_category_filter == Some(hub.category.as_str());
                    let col = accent_color(&hub.accent);
                    if filter_chip(ui, &hub.category, is_active, col) {
                        let next = if is_active {
                            None
                        } else {
                            Some(hub.category.clone())
                        };
                        action = PanelAction::SetCategoryFilter(next);
                    }
                }
            });
        });

    action
}

fn paint_gradient_pill_border(painter: &egui::Painter, rect: Rect, left: Color32, right: Color32) {
    let r = rect.height() * 0.5;
    let steps = 24;
    let x0 = rect.left() + r;
    let x1 = rect.right() - r;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let col = lerp_color(left, right, (t0 + t1) * 0.5);
        let sx0 = x0 + (x1 - x0) * t0;
        let sx1 = x0 + (x1 - x0) * t1;
        painter.line_segment(
            [Pos2::new(sx0, rect.top()), Pos2::new(sx1, rect.top())],
            Stroke::new(1.6, with_alpha(col, 225)),
        );
        painter.line_segment(
            [Pos2::new(sx0, rect.bottom()), Pos2::new(sx1, rect.bottom())],
            Stroke::new(1.6, with_alpha(col, 225)),
        );
    }

    let arc_steps = 14;
    let lc = Pos2::new(rect.left() + r, rect.center().y);
    let rc = Pos2::new(rect.right() - r, rect.center().y);
    for i in 0..arc_steps {
        let a0 = std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32 / arc_steps as f32);
        let a1 = std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * ((i + 1) as f32 / arc_steps as f32);
        painter.line_segment(
            [
                Pos2::new(lc.x + a0.cos() * r, lc.y + a0.sin() * r),
                Pos2::new(lc.x + a1.cos() * r, lc.y + a1.sin() * r),
            ],
            Stroke::new(1.7, with_alpha(left, 240)),
        );

        let b0 = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32 / arc_steps as f32);
        let b1 = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * ((i + 1) as f32 / arc_steps as f32);
        painter.line_segment(
            [
                Pos2::new(rc.x + b0.cos() * r, rc.y + b0.sin() * r),
                Pos2::new(rc.x + b1.cos() * r, rc.y + b1.sin() * r),
            ],
            Stroke::new(1.7, with_alpha(right, 240)),
        );
    }
}

fn filter_chip(ui: &mut Ui, label: &str, active: bool, accent: Color32) -> bool {
    let p = palette();
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font_medium(11.0),
        if active { p.ink_primary } else { p.ink_muted },
    );
    let size = Vec2::new(galley.size().x + 20.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let fill = if active {
        with_alpha(accent, 46)
    } else if resp.hovered() {
        p.bg_surface_hover
    } else {
        with_alpha(p.bg_surface, 225)
    };
    let stroke = if active {
        with_alpha(accent, 230)
    } else if resp.hovered() {
        p.border_strong
    } else {
        p.border_subtle
    };

    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, Color32::TRANSPARENT);

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

pub fn render_drop_dock_overlay(
    ctx: &egui::Context,
    canvas_rect: Rect,
    nodes: &[GraphNode],
    dock_index: usize,
    is_file_drag_hovering: bool,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();
    if canvas_rect.width() < 680.0 || canvas_rect.height() < 220.0 {
        return action;
    }

    let dock_w = 276.0;
    let dock_h = 96.0;
    let pos = Pos2::new(
        canvas_rect.right() - dock_w - 18.0,
        canvas_rect.bottom() - dock_h - 26.0,
    );

    egui::Area::new(Id::unique("axel_dynamic_drop_dock"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.set_width(dock_w);
            let (dock_rect, _) = ui.allocate_exact_size(Vec2::new(dock_w, dock_h), Sense::hover());

            paint_soft_shadow(ui.painter(), dock_rect, 18);

            let border_col = if is_file_drag_hovering {
                p.copper_bright
            } else {
                p.border_strong
            };
            ui.painter().rect_filled(
                dock_rect,
                CornerRadius::same(18),
                with_alpha(p.bg_surface, 242),
            );
            ui.painter().rect_stroke(
                dock_rect,
                CornerRadius::same(18),
                Stroke::new(if is_file_drag_hovering { 1.8 } else { 1.0 }, border_col),
                StrokeKind::Inside,
            );

            let left_btn = Rect::from_center_size(
                Pos2::new(dock_rect.left() + 20.0, dock_rect.center().y),
                Vec2::splat(26.0),
            );
            let left_resp = ui.interact(left_btn, Id::unique("dock_prev_btn"), Sense::click());
            if left_resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                ui.painter()
                    .circle_filled(left_btn.center(), 12.0, p.bg_surface_hover);
            }
            draw_icon_chevron_left(
                ui.painter(),
                left_btn.center(),
                12.0,
                if left_resp.hovered() {
                    p.ink_primary
                } else {
                    p.ink_muted
                },
            );
            if left_resp.clicked() {
                action = PanelAction::CycleDropDock(-1);
            }

            let right_btn = Rect::from_center_size(
                Pos2::new(dock_rect.right() - 20.0, dock_rect.center().y),
                Vec2::splat(26.0),
            );
            let right_resp = ui.interact(right_btn, Id::unique("dock_next_btn"), Sense::click());
            if right_resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                ui.painter()
                    .circle_filled(right_btn.center(), 12.0, p.bg_surface_hover);
            }
            draw_icon_chevron_right(
                ui.painter(),
                right_btn.center(),
                12.0,
                if right_resp.hovered() {
                    p.ink_primary
                } else {
                    p.ink_muted
                },
            );
            if right_resp.clicked() {
                action = PanelAction::CycleDropDock(1);
            }

            if nodes.is_empty() || is_file_drag_hovering {
                let center_rect = Rect::from_center_size(dock_rect.center(), Vec2::new(140.0, 64.0));
                let resp = ui.interact(center_rect, Id::unique("dock_empty_drop"), Sense::click());
                draw_icon_upload(
                    ui.painter(),
                    Pos2::new(dock_rect.center().x, dock_rect.center().y - 10.0),
                    20.0,
                    if is_file_drag_hovering {
                        p.copper_bright
                    } else {
                        p.indigo_bright
                    },
                );
                ui.painter().text(
                    Pos2::new(dock_rect.center().x, dock_rect.center().y + 16.0),
                    Align2::CENTER_CENTER,
                    if is_file_drag_hovering {
                        "Drop to ingest"
                    } else {
                        "Drop or click to stage"
                    },
                    font_medium(11.0),
                    p.ink_secondary,
                );
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    action = PanelAction::PickFilesToIngest;
                }
            } else {
                let len = nodes.len();
                let center_idx = dock_index % len;
                let prev_idx = (center_idx + len - 1) % len;
                let next_idx = (center_idx + 1) % len;

                if len >= 2 {
                    let left_card = Rect::from_center_size(
                        Pos2::new(dock_rect.center().x - 64.0, dock_rect.center().y + 2.0),
                        Vec2::new(48.0, 56.0),
                    );
                    let l_node = &nodes[prev_idx];
                    let l_resp = paint_dock_card(
                        ui,
                        left_card,
                        l_node,
                        false,
                        Id::unique("dock_card_left"),
                    );
                    if l_resp.clicked() {
                        action = PanelAction::SelectDropDockIndex(prev_idx);
                    }
                }

                if len >= 3 {
                    let right_card = Rect::from_center_size(
                        Pos2::new(dock_rect.center().x + 64.0, dock_rect.center().y + 2.0),
                        Vec2::new(48.0, 56.0),
                    );
                    let r_node = &nodes[next_idx];
                    let r_resp = paint_dock_card(
                        ui,
                        right_card,
                        r_node,
                        false,
                        Id::unique("dock_card_right"),
                    );
                    if r_resp.clicked() {
                        action = PanelAction::SelectDropDockIndex(next_idx);
                    }
                }

                let center_card = Rect::from_center_size(
                    Pos2::new(dock_rect.center().x, dock_rect.center().y - 6.0),
                    Vec2::new(64.0, 70.0),
                );
                let c_node = &nodes[center_idx];
                let c_resp = paint_dock_card(
                    ui,
                    center_card,
                    c_node,
                    true,
                    Id::unique("dock_card_center"),
                );
                if c_resp.clicked() {
                    action = PanelAction::SelectNode(c_node.hash.clone());
                }
            }

            let dot_count = nodes.len().clamp(1, 5);
            let active_dot = if nodes.is_empty() {
                0
            } else {
                (dock_index % nodes.len()) % dot_count
            };
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let total_dots_w = dot_count as f32 * 12.0;
                ui.add_space(((dock_w - total_dots_w) * 0.5).max(0.0));
                for i in 0..dot_count {
                    let is_cur = i == active_dot;
                    let (d_rect, _) = ui.allocate_exact_size(
                        Vec2::new(if is_cur { 14.0 } else { 6.0 }, 6.0),
                        Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        d_rect,
                        CornerRadius::same(3),
                        if is_cur { p.copper_bright } else { p.border_strong },
                    );
                }
            });
        });

    action
}

fn paint_dock_card(
    ui: &mut Ui,
    rect: Rect,
    node: &GraphNode,
    featured: bool,
    id: Id,
) -> egui::Response {
    let p = palette();
    let resp = ui.interact(rect, id, Sense::click());
    let accent_name = category_accent(&node.category);
    let col = accent_color(accent_name);
    let bright = accent_bright(accent_name);

    if featured {
        paint_soft_shadow(ui.painter(), rect, 11);
    }

    let fill = if featured {
        lerp_color(p.bg_surface_elevated, col, 0.26)
    } else {
        lerp_color(p.bg_surface, col, 0.14)
    };
    let stroke = if resp.hovered() || featured {
        with_alpha(bright, if featured { 230 } else { 165 })
    } else {
        p.border_strong
    };

    ui.painter().rect_filled(rect, CornerRadius::same(11), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(11),
        Stroke::new(if featured { 1.5 } else { 1.0 }, stroke),
        StrokeKind::Inside,
    );

    draw_icon_file_type(
        ui.painter(),
        Pos2::new(rect.center().x, rect.center().y - 7.0),
        if featured { 22.0 } else { 17.0 },
        &node.file_type,
        bright,
    );

    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 11.0),
        Align2::CENTER_CENTER,
        truncate_label(&node.file, if featured { 8 } else { 6 }),
        font_sans(if featured { 9.5 } else { 8.5 }),
        if featured { p.ink_primary } else { p.ink_muted },
    );

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.on_hover_text(format!("{} ({})", node.file, node.category))
}

pub fn render_inspector_drawer(
    ctx: &egui::Context,
    canvas_rect: Rect,
    detail: &DocumentDetail,
    confirm_delete: &mut bool,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();
    let drawer_w = 344.0_f32.min((canvas_rect.width() - 32.0).max(260.0));
    let drawer_h = (canvas_rect.height() - 148.0).clamp(260.0, 560.0);
    let pos = Pos2::new(canvas_rect.right() - drawer_w - 16.0, canvas_rect.top() + 16.0);

    egui::Area::new(Id::unique("axel_inspector_drawer"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let accent_name = category_accent(&detail.item.category);
            let accent = accent_color(accent_name);
            let bright = accent_bright(accent_name);

            Frame::new()
                .fill(with_alpha(p.bg_chrome, 250))
                .stroke(Stroke::new(1.0, p.border_strong))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(16))
                .show(ui, |ui| {
                    ui.set_width(drawer_w - 32.0);
                    ui.set_height(drawer_h - 32.0);

                    ui.horizontal(|ui| {
                        let (ic_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                        ui.painter().rect_filled(
                            ic_rect,
                            CornerRadius::same(8),
                            with_alpha(accent, 48),
                        );
                        draw_icon_file_type(
                            ui.painter(),
                            ic_rect.center(),
                            17.0,
                            &detail.file_type,
                            bright,
                        );

                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(truncate_label(&detail.item.file, 26))
                                    .font(font_semibold(13.5))
                                    .color(p.ink_primary),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} · {} · {}",
                                    detail.item.category,
                                    detail.file_type.to_uppercase(),
                                    format_bytes(detail.size_bytes)
                                ))
                                .font(font_sans(11.0))
                                .color(bright),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                            let (c_rect, c_resp) =
                                ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                            if c_resp.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                ui.painter().rect_filled(
                                    c_rect,
                                    CornerRadius::same(6),
                                    p.bg_surface_hover,
                                );
                            }
                            draw_icon_close(
                                ui.painter(),
                                c_rect.center(),
                                11.0,
                                if c_resp.hovered() {
                                    p.ink_primary
                                } else {
                                    p.ink_muted
                                },
                            );
                            if c_resp.clicked() {
                                *confirm_delete = false;
                                action = PanelAction::CloseInspector;
                            }
                        });
                    });

                    ui.add_space(10.0);

                    let short_hash: String = detail.item.hash.chars().take(12).collect();
                    let rel = format_relative_time(detail.item.indexed_at.as_deref(), Utc::now());
                    Frame::new()
                        .fill(p.bg_surface)
                        .stroke(Stroke::new(1.0, p.border_subtle))
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("SHA {short_hash}"))
                                        .font(font_mono(10.5))
                                        .color(p.ink_muted),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new(if detail.file_exists {
                                            format!("{rel} · On disk")
                                        } else {
                                            format!("{rel} · Missing")
                                        })
                                        .font(font_sans(10.5))
                                        .color(if detail.file_exists {
                                            p.emerald
                                        } else {
                                            p.danger
                                        }),
                                    );
                                });
                            });
                        });

                    ui.add_space(10.0);

                    if detail.item.physical_paths.len() > 1 || detail.item.is_duplicate {
                        ui.label(
                            egui::RichText::new("Physical Copies & Deduplication")
                                .font(font_medium(11.0))
                                .color(p.copper_bright),
                        );
                        ui.add_space(4.0);
                        for (idx, ppath) in detail.item.physical_paths.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let is_primary = idx == 0;
                                let prefix = if is_primary { "1. " } else { "2. " };
                                ui.label(
                                    egui::RichText::new(format!("{}{}", prefix, truncate_label(ppath, 26)))
                                        .font(font_mono(10.5))
                                        .color(if is_primary { p.ink_primary } else { p.copper_bright }),
                                );
                                if !is_primary {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let (trash_rect, trash_resp) =
                                            ui.allocate_exact_size(Vec2::new(48.0, 19.0), Sense::click());
                                        let tr_bg = if trash_resp.hovered() {
                                            p.danger
                                        } else {
                                            with_alpha(p.danger, 45)
                                        };
                                        ui.painter().rect_filled(trash_rect, CornerRadius::same(4), tr_bg);
                                        ui.painter().text(
                                            trash_rect.center(),
                                            Align2::CENTER_CENTER,
                                            "Trash",
                                            font_medium(10.0),
                                            if trash_resp.hovered() { Color32::WHITE } else { p.danger },
                                        );
                                        if trash_resp.clicked() {
                                            action = PanelAction::TrashDuplicateFile {
                                                canonical_hash: detail.item.hash.clone(),
                                                trash_path: ppath.clone(),
                                            };
                                        }
                                    });
                                }
                            });
                        }
                        ui.add_space(8.0);
                    }

                    if !detail.item.symbols.is_empty() {
                        ui.label(
                            egui::RichText::new("Extracted AST Symbols")
                                .font(font_medium(11.0))
                                .color(p.ink_muted),
                        );
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                            for sym in &detail.item.symbols {
                                let sym_chip = format!("@{}", sym);
                                if filter_chip(ui, &sym_chip, false, p.indigo) {
                                    action = PanelAction::AppendSearchToken(sym_chip);
                                }
                            }
                        });
                        ui.add_space(10.0);
                    }

                    if !detail.item.keywords.is_empty() {
                        ui.label(
                            egui::RichText::new("Extracted Keywords")
                                .font(font_medium(11.0))
                                .color(p.ink_muted),
                        );
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                            for kw in &detail.item.keywords {
                                let kw_chip = format!("#{}", kw);
                                if filter_chip(ui, &kw_chip, false, accent) {
                                    action = PanelAction::AppendSearchToken(kw_chip);
                                }
                            }
                        });
                        ui.add_space(10.0);
                    }

                    ui.label(
                        egui::RichText::new("Extracted Content Preview")
                            .font(font_medium(11.0))
                            .color(p.ink_muted),
                    );
                    ui.add_space(4.0);

                    let is_sensitive = detail.item.category.contains("Identity")
                        || detail.item.category.contains("Government")
                        || detail.item.category.contains("Sensitive")
                        || detail.item.category.contains("Personal");

                    let shield_id = Id::unique(format!("shield_reveal_{}", detail.item.hash));
                    let is_revealed = ui.ctx().data_mut(|d| d.get_temp::<bool>(shield_id).unwrap_or(false));

                    let preview_h = (ui.available_height() - 46.0).max(90.0);
                    Frame::new()
                        .fill(p.bg_void)
                        .stroke(Stroke::new(
                            1.0,
                            if is_sensitive && !is_revealed {
                                with_alpha(p.copper_bright, 180)
                            } else {
                                p.border_subtle
                            },
                        ))
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.set_height(preview_h - 20.0);
                            if is_sensitive && !is_revealed {
                                ui.vertical_centered(|ui| {
                                    ui.add_space((preview_h - 100.0).max(10.0) * 0.4);
                                    ui.label(
                                        egui::RichText::new("🔒 Privacy Shield Active")
                                            .font(font_semibold(12.5))
                                            .color(p.copper_bright),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new("Government ID / Sensitive Personal Data")
                                            .font(font_sans(10.5))
                                            .color(p.ink_muted),
                                    );
                                    ui.add_space(8.0);
                                    let (rev_rect, rev_resp) =
                                        ui.allocate_exact_size(Vec2::new(105.0, 22.0), Sense::click());
                                    let r_bg = if rev_resp.hovered() {
                                        p.bg_surface_hover
                                    } else {
                                        p.bg_surface
                                    };
                                    ui.painter().rect_filled(rev_rect, CornerRadius::same(5), r_bg);
                                    ui.painter().rect_stroke(
                                        rev_rect,
                                        CornerRadius::same(5),
                                        Stroke::new(1.0, p.border_strong),
                                        StrokeKind::Inside,
                                    );
                                    ui.painter().text(
                                        rev_rect.center(),
                                        Align2::CENTER_CENTER,
                                        "👁 Reveal",
                                        font_medium(11.0),
                                        p.ink_primary,
                                    );
                                    if rev_resp.clicked() {
                                        ui.ctx().data_mut(|d| d.insert_temp(shield_id, true));
                                    }
                                });
                            } else {
                                ScrollArea::vertical()
                                    .id_salt("inspector_preview_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(&detail.text_preview)
                                                .font(font_mono(11.0))
                                                .color(p.ink_secondary),
                                        );
                                    });
                            }
                        });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if action_pill_btn(ui, "Open File", p.indigo, detail.file_exists) {
                            action = PanelAction::OpenFileExternal(detail.item.path.clone());
                        }
                        if action_pill_btn(ui, "Folder", p.border_strong, detail.file_exists) {
                            action = PanelAction::RevealFolderExternal(detail.item.path.clone());
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if *confirm_delete {
                                if action_pill_btn(ui, "Cancel", p.border_strong, true) {
                                    *confirm_delete = false;
                                }
                                if action_pill_btn(ui, "Confirm", p.danger, true) {
                                    *confirm_delete = false;
                                    action = PanelAction::DeleteDocument {
                                        hash: detail.item.hash.clone(),
                                        delete_file: false,
                                    };
                                }
                            } else if action_pill_btn(ui, "Remove", p.danger, true) {
                                *confirm_delete = true;
                            }
                        });
                    });
                });
        });

    action
}

pub fn render_telemetry_drawer(
    ctx: &egui::Context,
    canvas_rect: Rect,
    stats: &SystemStats,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();
    let drawer_w = 348.0_f32.min((canvas_rect.width() - 32.0).max(260.0));
    let pos = Pos2::new(canvas_rect.right() - drawer_w - 16.0, canvas_rect.top() + 16.0);

    egui::Area::new(Id::unique("axel_telemetry_drawer"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            Frame::new()
                .fill(with_alpha(p.bg_chrome, 250))
                .stroke(Stroke::new(1.0, p.border_strong))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(16))
                .show(ui, |ui| {
                    ui.set_width(drawer_w - 32.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("System Telemetry & Theme")
                                .font(font_semibold(14.0))
                                .color(p.ink_primary),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (c_rect, c_resp) =
                                ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                            if c_resp.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            draw_icon_close(
                                ui.painter(),
                                c_rect.center(),
                                11.0,
                                if c_resp.hovered() {
                                    p.ink_primary
                                } else {
                                    p.ink_muted
                                },
                            );
                            if c_resp.clicked() {
                                action = PanelAction::CloseTelemetry;
                            }
                        });
                    });

                    ui.add_space(10.0);

                    // Theme Preset Selector
                    ui.label(
                        egui::RichText::new("Appearance Theme")
                            .font(font_medium(11.0))
                            .color(p.ink_muted),
                    );
                    ui.add_space(5.0);
                    let cur_theme = active_theme();
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                        for preset in [
                            ThemePreset::NebulaObsidian,
                            ThemePreset::MidnightCopper,
                            ThemePreset::ArcticSlate,
                        ] {
                            let is_sel = cur_theme == preset;
                            if filter_chip(ui, preset.label(), is_sel, p.copper) {
                                action = PanelAction::SetTheme(preset);
                            }
                        }
                    });

                    ui.add_space(10.0);

                    telemetry_row(
                        ui,
                        "Indexed Documents",
                        &stats.total_documents.to_string(),
                        p.copper_bright,
                    );
                    telemetry_row(ui, "Active Model", &stats.active_model, p.indigo_bright);
                    telemetry_row(
                        ui,
                        "Groq Cloud AI",
                        if stats.groq_enabled {
                            "Connected"
                        } else {
                            "Local Rule Engine"
                        },
                        if stats.groq_enabled {
                            p.emerald
                        } else {
                            p.copper_bright
                        },
                    );
                    telemetry_row(
                        ui,
                        "Tesseract OCR",
                        if stats.tesseract_ready {
                            "Ready"
                        } else {
                            "Metadata Fallback"
                        },
                        if stats.tesseract_ready {
                            p.emerald
                        } else {
                            p.ink_muted
                        },
                    );

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Workspace Folder")
                            .font(font_medium(11.0))
                            .color(p.ink_muted),
                    );
                    ui.add_space(4.0);
                    Frame::new()
                        .fill(p.bg_surface)
                        .stroke(Stroke::new(1.0, p.border_subtle))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(10, 7))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (f_rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                                draw_icon_folder(
                                    ui.painter(),
                                    f_rect.center(),
                                    13.0,
                                    p.copper_bright,
                                );
                                ui.label(
                                    egui::RichText::new(truncate_label(&stats.input_folder, 24))
                                        .font(font_mono(10.5))
                                        .color(p.ink_secondary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if action_pill_btn(ui, "Switch", p.indigo, true) {
                                        action = PanelAction::ToggleWorkspacePopover;
                                    }
                                });
                            });
                        });

                    if !stats.categories.is_empty() {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("Category Distribution")
                                .font(font_medium(11.0))
                                .color(p.ink_muted),
                        );
                        ui.add_space(4.0);
                        let mut cats: Vec<(&String, &usize)> = stats.categories.iter().collect();
                        cats.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
                        let max_c = cats.first().map(|(_, c)| **c).unwrap_or(1).max(1) as f32;
                        for (cat, count) in cats.into_iter().take(6) {
                            let col = accent_color(category_accent(cat));
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(truncate_label(cat, 14))
                                        .font(font_sans(11.0))
                                        .color(p.ink_secondary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new(count.to_string())
                                            .font(font_mono(10.5))
                                            .color(p.ink_muted),
                                    );
                                    let bar_w = 110.0;
                                    let (b_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(bar_w, 6.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        b_rect,
                                        CornerRadius::same(3),
                                        p.bg_surface,
                                    );
                                    let fill_w = (bar_w * (*count as f32 / max_c)).max(6.0);
                                    let fill_rect = Rect::from_min_size(
                                        b_rect.min,
                                        Vec2::new(fill_w, 6.0),
                                    );
                                    ui.painter().rect_filled(
                                        fill_rect,
                                        CornerRadius::same(3),
                                        col,
                                    );
                                });
                            });
                        }
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if action_pill_btn(ui, "Scan New", p.copper, true) {
                            action = PanelAction::TriggerScan { force: false };
                        }
                        if action_pill_btn(ui, "Force Re-index", p.indigo, true) {
                            action = PanelAction::TriggerScan { force: true };
                        }
                        let _ = draw_icon_refresh;
                    });
                });
        });

    action
}

fn telemetry_row(ui: &mut Ui, label: &str, value: &str, val_color: Color32) {
    let p = palette();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .font(font_sans(11.5))
                .color(p.ink_muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                egui::RichText::new(truncate_label(value, 22))
                    .font(font_mono(11.0))
                    .color(val_color),
            );
        });
    });
    ui.add_space(3.0);
}

fn action_pill_btn(ui: &mut Ui, label: &str, accent: Color32, enabled: bool) -> bool {
    let p = palette();
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font_medium(11.5),
        if enabled { p.ink_primary } else { p.ink_dim },
    );
    let size = Vec2::new(galley.size().x + 20.0, 26.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);

    let fill = if !enabled {
        p.bg_surface
    } else if resp.hovered() {
        with_alpha(accent, 68)
    } else {
        with_alpha(accent, 36)
    };
    let stroke = if !enabled {
        p.border_subtle
    } else if resp.hovered() {
        accent
    } else {
        with_alpha(accent, 175)
    };

    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, Color32::TRANSPARENT);

    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

pub fn apply_overlay_action(current: &mut PanelAction, next: PanelAction) {
    merge_action(current, next);
}

pub fn render_properties_modal(
    ctx: &egui::Context,
    props: &crate::engine::FileProperties,
) -> PanelAction {
    let mut action = PanelAction::None;
    let p = palette();

    let modal_w = 480.0;
    egui::Window::new(format!("Properties — {}", truncate_label(&props.file_name, 28)))
        .id(Id::unique("axel_file_properties_modal"))
        .collapsible(false)
        .resizable(false)
        .default_width(modal_w)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(
            Frame::new()
                .fill(with_alpha(p.bg_chrome, 252))
                .stroke(Stroke::new(1.0, p.border_strong))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.set_width(modal_w - 32.0);

            // Header with filename and close
            ui.horizontal(|ui| {
                let (ic_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                let ext = Path::new(&props.file_name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                draw_icon_file_type(ui.painter(), ic_rect.center(), 18.0, ext, p.indigo_bright);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&props.file_name)
                            .font(font_semibold(13.5))
                            .color(p.ink_primary),
                    );
                    ui.label(
                        egui::RichText::new(&props.file_type_desc)
                            .font(font_sans(11.0))
                            .color(p.ink_muted),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    let (c_rect, c_resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                    draw_icon_close(
                        ui.painter(),
                        c_rect.center(),
                        10.0,
                        if c_resp.hovered() { p.ink_primary } else { p.ink_muted },
                    );
                    if c_resp.clicked() {
                        action = PanelAction::CloseProperties;
                    }
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // Properties Grid
            egui::Grid::new("file_props_grid")
                .num_columns(2)
                .spacing([14.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Location:").font(font_medium(11.0)).color(p.ink_muted));
                    ui.label(egui::RichText::new(&props.parent_dir).font(font_mono(10.5)).color(p.ink_secondary));
                    ui.end_row();

                    ui.label(egui::RichText::new("Absolute Path:").font(font_medium(11.0)).color(p.ink_muted));
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(truncate_label(&props.absolute_path, 34)).font(font_mono(10.5)).color(p.ink_primary));
                        if ui.small_button("Copy").clicked() {
                            action = PanelAction::CopyPath(props.absolute_path.clone());
                        }
                    });
                    ui.end_row();

                    ui.label(egui::RichText::new("File Size:").font(font_medium(11.0)).color(p.ink_muted));
                    ui.label(egui::RichText::new(&props.formatted_size).font(font_mono(11.0)).color(p.copper_bright));
                    ui.end_row();

                    ui.label(egui::RichText::new("Permissions:").font(font_medium(11.0)).color(p.ink_muted));
                    ui.label(egui::RichText::new(&props.permissions_mode).font(font_mono(10.5)).color(p.ink_secondary));
                    ui.end_row();

                    ui.label(egui::RichText::new("Created:").font(font_medium(11.0)).color(p.ink_muted));
                    ui.label(egui::RichText::new(&props.created_str).font(font_sans(11.0)).color(p.ink_secondary));
                    ui.end_row();

                    ui.label(egui::RichText::new("Modified:").font(font_medium(11.0)).color(p.ink_muted));
                    ui.label(egui::RichText::new(&props.modified_str).font(font_sans(11.0)).color(p.ink_secondary));
                    ui.end_row();

                    ui.label(egui::RichText::new("Accessed:").font(font_medium(11.0)).color(p.ink_muted));
                    ui.label(egui::RichText::new(&props.accessed_str).font(font_sans(11.0)).color(p.ink_secondary));
                    ui.end_row();

                    ui.label(egui::RichText::new("Extended Attr:").font(font_medium(11.0)).color(p.ink_muted));
                    if props.has_xattrs {
                        let cat = props.xattr_category.as_deref().unwrap_or("none");
                        ui.label(egui::RichText::new(format!("user.axel.* [Category: {cat}]")).font(font_mono(10.5)).color(p.emerald));
                    } else {
                        ui.label(egui::RichText::new("None (not tagged to inode)").font(font_sans(10.5)).color(p.ink_muted));
                    }
                    ui.end_row();
                });

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);

            // Bottom Buttons
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new("📂 Open File").font(font_medium(11.0))).clicked() {
                    action = PanelAction::OpenFileExternal(props.absolute_path.clone());
                }
                if ui.button(egui::RichText::new("🔍 In Folder").font(font_medium(11.0))).clicked() {
                    action = PanelAction::RevealFolderExternal(props.absolute_path.clone());
                }
                if ui.button(egui::RichText::new("📋 Copy Path").font(font_medium(11.0))).clicked() {
                    action = PanelAction::CopyPath(props.absolute_path.clone());
                }
                if ui.button(egui::RichText::new("🗑 Move to Trash").font(font_medium(11.0)).color(p.danger)).clicked() {
                    action = PanelAction::TrashFile(props.absolute_path.clone());
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(egui::RichText::new("Close").font(font_medium(11.0))).clicked() {
                        action = PanelAction::CloseProperties;
                    }
                });
            });
        });

    action
}
