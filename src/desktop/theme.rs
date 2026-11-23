use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect,
    Stroke, StrokeKind, Vec2, Visuals,
};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ThemePreset {
    NebulaObsidian = 0,
    MidnightCopper = 1,
    ArcticSlate = 2,
}

impl ThemePreset {
    pub fn label(self) -> &'static str {
        match self {
            ThemePreset::NebulaObsidian => "Nebula Obsidian",
            ThemePreset::MidnightCopper => "Midnight Copper",
            ThemePreset::ArcticSlate => "Arctic Slate",
        }
    }

    pub fn next(self) -> Self {
        match self {
            ThemePreset::NebulaObsidian => ThemePreset::MidnightCopper,
            ThemePreset::MidnightCopper => ThemePreset::ArcticSlate,
            ThemePreset::ArcticSlate => ThemePreset::NebulaObsidian,
        }
    }

    pub fn is_light(self) -> bool {
        matches!(self, ThemePreset::ArcticSlate)
    }
}

static ACTIVE_THEME: AtomicU8 = AtomicU8::new(0);

pub fn active_theme() -> ThemePreset {
    match ACTIVE_THEME.load(Ordering::Relaxed) {
        1 => ThemePreset::MidnightCopper,
        2 => ThemePreset::ArcticSlate,
        _ => ThemePreset::NebulaObsidian,
    }
}

pub fn set_active_theme(ctx: &egui::Context, preset: ThemePreset) {
    ACTIVE_THEME.store(preset as u8, Ordering::Relaxed);
    apply_theme_visuals(ctx, preset);
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub is_light: bool,
    pub bg_void: Color32,
    pub bg_canvas: Color32,
    pub bg_chrome: Color32,
    pub bg_surface: Color32,
    pub bg_surface_elevated: Color32,
    pub bg_surface_hover: Color32,
    pub border_subtle: Color32,
    pub border_strong: Color32,
    pub ink_primary: Color32,
    pub ink_secondary: Color32,
    pub ink_muted: Color32,
    pub ink_dim: Color32,
    pub copper: Color32,
    pub copper_bright: Color32,
    pub indigo: Color32,
    pub indigo_bright: Color32,
    pub cyan: Color32,
    pub emerald: Color32,
    pub rose: Color32,
    pub danger: Color32,
    pub nebula_center: Color32,
    pub nebula_mid: Color32,
    pub shadow_color: Color32,
}

