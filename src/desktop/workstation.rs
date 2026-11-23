use crate::desktop::chrome::{format_relative_time, PanelAction};
use crate::desktop::galaxy::{format_bytes, truncate_label, GalaxyAction};
use crate::desktop::theme::{
    accent_bright, accent_color, draw_icon_close, draw_icon_file_type, draw_icon_folder,
    draw_icon_search, draw_icon_shield, draw_icon_smart_view, draw_icon_trash,
    draw_icon_workstation, font_medium, font_mono, font_sans, font_semibold, palette, with_alpha,
    ThemePalette,
};
use crate::engine::{category_accent, DocumentDetail, GraphNode, GraphSnapshot, SystemStats};
use crate::index::{build_virtual_tree, DocumentIndex, VirtualFolder};
use chrono::Utc;
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Frame, Id, Layout, Margin, Pos2, Rect,
    ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
use std::collections::HashSet;
use std::path::Path;

pub fn is_sensitive_category(category: &str) -> bool {
    let cat_l = category.to_lowercase();
    cat_l.contains("identity")
        || cat_l.contains("sensitive")
        || cat_l.contains("government")
        || cat_l.contains("pan")
        || cat_l.contains("aadhaar")
        || cat_l.contains("passport")
        || cat_l.contains("tax")
        || cat_l.contains("personal")
}

