use crate::config::Config;
use crate::desktop::chrome::PanelAction;
use crate::desktop::theme::{
    draw_icon_close, draw_icon_galaxy, font_medium, font_mono, font_sans, font_semibold, palette,
    with_alpha,
};
use egui::{
    Align, Color32, CornerRadius, CursorIcon, Frame, Id, Layout, Margin, Rect, ScrollArea,
    Sense, Stroke, Ui, Vec2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingTab {
    Architecture,
    Permissions,
    Privacy,
    GroqCloud,
}

pub struct OnboardingState {
    pub active_tab: OnboardingTab,
    pub do_not_show_again: bool,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            active_tab: OnboardingTab::Architecture,
            do_not_show_again: true,
        }
    }
}

pub fn render_onboarding_modal(
    ctx: &egui::Context,
    open: &mut bool,
    config: &mut Config,
    state: &mut OnboardingState,
) -> PanelAction {
    let mut action = PanelAction::None;
    if !*open {
        return action;
    }

    let p = palette();
    let screen_rect = ctx.content_rect();
    let modal_w = 680.0_f32.min(screen_rect.width() - 32.0);
    let modal_h = 580.0_f32.min(screen_rect.height() - 40.0);

    // Background scrim overlay
    let scrim_id = Id::unique("axel_onboarding_scrim");
    egui::Area::new(scrim_id)
        .order(egui::Order::Middle)
        .fixed_pos(screen_rect.min)
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen_rect.size(), Sense::click());
            ui.painter().rect_filled(
                screen_rect,
                CornerRadius::ZERO,
                Color32::from_black_alpha(165),
            );
            if resp.clicked() {
                if state.do_not_show_again && !config.has_seen_onboarding {
                    config.has_seen_onboarding = true;
                    let _ = config.save();
                }
                *open = false;
                action = PanelAction::CloseOnboardingGuide;
            }
        });

    let modal_rect = Rect::from_center_size(screen_rect.center(), Vec2::new(modal_w, modal_h));

    egui::Area::new(Id::unique("axel_onboarding_dialog"))
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
                    // 1. Header Bar
                    ui.horizontal(|ui| {
                        let (ic_rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        draw_icon_galaxy(ui.painter(), ic_rect.center(), 18.0, p.copper_bright);

                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("AXEL Architecture & Getting Started Guide")
                                    .font(font_semibold(15.0))
                                    .color(p.ink_primary),
                            );
                            ui.label(
                                egui::RichText::new("High-performance desktop intelligence, security boundaries, and Groq setup")
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
                                if state.do_not_show_again && !config.has_seen_onboarding {
                                    config.has_seen_onboarding = true;
                                    let _ = config.save();
                                }
                                *open = false;
                                action = PanelAction::CloseOnboardingGuide;
                            }
                        });
                    });

                    ui.add_space(14.0);

                    // 2. Navigation Tab Pills
                    ui.horizontal(|ui| {
                        let tabs = [
                            (OnboardingTab::Architecture, "How It Works"),
                            (OnboardingTab::Permissions, "Native Permissions"),
                            (OnboardingTab::Privacy, "Privacy & Security"),
                            (OnboardingTab::GroqCloud, "Groq AI & API Key"),
                        ];

                        for (tab, label) in tabs {
                            let is_active = state.active_tab == tab;
                            let bg = if is_active {
                                with_alpha(p.copper_bright, 40)
                            } else {
                                p.bg_surface_elevated
                            };
                            let text_color = if is_active { p.copper_bright } else { p.ink_secondary };
                            let stroke_color = if is_active { p.copper_bright } else { p.border_subtle };

                            let btn = egui::Button::new(
                                egui::RichText::new(label)
                                    .font(font_medium(11.5))
                                    .color(text_color),
                            )
                            .fill(bg)
                            .stroke(Stroke::new(1.0, stroke_color))
                            .corner_radius(CornerRadius::same(6));

                            if ui.add(btn).clicked() {
                                state.active_tab = tab;
                            }
                        }
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // 3. Scrollable Tab Content
                    let scroll_h = modal_h - 170.0;
                    ScrollArea::vertical()
                        .max_height(scroll_h)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            match state.active_tab {
                                OnboardingTab::Architecture => render_architecture_tab(ui),
                                OnboardingTab::Permissions => render_permissions_tab(ui),
                                OnboardingTab::Privacy => render_privacy_tab(ui),
                                OnboardingTab::GroqCloud => render_groq_tab(ui),
                            }
                        });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // 4. Footer Bar
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut state.do_not_show_again,
                            egui::RichText::new("Do not show this guide on startup")
                                .font(font_sans(11.5))
                                .color(p.ink_secondary),
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let confirm_btn = egui::Button::new(
                                egui::RichText::new("Get Started")
                                    .font(font_semibold(12.0))
                                    .color(Color32::WHITE),
                            )
                            .fill(p.copper_bright)
                            .corner_radius(CornerRadius::same(6))
                            .min_size(Vec2::new(100.0, 28.0));

                            if ui.add(confirm_btn).clicked() {
                                if state.do_not_show_again && !config.has_seen_onboarding {
                                    config.has_seen_onboarding = true;
                                    let _ = config.save();
                                }
                                *open = false;
                                action = PanelAction::CloseOnboardingGuide;
                            }

                            ui.label(
                                egui::RichText::new("You can re-open this guide anytime from Settings")
                                    .font(font_sans(10.5))
                                    .color(p.ink_muted),
                            );
                        });
                    });
                });
        });

    action
}

