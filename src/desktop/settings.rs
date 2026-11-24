use crate::ai::{test_groq_api_key, GroqKeyVerification};
use crate::config::Config;
use crate::desktop::chrome::PanelAction;
use crate::desktop::theme::{
    active_theme, draw_icon_close, draw_icon_gear, font_medium, font_mono, font_sans,
    font_semibold, palette, set_active_theme, with_alpha, ThemePreset,
};
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Frame, Id, Layout, Margin, Pos2, Rect,
    ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Intelligence,
    Extraction,
    Workspace,
    Appearance,
}

#[derive(Clone)]
pub struct DesktopSettingsState {
    pub open: bool,
    pub active_tab: SettingsTab,
    pub api_key_input: String,
    pub show_api_key_plaintext: bool,
    pub selected_model: String,
    pub discovered_models: Vec<String>,
    pub is_testing_key: bool,
    pub test_result: Option<Result<GroqKeyVerification, String>>,
    pub force_offline: bool,
    pub enable_cloud_code: bool,
    pub enable_ocr: bool,
    pub max_file_size_mb: usize,
    pub excluded_patterns_str: String,
    pub selected_theme: String,
    pub top_k_results: usize,
    pub max_keywords: usize,
    pub port: u16,
    pub host: String,
    pub save_status: Option<String>,

    // Async key verification channel
    test_result_receiver: Arc<Mutex<Option<Result<GroqKeyVerification, String>>>>,
    test_in_flight: Arc<AtomicBool>,
}

impl Default for DesktopSettingsState {
    fn default() -> Self {
        Self::from_config(&Config::default())
    }
}