pub fn is_sensitive_item(
    category: &str,
    file_name: &str,
    keywords: &[String],
    preview_text: Option<&str>,
) -> bool {
    if is_sensitive_category(category) {
        return true;
    }
    let lower_file = file_name.to_lowercase();
    if lower_file.contains("aadhaar")
        || lower_file.contains("aadhar")
        || lower_file.contains("pan")
        || lower_file.contains("passport")
        || lower_file.contains("ssn")
        || lower_file.contains("voter")
    {
        return true;
    }
    for kw in keywords {
        let kw_l = kw.to_lowercase();
        if kw_l == "aadhaar" || kw_l == "aadhar" || kw_l == "pan" || kw_l == "ssn" || kw_l == "government id" {
            return true;
        }
    }
    if let Some(text) = preview_text {
        if crate::classifier::privacy::is_high_risk_identity_document(text, file_name) {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn render_workstation(
    ui: &mut Ui,
    rect: Rect,
    snapshot: &GraphSnapshot,
    stats: &SystemStats,
    search_index: &DocumentIndex,
    selected_node: Option<&str>,
    active_category_filter: Option<&str>,
    search_highlight_hashes: &HashSet<String>,
    active_detail: Option<&DocumentDetail>,
    confirm_delete: &mut bool,
    search_query: &str,
    focus_search_requested: bool,
) -> (GalaxyAction, PanelAction) {
    let mut galaxy_action = GalaxyAction::None;
    let mut panel_action = PanelAction::None;
    let p = palette();

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, p.bg_canvas);

    let inner = rect.shrink2(Vec2::new(16.0, 14.0));
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );

    // Top Header Status
    child_ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
        draw_icon_workstation(ui.painter(), icon_rect.center(), 18.0, p.indigo_bright);
        ui.add_space(6.0);

        ui.label(
            egui::RichText::new("AXEL Workstation")
                .font(font_semibold(15.5))
                .color(p.ink_primary),
        );
        ui.add_space(8.0);
        let items_len = search_index.items().len().max(snapshot.nodes.len());
        ui.label(
            egui::RichText::new(format!(
                "{} files indexed · {} taxonomy clusters",
                items_len,
                snapshot.hubs.len()
            ))
            .font(font_sans(11.5))
            .color(p.ink_muted),
        );

        if let Some(cat) = active_category_filter {
            ui.add_space(10.0);
            let (chip_rect, chip_resp) =
                ui.allocate_exact_size(Vec2::new(140.0, 20.0), Sense::click());
            ui.painter().rect_filled(
                chip_rect,
                CornerRadius::same(5),
                with_alpha(p.indigo, 40),
            );
            ui.painter().text(
                Pos2::new(chip_rect.left() + 6.0, chip_rect.center().y),
                Align2::LEFT_CENTER,
                format!("Filter: {} ✕", truncate_label(cat, 16)),
                font_medium(10.5),
                p.indigo_bright,
            );
            if chip_resp.clicked() {
                panel_action = PanelAction::SetCategoryFilter(None);
            }
        }
    });
    child_ui.add_space(10.0);

    let avail_rect = child_ui.available_rect_before_wrap();
    let total_w = avail_rect.width();
    let total_h = avail_rect.height();

    // 3-Column Width Calculations with interactive draggable splitters & disk persistence
    let id_left_w = Id::unique("workstation_left_col_w");
    let id_right_w = Id::unique("workstation_right_col_w");

    let saved_layout = crate::config::load_workstation_layout();
    let default_left = if let Some(ref l) = saved_layout {
        l.left_w
    } else if total_w >= 1050.0 {
        230.0_f32
    } else if total_w >= 780.0 {
        200.0_f32
    } else {
        180.0_f32
    };
    let default_right = if let Some(ref l) = saved_layout {
        l.right_w
    } else if total_w >= 1050.0 {
        340.0_f32
    } else if total_w >= 780.0 {
        290.0_f32
    } else {
        0.0_f32
    };

    let mut left_w = child_ui
        .ctx()
        .data_mut(|d| d.get_temp::<f32>(id_left_w))
        .unwrap_or(default_left);
    let mut right_w = child_ui
        .ctx()
        .data_mut(|d| d.get_temp::<f32>(id_right_w))
        .unwrap_or(default_right);

    // Auto-collapse right panel on narrow viewports
    if total_w < 780.0 {
        right_w = 0.0;
    }

    // Clamp widths to safe interactive bounds
    left_w = left_w.clamp(160.0, (total_w - right_w - 240.0).max(180.0));
    if right_w > 0.0 {
        right_w = right_w.clamp(230.0, (total_w - left_w - 240.0).max(250.0));
    }

    // Privacy Shield active reveal tracking (with auto-rehide on selection change & 30s timeout)
    let id_revealed = Id::unique("workstation_active_revealed_shield");
    let now = child_ui.input(|i| i.time);
    let mut active_revealed: Option<(String, f64)> = child_ui.ctx().data_mut(|d| d.get_temp(id_revealed));

    // 1. Auto-rehide on 30s timeout
    if let Some((_, reveal_time)) = active_revealed {
        if now - reveal_time >= 30.0 {
            active_revealed = None;
            child_ui.ctx().data_mut(|d| d.insert_temp(id_revealed, active_revealed.clone()));
        } else {
            let remaining = 30.0 - (now - reveal_time);
            child_ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(remaining.max(0.1)));
        }
    }

    // 2. Auto-rehide immediately if selection changed away from revealed item
    if let Some((ref rev_hash, _)) = active_revealed {
        if selected_node != Some(rev_hash.as_str()) {
            active_revealed = None;
            child_ui.ctx().data_mut(|d| d.insert_temp(id_revealed, None::<(String, f64)>));
        }
    }

    let col_gap = 12.0_f32;
    let mid_w = (total_w - left_w - right_w - if right_w > 0.0 { col_gap * 2.0 } else { col_gap }).max(120.0);

    let left_rect = Rect::from_min_size(avail_rect.min, Vec2::new(left_w, total_h));

    // Splitter 1: Between Column 1 and Column 2
    let s1_rect = Rect::from_min_size(
        Pos2::new(avail_rect.min.x + left_w, avail_rect.min.y),
        Vec2::new(col_gap, total_h),
    );
    let s1_resp = child_ui.allocate_rect(s1_rect, Sense::click_and_drag());
    if s1_resp.hovered() || s1_resp.dragged() {
        child_ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
    }
    if s1_resp.dragged() {
        let delta = s1_resp.drag_delta().x;
        left_w = (left_w + delta).clamp(160.0, (total_w - right_w - 240.0).max(180.0));
        child_ui.ctx().data_mut(|d| d.insert_temp(id_left_w, left_w));
        crate::config::save_workstation_layout(&crate::config::WorkstationLayoutState { left_w, right_w });
    }

    let mid_rect = Rect::from_min_size(
        Pos2::new(avail_rect.min.x + left_w + col_gap, avail_rect.min.y),
        Vec2::new(mid_w, total_h),
    );

    // Splitter 2: Between Column 2 and Column 3 (when visible)
    let (s2_rect, s2_resp) = if right_w > 0.0 {
        let r = Rect::from_min_size(
            Pos2::new(avail_rect.min.x + left_w + col_gap + mid_w, avail_rect.min.y),
            Vec2::new(col_gap, total_h),
        );
        let resp = child_ui.allocate_rect(r, Sense::click_and_drag());
        if resp.hovered() || resp.dragged() {
            child_ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
        }
        if resp.dragged() {
            let delta = resp.drag_delta().x;
            right_w = (right_w - delta).clamp(230.0, (total_w - left_w - 240.0).max(250.0));
            child_ui.ctx().data_mut(|d| d.insert_temp(id_right_w, right_w));
            crate::config::save_workstation_layout(&crate::config::WorkstationLayoutState { left_w, right_w });
        }
        (r, Some(resp))
    } else {
        (Rect::NOTHING, None)
    };

    let right_rect = if right_w > 0.0 {
        Rect::from_min_size(
            Pos2::new(avail_rect.min.x + left_w + mid_w + col_gap * 2.0, avail_rect.min.y),
            Vec2::new(right_w, total_h),
        )
    } else {
        Rect::NOTHING
    };

    // Render interactive splitter divider graphics
    {
        // Splitter 1 visuals
        let s1_active = s1_resp.hovered() || s1_resp.dragged();
        let s1_cx = s1_rect.center().x;
        let s1_col = if s1_resp.dragged() {
            p.indigo_bright
        } else if s1_resp.hovered() {
            p.indigo
        } else {
            with_alpha(p.border_subtle, 130)
        };
        child_ui.painter().line_segment(
            [
                Pos2::new(s1_cx, s1_rect.top() + 6.0),
                Pos2::new(s1_cx, s1_rect.bottom() - 6.0),
            ],
            Stroke::new(if s1_active { 2.0 } else { 1.0 }, s1_col),
        );
        if s1_active {
            let grip_rect = Rect::from_center_size(
                Pos2::new(s1_cx, s1_rect.center().y),
                Vec2::new(4.0, 26.0),
            );
            child_ui.painter().rect_filled(grip_rect, CornerRadius::same(2), p.indigo_bright);
        }

        // Splitter 2 visuals
        if let Some(resp) = s2_resp {
            let s2_active = resp.hovered() || resp.dragged();
            let s2_cx = s2_rect.center().x;
            let s2_col = if resp.dragged() {
                p.indigo_bright
            } else if resp.hovered() {
                p.indigo
            } else {
                with_alpha(p.border_subtle, 130)
            };
            child_ui.painter().line_segment(
                [
                    Pos2::new(s2_cx, s2_rect.top() + 6.0),
                    Pos2::new(s2_cx, s2_rect.bottom() - 6.0),
                ],
                Stroke::new(if s2_active { 2.0 } else { 1.0 }, s2_col),
            );
            if s2_active {
                let grip_rect = Rect::from_center_size(
                    Pos2::new(s2_cx, s2_rect.center().y),
                    Vec2::new(4.0, 26.0),
                );
                child_ui.painter().rect_filled(grip_rect, CornerRadius::same(2), p.indigo_bright);
            }
        }
    }

    // ==========================================
    // COLUMN 1: Smart Views & Virtual Directory Tree
    // ==========================================
    let mut left_ui = child_ui.new_child(
        egui::UiBuilder::new()
            .max_rect(left_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );

    Frame::new()
        .fill(p.bg_chrome)
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
        .show(&mut left_ui, |ui| {
            ui.set_width(left_w - 24.0);
            ui.set_height(total_h - 24.0);

            let card_h = 72.0_f32;
            let scroll_h = (total_h - 24.0 - card_h - 10.0).max(60.0);

            ui.allocate_ui(Vec2::new(left_w - 24.0, scroll_h), |ui| {
                ScrollArea::vertical()
                    .id_salt("workstation_col1_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("SMART VIEWS")
                            .font(font_semibold(10.5))
                            .color(p.ink_muted),
                    );
                    ui.add_space(6.0);

                    // 1. All Items
                    let is_all = active_category_filter.is_none();
                    let total_count = search_index.items().len().max(snapshot.nodes.len());
                    if render_smart_view_button(
                        ui,
                        "all",
                        "All Items",
                        total_count,
                        is_all,
                        p.indigo,
                        &p,
                    ) {
                        panel_action = PanelAction::SetCategoryFilter(None);
                        galaxy_action = GalaxyAction::ClearSelection;
                    }

                    // 2. Duplicates
                    let dup_count = search_index
                        .items()
                        .iter()
                        .filter(|i| i.is_duplicate || i.physical_paths.len() > 1)
                        .count();
                    let is_dup = active_category_filter == Some("Duplicates");
                    if render_smart_view_button(
                        ui,
                        "duplicates",
                        "Duplicates",
                        dup_count,
                        is_dup,
                        p.copper_bright,
                        &p,
                    ) {
                        panel_action = PanelAction::SetCategoryFilter(Some("Duplicates".to_string()));
                    }

                    // 3. User Scripts
                    let script_count = search_index
                        .items()
                        .iter()
                        .filter(|i| {
                            let ext = Path::new(&i.file)
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                                .to_lowercase();
                            let cat = i.category.to_lowercase();
                            matches!(ext.as_str(), "py" | "rs" | "js" | "ts" | "sh" | "bash" | "zsh" | "rb" | "go" | "c" | "cpp" | "h")
                                || cat.contains("code")
                                || cat.contains("script")
                        })
                        .count();
                    let is_scripts = active_category_filter == Some("User Scripts");
                    if render_smart_view_button(
                        ui,
                        "scripts",
                        "User Scripts",
                        script_count,
                        is_scripts,
                        p.indigo_bright,
                        &p,
                    ) {
                        panel_action = PanelAction::SetCategoryFilter(Some("User Scripts".to_string()));
                    }

                    // 4. Documents
                    let doc_count = search_index
                        .items()
                        .iter()
                        .filter(|i| {
                            let ext = Path::new(&i.file)
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                                .to_lowercase();
                            let cat = i.category.to_lowercase();
                            matches!(ext.as_str(), "pdf" | "docx" | "doc" | "txt" | "md" | "csv" | "json")
                                || cat.contains("document")
                                || cat.contains("paper")
                                || cat.contains("academic")
                                || cat.contains("report")
                                || cat.contains("study")
                        })
                        .count();
                    let is_docs = active_category_filter == Some("Documents");
                    if render_smart_view_button(
                        ui,
                        "documents",
                        "Documents",
                        doc_count,
                        is_docs,
                        p.emerald,
                        &p,
                    ) {
                        panel_action = PanelAction::SetCategoryFilter(Some("Documents".to_string()));
                    }

                    // 5. Media
                    let media_count = search_index
                        .items()
                        .iter()
                        .filter(|i| {
                            let ext = Path::new(&i.file)
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                                .to_lowercase();
                            let cat = i.category.to_lowercase();
                            matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "mp4" | "mkv" | "webm" | "mp3" | "wav")
                                || cat.contains("media")
                                || cat.contains("image")
                                || cat.contains("video")
                        })
                        .count();
                    let is_media = active_category_filter == Some("Media");
                    if render_smart_view_button(
                        ui,
                        "media",
                        "Media",
                        media_count,
                        is_media,
                        p.copper,
                        &p,
                    ) {
                        panel_action = PanelAction::SetCategoryFilter(Some("Media".to_string()));
                    }

                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new("VIRTUAL DIRECTORY TREE")
                            .font(font_semibold(10.5))
                            .color(p.ink_muted),
                    );
                    ui.add_space(6.0);

                    // Dynamic Virtual Directory Tree built from items
                    let vtree = build_virtual_tree(search_index.items());
                    for (_name, folder) in &vtree.subfolders {
                        render_virtual_folder_branch(
                            ui,
                            folder,
                            0,
                            active_category_filter,
                            &mut panel_action,
                            &p,
                        );
                    }
                });
            });

            ui.add_space(8.0);
            render_daemon_status_card(ui, stats, &p);
        });

    // ==========================================
    // COLUMN 2: Virtualized File Ledger (show_rows)
    // ==========================================
    let mut mid_ui = child_ui.new_child(
        egui::UiBuilder::new()
            .max_rect(mid_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );

    Frame::new()
        .fill(p.bg_chrome)
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
        .show(&mut mid_ui, |ui| {
            ui.set_width(mid_w - 24.0);
            ui.set_height(total_h - 24.0);

            // 1. Integrated Search Bar for File Ledger
            let (search_box_rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), 32.0),
                Sense::hover(),
            );
            ui.painter().rect_filled(
                search_box_rect,
                CornerRadius::same(6),
                p.bg_surface,
            );
            ui.painter().rect_stroke(
                search_box_rect,
                CornerRadius::same(6),
                Stroke::new(1.0, p.border_subtle),
                StrokeKind::Inside,
            );

            // Search icon
            let ic_pos = Pos2::new(search_box_rect.left() + 14.0, search_box_rect.center().y);
            draw_icon_search(ui.painter(), ic_pos, 13.0, p.ink_muted);

            // Text Edit field
            let mut search_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(
                        Pos2::new(search_box_rect.left() + 28.0, search_box_rect.top() + 3.0),
                        Vec2::new(search_box_rect.width() - 80.0, 26.0),
                    ))
                    .layout(Layout::left_to_right(Align::Center)),
            );

            let mut q_input = search_query.to_string();
            let text_edit = TextEdit::singleline(&mut q_input)
                .id(Id::unique("workstation_ledger_search_input"))
                .hint_text("Search documents, keywords, or symbols... (Ctrl+K)")
                .frame(Frame::NONE)
                .font(font_medium(11.5))
                .text_color(p.ink_primary)
                .margin(Margin::ZERO);

            let response = search_ui.add(text_edit);
            if focus_search_requested {
                response.request_focus();
            }
            if response.changed() {
                panel_action = PanelAction::SearchQueryChanged(q_input);
            }

            // Clear button or shortcut badge
            if !search_query.is_empty() {
                let clear_rect = Rect::from_center_size(
                    Pos2::new(search_box_rect.right() - 16.0, search_box_rect.center().y),
                    Vec2::splat(16.0),
                );
                let clear_resp = ui.interact(clear_rect, Id::unique("workstation_search_clear"), Sense::click());
                if clear_resp.hovered() {
                    ui.painter().circle_filled(clear_rect.center(), 8.0, p.bg_surface_hover);
                }
                draw_icon_close(ui.painter(), clear_rect.center(), 10.0, p.ink_muted);
                if clear_resp.clicked() {
                    panel_action = PanelAction::SearchQueryChanged(String::new());
                }
            } else {
                let badge_rect = Rect::from_center_size(
                    Pos2::new(search_box_rect.right() - 28.0, search_box_rect.center().y),
                    Vec2::new(42.0, 16.0),
                );
                ui.painter().rect_filled(badge_rect, CornerRadius::same(3), p.bg_chrome);
                ui.painter().text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    "Ctrl+K",
                    font_mono(9.5),
                    p.ink_muted,
                );
            }

            ui.add_space(8.0);

            // 2. Proportional Dynamic Column Header
            let (h_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
            let h_w = h_rect.width();
            let h_left = h_rect.left();
            let h_right = h_rect.right();
            let h_y = h_rect.center().y;

            let file_col_w = (h_w * 0.28).clamp(140.0, 240.0);
            let file_col_x = h_left + 28.0;
            let cat_col_x = file_col_x + file_col_w + 12.0;

            let size_col_w = 60.0;
            let size_col_x = h_right - 8.0; // right-aligned text
            let sym_col_right = size_col_x - size_col_w - 14.0; // right-aligned text

            let cat_label_header = if sym_col_right - cat_col_x > 260.0 {
                "CATEGORY / VIRTUAL PATH"
            } else if sym_col_right - cat_col_x > 200.0 {
                "CATEGORY / PATH"
            } else {
                "CATEGORY"
            };
            let sym_label_header = if sym_col_right - cat_col_x > 200.0 {
                "SYMBOLS / KEYWORDS"
            } else {
                "KEYWORDS / SYMBOLS"
            };

            ui.painter().text(
                Pos2::new(file_col_x, h_y),
                Align2::LEFT_CENTER,
                "FILE",
                font_semibold(10.5),
                p.ink_muted,
            );
            ui.painter().text(
                Pos2::new(cat_col_x, h_y),
                Align2::LEFT_CENTER,
                cat_label_header,
                font_semibold(10.5),
                p.ink_muted,
            );
            ui.painter().text(
                Pos2::new(sym_col_right, h_y),
                Align2::RIGHT_CENTER,
                sym_label_header,
                font_semibold(10.5),
                p.ink_muted,
            );
            ui.painter().text(
                Pos2::new(size_col_x, h_y),
                Align2::RIGHT_CENTER,
                "SIZE",
                font_semibold(10.5),
                p.ink_muted,
            );

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Filter ledger nodes
            let filtered_nodes: Vec<&GraphNode> = snapshot
                .nodes
                .iter()
                .filter(|n| {
                    if let Some(cat) = active_category_filter {
                        match cat {
                            "Duplicates" => {
                                if let Some(item) = search_index.find_by_hash(&n.hash) {
                                    if !item.is_duplicate && item.physical_paths.len() <= 1 {
                                        return false;
                                    }
                                } else {
                                    return false;
                                }
                            }
                            "User Scripts" => {
                                let ext = Path::new(&n.file)
                                    .extension()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or_default()
                                    .to_lowercase();
                                let cat_l = n.category.to_lowercase();
                                let is_script = matches!(ext.as_str(), "py" | "rs" | "js" | "ts" | "sh" | "bash" | "zsh" | "rb" | "go" | "c" | "cpp" | "h")
                                    || cat_l.contains("code")
                                    || cat_l.contains("script");
                                if !is_script {
                                    return false;
                                }
                            }
                            "Documents" => {
                                let ext = Path::new(&n.file)
                                    .extension()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or_default()
                                    .to_lowercase();
                                let cat_l = n.category.to_lowercase();
                                let is_doc = matches!(ext.as_str(), "pdf" | "docx" | "doc" | "txt" | "md" | "csv" | "json")
                                    || cat_l.contains("document")
                                    || cat_l.contains("paper")
                                    || cat_l.contains("academic")
                                    || cat_l.contains("report")
                                    || cat_l.contains("study");
                                if !is_doc {
                                    return false;
                                }
                            }
                            "Media" => {
                                let ext = Path::new(&n.file)
                                    .extension()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or_default()
                                    .to_lowercase();
                                let cat_l = n.category.to_lowercase();
                                let is_media = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "mp4" | "mkv" | "webm" | "mp3" | "wav")
                                    || cat_l.contains("media")
                                    || cat_l.contains("image")
                                    || cat_l.contains("video");
                                if !is_media {
                                    return false;
                                }
                            }
                            filter => {
                                let filter_l = filter.to_lowercase();
                                let cat_l = n.category.to_lowercase();
                                let matches_cat = cat_l.contains(&filter_l);
                                let matches_vp = search_index
                                    .find_by_hash(&n.hash)
                                    .map(|item| {
                                        item.virtual_paths
                                            .iter()
                                            .any(|vp| vp.to_lowercase().contains(&filter_l))
                                    })
                                    .unwrap_or(false);
                                if !matches_cat && !matches_vp {
                                    return false;
                                }
                            }
                        }
                    }

                    if !search_query.trim().is_empty() {
                        let q = search_query.to_lowercase();
                        let matches_name = n.file.to_lowercase().contains(&q);
                        let matches_cat = n.category.to_lowercase().contains(&q);
                        let matches_semantic = search_highlight_hashes.contains(&n.hash);
                        if !matches_name && !matches_cat && !matches_semantic {
                            return false;
                        }
                    }

                    true
                })
                .collect();

            let row_h = 36.0;
            let num_rows = filtered_nodes.len();

            // Keyboard navigation in File Ledger (Arrow Up/Down / J/K, Enter to Open)
            let search_focused = ui.ctx().memory(|m| m.has_focus(Id::unique("axel_semantic_search_input")));
            if !search_focused && !filtered_nodes.is_empty() {
                let (down, up, enter) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J),
                        i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K),
                        i.key_pressed(egui::Key::Enter),
                    )
                });

                if down {
                    let next_idx = match selected_node {
                        Some(cur_hash) => filtered_nodes
                            .iter()
                            .position(|n| n.hash == cur_hash)
                            .map(|i| (i + 1).min(filtered_nodes.len() - 1))
                            .unwrap_or(0),
                        None => 0,
                    };
                    galaxy_action = GalaxyAction::SelectNode(filtered_nodes[next_idx].hash.clone());
                } else if up {
                    let prev_idx = match selected_node {
                        Some(cur_hash) => filtered_nodes
                            .iter()
                            .position(|n| n.hash == cur_hash)
                            .map(|i| i.saturating_sub(1))
                            .unwrap_or(0),
                        None => 0,
                    };
                    galaxy_action = GalaxyAction::SelectNode(filtered_nodes[prev_idx].hash.clone());
                } else if enter {
                    if let Some(cur_hash) = selected_node {
                        if let Some(node) = filtered_nodes.iter().find(|n| n.hash == cur_hash) {
                            panel_action = PanelAction::OpenFileExternal(node.path.clone());
                        }
                    }
                }
            }

            ScrollArea::vertical()
                .id_salt("workstation_file_ledger_rows")
                .auto_shrink([false, false])
                .show_rows(ui, row_h, num_rows, |ui, row_range| {
                    for idx in row_range {
                        if let Some(node) = filtered_nodes.get(idx) {
                            let is_selected = selected_node == Some(&node.hash);
                            let (row_rect, row_resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), row_h - 2.0),
                                Sense::click(),
                            );

                            let row_bg = if is_selected {
                                with_alpha(p.indigo, 48)
                            } else if row_resp.hovered() {
                                p.bg_surface_hover
                            } else if idx % 2 == 1 {
                                with_alpha(p.bg_surface, 110)
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(row_rect, CornerRadius::same(6), row_bg);

                            if is_selected {
                                ui.painter().rect_stroke(
                                    row_rect,
                                    CornerRadius::same(6),
                                    Stroke::new(1.0, with_alpha(p.indigo, 210)),
                                    StrokeKind::Inside,
                                );
                            }

                            // Check Privacy Shield status for sensitive documents
                            let kws: Vec<String> = search_index
                                .find_by_hash(&node.hash)
                                .map(|i| i.keywords.clone())
                                .unwrap_or_default();
                            let is_sensitive = is_sensitive_item(&node.category, &node.file, &kws, None);
                            let is_revealed = active_revealed
                                .as_ref()
                                .map(|(h, _)| h == &node.hash)
                                .unwrap_or(false);
                            let is_shielded = is_sensitive && !is_revealed;

                            let row_w = row_rect.width();
                            let row_left = row_rect.left();
                            let row_right = row_rect.right();
                            let row_y = row_rect.center().y;

                            let file_col_w = (row_w * 0.28).clamp(140.0, 240.0);
                            let file_col_x = row_left + 28.0;
                            let cat_col_x = file_col_x + file_col_w + 12.0;

                            let size_col_w = 60.0;
                            let size_col_x = row_right - 8.0; // right-aligned text
                            let sym_col_right = size_col_x - size_col_w - 14.0; // right-aligned text

                            // Icon
                            let ic_center = Pos2::new(row_left + 14.0, row_y);
                            if is_shielded {
                                draw_icon_shield(ui.painter(), ic_center, 13.0, p.copper_bright);
                            } else {
                                draw_icon_file_type(
                                    ui.painter(),
                                    ic_center,
                                    14.0,
                                    &node.file_type,
                                    if is_selected { p.indigo_bright } else { p.ink_muted },
                                );
                            }

                            // Duplicate Badge
                            let is_dup = search_index
                                .find_by_hash(&node.hash)
                                .map(|i| i.is_duplicate || i.physical_paths.len() > 1)
                                .unwrap_or(false);

                            // File Name
                            let name_max_w = if is_dup { file_col_w - 38.0 } else { file_col_w - 6.0 };
                            let max_chars = ((name_max_w / 7.2) as usize).clamp(8, 30);
                            let display_name = if is_shielded {
                                let ext = Path::new(&node.file)
                                    .extension()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("doc");
                                format!("••••••••••.{}", ext)
                            } else {
                                truncate_label(&node.file, max_chars)
                            };

                            ui.painter().text(
                                Pos2::new(file_col_x, row_y),
                                Align2::LEFT_CENTER,
                                display_name,
                                font_medium(11.5),
                                if is_shielded {
                                    p.copper_bright
                                } else if is_selected {
                                    p.ink_primary
                                } else {
                                    p.ink_secondary
                                },
                            );

                            if is_dup {
                                let badge_rect = Rect::from_center_size(
                                    Pos2::new(file_col_x + file_col_w - 18.0, row_y),
                                    Vec2::new(32.0, 15.0),
                                );
                                ui.painter().rect_filled(
                                    badge_rect,
                                    CornerRadius::same(3),
                                    with_alpha(p.copper_bright, 45),
                                );
                                ui.painter().text(
                                    badge_rect.center(),
                                    Align2::CENTER_CENTER,
                                    "DUP",
                                    font_semibold(9.0),
                                    p.copper_bright,
                                );
                            }

                            // Category / Virtual Path
                            let cat_max_w = (sym_col_right - 130.0 - cat_col_x).max(40.0);
                            let cat_chars = ((cat_max_w / 7.0) as usize).clamp(6, 32);
                            let accent_name = category_accent(&node.category);
                            let cat_col = if is_shielded { p.copper } else { accent_color(accent_name) };
                            let cat_label = if is_shielded {
                                "[Shielded Identity]".to_string()
                            } else {
                                truncate_label(&node.category, cat_chars)
                            };
                            ui.painter().text(
                                Pos2::new(cat_col_x, row_y),
                                Align2::LEFT_CENTER,
                                cat_label,
                                font_sans(11.0),
                                cat_col,
                            );

                            // Symbols / Keywords
                            if is_shielded {
                                ui.painter().text(
                                    Pos2::new(sym_col_right, row_y),
                                    Align2::RIGHT_CENTER,
                                    "[Masked]",
                                    font_mono(10.0),
                                    p.copper_bright,
                                );
                            } else if let Some(item) = search_index.find_by_hash(&node.hash) {
                                if !item.symbols.is_empty() {
                                    let syms = item
                                        .symbols
                                        .iter()
                                        .take(2)
                                        .map(|s| format!("@{}", s))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    ui.painter().text(
                                        Pos2::new(sym_col_right, row_y),
                                        Align2::RIGHT_CENTER,
                                        truncate_label(&syms, 18),
                                        font_mono(10.0),
                                        p.ink_muted,
                                    );
                                } else if !item.keywords.is_empty() {
                                    let kws = item
                                        .keywords
                                        .iter()
                                        .take(2)
                                        .map(|k| format!("#{}", k))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    ui.painter().text(
                                        Pos2::new(sym_col_right, row_y),
                                        Align2::RIGHT_CENTER,
                                        truncate_label(&kws, 18),
                                        font_sans(10.0),
                                        p.ink_muted,
                                    );
                                }
                            }

                            // Right-aligned Size
                            let size_str = format_bytes(node.size_bytes);
                            ui.painter().text(
                                Pos2::new(size_col_x, row_y),
                                Align2::RIGHT_CENTER,
                                size_str,
                                font_mono(10.5),
                                p.ink_muted,
                            );

                            if row_resp.clicked() {
                                galaxy_action = GalaxyAction::SelectNode(node.hash.clone());
                            }
                            if row_resp.double_clicked() {
                                panel_action = PanelAction::OpenFileExternal(node.path.clone());
                            }
                        }
                    }
                });
        });

    // ==========================================
    // COLUMN 3: Intelligence Inspector
    // ==========================================
    if right_w > 0.0 {
        let mut right_ui = child_ui.new_child(
            egui::UiBuilder::new()
                .max_rect(right_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );

        Frame::new()
            .fill(p.bg_chrome)
            .stroke(Stroke::new(1.0, p.border_subtle))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(12))
            .show(&mut right_ui, |ui| {
                ui.set_width(right_w - 24.0);
                ui.set_height(total_h - 24.0);

                if let Some(detail) = active_detail {
                    render_inspector_content(
                        ui,
                        detail,
                        confirm_delete,
                        &mut panel_action,
                        &mut galaxy_action,
                        &active_revealed,
                        id_revealed,
                        now,
                        &p,
                    );
                } else {
                    render_inspector_empty_state(ui, search_index, snapshot, &mut panel_action, &p);
                }
            });
    }

    (galaxy_action, panel_action)
}