fn render_card<R>(ui: &mut Ui, title: &str, subtitle: &str, content: impl FnOnce(&mut Ui) -> R) {
    let p = palette();
    Frame::new()
        .fill(p.bg_surface)
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).font(font_semibold(12.5)).color(p.ink_primary));
            if !subtitle.is_empty() {
                ui.label(egui::RichText::new(subtitle).font(font_sans(11.0)).color(p.ink_muted));
            }
            ui.add_space(6.0);
            content(ui);
        });
    ui.add_space(8.0);
}

fn render_architecture_tab(ui: &mut Ui) {
    let p = palette();

    render_card(
        ui,
        "Single-Binary Desktop Architecture",
        "High-performance native Rust core with zero heavy runtimes",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "AXEL is engineered in pure Rust as a standalone native application to replace conventional \
                    operating system file managers with intelligent, zero-overhead document understanding. \
                    Unlike Electron or Python-based tools, AXEL runs within a lean memory budget (~10 MB RSS) \
                    with instantaneous sub-millisecond startup."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Zero Byte Duplication & Virtual Hierarchies",
        "Organize files into dynamic multi-pointer constellations without copying bytes",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "Conventional file systems force documents into rigid, single-path directory structures. \
                    AXEL computes dynamic Virtual Paths. A single file can belong to multiple logical constellations \
                    simultaneously (for example, a machine learning lecture note can be indexed under Academics/Data Science, \
                    Career/Coursework, and Research/AI) without duplicating physical bytes on your storage device."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Multi-Format Extraction & OCR",
        "Pure-Rust parsers for PDF, DOCX, Code, Text, and Scanned Documents",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "AXEL extracts clean semantic text across formats directly: pure-Rust PDF parsing via pdf-extract, \
                    zero-dependency DOCX XML document streaming, code symbol deconstruction, and automatic Tesseract OCR \
                    discovery for scanned PDFs and images."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Sub-Millisecond Hybrid Search & Galaxy Canvas",
        "Instant token intersection, exact substring match, and Jaro-Winkler string similarity",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "Search queries execute in sub-millisecond time across thousands of indexed files using hybrid score \
                    blending. The interactive Galaxy Canvas visualizes topic clusters, keyword constellations, and semantic links \
                    in real time with hardware-accelerated 60 FPS rendering."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );
}

fn render_permissions_tab(ui: &mut Ui) {
    let p = palette();

    render_card(
        ui,
        "Local Filesystem Access (Selected Workspaces Only)",
        "Strict isolation to your designated document directories",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "AXEL only accesses directories you explicitly configure or drag into the application window. \
                    It reads files to extract metadata, generates SHA-256 hashes for deduplication, and monitors folder changes \
                    via the native OS kernel notification subsystem (inotify on Linux, kqueue on macOS, ReadDirectoryChanges on Windows). \
                    AXEL never scans your entire root filesystem."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Safe OS Trash Integration",
        "Deletions are non-destructive and routed to system Recycle Bin / Trash",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "When trashing duplicate files or removing obsolete items, AXEL dispatches them to your operating system's \
                    native trash bin using the platform's trash-cli/XDG/Finder API. Your files remain recoverable until you \
                    manually empty your system trash. Permanent unrecoverable deletion is only executed if explicitly confirmed."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Network Sockets & Port Binding",
        "Local loopback HTTP server and optional outbound HTTPS to Groq",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "AXEL binds an embedded HTTP server to 127.0.0.1 (default port 8765) to provide the local REST API and Web UI. \
                    It never exposes ports to external interfaces unless you explicitly configure the host to 0.0.0.0. \
                    The only outbound network requests AXEL makes are HTTPS calls to Groq's API endpoint (api.groq.com) when an API key \
                    is configured and Force Offline Mode is disabled."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Zero Telemetry & Zero Data Collection",
        "No telemetry, tracking, analytics, or behavioral surveillance",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "AXEL does not include Google Analytics, Sentry, Mixpanel, or any third-party telemetry libraries. \
                    No diagnostic beacons, crash telemetry, or usage metrics are transmitted over the network."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );
}