impl DesktopSettingsState {
    pub fn from_config(config: &Config) -> Self {
        let mut models = vec![
            config.model_name.clone(),
            "llama-3.3-70b-versatile".to_string(),
            "llama-3.1-8b-instant".to_string(),
            "mixtral-8x7b-32768".to_string(),
            "gemma2-9b-it".to_string(),
            "qwen/qwen3.8-27b".to_string(),
        ];
        models.sort();
        models.dedup();

        Self {
            open: false,
            active_tab: SettingsTab::Intelligence,
            api_key_input: config.groq_api_key.clone().unwrap_or_default(),
            show_api_key_plaintext: false,
            selected_model: config.model_name.clone(),
            discovered_models: models,
            is_testing_key: false,
            test_result: None,
            force_offline: config.force_offline,
            enable_cloud_code: config.enable_cloud_code,
            enable_ocr: config.enable_ocr,
            max_file_size_mb: config.max_file_size_mb,
            excluded_patterns_str: config.excluded_patterns.join(", "),
            selected_theme: config.theme.clone(),
            top_k_results: config.top_k_results,
            max_keywords: config.max_keywords,
            port: config.port,
            host: config.host.clone(),
            save_status: None,
            test_result_receiver: Arc::new(Mutex::new(None)),
            test_in_flight: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn to_config(&self, base: &Config) -> Config {
        let trimmed_key = self.api_key_input.trim();
        let groq_api_key = if trimmed_key.is_empty() {
            None
        } else {
            Some(trimmed_key.to_string())
        };

        let excluded_patterns: Vec<String> = self
            .excluded_patterns_str
            .split([',', '\n'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Config {
            groq_api_key,
            model_name: self.selected_model.clone(),
            model_fallbacks: base.model_fallbacks.clone(),
            input_folder: base.input_folder.clone(),
            index_file: base.index_file.clone(),
            max_keywords: self.max_keywords,
            top_k_results: self.top_k_results,
            port: self.port,
            host: self.host.clone(),
            api_token: base.api_token.clone(),
            allowed_origins: base.allowed_origins.clone(),
            force_offline: self.force_offline,
            enable_cloud_code: self.enable_cloud_code,
            enable_ocr: self.enable_ocr,
            max_file_size_mb: self.max_file_size_mb,
            excluded_patterns,
            theme: self.selected_theme.clone(),
            has_seen_onboarding: base.has_seen_onboarding,
        }
    }

    pub fn poll_test_result(&mut self) {
        if self.test_in_flight.load(Ordering::Relaxed) {
            let mut lock = self.test_result_receiver.lock().unwrap();
            if let Some(res) = lock.take() {
                self.is_testing_key = false;
                self.test_in_flight.store(false, Ordering::Relaxed);
                if let Ok(ref verif) = res {
                    if verif.valid && !verif.available_models.is_empty() {
                        for m in &verif.available_models {
                            if !self.discovered_models.contains(m) {
                                self.discovered_models.push(m.clone());
                            }
                        }
                        self.discovered_models.sort();
                    }
                }
                self.test_result = Some(res);
            }
        }
    }

    pub fn trigger_key_test(&mut self) {
        let key = self.api_key_input.trim().to_string();
        if key.is_empty() {
            self.test_result = Some(Err("API key cannot be empty".to_string()));
            return;
        }

        self.is_testing_key = true;
        self.test_result = None;
        self.test_in_flight.store(true, Ordering::Relaxed);

        let receiver = Arc::clone(&self.test_result_receiver);
        let in_flight = Arc::clone(&self.test_in_flight);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            let res = match runtime {
                Ok(rt) => rt.block_on(test_groq_api_key(&key)),
                Err(e) => Err(format!("Tokio runtime error: {e}")),
            };

            let mut lock = receiver.lock().unwrap();
            *lock = Some(res);
            in_flight.store(true, Ordering::Relaxed);
        });
    }
}

pub fn render_settings_modal(
    ctx: &egui::Context,
    state: &mut DesktopSettingsState,
    base_config: &Config,
) -> PanelAction {
    let mut action = PanelAction::None;
    if !state.open {
        return action;
    }

    state.poll_test_result();
    let p = palette();

    let screen_rect = ctx.content_rect();
    let modal_w = 640.0_f32.min(screen_rect.width() - 32.0);
    let modal_h = 580.0_f32.min(screen_rect.height() - 40.0);

    // Dimmed background scrim overlay
    let scrim_id = Id::unique("axel_settings_scrim");
    egui::Area::new(scrim_id)
        .order(egui::Order::Middle)
        .fixed_pos(screen_rect.min)
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen_rect.size(), Sense::click());
            ui.painter().rect_filled(
                screen_rect,
                CornerRadius::ZERO,
                Color32::from_black_alpha(150),
            );
            if resp.clicked() {
                state.open = false;
                action = PanelAction::CloseSettingsModal;
            }
        });

    let modal_rect = Rect::from_center_size(screen_rect.center(), Vec2::new(modal_w, modal_h));

    egui::Area::new(Id::unique("axel_settings_dialog"))
        .order(egui::Order::Foreground)
        .fixed_pos(modal_rect.min)
        .show(ctx, |ui| {
            ui.set_width(modal_w);
            ui.set_height(modal_h);

            Frame::new()
                .fill(with_alpha(p.bg_chrome, 252))
                .stroke(Stroke::new(1.0, p.border_strong))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(20))
                .show(ui, |ui| {
                    // Header Bar
                    ui.horizontal(|ui| {
                        let (ic_rect, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                        draw_icon_gear(ui.painter(), ic_rect.center(), 16.0, p.copper_bright);

                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("AXEL Application Settings")
                                    .font(font_semibold(15.0))
                                    .color(p.ink_primary),
                            );
                            ui.label(
                                egui::RichText::new("Dynamic AI configuration, OCR extraction, and storage controls")
                                    .font(font_sans(11.0))
                                    .color(p.ink_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (c_rect, c_resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                            if c_resp.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            draw_icon_close(
                                ui.painter(),
                                c_rect.center(),
                                12.0,
                                if c_resp.hovered() { p.ink_primary } else { p.ink_muted },
                            );
                            if c_resp.clicked() {
                                state.open = false;
                                action = PanelAction::CloseSettingsModal;
                            }
                        });
                    });

                    ui.add_space(14.0);

                    // Tab Navigation
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        tab_button(ui, "AI & Cloud", state.active_tab == SettingsTab::Intelligence, || {
                            state.active_tab = SettingsTab::Intelligence;
                        });
                        tab_button(ui, "Extraction & OCR", state.active_tab == SettingsTab::Extraction, || {
                            state.active_tab = SettingsTab::Extraction;
                        });
                        tab_button(ui, "Workspace & Network", state.active_tab == SettingsTab::Workspace, || {
                            state.active_tab = SettingsTab::Workspace;
                        });
                        tab_button(ui, "Appearance", state.active_tab == SettingsTab::Appearance, || {
                            state.active_tab = SettingsTab::Appearance;
                        });
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(10.0);

                    // Content Area (Scrollable)
                    let body_h = modal_h - 170.0;
                    ScrollArea::vertical()
                        .id_salt("axel_settings_scroll")
                        .max_height(body_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            match state.active_tab {
                                SettingsTab::Intelligence => render_intelligence_tab(ui, state),
                                SettingsTab::Extraction => render_extraction_tab(ui, state),
                                SettingsTab::Workspace => render_workspace_tab(ui, state, base_config),
                                SettingsTab::Appearance => {
                                    if let Some(new_theme_action) = render_appearance_tab(ui, state) {
                                        action = new_theme_action;
                                    }
                                }
                            }
                        });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(10.0);

                    // Modal Footer Actions
                    ui.horizontal(|ui| {
                        let guide_btn = egui::Button::new(
                            egui::RichText::new("Architecture & Privacy Guide")
                                .font(font_medium(11.5))
                                .color(p.copper_bright),
                        )
                        .fill(with_alpha(p.copper, 25))
                        .stroke(Stroke::new(1.0, with_alpha(p.copper_bright, 140)))
                        .corner_radius(CornerRadius::same(6));

                        if ui.add(guide_btn).clicked() {
                            action = PanelAction::ToggleOnboardingGuide;
                        }

                        if let Some(ref status) = state.save_status {
                            ui.label(
                                egui::RichText::new(status)
                                    .font(font_medium(11.5))
                                    .color(p.emerald),
                            );
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;

                            // Save & Apply Button
                            let (s_rect, s_resp) = ui.allocate_exact_size(Vec2::new(140.0, 32.0), Sense::click());
                            if s_resp.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            let s_bg = if s_resp.hovered() { p.indigo_bright } else { p.indigo };
                            ui.painter().rect_filled(s_rect, CornerRadius::same(8), s_bg);
                            ui.painter().text(
                                s_rect.center(),
                                Align2::CENTER_CENTER,
                                "Save & Apply",
                                font_semibold(12.0),
                                Color32::WHITE,
                            );

                            if s_resp.clicked() {
                                let new_config = state.to_config(base_config);
                                if let Err(err) = new_config.save() {
                                    state.save_status = Some(format!("Error: {err}"));
                                } else {
                                    state.save_status = Some("✓ Settings saved to disk & active".to_string());
                                    action = PanelAction::SaveSettings(new_config);
                                }
                            }

                            // Close / Cancel Button
                            let (c_rect, c_resp) = ui.allocate_exact_size(Vec2::new(80.0, 32.0), Sense::click());
                            if c_resp.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            let c_bg = if c_resp.hovered() { p.bg_surface_hover } else { p.bg_surface };
                            ui.painter().rect_filled(c_rect, CornerRadius::same(8), c_bg);
                            ui.painter().rect_stroke(c_rect, CornerRadius::same(8), Stroke::new(1.0, p.border_strong), StrokeKind::Inside);
                            ui.painter().text(
                                c_rect.center(),
                                Align2::CENTER_CENTER,
                                "Cancel",
                                font_medium(12.0),
                                p.ink_secondary,
                            );
                            if c_resp.clicked() {
                                state.open = false;
                                action = PanelAction::CloseSettingsModal;
                            }
                        });
                    });
                });
        });

    action
}

