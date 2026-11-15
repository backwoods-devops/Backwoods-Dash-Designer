// Sizing the whole dash (or one piece): size read-out in the corner, drag bars around a box, right-click menu with sizes and a scale.
use crate::app::*;
use crate::engine::Snap;
use crate::model::BezPt;
use crate::theme::*;
use crate::units::MmExt;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Stroke};

fn fade(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub type Box3 = [[f64; 3]; 2];

/// What the model looked like before a resize started; every update is applied to this, so nothing drifts.
#[derive(Clone)]
pub struct SizeBase {
    pub snap: Snap,
    pub back: Vec<BezPt>,
}

pub struct SizeDrag {
    pub axis: usize,
    pub side: f64,
    pub base: SizeBase,
    pub b0: Box3,
    pub p0: Pos2,
    /// screen direction of +axis and screen pixels per mm along it
    pub u: [f32; 2],
    pub pxmm: f32,
    pub sym: bool,
    pub now: f64,
}

const AXN: [&str; 3] = ["X", "Y", "Z"];
const AXD: [&str; 3] = ["Width", "Depth", "Height"];

fn union(a: Option<Box3>, b: Option<Box3>) -> Option<Box3> {
    match (a, b) {
        (Some(a), Some(b)) => Some([[a[0][0].min(b[0][0]), a[0][1].min(b[0][1]), a[0][2].min(b[0][2])], [a[1][0].max(b[1][0]), a[1][1].max(b[1][1]), a[1][2].max(b[1][2])]]),
        (a, None) => a,
        (None, b) => b,
    }
}

impl App {
    /// The box that is being resized: the whole dash or just the active piece.
    pub fn size_box(&self) -> Option<Box3> {
        if self.size_all {
            (0..3).filter(|&i| i < 2 || self.st.trim_on).fold(None, |acc, i| union(acc, self.st.piece(i).bounds()))
        } else {
            self.st.proj.bounds()
        }
    }

    /// The original size of every piece: read from size_orig.txt, or taken now (before the first resize) and saved.
    fn ensure_orig(&mut self) {
        if self.size_orig.is_some() {
            return;
        }
        let path = self.st.dir.join("size_orig.txt");
        if let Ok(t) = std::fs::read_to_string(&path) {
            let n: Vec<f64> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if n.len() == 18 {
                let mut o = [[[0.0; 3]; 2]; 3];
                for i in 0..3 {
                    for h in 0..2 {
                        for k in 0..3 {
                            o[i][h][k] = n[i * 6 + h * 3 + k];
                        }
                    }
                }
                self.size_orig = Some(o);
                return;
            }
        }
        let mut o = [[[0.0; 3]; 2]; 3];
        for i in 0..3 {
            if let Some(b) = self.st.piece(i).bounds() {
                o[i] = b;
            }
        }
        let txt: Vec<String> = o.iter().flat_map(|b| b.iter().flat_map(|h| h.iter().map(|v| format!("{}", v)))).collect();
        let _ = std::fs::write(&path, txt.join(" "));
        self.size_orig = Some(o);
    }

    /// Put the size (and place) back to what it was before any resize.
    fn reset_size(&mut self) {
        self.ensure_orig();
        let (Some(o), Some(cur)) = (self.size_orig, self.size_box()) else { return };
        let ob = if self.size_all { (0..3).fold(None, |acc, i| union(acc, Some(o[i]))) } else { Some(o[self.st.active as usize]) };
        let Some(ob) = ob else { return };
        let mut a = [0.0; 3];
        let mut s = [1.0; 3];
        for k in 0..3 {
            let (cd, od) = (cur[1][k] - cur[0][k], ob[1][k] - ob[0][k]);
            if cd > 1e-6 && od > 1e-6 {
                s[k] = od / cd;
                if (1.0 - s[k]).abs() > 1e-9 {
                    a[k] = (ob[0][k] - s[k] * cur[0][k]) / (1.0 - s[k]);
                }
            }
        }
        self.op_s(r#"{"op":"checkpoint"}"#);
        let base = self.size_base();
        self.apply_scale(&base, a, s);
        self.toast("Back to the original size.", false);
    }

    fn size_base(&self) -> SizeBase {
        SizeBase { snap: self.st.snap(), back: self.sw.back.clone() }
    }

    fn size_sym(&self) -> bool {
        self.size_all && self.st.proj.rail_sym
    }

    fn centre_anchor(&self, b: &Box3) -> [f64; 3] {
        let mut a = [(b[0][0] + b[1][0]) / 2.0, (b[0][1] + b[1][1]) / 2.0, (b[0][2] + b[1][2]) / 2.0];
        if self.size_sym() {
            a[0] = 0.0;
        }
        a
    }

    /// Put the model back to `base`, then stretch it by `s` about `a`.
    pub fn apply_scale(&mut self, base: &SizeBase, a: [f64; 3], s: [f64; 3]) {
        self.st.proj = base.snap.proj.clone();
        self.st.shelf = base.snap.shelf.clone();
        self.st.active = base.snap.active;
        let n = if self.size_all { 0..3 } else { self.st.active as usize..self.st.active as usize + 1 };
        let parts = self.size_parts;
        for i in n {
            if let Some(p) = self.st.piece_mut(i) {
                p.scale_about(a, s, parts);
            }
        }
        if self.size_all {
            self.sw.back = base.back.iter().map(|q| BezPt { co: [q.co[0] * s[1], q.co[1] * s[2]], hl: [q.hl[0] * s[1], q.hl[1] * s[2]], hr: [q.hr[0] * s[1], q.hr[1] * s[2]] }).collect();
        }
        self.st.sync_lock();
        self.st.dirty = true;
        self.rev += 1;
    }

    /// The corner brackets: for every edge of the box, a stub at each end (a quarter of the edge long).
    /// Each stub stretches the axis it lies along, on the side of the end it sits at.
    fn stubs(&self, rect: Rect, b: &Box3) -> Vec<(usize, [f64; 3], Pos2, Pos2)> {
        let (w, h) = (rect.width(), rect.height());
        let pr = |q: [f64; 3]| self.cam.project([q[0] as f32, q[1] as f32, q[2] as f32], w, h).map(|s| pos2(rect.left() + s[0], rect.top() + s[1]));
        let mut v = Vec::new();
        for k in 0..3 {
            let (i, j) = ((k + 1) % 3, (k + 2) % 3);
            for ci in 0..2 {
                for cj in 0..2 {
                    let mut lo = [0.0; 3];
                    lo[i] = b[ci][i];
                    lo[j] = b[cj][j];
                    let len = (b[1][k] - b[0][k]) / 4.0;
                    for e in 0..2 {
                        let mut corner = [0.0; 3];
                        corner[i] = if ci == 0 { -1.0 } else { 1.0 };
                        corner[j] = if cj == 0 { -1.0 } else { 1.0 };
                        corner[k] = if e == 0 { -1.0 } else { 1.0 };
                        let (mut p0, mut p1) = (lo, lo);
                        if e == 0 {
                            p0[k] = b[0][k];
                            p1[k] = b[0][k] + len;
                        } else {
                            p0[k] = b[1][k];
                            p1[k] = b[1][k] - len;
                        }
                        if let (Some(a), Some(c)) = (pr(p0), pr(p1)) {
                            v.push((k, corner, a, c));
                        }
                    }
                }
            }
        }
        v
    }

    /// The corner of the box whose bracket is under the pointer, as -1 / +1 on each axis.
    pub fn size_handle_at(&self, p: Pos2, rect: Rect) -> Option<[f64; 3]> {
        if !self.size_mode {
            return None;
        }
        let b = self.size_box()?;
        let dist = |a: Pos2, c: Pos2| {
            let (ab, ap) = (c - a, p - a);
            let l2 = ab.length_sq().max(1e-6);
            let t = (ap.dot(ab) / l2).clamp(0.0, 1.0);
            (a + ab * t).distance(p)
        };
        self.stubs(rect, &b).into_iter().map(|(_, c, a, e)| (c, dist(a, e))).filter(|h| h.1 < 14.0).min_by(|a, c| a.1.partial_cmp(&c.1).unwrap()).map(|h| h.0)
    }

    /// Blender-style axis ball tool in the top right corner: (axis, side, screen position) for the six ends.
    fn balls(&self, rect: Rect) -> Vec<(usize, f64, Pos2)> {
        let c = rect.right_top() + vec2(-86.0, 112.0);
        let t = self.cam.target;
        let (w, h) = (rect.width(), rect.height());
        let Some(s0) = self.cam.project(t, w, h) else { return Vec::new() };
        let mut ds = Vec::new();
        for ax in 0..3 {
            let mut q = t;
            q[ax] += 100.0;
            if let Some(s1) = self.cam.project(q, w, h) {
                ds.push((ax, vec2(s1[0] - s0[0], s1[1] - s0[1])));
            }
        }
        let refl = ds.iter().map(|d| d.1.length()).fold(1e-3, f32::max);
        let mut v = Vec::new();
        for (ax, d) in ds {
            let l = d.length().max(1e-3);
            let d = d / l * 40.0 * (l / refl).max(0.3);
            v.push((ax, 1.0, c + d));
            v.push((ax, -1.0, c - d));
        }
        v
    }

    pub fn size_ball_at(&self, _p: Pos2, _rect: Rect) -> Option<(usize, f64)> {
        // the X / Y / Z balls are now the always-on view gizmo (ui_gizmo.rs); resizing uses the corner brackets
        return None;
        #[allow(unreachable_code)]
        {
        let (p, rect) = (_p, _rect);
        self.balls(rect).into_iter().filter(|b| b.2.distance(p) < 13.0).min_by(|a, c| a.2.distance(p).partial_cmp(&c.2.distance(p)).unwrap()).map(|b| (b.0, b.1))
        }
    }

    fn draw_balls(&self, painter: &egui::Painter, rect: Rect, hover: Option<Pos2>) {
        let axc = [Color32::from_rgb(214, 80, 90), Color32::from_rgb(130, 190, 60), Color32::from_rgb(80, 140, 235)];
        let c = rect.right_top() + vec2(-86.0, 112.0);
        let sel = self.size_drag.as_ref().map(|d| d.axis).or(self.size_axis);
        let hov = hover.and_then(|h| self.size_ball_at(h, rect));
        let mut bs = self.balls(rect);
        bs.sort_by(|a, b| (a.1 > 0.0).cmp(&(b.1 > 0.0)));
        for (ax, side, p) in bs {
            if side > 0.0 {
                painter.line_segment([c, p], Stroke::new(3.0, axc[ax]));
            }
        }
        for (ax, side, p) in self.balls(rect).into_iter().filter(|b| b.1 < 0.0).chain(self.balls(rect).into_iter().filter(|b| b.1 > 0.0)) {
            let on = Some((ax, side)) == hov || self.size_drag.as_ref().map_or(false, |d| d.axis == ax && d.side == side);
            let r = if on { 13.0 } else { 11.0 };
            if side > 0.0 {
                painter.circle_filled(p, r, axc[ax]);
                painter.text(p, Align2::CENTER_CENTER, AXN[ax], FontId::proportional(12.0), Color32::from_rgb(20, 22, 26));
            } else {
                painter.circle_filled(p, r, fade(axc[ax], 70));
                painter.circle_stroke(p, r, Stroke::new(1.5, fade(axc[ax], 200)));
            }
            if on || (side > 0.0 && sel == Some(ax)) {
                painter.circle_stroke(p, r + 2.5, Stroke::new(2.0, Color32::WHITE));
            }
        }
        painter.text(c + vec2(0.0, 62.0), Align2::CENTER_CENTER, "drag a ball up / down", FontId::proportional(10.5), fade(MUTED, 190));
    }

    /// Start dragging a bar. Returns false when the pointer is not on one.
    pub fn size_drag_begin(&mut self, p: Pos2, cur: Pos2, rect: Rect) -> bool {
        if let Some((axis, side)) = self.size_ball_at(p, rect) {
            let Some(b0) = self.size_box() else { return false };
            let size0 = (b0[1][axis] - b0[0][axis]).max(1e-6);
            self.op_s(r#"{"op":"checkpoint"}"#);
            self.size_axis = Some(axis);
            let sym = axis == 0 && self.size_sym();
            // dragging up grows the size: a full 400 px of drag is the whole size
            self.size_drag = Some(SizeDrag { axis, side, base: self.size_base(), b0, p0: p, u: [0.0, -side as f32], pxmm: (400.0 / size0) as f32, sym, now: size0 });
            return true;
        }
        let Some(corner) = self.size_handle_at(p, rect) else { return false };
        let Some(b0) = self.size_box() else { return false };
        let pt: [f64; 3] = [0, 1, 2].map(|k| if corner[k] > 0.0 { b0[1][k] } else { b0[0][k] });
        let (w, h) = (rect.width(), rect.height());
        let f = |q: [f64; 3]| self.cam.project([q[0] as f32, q[1] as f32, q[2] as f32], w, h);
        let Some(s0) = f(pt) else { return false };
        // which way the pointer is pulling decides the axis: the edge at this corner that points most the same way on screen
        let v = vec2(cur.x - p.x, cur.y - p.y);
        let mut best: Option<(usize, f32, [f32; 2], f32)> = None;
        for k in 0..3 {
            let mut q = pt;
            q[k] += 100.0;
            let Some(s1) = f(q) else { continue };
            let d = vec2(s1[0] - s0[0], s1[1] - s0[1]);
            let l = d.length();
            if l < 3.0 {
                continue;
            }
            let cos = if v.length() > 2.0 { (v.dot(d) / (v.length() * l)).abs() } else { l / 100.0 };
            if best.map_or(true, |b| cos > b.1) {
                best = Some((k, cos, [d.x / l, d.y / l], l));
            }
        }
        let Some((axis, _, u, l)) = best else { return false };
        let side = corner[axis];
        self.op_s(r#"{"op":"checkpoint"}"#);
        self.size_axis = Some(axis);
        let sym = axis == 0 && self.size_sym();
        self.size_drag = Some(SizeDrag { axis, side, base: self.size_base(), b0, p0: p, u, pxmm: l / 100.0, sym, now: b0[1][axis] - b0[0][axis] });
        true
    }

    pub fn size_drag_update(&mut self, p: Pos2, uniform: bool) {
        let Some(d) = self.size_drag.as_ref() else { return };
        let (axis, side, b0, sym) = (d.axis, d.side, d.b0, d.sym);
        let delta = (((p.x - d.p0.x) * d.u[0] + (p.y - d.p0.y) * d.u[1]) / d.pxmm) as f64;
        let size0 = (b0[1][axis] - b0[0][axis]).max(1e-6);
        let face0 = if side > 0.0 { b0[1][axis] } else { b0[0][axis] };
        let mut a = [0.0; 3];
        let f = if sym && face0.abs() > 1e-6 {
            a[0] = 0.0;
            (face0 + delta) / face0
        } else {
            a[axis] = if side > 0.0 { b0[0][axis] } else { b0[1][axis] };
            (size0 + side * delta) / size0
        };
        let f = f.clamp(0.05, 20.0);
        let mut s = [1.0; 3];
        if uniform {
            s = [f; 3];
            let c = self.centre_anchor(&b0);
            for k in 0..3 {
                if k != axis {
                    a[k] = c[k];
                }
            }
        } else {
            s[axis] = f;
        }
        let base = d.base.clone();
        self.apply_scale(&base, a, s);
        if let Some(d) = self.size_drag.as_mut() {
            d.now = size0 * f;
        }
    }

    /// Small size read-out in the corner, plus the box and bars while resizing.
    pub fn size_overlay(&self, painter: &egui::Painter, rect: Rect, resp: &egui::Response) {
        let Some(b) = self.size_box() else { return };
        let dim = |k: usize| b[1][k] - b[0][k];
        let light = Color32::from_rgb(170, 178, 190);
        let txt = format!("W {}   D {}   H {}", crate::units::num(dim(0), 1), crate::units::num(dim(1), 1), crate::units::len(dim(2), 1));
        let scope = if self.size_all { "whole dash" } else { crate::engine::PIECE_NAMES[self.st.active as usize] };
        painter.text(rect.center_top() + vec2(0.0, 13.0), Align2::CENTER_CENTER, format!("{}    ·    {}", txt, scope), FontId::proportional(12.5), fade(light, 200));
        if !self.size_mode {
            return;
        }
        let hover = resp.hover_pos();
        if self.size_drag.is_none() && hover.map_or(false, |h| self.size_ball_at(h, rect).is_some()) {
            painter.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        }
        let ball = hover.map_or(false, |h| self.size_ball_at(h, rect).is_some());
        let drag_corner = self.size_drag.as_ref().map(|d| d.side);
        let _ = drag_corner;
        let corner = if self.size_drag.is_some() {
            self.size_drag.as_ref().map(|d| {
                // the corner being pulled: the dragged side on its axis; others are whichever corner stubs share the dragged stub's end
                let mut c = [0.0; 3];
                c[d.axis] = d.side;
                c
            })
        } else if ball {
            None
        } else {
            hover.and_then(|h| self.size_handle_at(h, rect))
        };
        for (k, c, a, e) in self.stubs(rect, &b) {
            let on = match (&self.size_drag, corner) {
                (Some(d), _) => k == d.axis && c[k] == d.side,
                (None, Some(cc)) => c == cc,
                _ => false,
            };
            let (col, wd) = if on { (ACCENT, 4.5) } else { (fade(ACCENT, 170), 1.6) };
            painter.line_segment([a, e], Stroke::new(wd, col));
        }
        if self.size_drag.is_none() {
            if let (Some(_), Some(h)) = (corner, hover) {
                painter.text(h + vec2(16.0, 16.0), Align2::LEFT_CENTER, "Pull along an edge: width, depth or height", FontId::proportional(12.0), ACCENT);
                painter.ctx().set_cursor_icon(CursorIcon::Grab);
            }
        }
        // the number that is changing, by the pointer
        if let (Some(d), Some(h)) = (self.size_drag.as_ref(), hover) {
            let orig = d.b0[1][d.axis] - d.b0[0][d.axis];
            let pct = d.now / orig.max(1e-9) * 100.0;
            let t = format!("{} {}   (was {}, {:.0}%)", AXD[d.axis], crate::units::len(d.now, 1), crate::units::num(orig, 1), pct);
            let g = painter.layout_no_wrap(t.clone(), FontId::proportional(13.0), Color32::WHITE);
            let r = Rect::from_min_size(h + vec2(18.0, 14.0), g.size() + vec2(14.0, 8.0));
            painter.rect_filled(r, CornerRadius::same(6), fade(Color32::from_rgb(20, 22, 26), 235));
            painter.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, ACCENT), egui::StrokeKind::Inside);
            painter.text(r.center(), Align2::CENTER_CENTER, t, FontId::proportional(13.0), Color32::WHITE);
            if d.base.snap.proj.rail.len() > 0 {
                painter.text(r.left_bottom() + vec2(0.0, 12.0), Align2::LEFT_CENTER, "Hold Shift to scale all three together", FontId::proportional(10.5), fade(MUTED, 200));
            }
        }
    }

    /// Right-click menu on a piece of the dash.
    pub fn size_menu_ui(&mut self, ctx: &egui::Context) {
        let Some(pos) = self.size_menu else { return };
        let Some(b) = self.size_box() else {
            self.size_menu = None;
            return;
        };
        self.ensure_orig();
        let mut close = false;
        let mut reset = false;
        let mut edit: Option<(usize, f64)> = None;
        let mut apply_pct = false;
        let mut edit_start = false;
        let mut edit_end = false;
        let area = egui::Area::new(egui::Id::new("size_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(240.0);
                ui.label(egui::RichText::new(format!("Size: {}", if self.size_all { "whole dash".to_string() } else { crate::engine::PIECE_NAMES[self.st.active as usize].to_string() })).strong());
                ui.horizontal(|ui| {
                    if pill_button(ui, "Whole dash", self.size_all).on_hover_text("All three pieces, side walls and parts change together so everything still fits").clicked() {
                        self.size_all = true;
                    }
                    if pill_button(ui, "This piece only", !self.size_all).clicked() {
                        self.size_all = false;
                    }
                });
                if pill_button(ui, if self.size_mode { "Corner handles: on" } else { "Corner handles: off" }, self.size_mode).on_hover_text("Show corner brackets around the dash. Grab a bracket and pull along it to stretch that way; hold Shift to scale all three together.").clicked() {
                    self.size_mode = !self.size_mode;
                }
                ui.separator();
                ui.label(egui::RichText::new(format!("Type a size ({})", crate::units::label())).size(11.5).color(MUTED));
                for k in 0..3 {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} ({})", AXD[k], AXN[k]));
                        let mut v = b[1][k] - b[0][k];
                        let r = ui.add(egui::DragValue::new(&mut v).range(5.0..=5000.0).speed(1.0).mm());
                        if r.drag_started() || r.gained_focus() {
                            edit_start = true;
                        }
                        if r.changed() {
                            edit = Some((k, v));
                        }
                        if r.drag_stopped() || r.lost_focus() {
                            edit_end = true;
                        }
                    });
                }
                ui.separator();
                ui.label(egui::RichText::new("Scale everything evenly").size(11.5).color(MUTED));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.scale_pct).range(5.0..=500.0).speed(0.5).suffix(" %"));
                    if ui.button("Scale").clicked() {
                        apply_pct = true;
                    }
                    if ui.button("Reset %").clicked() {
                        self.scale_pct = 100.0;
                    }
                });
                ui.checkbox(&mut self.size_parts, "Resize parts and cut-outs too").on_hover_text("On an even scale the gauges, vents and shapes grow or shrink with the dash. When one side is stretched they only move.");
                ui.separator();
                if ui.button("Reset to original size").on_hover_text("Put the size back to what it was before any resizing (whole dash or this piece, as chosen above)").clicked() {
                    reset = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
        if edit_start && self.size_edit.is_none() {
            self.op_s(r#"{"op":"checkpoint"}"#);
            self.size_edit = Some((self.size_base(), b));
        }
        if let (Some((k, v)), Some((base, b0))) = (edit, self.size_edit.clone()) {
            let size0 = (b0[1][k] - b0[0][k]).max(1e-6);
            let f = (v / size0).clamp(0.05, 20.0);
            let a = self.centre_anchor(&b0);
            let mut s = [1.0; 3];
            s[k] = f;
            self.apply_scale(&base, a, s);
        }
        if edit_end {
            self.size_edit = None;
        }
        if reset {
            self.reset_size();
        }
        if apply_pct {
            self.op_s(r#"{"op":"checkpoint"}"#);
            let base = self.size_base();
            let f = (self.scale_pct / 100.0).clamp(0.05, 20.0);
            let a = self.centre_anchor(&b);
            self.apply_scale(&base, a, [f; 3]);
            self.scale_pct = 100.0;
        }
        let r = area.response.rect;
        let (pressed, ppos, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
        if close || esc || (pressed && ppos.map_or(false, |p| !r.contains(p))) {
            self.size_menu = None;
            self.size_edit = None;
        }
    }
}
