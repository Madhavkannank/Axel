use crate::desktop::theme::{
    accent_bright, accent_color, accent_core, draw_icon_file_type, font_medium, font_sans,
    font_semibold, lerp_color, paint_soft_shadow, palette, with_alpha,
};
use crate::engine::{GraphEdge, GraphHub, GraphNode, GraphSnapshot};
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, Painter, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct HubLayout {
    pub id: String,
    pub category: String,
    pub accent: String,
    pub count: usize,
    pub top_keywords: Vec<String>,
    pub pos: Pos2,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct NodeLayout {
    pub hash: String,
    pub file: String,
    pub category: String,
    pub file_type: String,
    pub keywords: Vec<String>,
    pub size_bytes: Option<u64>,
    pub file_exists: bool,
    pub accent: String,
    pub pos: Pos2,
    pub radius: f32,
    pub capsule_mode: bool,
}

#[derive(Debug, Clone)]
pub struct GalaxyCamera {
    pub pan: Vec2,
    pub target_pan: Vec2,
    pub zoom: f32,
    pub target_zoom: f32,
}

impl Default for GalaxyCamera {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            target_pan: Vec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
        }
    }
}

impl GalaxyCamera {
    pub fn reset(&mut self) {
        self.target_pan = Vec2::ZERO;
        self.target_zoom = 1.0;
    }

    pub fn step(&mut self, dt: f32) -> bool {
        let factor = 1.0 - (-12.0 * dt.clamp(0.001, 0.05)).exp();
        let pan_diff = self.target_pan - self.pan;
        let zoom_diff = self.target_zoom - self.zoom;
        if pan_diff.length_sq() > 0.05 || zoom_diff.abs() > 0.0005 {
            self.pan += pan_diff * factor;
            self.zoom += zoom_diff * factor;
            true
        } else {
            self.pan = self.target_pan;
            self.zoom = self.target_zoom;
            false
        }
    }

    pub fn world_to_screen(&self, world: Pos2, center: Pos2) -> Pos2 {
        center + self.pan + world.to_vec2() * self.zoom
    }

    pub fn screen_to_world(&self, screen: Pos2, center: Pos2) -> Pos2 {
        ((screen - center - self.pan) / self.zoom).to_pos2()
    }
}

#[derive(Default)]
pub struct GalaxyLayout {
    pub hubs: Vec<HubLayout>,
    pub nodes: Vec<NodeLayout>,
    pub hub_index: HashMap<String, usize>,
    pub node_index: HashMap<String, usize>,
    pub dragged_hub: Option<String>,
    pub hovered_hub: Option<String>,
    pub hovered_node: Option<String>,
    pub hovered_edge: Option<usize>,
}

pub enum GalaxyAction {
    None,
    SelectHub(String),
    SelectNode(String),
    ClearSelection,
}

impl GalaxyLayout {
    pub fn clear(&mut self) {
        self.hubs.clear();
        self.nodes.clear();
        self.hub_index.clear();
        self.node_index.clear();
        self.dragged_hub = None;
        self.hovered_hub = None;
        self.hovered_node = None;
        self.hovered_edge = None;
    }

    pub fn rebuild(&mut self, snapshot: &GraphSnapshot) {
        let prev_hub_positions: HashMap<String, Pos2> = self
            .hubs
            .iter()
            .map(|h| (h.id.clone(), h.pos))
            .collect();

        self.hubs.clear();
        self.nodes.clear();
        self.hub_index.clear();
        self.node_index.clear();

        let hub_count = snapshot.hubs.len();
        if hub_count == 0 && snapshot.nodes.is_empty() {
            return;
        }

        let preset_anchors: &[(f32, f32)] = &[
            (-215.0, -135.0),
            (-245.0, 115.0),
            (15.0, -15.0),
            (235.0, -130.0),
            (215.0, 105.0),
            (-35.0, -195.0),
            (35.0, -185.0),
            (-340.0, -10.0),
        ];

        for (idx, hub) in snapshot.hubs.iter().enumerate() {
            let radius = (42.0 + (hub.count as f32).sqrt() * 8.5).clamp(44.0, 64.0);
            let pos = if let Some(&existing) = prev_hub_positions.get(&hub.id) {
                existing
            } else if idx < preset_anchors.len() {
                let (x, y) = preset_anchors[idx];
                Pos2::new(x, y)
            } else {
                let angle = (idx as f32) * std::f32::consts::TAU / (hub_count as f32) - 0.6;
                let rx = 290.0 + ((idx % 2) as f32) * 55.0;
                let ry = 165.0 + ((idx % 3) as f32) * 25.0;
                Pos2::new(angle.cos() * rx, angle.sin() * ry - 25.0)
            };

            self.hub_index.insert(hub.id.clone(), self.hubs.len());
            self.hubs.push(HubLayout {
                id: hub.id.clone(),
                category: hub.category.clone(),
                accent: hub.accent.clone(),
                count: hub.count,
                top_keywords: hub.top_keywords.clone(),
                pos,
                radius,
            });
        }

        let mut nodes_by_hub: HashMap<String, Vec<&GraphNode>> = HashMap::new();
        for node in &snapshot.nodes {
            nodes_by_hub
                .entry(format!("hub:{}", node.category))
                .or_default()
                .push(node);
        }

        for hub in &self.hubs {
            let Some(cat_nodes) = nodes_by_hub.get(&hub.id) else {
                continue;
            };
            let n = cat_nodes.len();
            let outward_angle = if hub.pos.to_vec2().length_sq() > 1600.0 {
                hub.pos.y.atan2(hub.pos.x)
            } else {
                0.58
            };

            let spread = if n <= 1 {
                0.0
            } else if n <= 4 {
                std::f32::consts::PI * 1.15
            } else {
                std::f32::consts::TAU * 0.88
            };

            for (i, node) in cat_nodes.iter().enumerate() {
                let angle = if n == 1 {
                    outward_angle
                } else {
                    let t = (i as f32) / ((n - 1).max(1) as f32) - 0.5;
                    outward_angle + t * spread
                };

                let ring_tier = (i / 6) as f32;
                let orbit = hub.radius + 76.0 + ring_tier * 54.0;
                let pos = Pos2::new(
                    hub.pos.x + angle.cos() * orbit,
                    hub.pos.y + angle.sin() * orbit,
                );

                let capsule_mode = i == 0 && n >= 2;

                self.node_index.insert(node.hash.clone(), self.nodes.len());
                self.nodes.push(NodeLayout {
                    hash: node.hash.clone(),
                    file: node.file.clone(),
                    category: node.category.clone(),
                    file_type: node.file_type.clone(),
                    keywords: node.keywords.clone(),
                    size_bytes: node.size_bytes,
                    file_exists: node.file_exists,
                    accent: hub.accent.clone(),
                    pos,
                    radius: 19.5,
                    capsule_mode,
                });
            }
        }

        // M12: Lay out any isolated or unmapped nodes
        for node in &snapshot.nodes {
            if !self.node_index.contains_key(&node.hash) {
                let idx = self.nodes.len();
                let angle = (idx as f32) * 2.3999632; // Golden angle in radians
                let orbit = 110.0 + (idx as f32) * 32.0;
                let pos = Pos2::new(angle.cos() * orbit, angle.sin() * orbit);
                self.node_index.insert(node.hash.clone(), idx);
                self.nodes.push(NodeLayout {
                    hash: node.hash.clone(),
                    file: node.file.clone(),
                    category: node.category.clone(),
                    file_type: node.file_type.clone(),
                    keywords: node.keywords.clone(),
                    size_bytes: node.size_bytes,
                    file_exists: node.file_exists,
                    accent: "#e59a54".to_string(),
                    pos,
                    radius: 19.5,
                    capsule_mode: false,
                });
            }
        }

        self.relax_collisions();
    }