fn render_smart_view_button(
    ui: &mut Ui,
    icon_kind: &str,
    label: &str,
    count: usize,
    is_active: bool,
    accent: Color32,
    p: &ThemePalette,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let bg = if is_active {
        with_alpha(accent, 42)
    } else if resp.hovered() {
        p.bg_surface_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
    if is_active {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, with_alpha(accent, 140)),
            StrokeKind::Inside,
        );
    }

    // Procedural vector icon
    let ic_center = Pos2::new(rect.left() + 14.0, rect.center().y);
    draw_icon_smart_view(
        ui.painter(),
        ic_center,
        14.0,
        icon_kind,
        if is_active { accent } else { p.ink_muted },
    );

    // Label
    ui.painter().text(
        Pos2::new(rect.left() + 28.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font_medium(11.5),
        if is_active { accent } else { p.ink_primary },
    );

    // Right-aligned Count Badge
    let count_str = format!("{}", count);
    ui.painter().text(
        Pos2::new(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        count_str,
        font_mono(10.5),
        if is_active { accent } else { p.ink_muted },
    );

    resp.clicked()
}

fn render_virtual_folder_branch(
    ui: &mut Ui,
    folder: &VirtualFolder,
    depth: usize,
    active_filter: Option<&str>,
    panel_action: &mut PanelAction,
    p: &ThemePalette,
) {
    let is_active = active_filter == Some(&folder.name) || active_filter == Some(&folder.path);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());

    let bg = if is_active {
        with_alpha(p.indigo, 40)
    } else if resp.hovered() {
        p.bg_surface_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);

    let indent = 8.0 + (depth as f32) * 12.0;
    let ic_center = Pos2::new(rect.left() + indent + 6.0, rect.center().y);
    draw_icon_folder(
        ui.painter(),
        ic_center,
        12.0,
        if is_active { p.copper_bright } else { p.indigo_bright },
    );

    let label_pos = Pos2::new(rect.left() + indent + 18.0, rect.center().y);
    let count = folder.total_item_count();
    let label = format!("{} ({})", truncate_label(&folder.name, 16), count);
    ui.painter().text(
        label_pos,
        Align2::LEFT_CENTER,
        label,
        font_sans(10.5),
        if is_active { p.indigo_bright } else { p.ink_secondary },
    );

    if resp.clicked() {
        *panel_action = PanelAction::SetCategoryFilter(Some(folder.name.clone()));
    }

    // Render subfolders (up to 2 levels deep for clean hierarchy)
    if depth < 2 {
        for (_child_name, child_folder) in &folder.subfolders {
            render_virtual_folder_branch(
                ui,
                child_folder,
                depth + 1,
                active_filter,
                panel_action,
                p,
            );
        }
    }
}

