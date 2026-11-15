// Move / rotate handles for a selected 3D part, and its right-click menu.
use crate::app::*;
use crate::cuts::{Cut, CutOp};
use crate::theme::*;
use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Shape, Stroke};

const AXC: [Color32; 3] = [Color32::from_rgb(235, 90, 90), Color32::from_rgb(100, 205, 110), Color32::from_rgb(100, 150, 245)];
const AXN: [&str; 3] = ["X", "Y", "Z"];
const UNIT: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

pub struct Geom {
    pub c: Pos2,
    pub len: f64,
    /// Screen end of each axis arrow (move mode).
    pub tip: [Option<Pos2>; 3],
    /// Screen polyline of each ring (rotate mode).
    pub ring: [Vec<Pos2>; 3],
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.x * ab.x + ab.y * ab.y;
    let t = if l2 < 1e-6 { 0.0 } else { (((p - a).x * ab.x + (p - a).y * ab.y) / l2).clamp(0.0, 1.0) };
    (p - (a + ab * t)).length()
}

impl App {
    fn sel3d(&self) -> Option<&Cut> {
        if self.focus != Focus::Cut {
            return None;
        }
        let id = self.sel_cut?;
        self.st.proj.cuts.iter().find(|c| c.id == id && c.visible && c.is3d() && !c.locked)
    }

    fn pt(&self, rect: Rect, p: [f64; 3]) -> Option<Pos2> {
        self.cam.project([p[0] as f32, p[1] as f32, p[2] as f32], rect.width(), rect.height()).map(|s| pos2(rect.left() + s[0], rect.top() + s[1]))
    }

    pub fn gizmo_geom(&self, rect: Rect) -> Option<Geom> {
        let c = self.sel3d()?;
        let o = [c.x, c.y, c.z];
        let e = self.cam.eye();
        let dist = ((e[0] as f64 - o[0]).powi(2) + (e[1] as f64 - o[1]).powi(2) + (e[2] as f64 - o[2]).powi(2)).sqrt();
        let len = (dist * 0.13).max(25.0);
        let cs = self.pt(rect, o)?;
        let mut tip = [None, None, None];
        let mut ring: [Vec<Pos2>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for a in 0..3 {
            let ax = UNIT[a];
            tip[a] = self.pt(rect, [o[0] + ax[0] * len, o[1] + ax[1] * len, o[2] + ax[2] * len]);
            let (u, v) = (UNIT[(a + 1) % 3], UNIT[(a + 2) % 3]);
            for i in 0..=64 {
                let t = i as f64 / 64.0 * std::f64::consts::TAU;
                let r = len * 0.85;
                let p = [o[0] + (u[0] * t.cos() + v[0] * t.sin()) * r, o[1] + (u[1] * t.cos() + v[1] * t.sin()) * r, o[2] + (u[2] * t.cos() + v[2] * t.sin()) * r];
                if let Some(s) = self.pt(rect, p) {
                    ring[a].push(s);
                }
            }
        }
        Some(Geom { c: cs, len, tip, ring })
    }

    pub fn draw_gizmo(&self, painter: &egui::Painter, rect: Rect) {
        let Some(g) = self.gizmo_geom(rect) else { return };
        painter.circle_filled(g.c, 4.0, Color32::WHITE);
        for a in 0..3 {
            if self.gizmo_rot {
                if g.ring[a].len() > 2 {
                    painter.add(Shape::line(g.ring[a].clone(), Stroke::new(2.6, AXC[a])));
                }
            } else if let Some(t) = g.tip[a] {
                painter.line_segment([g.c, t], Stroke::new(3.0, AXC[a]));
                painter.circle_filled(t, 7.0, AXC[a]);
                painter.text(t + vec2(10.0, -10.0), egui::Align2::LEFT_CENTER, AXN[a], egui::FontId::proportional(12.0), AXC[a]);
            }
        }
    }

    /// Which handle is under the pointer (axis 0..2), if any.
    pub fn gizmo_hit(&self, p: Pos2, rect: Rect) -> Option<usize> {
        let g = self.gizmo_geom(rect)?;
        let mut best: Option<(usize, f32)> = None;
        for a in 0..3 {
            let d = if self.gizmo_rot {
                g.ring[a].windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f32::MAX, f32::min)
            } else if let Some(t) = g.tip[a] {
                if (t - g.c).length() < 14.0 {
                    f32::MAX
                } else {
                    seg_dist(p, g.c, t).min((p - t).length() - 3.0)
                }
            } else {
                f32::MAX
            };
            if d < 9.0 && best.map_or(true, |b| d < b.1) {
                best = Some((a, d));
            }
        }
        best.map(|b| b.0)
    }