    pub fn relax_collisions(&mut self) {
        for _ in 0..28 {
            for i in 0..self.hubs.len() {
                for j in (i + 1)..self.hubs.len() {
                    let delta = self.hubs[j].pos - self.hubs[i].pos;
                    let dist = delta.length().max(1.0);
                    let min_dist = self.hubs[i].radius + self.hubs[j].radius + 95.0;
                    if dist < min_dist {
                        let push = (delta / dist) * ((min_dist - dist) * 0.35);
                        self.hubs[i].pos -= push;
                        self.hubs[j].pos += push;
                    }
                }
            }

            for node in &mut self.nodes {
                for hub in &self.hubs {
                    let delta = node.pos - hub.pos;
                    let dist = delta.length().max(1.0);
                    let min_dist = hub.radius + node.radius + 52.0;
                    if dist < min_dist {
                        let push = (delta / dist) * ((min_dist - dist) * 0.45);
                        node.pos += push;
                    }
                }
            }

            for i in 0..self.nodes.len() {
                for j in (i + 1)..self.nodes.len() {
                    let delta = self.nodes[j].pos - self.nodes[i].pos;
                    let dist = delta.length().max(1.0);
                    let min_dist = 78.0;
                    if dist < min_dist {
                        let push = (delta / dist) * ((min_dist - dist) * 0.45);
                        self.nodes[i].pos -= push;
                        self.nodes[j].pos += push;
                    }
                }
            }
        }
    }

    pub fn reposition_hub_with_satellites(&mut self, hub_id: &str, delta_world: Vec2) {
        let Some(&h_idx) = self.hub_index.get(hub_id) else {
            return;
        };
        let category = self.hubs[h_idx].category.clone();
        self.hubs[h_idx].pos += delta_world;
        for node in &mut self.nodes {
            if node.category == category {
                node.pos += delta_world;
            }
        }
    }

    pub fn entity_world_pos(&self, id: &str) -> Option<Pos2> {
        if let Some(&idx) = self.hub_index.get(id) {
            return Some(self.hubs[idx].pos);
        }
        if let Some(&idx) = self.node_index.get(id) {
            return Some(self.nodes[idx].pos);
        }
        None
    }

    pub fn entity_accent(&self, id: &str) -> &str {
        if let Some(&idx) = self.hub_index.get(id) {
            return &self.hubs[idx].accent;
        }
        if let Some(&idx) = self.node_index.get(id) {
            return &self.nodes[idx].accent;
        }
        "violet"
    }
}

fn paint_radial_gradient(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    stops: &[(f32, Color32)],
    segments: usize,
) {
    if radius <= 0.5 || stops.is_empty() {
        return;
    }

    let mut mesh = Mesh::default();
    let center_color = stops[0].1;
    mesh.vertices.push(Vertex {
        pos: center,
        uv: WHITE_UV,
        color: center_color,
    });

    let ring_count = stops.len().saturating_sub(1).max(1);
    for ring_idx in 1..=ring_count {
        let (t, color) = if ring_idx < stops.len() {
            stops[ring_idx]
        } else {
            (1.0, Color32::TRANSPARENT)
        };
        let r = radius * t;
        for s in 0..segments {
            let angle = (s as f32) * std::f32::consts::TAU / (segments as f32);
            mesh.vertices.push(Vertex {
                pos: Pos2::new(center.x + angle.cos() * r, center.y + angle.sin() * r),
                uv: WHITE_UV,
                color,
            });
        }
    }

    for s in 0..segments {
        let next = (s + 1) % segments;
        mesh.indices.push(0);
        mesh.indices.push(1 + s as u32);
        mesh.indices.push(1 + next as u32);
    }

    for ring_idx in 1..ring_count {
        let inner_base = 1 + (ring_idx - 1) * segments;
        let outer_base = 1 + ring_idx * segments;
        for s in 0..segments {
            let next = (s + 1) % segments;
            let i0 = (inner_base + s) as u32;
            let i1 = (inner_base + next) as u32;
            let o0 = (outer_base + s) as u32;
            let o1 = (outer_base + next) as u32;
            mesh.indices.extend_from_slice(&[i0, o0, o1, i0, o1, i1]);
        }
    }

    painter.add(Shape::mesh(mesh));
}