fn render_inspector_content(
    ui: &mut Ui,
    detail: &DocumentDetail,
    confirm_delete: &mut bool,
    panel_action: &mut PanelAction,
    galaxy_action: &mut GalaxyAction,
    active_revealed: &Option<(String, f64)>,
    id_revealed: Id,
    now: f64,
    p: &ThemePalette,
) {
    let accent_name = category_accent(&detail.item.category);
    let accent = accent_color(accent_name);
    let bright = accent_bright(accent_name);

    ScrollArea::vertical()
        .id_salt("workstation_inspector_content_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                let (ic_rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                ui.painter().rect_filled(
                    ic_rect,
                    CornerRadius::same(7),
                    with_alpha(accent, 45),
                );
                draw_icon_file_type(ui.painter(), ic_rect.center(), 16.0, &detail.file_type, bright);

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(truncate_label(&detail.item.file, 22))
                            .font(font_semibold(13.0))
                            .color(p.ink_primary),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            detail.item.category,
                            format_bytes(detail.size_bytes)
                        ))
                        .font(font_sans(10.5))
                        .color(bright),
                    );
                });

                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    let (c_rect, c_resp) =
                        ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                    draw_icon_close(
                        ui.painter(),
                        c_rect.center(),
                        10.0,
                        if c_resp.hovered() { p.ink_primary } else { p.ink_muted },
                    );
                    if c_resp.clicked() {
                        *galaxy_action = GalaxyAction::ClearSelection;
                        *panel_action = PanelAction::CloseInspector;
                    }
                });
            });
            ui.add_space(8.0);

            // SHA-256 & Status Card
            let short_hash: String = detail.item.hash.chars().take(12).collect();
            let rel = format_relative_time(detail.item.indexed_at.as_deref(), Utc::now());
            Frame::new()
                .fill(p.bg_surface)
                .stroke(Stroke::new(1.0, p.border_subtle))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("SHA {short_hash}"))
                                .font(font_mono(10.0))
                                .color(p.ink_muted),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(if detail.file_exists {
                                    format!("{rel} · On disk")
                                } else {
                                    format!("{rel} · Missing")
                                })
                                .font(font_sans(10.0))
                                .color(if detail.file_exists { p.emerald } else { p.danger }),
                            );
                        });
                    });
                });
            ui.add_space(10.0);

            // Physical Copies & Safe OS Trashing
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
                            egui::RichText::new(format!("{}{}", prefix, truncate_label(ppath, 20)))
                                .font(font_mono(10.0))
                                .color(if is_primary { p.ink_primary } else { p.copper_bright }),
                        );

                        if !is_primary {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (tr_rect, tr_resp) =
                                    ui.allocate_exact_size(Vec2::new(56.0, 19.0), Sense::click());
                                let tr_bg = if tr_resp.hovered() {
                                    p.danger
                                } else {
                                    with_alpha(p.danger, 45)
                                };
                                ui.painter().rect_filled(tr_rect, CornerRadius::same(4), tr_bg);
                                let tr_icon_pos = Pos2::new(tr_rect.left() + 9.0, tr_rect.center().y);
                                draw_icon_trash(
                                    ui.painter(),
                                    tr_icon_pos,
                                    11.0,
                                    if tr_resp.hovered() { Color32::WHITE } else { p.danger },
                                );
                                ui.painter().text(
                                    Pos2::new(tr_rect.left() + 18.0, tr_rect.center().y),
                                    Align2::LEFT_CENTER,
                                    "Trash",
                                    font_medium(9.5),
                                    if tr_resp.hovered() { Color32::WHITE } else { p.danger },
                                );
                                if tr_resp.clicked() {
                                    *panel_action = PanelAction::TrashDuplicateFile {
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

            // Virtual Multi-Pointers
            if !detail.item.virtual_paths.is_empty() {
                ui.label(
                    egui::RichText::new("Virtual Multi-Pointers")
                        .font(font_medium(11.0))
                        .color(p.ink_muted),
                );
                ui.add_space(4.0);
                for vp in &detail.item.virtual_paths {
                    ui.horizontal(|ui| {
                        let (ic_rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        draw_icon_folder(ui.painter(), ic_rect.center(), 11.0, p.indigo_bright);
                        ui.label(
                            egui::RichText::new(truncate_label(vp, 28))
                                .font(font_mono(10.0))
                                .color(p.ink_secondary),
                        );
                    });
                }
                ui.add_space(8.0);
            }

            // Extracted AST Symbols
            if !detail.item.symbols.is_empty() {
                ui.label(
                    egui::RichText::new("Extracted AST Symbols")
                        .font(font_medium(11.0))
                        .color(p.ink_muted),
                );
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for sym in &detail.item.symbols {
                        let chip = format!("@{}", sym);
                        if render_chip_button(ui, &chip, p.indigo, p) {
                            *panel_action = PanelAction::AppendSearchToken(chip);
                        }
                    }
                });
                ui.add_space(8.0);
            }

            // Extracted Keywords
            if !detail.item.keywords.is_empty() {
                ui.label(
                    egui::RichText::new("Extracted Keywords")
                        .font(font_medium(11.0))
                        .color(p.ink_muted),
                );
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for kw in &detail.item.keywords {
                        let chip = format!("#{}", kw);
                        if render_chip_button(ui, &chip, accent, p) {
                            *panel_action = PanelAction::AppendSearchToken(chip);
                        }
                    }
                });
                ui.add_space(8.0);
            }

            // Shoulder-Surfing Privacy Shield
            let is_sensitive = is_sensitive_item(
                &detail.item.category,
                &detail.item.file,
                &detail.item.keywords,
                Some(&detail.text_preview),
            );

            let is_revealed = active_revealed
                .as_ref()
                .map(|(h, _)| h == &detail.item.hash)
                .unwrap_or(false);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Content Preview")
                        .font(font_medium(11.0))
                        .color(p.ink_muted),
                );
                if is_sensitive && is_revealed {
                    if let Some((_, rev_time)) = active_revealed {
                        let remaining = (30.0 - (now - rev_time)).max(0.0).ceil() as u32;
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (rem_rect, rem_resp) =
                                ui.allocate_exact_size(Vec2::new(76.0, 18.0), Sense::click());
                            let rem_bg = if rem_resp.hovered() {
                                p.copper_bright
                            } else {
                                with_alpha(p.copper_bright, 40)
                            };
                            ui.painter().rect_filled(rem_rect, CornerRadius::same(4), rem_bg);
                            ui.painter().text(
                                rem_rect.center(),
                                Align2::CENTER_CENTER,
                                format!("Hide ({}s)", remaining),
                                font_medium(9.5),
                                if rem_resp.hovered() { Color32::WHITE } else { p.copper_bright },
                            );
                            if rem_resp.clicked() {
                                ui.ctx().data_mut(|d| d.insert_temp(id_revealed, None::<(String, f64)>));
                                ui.ctx().request_repaint();
                            }
                        });
                    }
                }
            });
            ui.add_space(4.0);

            let preview_h = 130.0;
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
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(preview_h);

                    if is_sensitive && !is_revealed {
                        ui.vertical_centered(|ui| {
                            ui.add_space(14.0);
                            let (sh_rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                            draw_icon_shield(ui.painter(), sh_rect.center(), 20.0, p.copper_bright);
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("Privacy Shield Active")
                                    .font(font_semibold(11.5))
                                    .color(p.copper_bright),
                            );
                            ui.label(
                                egui::RichText::new("Sensitive Document Masked")
                                    .font(font_sans(9.5))
                                    .color(p.ink_muted),
                            );
                            ui.add_space(8.0);
                            let (rev_rect, rev_resp) =
                                ui.allocate_exact_size(Vec2::new(104.0, 20.0), Sense::click());
                            ui.painter().rect_filled(
                                rev_rect,
                                CornerRadius::same(4),
                                if rev_resp.hovered() { p.bg_surface_hover } else { p.bg_surface },
                            );
                            ui.painter().text(
                                rev_rect.center(),
                                Align2::CENTER_CENTER,
                                "Click to Reveal",
                                font_medium(10.0),
                                p.ink_primary,
                            );
                            if rev_resp.clicked() {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(id_revealed, Some((detail.item.hash.clone(), now)))
                                });
                                ui.ctx().request_repaint();
                            }
                        });
                    } else {
                        ScrollArea::vertical()
                            .id_salt("workstation_preview_text_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(&detail.text_preview)
                                        .font(font_mono(10.5))
                                        .color(p.ink_secondary),
                                );
                            });
                    }
                });

            ui.add_space(10.0);

            // Action Buttons
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new("Open File").font(font_medium(10.5))).clicked() {
                    *panel_action = PanelAction::OpenFileExternal(detail.item.path.clone());
                }
                if ui.button(egui::RichText::new("In Folder").font(font_medium(10.5))).clicked() {
                    *panel_action = PanelAction::RevealFolderExternal(detail.item.path.clone());
                }
                if *confirm_delete {
                    if ui.button(egui::RichText::new("Confirm Remove").font(font_medium(10.5)).color(p.danger)).clicked() {
                        *confirm_delete = false;
                        *panel_action = PanelAction::DeleteDocument {
                            hash: detail.item.hash.clone(),
                            delete_file: false,
                        };
                    }
                } else if ui.button(egui::RichText::new("Remove").font(font_medium(10.5)).color(p.ink_muted)).clicked() {
                    *confirm_delete = true;
                }
            });
        });
}