    pub fn gizmo_begin(&mut self, axis: usize, p: Pos2, rect: Rect) {
        let Some(g) = self.gizmo_geom(rect) else { return };
        let Some(c) = self.sel3d().cloned() else { return };
        self.op_s(r#"{"op":"checkpoint"}"#);
        let o = [c.x, c.y, c.z];
        let a0 = (p.y - g.c.y).atan2(p.x - g.c.x);
        let e = self.cam.eye();
        let toward = (e[0] as f64 - o[0]) * UNIT[axis][0] + (e[1] as f64 - o[1]) * UNIT[axis][1] + (e[2] as f64 - o[2]) * UNIT[axis][2] > 0.0;
        let (u, k) = match g.tip[axis] {
            Some(t) if (t - g.c).length() > 1.0 => {
                let d = t - g.c;
                let l = d.length();
                ([d.x / l, d.y / l], (g.len / l as f64) as f32)
            }
            _ => ([1.0, 0.0], 1.0),
        };
        let v0 = [c.x, c.y, c.z][axis];
        self.vp_drag = VpDrag::Gizmo { id: c.id, axis, rotate: self.gizmo_rot, p0: [p.x, p.y], u, k, v0, cs: [g.c.x, g.c.y], a0, toward, e0: [c.rx, c.ry, c.rot] };
    }

    pub fn gizmo_drag(&mut self, p: Pos2) {
        let VpDrag::Gizmo { id, axis, rotate, p0, u, k, v0, cs, a0, toward, e0 } = self.vp_drag else { return };
        let Some(mut c) = self.st.proj.cuts.iter().find(|c| c.id == id).cloned() else { return };
        if rotate {
            let a = (p.y - cs[1]).atan2(p.x - cs[0]);
            let mut da = (a - a0) as f64;
            while da > std::f64::consts::PI {
                da -= std::f64::consts::TAU;
            }
            while da < -std::f64::consts::PI {
                da += std::f64::consts::TAU;
            }
            let delta = if toward { -da } else { da };
            let (rx, ry, rz) = crate::cuts::rotate_about_world(e0, axis, delta);
            c.rx = rx;
            c.ry = ry;
            c.rot = rz;
        } else {
            let t = ((p.x - p0[0]) * u[0] + (p.y - p0[1]) * u[1]) * k;
            let v = v0 + t as f64;
            match axis {
                0 => c.x = self.snap_x(v),
                1 => c.y = v,
                _ => c.z = v,
            }
        }
        self.cut_put(&c, true);
    }

    /// Put the part so its underside touches the dash (or its top is flush with it).
    pub fn cut_to_surface(&self, c: &mut Cut, flush: bool) {
        let surf = self.mesh.as_ref().and_then(|m| m.surface_at(c.x as f32, c.y as f32)).map(|h| h.p[2] as f64);
        let ext = c.world_obj(true).map(|m| {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for p in &m.ps {
                lo = lo.min(p[2] - c.z);
                hi = hi.max(p[2] - c.z);
            }
            (lo, hi)
        });
        if let (Some(s), Some((lo, hi))) = (surf, ext) {
            c.z = if flush { s - hi } else { s - lo };
        }
    }

    /// The right-click menu on a placed shape or part.
    pub fn cut_menu_ui(&mut self, ctx: &egui::Context) {
        let Some((id, pos)) = self.cut_menu else { return };
        let Some(c0) = self.st.proj.cuts.iter().find(|c| c.id == id).cloned() else {
            self.cut_menu = None;
            return;
        };
        let mut c = c0.clone();
        let mut hard = false;
        let mut delete = false;
        let is3d = c.is3d();
        let area = egui::Area::new(egui::Id::new("cut_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(210.0);
                ui.label(egui::RichText::new(&c.name).strong());
                if ui.button(if c.locked { "Unlock" } else { "Lock in place" }).clicked() {
                    c.locked = !c.locked;
                    hard = true;
                }
                if c.locked {
                    ui.label(egui::RichText::new("Locked: unlock it to move, turn or delete it.").size(11.5).color(MUTED));
                    return;
                }
                ui.separator();
                if is3d {
                    ui.label(egui::RichText::new("Handles").size(11.5).color(MUTED));
                    ui.horizontal(|ui| {
                        if pill_button(ui, "Move", !self.gizmo_rot).clicked() {
                            self.gizmo_rot = false;
                        }
                        if pill_button(ui, "Rotate", self.gizmo_rot).clicked() {
                            self.gizmo_rot = true;
                        }
                    });
                    ui.label(egui::RichText::new("Turn 90 degrees").size(11.5).color(MUTED));
                    for (a, lab) in [(0, "X"), (1, "Y"), (2, "Z")] {
                        ui.horizontal(|ui| {
                            ui.label(lab);
                            let v = match a {
                                0 => &mut c.rx,
                                1 => &mut c.ry,
                                _ => &mut c.rot,
                            };
                            if ui.small_button("-90").clicked() {
                                *v = (*v - 90.0).rem_euclid(360.0);
                                hard = true;
                            }
                            if ui.small_button("+90").clicked() {
                                *v = (*v + 90.0).rem_euclid(360.0);
                                hard = true;
                            }
                            ui.label(egui::RichText::new(format!("{:.0} deg", *v)).size(11.5).color(MUTED));
                        });
                    }
                    if ui.button("Reset rotation").clicked() {
                        c.rx = 0.0;
                        c.ry = 0.0;
                        c.rot = 0.0;
                        hard = true;
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if pill_button(ui, "Add", c.op != CutOp::Cut).clicked() {
                            c.op = CutOp::Raise;
                            hard = true;
                        }
                        if pill_button(ui, "Cut", c.op == CutOp::Cut).clicked() {
                            c.op = CutOp::Cut;
                            hard = true;
                        }
                    });
                    if ui.button("Sit on surface").clicked() {
                        self.cut_to_surface(&mut c, false);
                        hard = true;
                    }
                    if ui.button("Sink flush with surface").clicked() {
                        self.cut_to_surface(&mut c, true);
                        hard = true;
                    }
                    if ui.button("Center on dash (x = 0)").clicked() {
                        c.x = 0.0;
                        hard = true;
                    }
                }
                if ui.checkbox(&mut c.mirror, "Symmetrical: also other side").changed() {
                    hard = true;
                }
                ui.separator();
                if ui.button("Delete").clicked() {
                    delete = true;
                }
            });
        });
        if hard {
            self.cut_put(&c, false);
        }
        if delete {
            self.op(format!(r#"{{"op":"cut_remove","id":{}}}"#, id));
            self.sel_cut = None;
            self.focus = Focus::Dash;
            self.cut_menu = None;
            return;
        }
        let r = area.response.rect;
        let (pressed, ppos, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
        if esc || (pressed && ppos.map_or(false, |p| !r.contains(p)) && ctx.input(|i| i.pointer.hover_pos()).map_or(true, |p| !r.contains(p))) {
            self.cut_menu = None;
        }
    }
}

// ---- Blender-style view gizmo: always on in the main view ----
const NAV_R: f32 = 60.0;

impl App {
    fn nav_center(&self, rect: Rect) -> Pos2 {
        rect.right_top() + vec2(-86.0, 112.0)
    }

    /// True when the pointer is over the gizmo's disc (drag there turns the view).
    pub fn nav_area(&self, p: Pos2, rect: Rect) -> bool {
        self.nav_center(rect).distance(p) <= NAV_R
    }

    /// The six ball ends: (axis, side, screen position, depth toward the viewer).
    fn nav_balls(&self, rect: Rect) -> Vec<(usize, f64, Pos2, f32)> {
        let c = self.nav_center(rect);
        let t = self.cam.target;
        let (w, h) = (rect.width(), rect.height());
        let e = self.cam.eye();
        let to_eye = [e[0] - t[0], e[1] - t[1], e[2] - t[2]];
        let n = (to_eye[0] * to_eye[0] + to_eye[1] * to_eye[1] + to_eye[2] * to_eye[2]).sqrt().max(1e-3);
        let Some(s0) = self.cam.project(t, w, h) else { return Vec::new() };
        let mut v = Vec::new();
        for ax in 0..3 {
            let mut q = t;
            q[ax] += 100.0;
            let Some(s1) = self.cam.project(q, w, h) else { continue };
            let d = vec2(s1[0] - s0[0], s1[1] - s0[1]);
            let l = d.length().max(1e-3);
            // the picture is flat, so scale every axis by the same factor: a ball facing the viewer sits in the middle
            let len = d / l * 38.0 * (l / (l.max(1.0))).min(1.0);
            let depth = to_eye[ax] / n;
            let flat = (1.0 - depth * depth).max(0.0).sqrt();
            let d2 = if l > 1e-2 { d / l * 38.0 * flat } else { vec2(0.0, 0.0) };
            let _ = len;
            v.push((ax, 1.0, c + d2, depth));
            v.push((ax, -1.0, c - d2, -depth));
        }
        v
    }

    pub fn nav_ball_at(&self, p: Pos2, rect: Rect) -> Option<(usize, f64)> {
        let mut best: Option<(f32, usize, f64)> = None;
        for (ax, side, q, depth) in self.nav_balls(rect) {
            let d = q.distance(p);
            // nearer balls win when two overlap
            if d < 12.0 && best.map_or(true, |b| depth > b.0) {
                best = Some((depth, ax, side));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Turn the camera to look along an axis (click a ball): the ball's side is where you look from.
    pub fn nav_snap(&mut self, axis: usize, side: f64) {
        use std::f32::consts::FRAC_PI_2;
        let (yaw, pitch) = match (axis, side > 0.0) {
            (0, true) => (FRAC_PI_2, 0.0),
            (0, false) => (-FRAC_PI_2, 0.0),
            (1, true) => (std::f32::consts::PI, 0.0),
            (1, false) => (0.0, 0.0),
            (_, true) => (self.cam.yaw, 1.5),
            (_, false) => (self.cam.yaw, -1.5),
        };
        self.cam.yaw = yaw;
        self.cam.pitch = pitch;
    }

    pub fn draw_nav(&self, painter: &egui::Painter, rect: Rect, hover: Option<Pos2>) {
        let c = self.nav_center(rect);
        let over = hover.map_or(false, |h| self.nav_area(h, rect));
        painter.circle_filled(c, NAV_R, Color32::from_white_alpha(if over { 22 } else { 8 }));
        let hov = hover.and_then(|h| self.nav_ball_at(h, rect));
        let mut bs = self.nav_balls(rect);
        // far balls first so near ones sit on top
        bs.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));
        for (ax, side, p, _) in &bs {
            if *side > 0.0 {
                painter.line_segment([c, *p], Stroke::new(3.0, AXC[*ax]));
            }
        }
        for (ax, side, p, _) in &bs {
            let on = hov == Some((*ax, *side));
            let r = if on { 12.5 } else { 10.5 };
            if *side > 0.0 {
                painter.circle_filled(*p, r, AXC[*ax]);
                painter.text(*p, egui::Align2::CENTER_CENTER, AXN[*ax], egui::FontId::proportional(12.0), Color32::from_rgb(20, 22, 26));
            } else {
                painter.circle_filled(*p, r, Color32::from_rgba_unmultiplied(AXC[*ax].r(), AXC[*ax].g(), AXC[*ax].b(), 70));
                painter.circle_stroke(*p, r, Stroke::new(1.5, Color32::from_rgba_unmultiplied(AXC[*ax].r(), AXC[*ax].g(), AXC[*ax].b(), 200)));
            }
            if on {
                painter.circle_stroke(*p, r + 2.5, Stroke::new(2.0, Color32::WHITE));
            }
        }
        if over {
            painter.ctx().set_cursor_icon(if hov.is_some() { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Grab });
        }
    }
}