fn cubic_bezier(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    Pos2::new(
        uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x,
        uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y,
    )
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-4 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn paint_braided_filament(
    painter: &Painter,
    p0: Pos2,
    p3: Pos2,
    c_start: Color32,
    c_end: Color32,
    kind: &str,
    highlighted: bool,
    dimmed: bool,
    time: f64,
    seed: usize,
) -> Vec<Pos2> {
    let delta = p3 - p0;
    let dist = delta.length().max(1.0);
    let dir = delta / dist;
    let perp = Vec2::new(-dir.y, dir.x);

    let curve_sign = if seed % 2 == 0 { 1.0 } else { -1.0 };
    let bend = match kind {
        "pipeline" => (dist * 0.16).clamp(18.0, 56.0) * curve_sign,
        "semantic" => (dist * 0.20).clamp(22.0, 68.0) * curve_sign,
        "backbone" => (dist * 0.12).clamp(14.0, 42.0) * curve_sign,
        _ => (dist * 0.06).clamp(4.0, 16.0) * curve_sign,
    };

    let p1 = p0 + delta * 0.32 + perp * bend;
    let p2 = p0 + delta * 0.68 - perp * (bend * 0.45);

    let steps = match kind {
        "pipeline" | "semantic" => 28,
        _ => 18,
    };

    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        pts.push(cubic_bezier(p0, p1, p2, p3, t));
    }

    let alpha_scale = if dimmed {
        0.18
    } else if highlighted {
        1.25
    } else {
        1.0
    };

    let (glow_w, mid_w, core_w, base_alpha) = match kind {
        "pipeline" => (8.5, 3.4, 1.55, 0.86),
        "semantic" => (7.5, 3.0, 1.45, 0.82),
        "cluster" => (4.8, 1.9, 1.15, 0.62),
        _ => (4.2, 1.7, 1.0, 0.42),
    };

    for i in 0..steps {
        let t = (i as f32 + 0.5) / steps as f32;
        let col = lerp_color(c_start, c_end, t);
        let a_glow = ((28.0_f32 * base_alpha * alpha_scale).clamp(4.0, 95.0)) as u8;
        let a_mid = ((78.0_f32 * base_alpha * alpha_scale).clamp(8.0, 170.0)) as u8;
        let a_core = ((215.0_f32 * base_alpha * alpha_scale).clamp(18.0, 255.0)) as u8;

        painter.line_segment([pts[i], pts[i + 1]], Stroke::new(glow_w, with_alpha(col, a_glow)));
        painter.line_segment([pts[i], pts[i + 1]], Stroke::new(mid_w, with_alpha(col, a_mid)));
        painter.line_segment([pts[i], pts[i + 1]], Stroke::new(core_w, with_alpha(col, a_core)));
    }

    if (kind == "pipeline" || kind == "semantic") && !dimmed {
        for strand in [-1.0_f32, 1.0] {
            let mut strand_pts = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let envelope = (t * std::f32::consts::PI).sin();
                let wave = (t * std::f32::consts::TAU * 1.5 + seed as f32).sin();
                let offset = perp * (strand * 4.5 * envelope + wave * 2.2 * envelope);
                strand_pts.push(pts[i] + offset);
            }
            let strand_col = lerp_color(c_start, c_end, 0.5);
            painter.add(Shape::line(
                strand_pts,
                Stroke::new(0.9, with_alpha(strand_col, if highlighted { 145 } else { 85 })),
            ));
        }

        let speed = if kind == "pipeline" { 0.22 } else { 0.28 };
        let pulse_t = ((time * speed + (seed as f64) * 0.31) % 1.0) as f32;
        let pulse_pos = cubic_bezier(p0, p1, p2, p3, pulse_t);
        let pulse_col = lerp_color(c_start, c_end, pulse_t);
        painter.circle_filled(pulse_pos, 7.0, with_alpha(pulse_col, 48));
        painter.circle_filled(pulse_pos, 2.8, with_alpha(Color32::WHITE, 230));
    }

    pts
}