fn render_inspector_empty_state(
    ui: &mut Ui,
    search_index: &DocumentIndex,
    snapshot: &GraphSnapshot,
    panel_action: &mut PanelAction,
    p: &ThemePalette,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        let (ic_rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
        draw_icon_workstation(ui.painter(), ic_rect.center(), 32.0, with_alpha(p.indigo, 120));
        ui.add_space(12.0);

        ui.label(
            egui::RichText::new("Intelligence Inspector")
                .font(font_semibold(13.5))
                .color(p.ink_primary),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Select any file from the ledger to inspect its AST symbols, semantic keywords, physical duplicate paths, and privacy controls.",
            )
            .font(font_sans(11.0))
            .color(p.ink_muted),
        );

        ui.add_space(24.0);
        Frame::new()
            .fill(p.bg_surface)
            .stroke(Stroke::new(1.0, p.border_subtle))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Indexed Files").font(font_sans(11.0)).color(p.ink_muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(egui::RichText::new(search_index.items().len().to_string()).font(font_semibold(11.5)).color(p.ink_primary));
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Taxonomy Clusters").font(font_sans(11.0)).color(p.ink_muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(egui::RichText::new(snapshot.hubs.len().to_string()).font(font_semibold(11.5)).color(p.indigo_bright));
                    });
                });
                ui.add_space(6.0);
                let dup_count = search_index.items().iter().filter(|i| i.is_duplicate || i.physical_paths.len() > 1).count();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Duplicates Tracked").font(font_sans(11.0)).color(p.ink_muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(egui::RichText::new(dup_count.to_string()).font(font_semibold(11.5)).color(p.copper_bright));
                    });
                });

                if dup_count > 0 {
                    ui.add_space(10.0);
                    let id_confirm_all = Id::unique("workstation_confirm_trash_all");
                    let confirming = ui
                        .ctx()
                        .data_mut(|d| d.get_temp::<bool>(id_confirm_all).unwrap_or(false));
                    if confirming {
                        ui.label(
                            egui::RichText::new("Move redundant duplicates to OS Trash?")
                                .font(font_sans(10.0))
                                .color(p.copper_bright),
                        );
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(
                                    egui::RichText::new("Confirm Trash")
                                        .font(font_medium(10.0))
                                        .color(p.danger),
                                )
                                .clicked()
                            {
                                ui.ctx().data_mut(|d| d.insert_temp(id_confirm_all, false));
                                *panel_action = PanelAction::TrashAllDuplicates;
                            }
                            if ui
                                .button(
                                    egui::RichText::new("Cancel")
                                        .font(font_sans(10.0))
                                        .color(p.ink_muted),
                                )
                                .clicked()
                            {
                                ui.ctx().data_mut(|d| d.insert_temp(id_confirm_all, false));
                            }
                        });
                    } else {
                        let (b_rect, b_resp) =
                            ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
                        let fill = if b_resp.hovered() {
                            p.danger
                        } else {
                            with_alpha(p.danger, 50)
                        };
                        ui.painter().rect_filled(b_rect, CornerRadius::same(6), fill);
                        let text_col = if b_resp.hovered() { Color32::WHITE } else { p.danger };
                        ui.painter().text(
                            b_rect.center(),
                            Align2::CENTER_CENTER,
                            "Trash All Duplicates (Safe)",
                            font_medium(10.5),
                            text_col,
                        );
                        if b_resp.clicked() {
                            ui.ctx().data_mut(|d| d.insert_temp(id_confirm_all, true));
                        }
                    }
                }
            });
    });
}