fn tab_button(ui: &mut Ui, label: &str, active: bool, mut on_click: impl FnMut()) {
    let p = palette();
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font_medium(12.0),
        if active { p.ink_primary } else { p.ink_muted },
    );
    let size = Vec2::new(galley.size().x + 24.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let fill = if active {
        with_alpha(p.copper, 40)
    } else if resp.hovered() {
        p.bg_surface_hover
    } else {
        p.bg_surface
    };
    let stroke = if active {
        p.copper_bright
    } else if resp.hovered() {
        p.border_strong
    } else {
        p.border_subtle
    };

    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, stroke), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, Color32::TRANSPARENT);

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() {
        on_click();
    }
}

fn render_intelligence_tab(ui: &mut Ui, state: &mut DesktopSettingsState) {
    let p = palette();

    // Mode Status Card
    let is_offline = state.force_offline;
    let card_bg = if is_offline {
        with_alpha(p.copper, 25)
    } else if !state.api_key_input.trim().is_empty() {
        with_alpha(p.emerald, 25)
    } else {
        p.bg_surface
    };
    let card_stroke = if is_offline {
        p.copper_bright
    } else if !state.api_key_input.trim().is_empty() {
        p.emerald
    } else {
        p.border_subtle
    };

    Frame::new()
        .fill(card_bg)
        .stroke(Stroke::new(1.0, card_stroke))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let dot_col = if is_offline {
                    p.copper_bright
                } else if !state.api_key_input.trim().is_empty() {
                    p.emerald
                } else {
                    p.ink_muted
                };
                let (d_rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().circle_filled(d_rect.center(), 4.5, dot_col);

                let title = if is_offline {
                    "Offline Mode Active: Local Centroid Cosine Similarity Engine"
                } else if !state.api_key_input.trim().is_empty() {
                    "Cloud AI Active: Groq High-Throughput Inference Engine"
                } else {
                    "Local Fallback: No Groq API Key Set"
                };

                ui.label(egui::RichText::new(title).font(font_semibold(12.0)).color(p.ink_primary));
            });

            ui.add_space(4.0);
            let desc = if is_offline {
                "Outbound cloud AI calls are completely suppressed. All semantic clustering uses mathematical vector-space domain centroids with local TF-IDF."
            } else if !state.api_key_input.trim().is_empty() {
                "Files are categorized using high-speed Groq models with local PII and secret redaction shields active before any outbound transmission."
            } else {
                "Input your Groq API key below to unlock fast cloud reasoning, or continue using the local offline centroid classifier."
            };
            ui.label(egui::RichText::new(desc).font(font_sans(11.0)).color(p.ink_muted));
        });

    ui.add_space(14.0);

    // Groq API Key Section
    ui.label(egui::RichText::new("Groq API Key").font(font_semibold(12.5)).color(p.ink_primary));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let text_edit = TextEdit::singleline(&mut state.api_key_input)
            .password(!state.show_api_key_plaintext)
            .hint_text("gsk_...")
            .font(font_mono(11.5))
            .desired_width(ui.available_width() - 150.0);
        ui.add(text_edit);

        // Eye toggle button
        let eye_label = if state.show_api_key_plaintext { "Hide" } else { "Show" };
        if ui.button(egui::RichText::new(eye_label).font(font_sans(11.0))).clicked() {
            state.show_api_key_plaintext = !state.show_api_key_plaintext;
        }

        // Test Key Button
        let btn_text = if state.is_testing_key { "Testing..." } else { "Test Key" };
        let (t_rect, t_resp) = ui.allocate_exact_size(Vec2::new(88.0, 26.0), Sense::click());
        if t_resp.hovered() && !state.is_testing_key {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let t_bg = if state.is_testing_key {
            p.bg_surface
        } else if t_resp.hovered() {
            p.indigo_bright
        } else {
            p.indigo
        };
        ui.painter().rect_filled(t_rect, CornerRadius::same(6), t_bg);
        ui.painter().text(
            t_rect.center(),
            Align2::CENTER_CENTER,
            btn_text,
            font_semibold(11.0),
            Color32::WHITE,
        );

        if t_resp.clicked() && !state.is_testing_key {
            state.trigger_key_test();
        }
    });

    // Verification feedback
    if let Some(ref res) = state.test_result {
        ui.add_space(6.0);
        match res {
            Ok(verif) => {
                let badge_bg = if verif.valid { with_alpha(p.emerald, 30) } else { with_alpha(p.danger, 30) };
                let badge_stroke = if verif.valid { p.emerald } else { p.danger };
                let badge_text = if verif.valid {
                    format!("✓ Validated: {} ms latency | {} models discovered on account", verif.latency_ms, verif.available_models.len())
                } else {
                    format!("✗ Verification Failed: {}", verif.message)
                };

                Frame::new()
                    .fill(badge_bg)
                    .stroke(Stroke::new(1.0, badge_stroke))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(badge_text).font(font_mono(11.0)).color(if verif.valid { p.emerald } else { p.danger }));
                    });
            }
            Err(err) => {
                Frame::new()
                    .fill(with_alpha(p.danger, 30))
                    .stroke(Stroke::new(1.0, p.danger))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(format!("✗ Error: {err}")).font(font_mono(11.0)).color(p.danger));
                    });
            }
        }
    }

    ui.add_space(14.0);

    // Model Selector
    ui.label(egui::RichText::new("Primary AI Model").font(font_semibold(12.5)).color(p.ink_primary));
    ui.label(egui::RichText::new("Dynamically populated from verified account models or standard defaults").font(font_sans(11.0)).color(p.ink_muted));
    ui.add_space(4.0);

    let models = state.discovered_models.clone();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        for m in &models {
            let is_selected = state.selected_model == *m;
            let (m_rect, m_resp) = ui.allocate_exact_size(Vec2::new(180.0, 26.0), Sense::click());
            if m_resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let fill = if is_selected { with_alpha(p.indigo, 45) } else if m_resp.hovered() { p.bg_surface_hover } else { p.bg_surface };
            let stroke = if is_selected { p.indigo_bright } else { p.border_subtle };

            ui.painter().rect_filled(m_rect, CornerRadius::same(6), fill);
            ui.painter().rect_stroke(m_rect, CornerRadius::same(6), Stroke::new(1.0, stroke), StrokeKind::Inside);
            ui.painter().text(
                m_rect.center(),
                Align2::CENTER_CENTER,
                m,
                font_mono(10.5),
                if is_selected { p.ink_primary } else { p.ink_muted },
            );

            if m_resp.clicked() {
                state.selected_model = m.clone();
            }
        }
    });

    ui.add_space(14.0);

    // Privacy & Safety Controls
    ui.label(egui::RichText::new("Privacy & Telemetry Guardrails").font(font_semibold(12.5)).color(p.ink_primary));
    ui.add_space(6.0);

    ui.checkbox(&mut state.force_offline, egui::RichText::new("Force Complete Offline Mode").font(font_sans(12.0)));
    ui.label(
        egui::RichText::new("   Prevents all outbound network requests. Mathematical cosine similarity against domain centroids will be used.")
            .font(font_sans(10.5))
            .color(p.ink_muted),
    );

    ui.add_space(4.0);
    ui.checkbox(&mut state.enable_cloud_code, egui::RichText::new("Enable Cloud Code Analysis (Opt-In)").font(font_sans(12.0)));
    ui.label(
        egui::RichText::new("   Allows sanitized, secret-redacted source code AST snippets to be analyzed by Groq for high-level semantic keywords.")
            .font(font_sans(10.5))
            .color(p.ink_muted),
    );
}