fn paint_specular_arc(painter: &Painter, center: Pos2, radius: f32, color: Color32, width: f32) {
    let steps = 18;
    let start = -2.55_f32;
    let end = -0.65_f32;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let a0 = start + (end - start) * t0;
        let a1 = start + (end - start) * t1;
        let mid_t = (t0 + t1) * 0.5;
        let intensity = (mid_t * std::f32::consts::PI).sin();
        let alpha = (intensity * (color.a() as f32)).round() as u8;
        let p0 = Pos2::new(center.x + a0.cos() * radius, center.y + a0.sin() * radius);
        let p1 = Pos2::new(center.x + a1.cos() * radius, center.y + a1.sin() * radius);
        painter.line_segment([p0, p1], Stroke::new(width, with_alpha(color, alpha)));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_galaxy_canvas(
    ui: &mut Ui,
    rect: Rect,
    snapshot: &GraphSnapshot,
    layout: &mut GalaxyLayout,
    camera: &mut GalaxyCamera,
    selected_hub: Option<&str>,
    selected_node: Option<&str>,
    active_category_filter: Option<&str>,
    search_highlight_hashes: &HashSet<String>,
    show_semantic_links: bool,
    time: f64,
) -> GalaxyAction {
    let mut action = GalaxyAction::None;
    let p = palette();
    let painter = ui.painter_at(rect);

    // 1. Deep Atmospheric Nebula Backdrop (GPU vertex-colored radial meshes)
    painter.rect_filled(rect, CornerRadius::ZERO, p.bg_void);
    let center = rect.center();

    paint_radial_gradient(
        &painter,
        Pos2::new(center.x - rect.width() * 0.04, center.y - rect.height() * 0.06),
        rect.width().max(rect.height()) * 0.66,
        &[
            (0.0, with_alpha(p.nebula_center, if p.is_light { 140 } else { 205 })),
            (0.48, with_alpha(p.nebula_mid, if p.is_light { 90 } else { 135 })),
            (1.0, with_alpha(p.bg_void, 0)),
        ],
        44,
    );

    // Warm copper nebula bloom on lower-left & cool indigo bloom on right (matching [Image #53])
    paint_radial_gradient(
        &painter,
        Pos2::new(center.x - rect.width() * 0.25, center.y + rect.height() * 0.12),
        rect.width() * 0.38,
        &[
            (0.0, with_alpha(p.copper, if p.is_light { 24 } else { 46 })),
            (0.52, with_alpha(p.copper, if p.is_light { 8 } else { 16 })),
            (1.0, Color32::TRANSPARENT),
        ],
        36,
    );
    paint_radial_gradient(
        &painter,
        Pos2::new(center.x + rect.width() * 0.22, center.y - rect.height() * 0.08),
        rect.width() * 0.40,
        &[
            (0.0, with_alpha(p.indigo, if p.is_light { 26 } else { 52 })),
            (0.52, with_alpha(p.indigo, if p.is_light { 10 } else { 18 })),
            (1.0, Color32::TRANSPARENT),
        ],
        36,
    );
    paint_radial_gradient(
        &painter,
        Pos2::new(center.x - rect.width() * 0.18, center.y - rect.height() * 0.24),
        rect.width() * 0.28,
        &[
            (0.0, with_alpha(p.cyan, if p.is_light { 14 } else { 24 })),
            (0.6, with_alpha(p.cyan, 6)),
            (1.0, Color32::TRANSPARENT),
        ],
        28,
    );

    // Ambient starfield dust (dark themes) or subtle coordinate dots (light theme)
    for i in 0..48 {
        let fx = ((i * 73 + 19) % 997) as f32 / 997.0;
        let fy = ((i * 137 + 43) % 991) as f32 / 991.0;
        let sx = rect.left() + fx * rect.width();
        let sy = rect.top() + fy * rect.height();
        let twinkle = 0.55 + 0.45 * ((time * 0.9 + i as f64 * 0.7).sin() as f32);
        let alpha = if p.is_light {
            (18.0 + 16.0 * twinkle) as u8
        } else {
            (18.0 + 28.0 * twinkle) as u8
        };
        let r = if i % 7 == 0 { 1.5 } else { 0.95 };
        painter.circle_filled(Pos2::new(sx, sy), r, with_alpha(p.indigo_bright, alpha));
    }

    if layout.hubs.is_empty() {
        painter.text(
            Pos2::new(center.x, center.y - 18.0),
            Align2::CENTER_CENTER,
            "No documents indexed in this workspace",
            font_semibold(16.0),
            p.ink_primary,
        );
        painter.text(
            Pos2::new(center.x, center.y + 10.0),
            Align2::CENTER_CENTER,
            "Drop PDF, DOCX, image, code, or text files here — or switch workspace above",
            font_sans(13.0),
            p.ink_muted,
        );
        return action;
    }

    // 2. Pointer Interaction
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let pointer_over_canvas = response.contains_pointer();
    let pointer_pos = if pointer_over_canvas || response.dragged() {
        ui.input(|i| i.pointer.hover_pos()).filter(|pt| rect.contains(*pt))
    } else {
        None
    };

    if pointer_over_canvas {
        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_y.abs() > 0.1 {
            let old_zoom = camera.target_zoom;
            let factor = (1.0 + scroll_y * 0.0018).clamp(0.82, 1.22);
            let new_zoom = (old_zoom * factor).clamp(0.52, 2.25);
            if let Some(cursor) = pointer_pos {
                let world_before = ((cursor - center - camera.target_pan) / old_zoom).to_pos2();
                camera.target_zoom = new_zoom;
                camera.target_pan = cursor - center - world_before.to_vec2() * new_zoom;
            } else {
                camera.target_zoom = new_zoom;
            }
        }
    }

    layout.hovered_node = None;
    layout.hovered_hub = None;
    layout.hovered_edge = None;

    if pointer_over_canvas {
        if let Some(cursor) = pointer_pos {
            for node in layout.nodes.iter().rev() {
                let sp = camera.world_to_screen(node.pos, center);
                let hit = if node.capsule_mode || selected_node == Some(node.hash.as_str()) {
                    Rect::from_center_size(
                        sp,
                        Vec2::new(
                            156.0 * camera.zoom.clamp(0.8, 1.2),
                            42.0 * camera.zoom.clamp(0.8, 1.2),
                        ),
                    )
                    .contains(cursor)
                } else {
                    cursor.distance(sp) <= (node.radius + 6.0) * camera.zoom
                };
                if hit {
                    layout.hovered_node = Some(node.hash.clone());
                    break;
                }
            }

            if layout.hovered_node.is_none() {
                for hub in layout.hubs.iter().rev() {
                    let sp = camera.world_to_screen(hub.pos, center);
                    if cursor.distance(sp) <= hub.radius * camera.zoom {
                        layout.hovered_hub = Some(hub.id.clone());
                        break;
                    }
                }
            }
        }
    }

    if response.drag_started() {
        if let Some(ref h_id) = layout.hovered_hub {
            layout.dragged_hub = Some(h_id.clone());
        }
    }
    if response.dragged() {
        let delta = response.drag_delta();
        if let Some(ref h_id) = layout.dragged_hub.clone() {
            layout.reposition_hub_with_satellites(h_id, delta / camera.zoom);
        } else {
            camera.pan += delta;
            camera.target_pan = camera.pan;
        }
    }
    if response.drag_stopped() {
        layout.dragged_hub = None;
    }

    if layout.hovered_node.is_some() || layout.hovered_hub.is_some() {
        ui.ctx().set_cursor_icon(if layout.dragged_hub.is_some() {
            CursorIcon::Grabbing
        } else {
            CursorIcon::PointingHand
        });
    } else if response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }

    if response.clicked() {
        if let Some(ref n_hash) = layout.hovered_node {
            action = GalaxyAction::SelectNode(n_hash.clone());
        } else if let Some(ref h_id) = layout.hovered_hub {
            action = GalaxyAction::SelectHub(h_id.clone());
        } else {
            action = GalaxyAction::ClearSelection;
        }
    }

    let has_search_filter = !search_highlight_hashes.is_empty();
    let is_node_active = |node: &NodeLayout| -> bool {
        if let Some(cat) = active_category_filter {
            if node.category != cat {
                return false;
            }
        }
        if has_search_filter && !search_highlight_hashes.contains(&node.hash) {
            return false;
        }
        true
    };

    // 3. Render Filaments (Cluster spokes + Hub bridges; cross-file semantic links ONLY when toggled or hovered/selected)
    let mut edge_polylines: Vec<(usize, Vec<Pos2>, &GraphEdge)> = Vec::new();
    for (idx, edge) in snapshot.edges.iter().enumerate() {
        if edge.kind == "semantic" {
            let touches_focus = selected_node
                .map(|s| edge.source == s || edge.target == s)
                .unwrap_or(false)
                || layout
                    .hovered_node
                    .as_deref()
                    .map(|h| edge.source == h || edge.target == h)
                    .unwrap_or(false);
            if !show_semantic_links && !touches_focus {
                continue;
            }
        }

        let Some(w_src) = layout.entity_world_pos(&edge.source) else {
            continue;
        };
        let Some(w_dst) = layout.entity_world_pos(&edge.target) else {
            continue;
        };
        let p0 = camera.world_to_screen(w_src, center);
        let p3 = camera.world_to_screen(w_dst, center);

        let c_start = accent_color(layout.entity_accent(&edge.source));
        let c_end = accent_color(layout.entity_accent(&edge.target));

        let touches_selected = selected_node
            .map(|s| edge.source == s || edge.target == s)
            .unwrap_or(false)
            || selected_hub
                .map(|h| edge.source == h || edge.target == h)
                .unwrap_or(false);

        let dimmed = if let Some(cat) = active_category_filter {
            let hub_id = format!("hub:{cat}");
            let src_in = edge.source == hub_id
                || layout
                    .node_index
                    .get(&edge.source)
                    .map(|&i| layout.nodes[i].category == cat)
                    .unwrap_or(false);
            let dst_in = edge.target == hub_id
                || layout
                    .node_index
                    .get(&edge.target)
                    .map(|&i| layout.nodes[i].category == cat)
                    .unwrap_or(false);
            !(src_in || dst_in)
        } else {
            false
        };

        let pts = paint_braided_filament(
            &painter,
            p0,
            p3,
            c_start,
            c_end,
            &edge.kind,
            touches_selected,
            dimmed,
            time,
            idx,
        );

        if edge.kind == "semantic" || edge.kind == "pipeline" {
            edge_polylines.push((idx, pts, edge));
        }
    }

    if layout.hovered_node.is_none() && layout.hovered_hub.is_none() {
        if let Some(cursor) = pointer_pos {
            for (idx, pts, _) in &edge_polylines {
                let hit = pts
                    .windows(2)
                    .any(|w| dist_to_segment(cursor, w[0], w[1]) <= 8.0);
                if hit {
                    layout.hovered_edge = Some(*idx);
                    break;
                }
            }
        }
    }

    // 4. Render Volumetric Luminous Hubs
    for (idx, hub) in layout.hubs.iter().enumerate() {
        let sp = camera.world_to_screen(hub.pos, center);
        let r = hub.radius * camera.zoom;
        let col = accent_color(&hub.accent);
        let bright = accent_bright(&hub.accent);
        let core = accent_core(&hub.accent);

        let is_selected = selected_hub == Some(hub.id.as_str())
            || active_category_filter == Some(hub.category.as_str());
        let is_hovered = layout.hovered_hub.as_deref() == Some(hub.id.as_str());
        let dimmed = active_category_filter
            .map(|cat| cat != hub.category)
            .unwrap_or(false);

        let alpha_mul = if dimmed { 0.32 } else { 1.0 };

        let show_ripples = is_selected || is_hovered || idx == 1 || idx == 0;
        if show_ripples && !dimmed {
            let ring_count = if is_selected || is_hovered { 4 } else { 3 };
            let pulse = ((time * 1.4 + idx as f64).sin() as f32) * 0.04;
            for ring in 1..=ring_count {
                let mult = 1.0 + (ring as f32) * 0.36 + pulse * (ring as f32);
                let ring_r = r * mult;
                let ring_alpha = ((64.0 / (ring as f32)) * if is_selected { 1.45 } else { 1.0 })
                    .clamp(10.0, 105.0) as u8;
                painter.circle_stroke(
                    sp,
                    ring_r,
                    Stroke::new(1.2, with_alpha(col, ring_alpha)),
                );
            }
        }

        // Volumetric outer halo
        paint_radial_gradient(
            &painter,
            sp,
            r * 2.35,
            &[
                (0.0, with_alpha(col, ((if p.is_light { 52.0 } else { 96.0 }) * alpha_mul) as u8)),
                (0.45, with_alpha(col, ((if p.is_light { 20.0 } else { 40.0 }) * alpha_mul) as u8)),
                (1.0, Color32::TRANSPARENT),
            ],
            36,
        );

        // Luminous glass sphere core
        paint_radial_gradient(
            &painter,
            sp,
            r,
            &[
                (0.0, with_alpha(p.bg_surface, (248.0 * alpha_mul) as u8)),
                (0.66, with_alpha(core, (242.0 * alpha_mul) as u8)),
                (1.0, with_alpha(col, (115.0 * alpha_mul) as u8)),
            ],
            36,
        );

        // Inner plasma ring + outer glowing rim + directional specular arc
        painter.circle_stroke(
            sp,
            r * 0.84,
            Stroke::new(1.0, with_alpha(col, (75.0 * alpha_mul) as u8)),
        );
        let ring_w = if is_selected || is_hovered { 2.5 } else { 1.8 };
        painter.circle_stroke(
            sp,
            r,
            Stroke::new(ring_w, with_alpha(bright, (230.0 * alpha_mul) as u8)),
        );
        paint_specular_arc(
            &painter,
            sp,
            r - 1.4,
            with_alpha(Color32::from_rgb(255, 250, 244), (185.0 * alpha_mul) as u8),
            1.9,
        );

        let label_size = (13.5 * camera.zoom).clamp(10.5, 17.0);
        painter.text(
            Pos2::new(sp.x, sp.y - 5.0 * camera.zoom),
            Align2::CENTER_CENTER,
            &hub.category,
            font_semibold(label_size),
            with_alpha(p.ink_primary, (255.0 * alpha_mul) as u8),
        );
        let count_label = if hub.count == 1 {
            "1 doc".to_string()
        } else {
            format!("{} docs", hub.count)
        };
        painter.text(
            Pos2::new(sp.x, sp.y + 11.0 * camera.zoom),
            Align2::CENTER_CENTER,
            count_label,
            font_sans((10.5 * camera.zoom).clamp(8.5, 13.0)),
            with_alpha(p.ink_secondary, (215.0 * alpha_mul) as u8),
        );
    }

    // 5. Render Satellite Document Nodes
    for node in &layout.nodes {
        let sp = camera.world_to_screen(node.pos, center);
        let col = accent_color(&node.accent);
        let bright = accent_bright(&node.accent);
        let is_selected = selected_node == Some(node.hash.as_str());
        let is_hovered = layout.hovered_node.as_deref() == Some(node.hash.as_str());
        let active = is_node_active(node);
        let alpha_mul = if active { 1.0 } else { 0.24 };

        let render_capsule = node.capsule_mode || is_selected;

        if render_capsule {
            let w = 158.0 * camera.zoom.clamp(0.82, 1.18);
            let h = 44.0 * camera.zoom.clamp(0.82, 1.18);
            let cap_rect = Rect::from_center_size(sp, Vec2::new(w, h));

            if active {
                paint_soft_shadow(&painter, cap_rect, 12);
            }

            if is_selected || is_hovered {
                paint_radial_gradient(
                    &painter,
                    sp,
                    w * 0.68,
                    &[
                        (0.0, with_alpha(col, 58)),
                        (1.0, Color32::TRANSPARENT),
                    ],
                    24,
                );
            }

            painter.rect_filled(
                cap_rect,
                CornerRadius::same(12),
                with_alpha(p.bg_surface_elevated, (242.0 * alpha_mul) as u8),
            );
            painter.rect_stroke(
                cap_rect,
                CornerRadius::same(12),
                Stroke::new(
                    if is_selected { 1.6 } else { 1.1 },
                    with_alpha(
                        if is_selected { p.copper_bright } else { bright },
                        (if is_selected { 240.0 } else { 155.0 } * alpha_mul) as u8,
                    ),
                ),
                StrokeKind::Inside,
            );

            let badge_rect = Rect::from_min_size(
                Pos2::new(cap_rect.left() + 8.0, cap_rect.center().y - 13.0),
                Vec2::splat(26.0),
            );
            painter.rect_filled(
                badge_rect,
                CornerRadius::same(7),
                with_alpha(col, (52.0 * alpha_mul) as u8),
            );
            draw_icon_file_type(
                &painter,
                badge_rect.center(),
                15.0,
                &node.file_type,
                with_alpha(bright, (245.0 * alpha_mul) as u8),
            );

            let short_name = truncate_label(&node.file, 16);
            painter.text(
                Pos2::new(badge_rect.right() + 8.0, cap_rect.center().y - 6.5),
                Align2::LEFT_CENTER,
                short_name,
                font_medium(11.5),
                with_alpha(p.ink_primary, (252.0 * alpha_mul) as u8),
            );
            let sub = format!("{} · {}", node.category, format_bytes(node.size_bytes));
            painter.text(
                Pos2::new(badge_rect.right() + 8.0, cap_rect.center().y + 7.5),
                Align2::LEFT_CENTER,
                sub,
                font_sans(10.0),
                with_alpha(p.ink_muted, (225.0 * alpha_mul) as u8),
            );
        } else {
            let r = node.radius * camera.zoom;
            if is_hovered || (has_search_filter && active) {
                paint_radial_gradient(
                    &painter,
                    sp,
                    r * 2.2,
                    &[
                        (0.0, with_alpha(col, 86)),
                        (1.0, Color32::TRANSPARENT),
                    ],
                    24,
                );
            }

            painter.circle_filled(
                sp,
                r,
                with_alpha(p.bg_surface, (242.0 * alpha_mul) as u8),
            );
            painter.circle_stroke(
                sp,
                r,
                Stroke::new(
                    if is_hovered { 1.8 } else { 1.25 },
                    with_alpha(bright, (if is_hovered { 240.0 } else { 165.0 } * alpha_mul) as u8),
                ),
            );
            paint_specular_arc(
                &painter,
                sp,
                r - 1.0,
                with_alpha(Color32::WHITE, (120.0 * alpha_mul) as u8),
                1.2,
            );

            draw_icon_file_type(
                &painter,
                sp,
                r * 0.95,
                &node.file_type,
                with_alpha(bright, (240.0 * alpha_mul) as u8),
            );

            let short_name = truncate_label(&node.file, 18);
            let parent_hub_pos = layout
                .hub_index
                .get(&format!("hub:{}", node.category))
                .map(|&hi| camera.world_to_screen(layout.hubs[hi].pos, center));
            let place_above = parent_hub_pos
                .map(|hp| sp.y < hp.y - 14.0)
                .unwrap_or(false);
            let label_pos = if place_above {
                Pos2::new(sp.x, sp.y - r - 13.0)
            } else {
                Pos2::new(sp.x, sp.y + r + 13.0)
            };
            let galley = painter.layout_no_wrap(
                short_name,
                font_sans((10.8 * camera.zoom).clamp(9.0, 12.5)),
                with_alpha(p.ink_secondary, (240.0 * alpha_mul) as u8),
            );
            let pill_rect = Rect::from_center_size(
                label_pos,
                galley.size() + Vec2::new(12.0, 4.0),
            );
            painter.rect_filled(
                pill_rect,
                CornerRadius::same(6),
                with_alpha(p.bg_canvas, (215.0 * alpha_mul) as u8),
            );
            painter.rect_stroke(
                pill_rect,
                CornerRadius::same(6),
                Stroke::new(0.8, with_alpha(p.border_subtle, (140.0 * alpha_mul) as u8)),
                StrokeKind::Inside,
            );
            painter.galley(
                pill_rect.center() - galley.size() * 0.5,
                galley,
                Color32::TRANSPARENT,
            );
        }
    }

    // 6. Crisp Hover Tooltip
    if let Some(cursor) = pointer_pos {
        if let Some(ref n_hash) = layout.hovered_node {
            if let Some(&idx) = layout.node_index.get(n_hash) {
                let node = &layout.nodes[idx];
                let kws = node.keywords.iter().take(4).cloned().collect::<Vec<_>>().join(" · ");
                paint_tooltip(
                    &painter,
                    rect,
                    cursor,
                    &node.file,
                    &format!(
                        "{} · {} · {}",
                        node.category,
                        node.file_type.to_uppercase(),
                        format_bytes(node.size_bytes)
                    ),
                    &kws,
                    accent_color(&node.accent),
                );
            }
        } else if let Some(ref h_id) = layout.hovered_hub {
            if let Some(&idx) = layout.hub_index.get(h_id) {
                let hub = &layout.hubs[idx];
                let kws = hub.top_keywords.join(" · ");
                paint_tooltip(
                    &painter,
                    rect,
                    cursor,
                    &format!("{} Cluster", hub.category),
                    &format!("{} indexed documents — click to filter", hub.count),
                    &kws,
                    accent_color(&hub.accent),
                );
            }
        } else if let Some(edge_idx) = layout.hovered_edge {
            if let Some(edge) = snapshot.edges.get(edge_idx) {
                let shared = if edge.shared_keywords.is_empty() {
                    "Cross-cluster semantic bridge".to_string()
                } else {
                    format!("Shared: {}", edge.shared_keywords.join(", "))
                };
                paint_tooltip(
                    &painter,
                    rect,
                    cursor,
                    if edge.kind == "pipeline" {
                        "Inter-Cluster Pipeline"
                    } else {
                        "Semantic Connection"
                    },
                    &shared,
                    "Click connected documents to inspect",
                    p.indigo_bright,
                );
            }
        }
    }

    action
}