fn render_chip_button(ui: &mut Ui, label: &str, accent: Color32, p: &ThemePalette) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font_mono(10.0),
        p.ink_primary,
    );
    let size = Vec2::new(galley.size().x + 14.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let fill = if resp.hovered() {
        with_alpha(accent, 60)
    } else {
        with_alpha(accent, 25)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, with_alpha(accent, 120)),
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, Color32::TRANSPARENT);

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

pub fn get_process_rss_mb() -> Option<f32> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
            if let Some(pages_str) = content.split_whitespace().nth(1) {
                if let Ok(pages) = pages_str.parse::<u64>() {
                    let mb = (pages * 4096) as f32 / (1024.0 * 1024.0);
                    if mb > 0.0 {
                        return Some(mb);
                    }
                }
            }
        }
    }
    None
}

fn render_daemon_status_card(ui: &mut Ui, stats: &SystemStats, p: &ThemePalette) {
    let card_w = ui.available_width();
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(card_w, 68.0), Sense::hover());

    // Inset card background & border
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        with_alpha(p.bg_void, 200),
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, with_alpha(p.border_subtle, 180)),
        StrokeKind::Inside,
    );

    let left_pad = rect.left() + 10.0;
    let top_pad = rect.top() + 8.0;

    // Line 1: Pulsing Green Dot + "Daemon: Active" + real RSS measurement
    let dot_center = Pos2::new(left_pad + 4.0, top_pad + 7.0);
    // Outer halo glow
    ui.painter().circle_filled(dot_center, 6.0, with_alpha(p.emerald, 45));
    // Core indicator dot
    ui.painter().circle_filled(dot_center, 3.5, p.emerald);

    ui.painter().text(
        Pos2::new(left_pad + 16.0, top_pad + 7.0),
        Align2::LEFT_CENTER,
        "Daemon: Active",
        font_semibold(11.0),
        p.ink_primary,
    );

    let rss_str = match get_process_rss_mb() {
        Some(mb) => format!("· {:.1}MB RSS", mb),
        None => "· RSS: n/a".to_string(),
    };
    ui.painter().text(
        Pos2::new(rect.right() - 8.0, top_pad + 7.0),
        Align2::RIGHT_CENTER,
        rss_str,
        font_mono(10.0),
        p.emerald,
    );

    // Line 2: Watching ~/Downloads
    let folder_display = if stats.input_folder.is_empty() {
        "~/Downloads".to_string()
    } else if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && stats.input_folder.starts_with(&home) {
            stats.input_folder.replacen(&home, "~", 1)
        } else {
            stats.input_folder.clone()
        }
    } else {
        stats.input_folder.clone()
    };

    ui.painter().text(
        Pos2::new(left_pad, top_pad + 27.0),
        Align2::LEFT_CENTER,
        format!("Watching {}", truncate_label(&folder_display, 22)),
        font_medium(10.5),
        p.ink_secondary,
    );

    // Line 3: AI Intelligence Engine status
    let (engine_label, engine_color) = if stats.groq_enabled {
        ("Groq LLM (Active)", p.indigo_bright)
    } else {
        ("TF-IDF Offline Engine", p.copper)
    };
    ui.painter().text(
        Pos2::new(left_pad, top_pad + 46.0),
        Align2::LEFT_CENTER,
        engine_label,
        font_mono(9.5),
        engine_color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rss_memory_measurement() {
        let rss = get_process_rss_mb();
        #[cfg(target_os = "linux")]
        {
            assert!(rss.is_some());
            let val = rss.unwrap();
            println!("Process RSS memory: {:.2} MB", val);
            assert!(val > 0.0);
            assert!(val < 150.0);
        }
    }
}