pub fn palette() -> ThemePalette {
    match active_theme() {
        ThemePreset::NebulaObsidian => ThemePalette {
            is_light: false,
            bg_void: Color32::from_rgb(7, 10, 20),
            bg_canvas: Color32::from_rgb(10, 15, 30),
            bg_chrome: Color32::from_rgb(13, 19, 36),
            bg_surface: Color32::from_rgb(20, 28, 52),
            bg_surface_elevated: Color32::from_rgb(28, 39, 70),
            bg_surface_hover: Color32::from_rgb(38, 52, 90),
            border_subtle: Color32::from_rgb(42, 56, 94),
            border_strong: Color32::from_rgb(74, 96, 152),
            ink_primary: Color32::from_rgb(244, 247, 255),
            ink_secondary: Color32::from_rgb(206, 216, 238),
            ink_muted: Color32::from_rgb(142, 158, 194),
            ink_dim: Color32::from_rgb(96, 112, 148),
            copper: Color32::from_rgb(238, 158, 82),
            copper_bright: Color32::from_rgb(255, 202, 138),
            indigo: Color32::from_rgb(128, 142, 255),
            indigo_bright: Color32::from_rgb(178, 188, 255),
            cyan: Color32::from_rgb(78, 204, 240),
            emerald: Color32::from_rgb(72, 212, 168),
            rose: Color32::from_rgb(236, 118, 148),
            danger: Color32::from_rgb(244, 102, 116),
            nebula_center: Color32::from_rgb(28, 40, 86),
            nebula_mid: Color32::from_rgb(16, 24, 54),
            shadow_color: Color32::from_rgba_unmultiplied(2, 4, 12, 145),
        },
        ThemePreset::MidnightCopper => ThemePalette {
            is_light: false,
            bg_void: Color32::from_rgb(11, 10, 14),
            bg_canvas: Color32::from_rgb(15, 14, 20),
            bg_chrome: Color32::from_rgb(19, 18, 25),
            bg_surface: Color32::from_rgb(28, 26, 36),
            bg_surface_elevated: Color32::from_rgb(38, 35, 48),
            bg_surface_hover: Color32::from_rgb(50, 46, 62),
            border_subtle: Color32::from_rgb(56, 50, 68),
            border_strong: Color32::from_rgb(96, 82, 108),
            ink_primary: Color32::from_rgb(248, 244, 238),
            ink_secondary: Color32::from_rgb(218, 210, 202),
            ink_muted: Color32::from_rgb(158, 148, 144),
            ink_dim: Color32::from_rgb(108, 100, 98),
            copper: Color32::from_rgb(242, 162, 78),
            copper_bright: Color32::from_rgb(255, 210, 142),
            indigo: Color32::from_rgb(156, 142, 248),
            indigo_bright: Color32::from_rgb(198, 188, 255),
            cyan: Color32::from_rgb(88, 202, 224),
            emerald: Color32::from_rgb(86, 208, 158),
            rose: Color32::from_rgb(238, 122, 138),
            danger: Color32::from_rgb(244, 102, 116),
            nebula_center: Color32::from_rgb(48, 34, 42),
            nebula_mid: Color32::from_rgb(26, 20, 30),
            shadow_color: Color32::from_rgba_unmultiplied(4, 3, 8, 155),
        },
        ThemePreset::ArcticSlate => ThemePalette {
            is_light: true,
            bg_void: Color32::from_rgb(238, 242, 249),
            bg_canvas: Color32::from_rgb(244, 247, 252),
            bg_chrome: Color32::from_rgb(252, 253, 255),
            bg_surface: Color32::from_rgb(255, 255, 255),
            bg_surface_elevated: Color32::from_rgb(248, 250, 255),
            bg_surface_hover: Color32::from_rgb(232, 238, 250),
            border_subtle: Color32::from_rgb(212, 220, 236),
            border_strong: Color32::from_rgb(168, 182, 212),
            ink_primary: Color32::from_rgb(18, 25, 44),
            ink_secondary: Color32::from_rgb(52, 64, 94),
            ink_muted: Color32::from_rgb(98, 112, 144),
            ink_dim: Color32::from_rgb(142, 154, 182),
            copper: Color32::from_rgb(214, 116, 36),
            copper_bright: Color32::from_rgb(194, 96, 22),
            indigo: Color32::from_rgb(78, 92, 224),
            indigo_bright: Color32::from_rgb(62, 74, 204),
            cyan: Color32::from_rgb(14, 142, 186),
            emerald: Color32::from_rgb(18, 152, 112),
            rose: Color32::from_rgb(204, 68, 104),
            danger: Color32::from_rgb(212, 54, 72),
            nebula_center: Color32::from_rgb(218, 228, 248),
            nebula_mid: Color32::from_rgb(232, 238, 250),
            shadow_color: Color32::from_rgba_unmultiplied(20, 32, 64, 36),
        },
    }
}

// ── Typography & Theme Installation ─────────────────────────────────────────
pub fn install_fonts_and_theme(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "ibm_plex_regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/IBMPlexSans-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "ibm_plex_medium".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/IBMPlexSans-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "ibm_plex_semibold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/IBMPlexSans-SemiBold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains_mono_regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/JetBrainsMono-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains_mono_medium".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/JetBrainsMono-Medium.ttf"
        ))),
    );

    if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
        prop.insert(0, "ibm_plex_regular".to_owned());
    }
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.insert(0, "jetbrains_mono_regular".to_owned());
    }

    fonts.families.insert(
        FontFamily::Name("medium".into()),
        vec![
            "ibm_plex_medium".to_owned(),
            "ibm_plex_regular".to_owned(),
        ],
    );
    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        vec![
            "ibm_plex_semibold".to_owned(),
            "ibm_plex_medium".to_owned(),
            "ibm_plex_regular".to_owned(),
        ],
    );
    fonts.families.insert(
        FontFamily::Name("mono_medium".into()),
        vec![
            "jetbrains_mono_medium".to_owned(),
            "jetbrains_mono_regular".to_owned(),
        ],
    );

    ctx.set_fonts(fonts);
    apply_theme_visuals(ctx, active_theme());
}

