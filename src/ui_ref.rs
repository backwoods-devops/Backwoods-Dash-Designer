// Reference-image window (load, calibrate from two points, position) and drawing in the dock editors.
use crate::app::*;
use crate::refimg::{self, RefImage, Stage};
use crate::theme::*;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, Stroke};

pub fn tab_index(t: DockTab) -> usize {
    match t {
        DockTab::Profile => 0,
        DockTab::Plan => 1,
        DockTab::Height => 2,
        DockTab::Walls => 0,
        DockTab::Part => 1,
    }
}

impl App {
    pub fn save_refs(&self) {
        refimg::save_all(&self.st.dir, &self.refs);
    }

    /// Widen fit bounds [x0, x1, y0, y1] to include a visible reference image.
    pub fn ref_extend(&self, idx: usize, b: &mut [f32; 4]) {
        if let Some(r) = self.refs[idx].as_ref().filter(|r| r.visible && r.w > 0) {
            for (px, py) in [(0.0, 0.0), (r.w as f64, 0.0), (r.w as f64, r.h as f64), (0.0, r.h as f64)] {
                let w = r.px_to_world(px, py);
                b[0] = b[0].min(w[0] as f32);
                b[1] = b[1].max(w[0] as f32);
                b[2] = b[2].min(w[1] as f32);
                b[3] = b[3].max(w[1] as f32);
            }
        }
    }

    pub fn load_ref_file(&mut self, ctx: &egui::Context, idx: usize, path: &str) {
        let mut r = RefImage::new(path);
        r.ensure(ctx);
        if r.tex.is_none() {
            self.ref_msg = if r.err.is_empty() { "Could not read that file".into() } else { r.err.clone() };
            return;
        }
        r.default_scale(400.0);
        self.refs[idx] = Some(r);
        self.side.fitted = false;
        self.plan.fitted = false;
        self.elev.fitted = false;
        self.ref_msg.clear();
        self.save_refs();
    }