fn paint_tooltip(
    painter: &Painter,
    bounds: Rect,
    cursor: Pos2,
    title: &str,
    subtitle: &str,
    detail: &str,
    accent: Color32,
) {
    let p = palette();
    let w = 252.0;
    let h = if detail.is_empty() { 52.0 } else { 70.0 };
    let mut pos = Pos2::new(cursor.x + 16.0, cursor.y - h - 10.0);
    if pos.x + w > bounds.right() - 12.0 {
        pos.x = cursor.x - w - 14.0;
    }
    if pos.y < bounds.top() + 12.0 {
        pos.y = cursor.y + 18.0;
    }
    let tip_rect = Rect::from_min_size(pos, Vec2::new(w, h));

    paint_soft_shadow(painter, tip_rect, 10);
    painter.rect_filled(
        tip_rect,
        CornerRadius::same(10),
        with_alpha(p.bg_surface_elevated, 248),
    );
    painter.rect_stroke(
        tip_rect,
        CornerRadius::same(10),
        Stroke::new(1.0, with_alpha(accent, 175)),
        StrokeKind::Inside,
    );

    painter.text(
        Pos2::new(tip_rect.left() + 12.0, tip_rect.top() + 16.0),
        Align2::LEFT_CENTER,
        truncate_label(title, 28),
        font_semibold(12.0),
        p.ink_primary,
    );
    painter.text(
        Pos2::new(tip_rect.left() + 12.0, tip_rect.top() + 34.0),
        Align2::LEFT_CENTER,
        truncate_label(subtitle, 34),
        font_sans(11.0),
        p.ink_secondary,
    );
    if !detail.is_empty() {
        painter.text(
            Pos2::new(tip_rect.left() + 12.0, tip_rect.top() + 52.0),
            Align2::LEFT_CENTER,
            truncate_label(detail, 36),
            font_sans(10.5),
            p.ink_muted,
        );
    }
}