pub fn apply_theme_visuals(ctx: &egui::Context, preset: ThemePreset) {
    let p = palette();
    let mut visuals = if preset.is_light() {
        Visuals::light()
    } else {
        Visuals::dark()
    };
    visuals.dark_mode = !preset.is_light();
    visuals.panel_fill = p.bg_chrome;
    visuals.window_fill = p.bg_surface;
    visuals.extreme_bg_color = p.bg_void;
    visuals.faint_bg_color = p.bg_surface;
    visuals.code_bg_color = p.bg_void;
    visuals.override_text_color = Some(p.ink_primary);
    visuals.selection.bg_fill = with_alpha(p.indigo, 72);
    visuals.selection.stroke = Stroke::new(1.0, p.indigo_bright);

    visuals.widgets.noninteractive.bg_fill = p.bg_surface;
    visuals.widgets.noninteractive.weak_bg_fill = p.bg_surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border_subtle);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.ink_secondary);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = p.bg_surface;
    visuals.widgets.inactive.weak_bg_fill = p.bg_surface;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border_subtle);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.ink_secondary);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = p.bg_surface_hover;
    visuals.widgets.hovered.weak_bg_fill = p.bg_surface_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border_strong);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2, p.ink_primary);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);

    visuals.widgets.active.bg_fill = p.bg_surface_elevated;
    visuals.widgets.active.weak_bg_fill = p.bg_surface_elevated;
    visuals.widgets.active.bg_stroke = Stroke::new(1.2, p.copper);
    visuals.widgets.active.fg_stroke = Stroke::new(1.2, p.ink_primary);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);

    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.window_stroke = Stroke::new(1.0, p.border_subtle);

    let egui_theme = if preset.is_light() {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    ctx.set_theme(egui_theme);
    ctx.set_visuals(visuals);

    ctx.style_mut_of(egui_theme, |style| {
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style.spacing.scroll.bar_width = 5.0;
        style.spacing.scroll.bar_inner_margin = 3.0;
        style.spacing.scroll.bar_outer_margin = 2.0;
        style.interaction.tooltip_delay = 0.25;
    });
}

pub fn font_sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn font_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

pub fn font_semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn font_mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn font_mono_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("mono_medium".into()))
}

// ── Color & Depth Utilities ─────────────────────────────────────────────────
pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color32::from_rgba_unmultiplied(
        (a.r() as f32 * inv + b.r() as f32 * t).round() as u8,
        (a.g() as f32 * inv + b.g() as f32 * t).round() as u8,
        (a.b() as f32 * inv + b.b() as f32 * t).round() as u8,
        (a.a() as f32 * inv + b.a() as f32 * t).round() as u8,
    )
}

pub fn paint_soft_shadow(painter: &Painter, rect: Rect, radius: u8) {
    let p = palette();
    let base_a = p.shadow_color.a() as f32;
    for (expand, offset_y, mul) in [(10.0, 6.0, 0.28), (5.0, 3.0, 0.48), (2.0, 1.5, 0.72)] {
        let r = rect.translate(Vec2::new(0.0, offset_y)).expand(expand);
        let a = (base_a * mul).clamp(4.0, 220.0) as u8;
        painter.rect_filled(
            r,
            CornerRadius::same(radius.saturating_add(expand as u8)),
            with_alpha(p.shadow_color, a),
        );
    }
}

pub fn accent_color(accent: &str) -> Color32 {
    let p = palette();
    match accent {
        "amber" => p.copper,
        "violet" => p.indigo,
        "cyan" => p.cyan,
        "emerald" => p.emerald,
        "rose" => p.rose,
        _ => p.indigo,
    }
}

pub fn accent_bright(accent: &str) -> Color32 {
    let p = palette();
    if p.is_light {
        return accent_color(accent);
    }
    match accent {
        "amber" => p.copper_bright,
        "violet" => p.indigo_bright,
        "cyan" => Color32::from_rgb(156, 232, 252),
        "emerald" => Color32::from_rgb(142, 238, 204),
        "rose" => Color32::from_rgb(250, 174, 190),
        _ => p.indigo_bright,
    }
}