fn render_privacy_tab(ui: &mut Ui) {
    let p = palette();

    render_card(
        ui,
        "Automated PII & Credential Redaction Shield",
        "Deterministic pre-flight regex sanitization before LLM payloads",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "Before any document text or code excerpt is dispatched to Groq's LLM endpoint, AXEL's privacy shield \
                    automatically detects and redacts high-risk sensitive patterns: passwords, API keys, private tokens, \
                    Aadhaar numbers, PAN numbers, Social Security numbers, credit card sequences, and personal email addresses. \
                    Sensitive tokens are replaced with [REDACTED_SECRET] or [REDACTED_ID] markers."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "100% Air-Gapped / Force Offline Mode",
        "Complete network isolation with local vector centroid classification",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "For classified documents, proprietary intellectual property, or confidential legal files, you can \
                    toggle 'Force Offline Mode' in Settings at any time. When active, AXEL shuts down all outbound network \
                    communication and categorizes files using local deterministic rule engines, TF-IDF symbol extractors, \
                    and vector space cosine similarity against pre-computed domain centroids."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Atomic Persistence & Crash Safety",
        "Zero corrupted indices through lockfiles and atomic temporary staging",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "Document indices and settings are written to temporary files (.tmp) and atomically committed via \
                    filesystem renames. If your system encounters a sudden power loss or crash, AXEL's index is never \
                    left in a corrupted state, and automatic backup snapshots (.bak) are maintained continuously."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );
}

fn render_groq_tab(ui: &mut Ui) {
    let p = palette();

    render_card(
        ui,
        "Where to Get Your Groq API Key",
        "Free tier access to high-speed LPU inference",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "1. Visit the Groq Console at https://console.groq.com/keys\n\
                     2. Sign up or log in with your account.\n\
                     3. Click 'Create API Key', give it a label (e.g., 'Axel Desktop'), and copy the key.\n\
                     4. Paste the key into AXEL's Settings (AI & Cloud tab) and click 'Test Key' to verify latency."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );

            ui.add_space(4.0);
            if ui.button(egui::RichText::new("Copy Groq Console URL").font(font_mono(11.0))).clicked() {
                ui.ctx().copy_text("https://console.groq.com/keys".to_string());
            }
        },
    );

    render_card(
        ui,
        "Why Groq? (LPU Hardware Acceleration & Context Windows)",
        "Near-zero round-trip latency and massive token capacity",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "Groq runs on specialized Language Processing Unit (LPU) silicon rather than conventional GPUs. \
                    This architecture delivers 500+ tokens per second with typical round-trip request latencies under \
                    180 milliseconds, turning document categorization from a sluggish background job into an instant, \
                    real-time interactive experience.\n\n\
                    Furthermore, Groq provides massive context windows (up to 128,000 tokens on models like Llama 3.3 70B), \
                    allowing AXEL to process comprehensive academic textbooks, long legal contracts, and dense multi-file \
                    source code repositories without aggressive chunking or context loss."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );

    render_card(
        ui,
        "Dynamic Model Discovery & Open-Weights Freedom",
        "Zero hardcoded model lists with automated account discovery",
        |ui| {
            ui.label(
                egui::RichText::new(
                    "AXEL queries the live Groq endpoint at runtime, automatically populating the available models for your \
                    account (such as Llama 3.3 70B, Llama 3.1 8B, Mixtral 8x7B, and Gemma 2 9B). You are never locked into a \
                    static list, and you can switch primary and fallback models dynamically as new architectures are released."
                )
                .font(font_sans(11.5))
                .color(p.ink_secondary),
            );
        },
    );
}