pub fn render_cluster_grid(
    ui: &mut Ui,
    rect: Rect,
    snapshot: &GraphSnapshot,
    selected_node: Option<&str>,
    active_category_filter: Option<&str>,
) -> GalaxyAction {
    let mut action = GalaxyAction::None;
    let p = palette();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, p.bg_canvas);

    let inner = rect.shrink2(Vec2::new(28.0, 24.0));
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );

    child_ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Cluster Matrix")
                .font(font_semibold(16.0))
                .color(p.ink_primary),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!(
                "{} clusters · {} documents",
                snapshot.hubs.len(),
                snapshot.nodes.len()
            ))
            .font(font_sans(12.5))
            .color(p.ink_muted),
        );
    });
    child_ui.add_space(14.0);

    egui::ScrollArea::vertical()
        .id_salt("cluster_grid_scroll")
        .auto_shrink([false, false])
        .show(&mut child_ui, |ui| {
            let avail_w = ui.available_width();
            let cols = if avail_w >= 820.0 {
                3
            } else if avail_w >= 520.0 {
                2
            } else {
                1
            };
            let gap = 14.0;
            let card_w = ((avail_w - gap * ((cols - 1) as f32)) / (cols as f32)).max(220.0);

            let hubs: Vec<&GraphHub> = snapshot
                .hubs
                .iter()
                .filter(|h| {
                    active_category_filter
                        .map(|c| c == h.category)
                        .unwrap_or(true)
                })
                .collect();

            for row in hubs.chunks(cols) {
                ui.horizontal_top(|ui| {
                    for (c_idx, hub) in row.iter().enumerate() {
                        if c_idx > 0 {
                            ui.add_space(gap);
                        }
                        let col = accent_color(&hub.accent);
                        let bright = accent_bright(&hub.accent);
                        let frame = egui::Frame::new()
                            .fill(p.bg_surface)
                            .stroke(Stroke::new(1.0, p.border_subtle))
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(egui::Margin::same(14));

                        frame.show(ui, |ui| {
                            ui.set_width(card_w - 28.0);
                            ui.horizontal(|ui| {
                                let (dot_rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                                ui.painter().circle_filled(dot_rect.center(), 4.5, col);
                                ui.label(
                                    egui::RichText::new(&hub.category)
                                        .font(font_semibold(14.0))
                                        .color(p.ink_primary),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(format!("{} docs", hub.count))
                                                .font(font_sans(11.5))
                                                .color(bright),
                                        );
                                    },
                                );
                            });

                            if !hub.top_keywords.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(hub.top_keywords.join(" · "))
                                        .font(font_sans(11.0))
                                        .color(p.ink_muted),
                                );
                            }

                            ui.add_space(10.0);
                            for node in snapshot.nodes.iter().filter(|n| n.category == hub.category)
                            {
                                let is_sel = selected_node == Some(node.hash.as_str());
                                let row_fill = if is_sel {
                                    with_alpha(p.copper, 38)
                                } else {
                                    p.bg_surface_elevated
                                };
                                let row_stroke = if is_sel {
                                    Stroke::new(1.0, p.copper)
                                } else {
                                    Stroke::new(1.0, p.border_subtle)
                                };
                                let resp = egui::Frame::new()
                                    .fill(row_fill)
                                    .stroke(row_stroke)
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::symmetric(10, 7))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            let (ic_rect, _) = ui.allocate_exact_size(
                                                Vec2::splat(16.0),
                                                Sense::hover(),
                                            );
                                            draw_icon_file_type(
                                                ui.painter(),
                                                ic_rect.center(),
                                                14.0,
                                                &node.file_type,
                                                bright,
                                            );
                                            ui.label(
                                                egui::RichText::new(truncate_label(&node.file, 24))
                                                    .font(font_medium(12.0))
                                                    .color(p.ink_primary),
                                            );
                                        });
                                    })
                                    .response
                                    .interact(Sense::click());

                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                }
                                if resp.clicked() {
                                    action = GalaxyAction::SelectNode(node.hash.clone());
                                }
                                ui.add_space(5.0);
                            }
                        });
                    }
                });
                ui.add_space(gap);
            }
            ui.add_space(90.0);
        });

    action
}