pub fn accent_core(accent: &str) -> Color32 {
    let p = palette();
    if p.is_light {
        return lerp_color(p.bg_surface, accent_color(accent), 0.16);
    }
    match accent {
        "amber" => Color32::from_rgb(62, 40, 32),
        "violet" => Color32::from_rgb(36, 44, 92),
        "cyan" => Color32::from_rgb(22, 56, 78),
        "emerald" => Color32::from_rgb(22, 60, 54),
        "rose" => Color32::from_rgb(64, 32, 50),
        _ => Color32::from_rgb(36, 44, 92),
    }
}

// ── Crisp Vector Icon Painters (Zero Emoji, Consistent 1.5px Stroke) ────────
pub fn draw_icon_menu(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let w = s * 0.42;
    let dy = s * 0.24;
    let st = Stroke::new(1.5, color);
    p.line_segment([Pos2::new(c.x - w, c.y - dy), Pos2::new(c.x + w, c.y - dy)], st);
    p.line_segment([Pos2::new(c.x - w, c.y), Pos2::new(c.x + w, c.y)], st);
    p.line_segment([Pos2::new(c.x - w, c.y + dy), Pos2::new(c.x + w, c.y + dy)], st);
}

pub fn draw_icon_search(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let r = s * 0.28;
    let center = Pos2::new(c.x - s * 0.08, c.y - s * 0.08);
    let st = Stroke::new(1.5, color);
    p.circle_stroke(center, r, st);
    let dir = Vec2::new(0.707, 0.707);
    p.line_segment([center + dir * r, center + dir * (r + s * 0.26)], st);
}

pub fn draw_icon_galaxy(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    p.circle_stroke(c, s * 0.16, st);
    p.circle_stroke(c, s * 0.38, Stroke::new(1.1, with_alpha(color, 170)));
    let sat1 = Pos2::new(c.x + s * 0.34, c.y - s * 0.18);
    let sat2 = Pos2::new(c.x - s * 0.32, c.y + s * 0.22);
    p.circle_filled(sat1, 2.1, color);
    p.circle_filled(sat2, 1.8, color);
}

pub fn draw_icon_grid(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    let box_sz = s * 0.28;
    let gap = s * 0.10;
    for dx in [-1.0_f32, 1.0] {
        for dy in [-1.0_f32, 1.0] {
            let center = Pos2::new(
                c.x + dx * (box_sz * 0.5 + gap * 0.5),
                c.y + dy * (box_sz * 0.5 + gap * 0.5),
            );
            let rect = Rect::from_center_size(center, Vec2::splat(box_sz));
            p.rect_stroke(rect, CornerRadius::same(2), st, StrokeKind::Inside);
        }
    }
}

pub fn draw_icon_links(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    let a = Pos2::new(c.x - s * 0.26, c.y + s * 0.18);
    let b = Pos2::new(c.x + s * 0.26, c.y - s * 0.18);
    p.line_segment([a, b], st);
    p.circle_filled(a, 2.6, color);
    p.circle_filled(b, 2.6, color);
}

pub fn draw_icon_palette(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    p.circle_stroke(c, s * 0.36, st);
    p.circle_filled(Pos2::new(c.x - s * 0.12, c.y - s * 0.10), 1.8, color);
    p.circle_filled(Pos2::new(c.x + s * 0.12, c.y - s * 0.10), 1.8, color);
    p.circle_filled(Pos2::new(c.x, c.y + s * 0.14), 1.8, color);
}

pub fn draw_icon_refresh(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let r = s * 0.34;
    let st = Stroke::new(1.5, color);
    let steps = 18;
    let start = -0.4_f32;
    let end = 4.4_f32;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let a = start + (end - start) * (i as f32 / steps as f32);
        pts.push(Pos2::new(c.x + a.cos() * r, c.y + a.sin() * r));
    }
    p.add(egui::Shape::line(pts, st));
    let tip = Pos2::new(c.x + start.cos() * r, c.y + start.sin() * r);
    p.line_segment([tip, Pos2::new(tip.x - 4.0, tip.y - 1.5)], st);
    p.line_segment([tip, Pos2::new(tip.x + 0.5, tip.y - 4.5)], st);
}

pub fn draw_icon_activity(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    let rect = Rect::from_center_size(c, Vec2::new(s * 0.76, s * 0.62));
    p.rect_stroke(rect, CornerRadius::same(4), st, StrokeKind::Inside);
    let y1 = c.y - s * 0.10;
    let y2 = c.y + s * 0.10;
    p.line_segment([Pos2::new(c.x - s * 0.20, y1), Pos2::new(c.x + s * 0.20, y1)], st);
    p.line_segment([Pos2::new(c.x - s * 0.20, y2), Pos2::new(c.x + s * 0.08, y2)], st);
}