fn render_extraction_tab(ui: &mut Ui, state: &mut DesktopSettingsState) {
    let p = palette();

    ui.label(egui::RichText::new("Extraction Pipeline Configuration").font(font_semibold(13.0)).color(p.ink_primary));
    ui.add_space(8.0);

    let tesseract_detected = crate::extractor::ocr::find_tesseract_binary().is_some();
    ui.checkbox(&mut state.enable_ocr, egui::RichText::new("Enable Optical Character Recognition (OCR)").font(font_sans(12.0)));
    ui.label(
        egui::RichText::new(format!(
            "   Tesseract binary status: {}",
            if tesseract_detected { "Detected on system (/usr/bin/tesseract)" } else { "Not installed (fallback metadata mode active)" }
        ))
        .font(font_mono(10.5))
        .color(if tesseract_detected { p.emerald } else { p.copper_bright }),
    );

    ui.add_space(14.0);

    ui.label(egui::RichText::new(format!("Maximum File Size Limit: {} MB", state.max_file_size_mb)).font(font_medium(12.0)).color(p.ink_primary));
    ui.add(egui::Slider::new(&mut state.max_file_size_mb, 5..=500).suffix(" MB"));
    ui.label(
        egui::RichText::new("   Files exceeding this limit will only have their metadata and symbols extracted to prevent memory exhaustion.")
            .font(font_sans(10.5))
            .color(p.ink_muted),
    );

    ui.add_space(14.0);

    ui.label(egui::RichText::new("Excluded Directories & Patterns").font(font_semibold(12.5)).color(p.ink_primary));
    ui.label(
        egui::RichText::new("Comma-separated list of paths, patterns, and folder names to ignore during scanning:")
            .font(font_sans(11.0))
            .color(p.ink_muted),
    );
    ui.add_space(4.0);
    ui.add(
        TextEdit::multiline(&mut state.excluded_patterns_str)
            .desired_rows(4)
            .font(font_mono(11.0))
            .desired_width(ui.available_width()),
    );
}