pub fn truncate_label(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

pub fn format_bytes(bytes: Option<u64>) -> String {
    match bytes {
        None => "missing".to_string(),
        Some(b) if b < 1024 => format!("{b} B"),
        Some(b) if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        Some(b) => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::build_graph_snapshot;
    use crate::index::IndexItem;

    #[test]
    fn test_galaxy_layout_avoids_hub_and_node_overlap() {
        let items = vec![
            IndexItem {
                file: "invoice_1.txt".into(),
                category: "Invoice".into(),
                keywords: vec!["invoice".into(), "tax".into()],
                symbols: Vec::new(),
                path: "documents/invoice_1.txt".into(),
                hash: "h1".into(),
                indexed_at: None,
                ..Default::default()
            },
            IndexItem {
                file: "medical_1.txt".into(),
                category: "Medical".into(),
                keywords: vec!["patient".into(), "research".into()],
                symbols: Vec::new(),
                path: "documents/medical_1.txt".into(),
                hash: "h2".into(),
                indexed_at: None,
                ..Default::default()
            },
            IndexItem {
                file: "legal_1.txt".into(),
                category: "Legal".into(),
                keywords: vec!["contract".into()],
                symbols: Vec::new(),
                path: "documents/legal_1.txt".into(),
                hash: "h3".into(),
                indexed_at: None,
                ..Default::default()
            },
            IndexItem {
                file: "tech_1.txt".into(),
                category: "Technical".into(),
                keywords: vec!["rust".into()],
                symbols: Vec::new(),
                path: "documents/tech_1.txt".into(),
                hash: "h4".into(),
                indexed_at: None,
                ..Default::default()
            },
            IndexItem {
                file: "academic_1.txt".into(),
                category: "Academic".into(),
                keywords: vec!["research".into()],
                symbols: Vec::new(),
                path: "documents/academic_1.txt".into(),
                hash: "h5".into(),
                indexed_at: None,
                ..Default::default()
            },
        ];

        let snap = build_graph_snapshot(&items);
        let mut layout = GalaxyLayout::default();
        layout.rebuild(&snap);

        assert_eq!(layout.hubs.len(), 5);
        assert_eq!(layout.nodes.len(), 5);

        for i in 0..layout.hubs.len() {
            for j in (i + 1)..layout.hubs.len() {
                let d = layout.hubs[i].pos.distance(layout.hubs[j].pos);
                assert!(
                    d >= layout.hubs[i].radius + layout.hubs[j].radius + 40.0,
                    "Hubs {} and {} overlapped (dist={d})",
                    layout.hubs[i].category,
                    layout.hubs[j].category
                );
            }
        }

        for i in 0..layout.nodes.len() {
            for j in (i + 1)..layout.nodes.len() {
                let d = layout.nodes[i].pos.distance(layout.nodes[j].pos);
                assert!(d >= 50.0, "Nodes {} and {} too close (dist={d})", i, j);
            }
        }
    }
}