pub fn draw_icon_crosshair(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    let r = s * 0.28;
    let arm = s * 0.44;
    p.circle_stroke(c, r, st);
    p.line_segment([Pos2::new(c.x - arm, c.y), Pos2::new(c.x - r * 0.55, c.y)], st);
    p.line_segment([Pos2::new(c.x + r * 0.55, c.y), Pos2::new(c.x + arm, c.y)], st);
    p.line_segment([Pos2::new(c.x, c.y - arm), Pos2::new(c.x, c.y - r * 0.55)], st);
    p.line_segment([Pos2::new(c.x, c.y + r * 0.55), Pos2::new(c.x, c.y + arm)], st);
}

pub fn draw_icon_gear(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    p.circle_stroke(c, s * 0.16, st);
    p.circle_stroke(c, s * 0.31, st);
    for i in 0..6 {
        let a = (i as f32) * std::f32::consts::TAU / 6.0;
        let p1 = Pos2::new(c.x + a.cos() * s * 0.31, c.y + a.sin() * s * 0.31);
        let p2 = Pos2::new(c.x + a.cos() * s * 0.43, c.y + a.sin() * s * 0.43);
        p.line_segment([p1, p2], st);
    }
}

pub fn draw_icon_upload(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.5, color);
    let tray = Rect::from_center_size(Pos2::new(c.x, c.y + s * 0.12), Vec2::new(s * 0.72, s * 0.46));
    p.line_segment([tray.left_top(), tray.left_bottom()], st);
    p.line_segment([tray.left_bottom(), tray.right_bottom()], st);
    p.line_segment([tray.right_bottom(), tray.right_top()], st);
    let top = Pos2::new(c.x, c.y - s * 0.34);
    let bot = Pos2::new(c.x, c.y + s * 0.14);
    p.line_segment([top, bot], st);
    p.line_segment([top, Pos2::new(top.x - s * 0.18, top.y + s * 0.18)], st);
    p.line_segment([top, Pos2::new(top.x + s * 0.18, top.y + s * 0.18)], st);
}

pub fn draw_icon_folder(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    let w = s * 0.42;
    let h = s * 0.30;
    let pts = vec![
        Pos2::new(c.x - w, c.y - h),
        Pos2::new(c.x - w * 0.25, c.y - h),
        Pos2::new(c.x - w * 0.05, c.y - h * 0.55),
        Pos2::new(c.x + w, c.y - h * 0.55),
        Pos2::new(c.x + w, c.y + h),
        Pos2::new(c.x - w, c.y + h),
        Pos2::new(c.x - w, c.y - h),
    ];
    p.add(egui::Shape::line(pts, st));
}