fn render_workspace_tab(ui: &mut Ui, state: &mut DesktopSettingsState, base_config: &Config) {
    let p = palette();

    ui.label(egui::RichText::new("Storage & Server Parameters").font(font_semibold(13.0)).color(p.ink_primary));
    ui.add_space(8.0);

    info_row(ui, "Current Workspace Directory", &base_config.input_folder.display().to_string());
    info_row(ui, "Primary Index File", &base_config.index_file.display().to_string());
    info_row(ui, "Application Data Path", &crate::config::app_data_dir().display().to_string());
    info_row(ui, "Settings Persistence File", &crate::config::Config::settings_file_path().display().to_string());

    ui.add_space(14.0);

    ui.label(egui::RichText::new("Search & Indexing Tuners").font(font_semibold(12.5)).color(p.ink_primary));
    ui.add_space(6.0);

    ui.label(egui::RichText::new(format!("Top-K Semantic Results: {}", state.top_k_results)).font(font_medium(12.0)));
    ui.add(egui::Slider::new(&mut state.top_k_results, 1..=30));

    ui.add_space(8.0);
    ui.label(egui::RichText::new(format!("Max Keywords per Document: {}", state.max_keywords)).font(font_medium(12.0)));
    ui.add(egui::Slider::new(&mut state.max_keywords, 5..=30));

    ui.add_space(14.0);
    ui.label(egui::RichText::new("Local API Server Network Binding").font(font_semibold(12.5)).color(p.ink_primary));
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Host:").font(font_sans(11.5)));
        ui.add(TextEdit::singleline(&mut state.host).desired_width(120.0).font(font_mono(11.0)));

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Port:").font(font_sans(11.5)));
        let mut port_str = state.port.to_string();
        if ui.add(TextEdit::singleline(&mut port_str).desired_width(70.0).font(font_mono(11.0))).changed() {
            if let Ok(p) = port_str.parse::<u16>() {
                state.port = p;
            }
        }
    });
}