    /// Draw the reference image (and the marked points) under an editor. `v` maps editor coordinates to the screen.
    pub fn draw_ref(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, v: &View2, idx: usize) {
        let Some(r) = self.refs[idx].as_mut() else {
            return;
        };
        r.ensure(ctx);
        if !r.visible {
            return;
        }
        let Some(tex) = r.tex.clone() else {
            return;
        };
        let corners = [(0.0, 0.0), (r.w as f64, 0.0), (r.w as f64, r.h as f64), (0.0, r.h as f64)];
        let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
        let col = Color32::from_white_alpha((r.opacity.clamp(0.05, 1.0) * 255.0) as u8);
        let mut mesh = egui::Mesh::with_texture(tex.id());
        for (c, uv) in corners.iter().zip(uvs.iter()) {
            let w = r.px_to_world(c.0, c.1);
            mesh.vertices.push(egui::epaint::Vertex { pos: v.to_screen(rect, w[0], w[1]), uv: *uv, color: col });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
        // marked points
        let mk = |p: [f64; 2], name: &str| {
            let w = r.px_to_world(p[0], p[1]);
            let s = v.to_screen(rect, w[0], w[1]);
            painter.line_segment([s + vec2(-9.0, 0.0), s + vec2(9.0, 0.0)], Stroke::new(2.0, TEAL));
            painter.line_segment([s + vec2(0.0, -9.0), s + vec2(0.0, 9.0)], Stroke::new(2.0, TEAL));
            painter.circle_stroke(s, 5.0, Stroke::new(1.5, Color32::WHITE));
            painter.text(s + vec2(11.0, -11.0), Align2::LEFT_CENTER, name, FontId::proportional(13.0), TEAL);
        };
        if let Some(a) = r.a {
            mk(a, "A");
        }
        if let Some(b) = r.b {
            mk(b, "B");
        }
        if let (Some(a), Some(b)) = (r.a, r.b) {
            let (wa, wb) = (r.px_to_world(a[0], a[1]), r.px_to_world(b[0], b[1]));
            painter.line_segment([v.to_screen(rect, wa[0], wa[1]), v.to_screen(rect, wb[0], wb[1])], Stroke::new(1.2, TEAL));
        }
        if r.move_mode {
            let pts: Vec<egui::Pos2> = corners.iter().map(|c| { let w = r.px_to_world(c.0, c.1); v.to_screen(rect, w[0], w[1]) }).collect();
            for k in 0..4 {
                painter.line_segment([pts[k], pts[(k + 1) % 4]], Stroke::new(1.5, ACCENT));
                painter.rect_filled(Rect::from_center_size(pts[k], vec2(11.0, 11.0)), CornerRadius::same(2), ACCENT);
            }
        }
        match r.stage {
            Stage::PickA => {
                painter.text(rect.center_top() + vec2(0.0, 18.0), Align2::CENTER_CENTER, "Click point A on the image", FontId::proportional(14.0), TEAL);
            }
            Stage::PickB => {
                painter.text(rect.center_top() + vec2(0.0, 18.0), Align2::CENTER_CENTER, "Click point B on the image", FontId::proportional(14.0), TEAL);
            }
            Stage::Idle => {
                if r.move_mode {
                    painter.text(rect.center_top() + vec2(0.0, 18.0), Align2::CENTER_CENTER, "Drag the image to move it; drag a corner square to resize it", FontId::proportional(14.0), ACCENT);
                }
            }
        }
    }

    /// Handles marking points / moving the image. Returns true when the click or drag was used for that.
    pub fn ref_interact(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: Rect, v: &View2, idx: usize) -> bool {
        let Some(r) = self.refs[idx].as_mut() else {
            return false;
        };
        if r.tex.is_none() {
            return false;
        }
        if r.stage != Stage::Idle {
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let (wx, wy) = v.from_screen(rect, p);
                    let px = r.world_to_px(wx, wy);
                    match r.stage {
                        Stage::PickA => {
                            r.a = Some(px);
                            r.b = None;
                            r.stage = Stage::PickB;
                        }
                        Stage::PickB => {
                            r.b = Some(px);
                            r.stage = Stage::Idle;
                        }
                        Stage::Idle => {}
                    }
                    self.save_refs();
                }
            }
            return true;
        }
        if r.move_mode {
            let corners = [(0.0, 0.0), (r.w as f64, 0.0), (r.w as f64, r.h as f64), (0.0, r.h as f64)];
            if resp.drag_started() {
                r.drag = 1;
                if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                    for (k, c) in corners.iter().enumerate() {
                        let w = r.px_to_world(c.0, c.1);
                        if v.to_screen(rect, w[0], w[1]).distance(p) < 16.0 {
                            let o = corners[(k + 2) % 4];
                            r.drag = 2;
                            r.d_anchor_px = [o.0, o.1];
                            r.d_anchor_w = r.px_to_world(o.0, o.1);
                            r.d_corner_w = w;
                            r.d_s0 = r.s;
                            break;
                        }
                    }
                }
            }
            if resp.dragged() {
                if r.drag == 1 {
                    let d = resp.drag_delta();
                    r.cx += (d.x / v.s) as f64;
                    r.cy -= (d.y / v.s) as f64;
                } else if r.drag == 2 {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let (wx, wy) = v.from_screen(rect, p);
                        let a = r.d_anchor_w;
                        let d0 = ((r.d_corner_w[0] - a[0]).powi(2) + (r.d_corner_w[1] - a[1]).powi(2)).sqrt().max(1e-6);
                        let d1 = (((wx - a[0]) as f64).powi(2) + ((wy - a[1]) as f64).powi(2)).sqrt();
                        r.s = (r.d_s0 * d1 / d0).max(1e-4);
                        // keep the opposite corner where it was
                        let now = r.px_to_world(r.d_anchor_px[0], r.d_anchor_px[1]);
                        r.cx += a[0] - now[0];
                        r.cy += a[1] - now[1];
                    }
                }
            }
            if resp.drag_stopped() {
                r.drag = 0;
                self.save_refs();
            }
            return true;
        }
        false
    }

    /// A file chosen in the system dialog, or dropped on the window while an editor tab is showing.
    pub fn ref_window(&mut self, ctx: &egui::Context) {
        let idx = tab_index(self.dock_tab);
        let mut chosen: Option<String> = None;
        if let Some(rx) = &self.ref_rx {
            if let Ok(p) = rx.try_recv() {
                chosen = p;
                self.ref_rx = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
        if self.dock_open && self.dock_tab != DockTab::Walls && self.dock_tab != DockTab::Part {
            let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| Some(f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty())));
            if let Some(p) = dropped {
                chosen = Some(p.to_string_lossy().to_string());
            }
        }
        if let Some(p) = chosen {
            self.load_ref_file(ctx, idx, &p);
        }
    }

    /// The trace-image controls in the editor's tool row: pick an image, show/hide it, move and size it.
    pub fn trace_controls(&mut self, ui: &mut egui::Ui) {
        let idx = tab_index(self.dock_tab);
        let mut changed = false;
        let mut remove = false;
        let mut pick = false;
        match self.refs[idx].as_mut() {
            None => {
                if ui.button("+ Trace image...").on_hover_text("Put a photo or drawing behind this editor to trace over. You can also drop an image file on the window.").clicked() {
                    pick = true;
                }
            }
            Some(r) => {
                changed |= ui.checkbox(&mut r.visible, "Trace image").on_hover_text("Show or hide the image behind the editor").changed();
                if r.visible {
                    if pill_button(ui, "Move / size", r.move_mode).on_hover_text("Drag the image to move it, drag a corner square to resize it").clicked() {
                        r.move_mode = !r.move_mode;
                        r.stage = Stage::Idle;
                    }
                    ui.label(egui::RichText::new("See-through").size(12.0).color(MUTED));
                    changed |= ui.add(egui::Slider::new(&mut r.opacity, 0.1..=1.0).show_value(false)).changed();
                }
                if ui.small_button("Change...").clicked() {
                    pick = true;
                }
                if ui.small_button("Remove").clicked() {
                    remove = true;
                }
            }
        }
        if pick && self.ref_rx.is_none() {
            self.ref_rx = Some(refimg::pick_file());
        }
        if remove {
            self.refs[idx] = None;
            self.side.fitted = false;
            self.plan.fitted = false;
            self.elev.fitted = false;
            changed = true;
        }
        if !self.ref_msg.is_empty() && self.refs[idx].is_none() {
            ui.label(egui::RichText::new(self.ref_msg.clone()).size(12.0).color(DANGER));
        }
        if changed {
            self.save_refs();
        }
    }
}

#[allow(dead_code)]
fn unused(_: CornerRadius) {}