pub fn draw_icon_file_type(p: &Painter, c: Pos2, s: f32, file_type: &str, color: Color32) {
    let st = Stroke::new(1.4, color);
    match file_type {
        "image" => {
            let rect = Rect::from_center_size(c, Vec2::new(s * 0.74, s * 0.62));
            p.rect_stroke(rect, CornerRadius::same(3), st, StrokeKind::Inside);
            p.circle_filled(Pos2::new(c.x - s * 0.16, c.y - s * 0.12), 1.8, color);
            let m1 = Pos2::new(rect.left() + 2.5, rect.bottom() - 2.5);
            let m2 = Pos2::new(c.x - s * 0.04, c.y + s * 0.02);
            let m3 = Pos2::new(c.x + s * 0.08, c.y + s * 0.14);
            let m4 = Pos2::new(c.x + s * 0.20, c.y - s * 0.02);
            let m5 = Pos2::new(rect.right() - 2.5, rect.bottom() - 2.5);
            p.add(egui::Shape::line(vec![m1, m2, m3, m4, m5], st));
        }
        "pdf" => {
            let rect = Rect::from_center_size(c, Vec2::new(s * 0.62, s * 0.76));
            p.rect_stroke(rect, CornerRadius::same(3), st, StrokeKind::Inside);
            let band = Rect::from_center_size(
                Pos2::new(c.x, c.y + s * 0.08),
                Vec2::new(s * 0.46, s * 0.22),
            );
            p.rect_filled(band, CornerRadius::same(2), with_alpha(color, 65));
            p.line_segment(
                [
                    Pos2::new(c.x - s * 0.16, c.y - s * 0.16),
                    Pos2::new(c.x + s * 0.16, c.y - s * 0.16),
                ],
                st,
            );
        }
        "docx" => {
            let rect = Rect::from_center_size(c, Vec2::new(s * 0.62, s * 0.76));
            p.rect_stroke(rect, CornerRadius::same(3), st, StrokeKind::Inside);
            for dy in [-0.16_f32, 0.0, 0.16] {
                let w = if dy > 0.1 { 0.10 } else { 0.18 };
                p.line_segment(
                    [
                        Pos2::new(c.x - s * 0.18, c.y + s * dy),
                        Pos2::new(c.x + s * w, c.y + s * dy),
                    ],
                    st,
                );
            }
        }
        _ => {
            let rect = Rect::from_center_size(c, Vec2::new(s * 0.62, s * 0.76));
            p.rect_stroke(rect, CornerRadius::same(3), st, StrokeKind::Inside);
            p.line_segment(
                [
                    Pos2::new(c.x - s * 0.16, c.y - s * 0.12),
                    Pos2::new(c.x + s * 0.16, c.y - s * 0.12),
                ],
                st,
            );
            p.line_segment(
                [
                    Pos2::new(c.x - s * 0.16, c.y + s * 0.04),
                    Pos2::new(c.x + s * 0.16, c.y + s * 0.04),
                ],
                st,
            );
            p.line_segment(
                [
                    Pos2::new(c.x - s * 0.16, c.y + s * 0.20),
                    Pos2::new(c.x + s * 0.04, c.y + s * 0.20),
                ],
                st,
            );
        }
    }
}

pub fn draw_icon_close(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let r = s * 0.28;
    let st = Stroke::new(1.5, color);
    p.line_segment([Pos2::new(c.x - r, c.y - r), Pos2::new(c.x + r, c.y + r)], st);
    p.line_segment([Pos2::new(c.x + r, c.y - r), Pos2::new(c.x - r, c.y + r)], st);
}

pub fn draw_icon_minimize(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let r = s * 0.30;
    p.line_segment(
        [Pos2::new(c.x - r, c.y + 1.0), Pos2::new(c.x + r, c.y + 1.0)],
        Stroke::new(1.5, color),
    );
}

pub fn draw_icon_maximize(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let rect = Rect::from_center_size(c, Vec2::splat(s * 0.56));
    p.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.4, color),
        StrokeKind::Inside,
    );
}

pub fn draw_icon_chevron_left(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.6, color);
    let dx = s * 0.16;
    let dy = s * 0.28;
    p.line_segment([Pos2::new(c.x + dx, c.y - dy), Pos2::new(c.x - dx, c.y)], st);
    p.line_segment([Pos2::new(c.x - dx, c.y), Pos2::new(c.x + dx, c.y + dy)], st);
}

pub fn draw_icon_chevron_right(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.6, color);
    let dx = s * 0.16;
    let dy = s * 0.28;
    p.line_segment([Pos2::new(c.x - dx, c.y - dy), Pos2::new(c.x + dx, c.y)], st);
    p.line_segment([Pos2::new(c.x + dx, c.y), Pos2::new(c.x - dx, c.y + dy)], st);
}

pub fn draw_icon_chevron_down(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.5, color);
    let dx = s * 0.28;
    let dy = s * 0.15;
    p.line_segment([Pos2::new(c.x - dx, c.y - dy), Pos2::new(c.x, c.y + dy)], st);
    p.line_segment([Pos2::new(c.x, c.y + dy), Pos2::new(c.x + dx, c.y - dy)], st);
}

pub fn draw_icon_workstation(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    let rect = Rect::from_center_size(c, Vec2::new(s * 0.85, s * 0.68));
    p.rect_stroke(rect, CornerRadius::same(3), st, StrokeKind::Inside);
    let col1_x = rect.left() + rect.width() * 0.28;
    let col2_x = rect.left() + rect.width() * 0.70;
    p.line_segment([Pos2::new(col1_x, rect.top()), Pos2::new(col1_x, rect.bottom())], st);
    p.line_segment([Pos2::new(col2_x, rect.top()), Pos2::new(col2_x, rect.bottom())], st);
}