fn render_appearance_tab(ui: &mut Ui, state: &mut DesktopSettingsState) -> Option<PanelAction> {
    let mut action = None;
    let p = palette();

    ui.label(egui::RichText::new("Visual Atmosphere & Color System").font(font_semibold(13.0)).color(p.ink_primary));
    ui.label(
        egui::RichText::new("Select an orchestrated desktop palette. Switch applies immediately to the active window.")
            .font(font_sans(11.0))
            .color(p.ink_muted),
    );
    ui.add_space(12.0);

    let cur = active_theme();
    let themes = [
        (ThemePreset::NebulaObsidian, "Nebula Obsidian", "Deep black void with vivid indigo accents and copper highlights"),
        (ThemePreset::MidnightCopper, "Midnight Copper", "Warm metallic copper tones with charcoal surfaces and bronze contrast"),
        (ThemePreset::ArcticSlate, "Arctic Slate", "Cool high-contrast slate with emerald telemetry and crisp typography"),
    ];

    for (preset, label, desc) in themes {
        let is_selected = cur == preset;
        let (card_rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        let bg = if is_selected { with_alpha(p.copper, 35) } else if resp.hovered() { p.bg_surface_hover } else { p.bg_surface };
        let stroke = if is_selected { p.copper_bright } else { p.border_subtle };

        ui.painter().rect_filled(card_rect, CornerRadius::same(10), bg);
        ui.painter().rect_stroke(card_rect, CornerRadius::same(10), Stroke::new(if is_selected { 1.5 } else { 1.0 }, stroke), StrokeKind::Inside);

        let dot_pos = Pos2::new(card_rect.left() + 18.0, card_rect.center().y);
        ui.painter().circle_filled(dot_pos, 5.0, if is_selected { p.copper_bright } else { p.ink_muted });

        ui.painter().text(
            Pos2::new(card_rect.left() + 36.0, card_rect.center().y - 8.0),
            Align2::LEFT_CENTER,
            label,
            font_semibold(12.5),
            p.ink_primary,
        );
        ui.painter().text(
            Pos2::new(card_rect.left() + 36.0, card_rect.center().y + 9.0),
            Align2::LEFT_CENTER,
            desc,
            font_sans(10.5),
            p.ink_muted,
        );

        if resp.clicked() {
            set_active_theme(ui.ctx(), preset);
            state.selected_theme = label.to_string();
            action = Some(PanelAction::SetTheme(preset));
        }

        ui.add_space(6.0);
    }

    action
}

fn info_row(ui: &mut Ui, label: &str, val: &str) {
    let p = palette();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).font(font_sans(11.5)).color(p.ink_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(egui::RichText::new(val).font(font_mono(10.5)).color(p.ink_secondary));
        });
    });
    ui.add_space(3.0);
}