pub fn draw_icon_shield(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    let w = s * 0.36;
    let h_top = s * 0.32;
    let h_bot = s * 0.40;
    let pts = vec![
        Pos2::new(c.x - w, c.y - h_top),
        Pos2::new(c.x + w, c.y - h_top),
        Pos2::new(c.x + w, c.y + h_bot * 0.2),
        Pos2::new(c.x, c.y + h_bot),
        Pos2::new(c.x - w, c.y + h_bot * 0.2),
        Pos2::new(c.x - w, c.y - h_top),
    ];
    p.add(egui::Shape::line(pts, st));
    p.line_segment([Pos2::new(c.x, c.y - h_top), Pos2::new(c.x, c.y + h_bot * 0.7)], st);
}

pub fn draw_icon_trash(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.3, color);
    let w = s * 0.26;
    let h = s * 0.32;
    // Lid
    p.line_segment(
        [Pos2::new(c.x - w * 1.25, c.y - h), Pos2::new(c.x + w * 1.25, c.y - h)],
        st,
    );
    p.line_segment(
        [Pos2::new(c.x - w * 0.45, c.y - h * 1.25), Pos2::new(c.x + w * 0.45, c.y - h * 1.25)],
        st,
    );
    // Can body
    p.line_segment(
        [Pos2::new(c.x - w, c.y - h), Pos2::new(c.x - w * 0.85, c.y + h)],
        st,
    );
    p.line_segment(
        [Pos2::new(c.x + w, c.y - h), Pos2::new(c.x + w * 0.85, c.y + h)],
        st,
    );
    p.line_segment(
        [Pos2::new(c.x - w * 0.85, c.y + h), Pos2::new(c.x + w * 0.85, c.y + h)],
        st,
    );
    // Grooves
    p.line_segment([Pos2::new(c.x - w * 0.35, c.y - h * 0.6), Pos2::new(c.x - w * 0.30, c.y + h * 0.7)], st);
    p.line_segment([Pos2::new(c.x + w * 0.35, c.y - h * 0.6), Pos2::new(c.x + w * 0.30, c.y + h * 0.7)], st);
}

pub fn draw_icon_smart_view(p: &Painter, c: Pos2, s: f32, kind: &str, color: Color32) {
    let st = Stroke::new(1.3, color);
    match kind {
        "all" => {
            let w = s * 0.32;
            for dy in [-0.22_f32, 0.0, 0.22] {
                p.line_segment(
                    [Pos2::new(c.x - w, c.y + s * dy), Pos2::new(c.x + w, c.y + s * dy)],
                    st,
                );
            }
        }
        "duplicates" => {
            let r1 = Rect::from_min_size(
                Pos2::new(c.x - s * 0.34, c.y - s * 0.34),
                Vec2::new(s * 0.44, s * 0.52),
            );
            let r2 = Rect::from_min_size(
                Pos2::new(c.x - s * 0.12, c.y - s * 0.16),
                Vec2::new(s * 0.44, s * 0.52),
            );
            p.rect_stroke(r1, CornerRadius::same(2), Stroke::new(1.1, color), StrokeKind::Inside);
            p.rect_filled(r2, CornerRadius::same(2), with_alpha(color, 40));
            p.rect_stroke(r2, CornerRadius::same(2), st, StrokeKind::Inside);
        }
        "scripts" | "code" => {
            let dx = s * 0.28;
            let dy = s * 0.24;
            p.line_segment([Pos2::new(c.x - s * 0.08, c.y - dy), Pos2::new(c.x - dx, c.y)], st);
            p.line_segment([Pos2::new(c.x - dx, c.y), Pos2::new(c.x - s * 0.08, c.y + dy)], st);
            p.line_segment([Pos2::new(c.x + s * 0.08, c.y - dy), Pos2::new(c.x + dx, c.y)], st);
            p.line_segment([Pos2::new(c.x + dx, c.y), Pos2::new(c.x + s * 0.08, c.y + dy)], st);
        }
        "documents" => {
            draw_icon_file_type(p, c, s, "docx", color);
        }
        "media" => {
            draw_icon_file_type(p, c, s, "image", color);
        }
        _ => {
            p.circle_stroke(c, s * 0.28, st);
        }
    }
}
