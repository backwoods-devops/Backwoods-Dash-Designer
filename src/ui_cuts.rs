// Shapes placed on the dash: placing, picking, the background boolean job, the list and the inspector.
use crate::app::*;
use crate::cuts::{Cut, CutOp, CutResult, TriMesh};
use crate::json;
use crate::render::{Hit, Vtx, V3};
use crate::shapes::Shape as Sh;
use crate::theme::*;
use crate::units::MmExt;
use eframe::egui::{self, vec2, Color32, Rect};

pub fn op_color(op: CutOp) -> Color32 {
    match op {
        CutOp::Cut => DANGER,
        CutOp::Recess => BLUE,
        CutOp::Raise => TEAL,
        CutOp::Inlay => Color32::from_rgb(200, 150, 255),
    }
}

/// Smooth-shaded vertices of a triangle mesh (creases sharper than ~40 degrees stay sharp).
pub fn shade(tm: &TriMesh, tint: [f32; 3]) -> Vec<Vtx> {
    let nt = tm.tris.len();
    let mut fnorm: Vec<[f64; 3]> = Vec::with_capacity(nt);
    let mut inc: Vec<Vec<u32>> = vec![Vec::new(); tm.ps.len()];
    for (i, t) in tm.tris.iter().enumerate() {
        let (a, b, c) = (tm.ps[t[0] as usize], tm.ps[t[1] as usize], tm.ps[t[2] as usize]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        fnorm.push(n); // length = 2 * area, used as the weight
        for k in 0..3 {
            inc[t[k] as usize].push(i as u32);
        }
    }
    let unit = |n: [f64; 3]| {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-18);
        [n[0] / l, n[1] / l, n[2] / l]
    };
    let cos_lim = 40.0f64.to_radians().cos();
    let mut out = Vec::with_capacity(nt * 3);
    for (i, t) in tm.tris.iter().enumerate() {
        let fi = unit(fnorm[i]);
        for k in 0..3 {
            let mut acc = [0.0f64; 3];
            for &f in &inc[t[k] as usize] {
                let fu = unit(fnorm[f as usize]);
                if fu[0] * fi[0] + fu[1] * fi[1] + fu[2] * fi[2] > cos_lim {
                    for a in 0..3 {
                        acc[a] += fnorm[f as usize][a];
                    }
                }
            }
            let n = unit(acc);
            let p = tm.ps[t[k] as usize];
            out.push(Vtx { p: [p[0] as f32, p[1] as f32, p[2] as f32], n: [n[0] as f32, n[1] as f32, n[2] as f32], c: tint });
        }
    }
    out
}

fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

fn dv(ui: &mut egui::Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str, start: &mut bool) -> bool {
    let d = egui::DragValue::new(v).range(range).speed(speed);
    let r = ui.add(if suffix.trim() == "mm" { d.mm() } else { d.suffix(suffix) });
    if r.drag_started() || r.gained_focus() {
        *start = true;
    }
    r.changed()
}

impl App {
    pub fn cut_put(&mut self, c: &Cut, quiet: bool) {
        self.op(format!(r#"{{"op":"cut_set","quiet":{},"cut":{}}}"#, quiet, json::to_string(&c.to_json())));
    }

    pub fn select_cut(&mut self, id: u32) {
        if self.sel_cut != Some(id) {
            self.gizmo_rot = false; // a newly picked part starts with the move arrows
        }
        self.sel_cut = Some(id);
        self.sel_part = None;
        self.focus = Focus::Cut;
        if let Some(x) = self.st.proj.cuts.iter().find(|c| c.id == id).map(|c| c.x) {
            self.flip_dock_to_x(x);
        }
    }

    /// Drop a copy of a shape on the surface at a ray hit.
    pub fn add_cut(&mut self, shape: &Sh, h: Hit) {
        let n = crate::cuts::unit([h.n[0] as f64, h.n[1] as f64, h.n[2] as f64]);
        let c = Cut::new(0, shape, [self.snap_x(h.p[0] as f64), h.p[1] as f64, h.p[2] as f64], n);
        self.op(format!(r#"{{"op":"cut_add","cut":{}}}"#, json::to_string(&c.to_json())));
        if let Some(c) = self.st.proj.cuts.last() {
            let id = c.id;
            self.select_cut(id);
        }
        self.show_cuts = true;
    }

    pub fn place_shape_at_view_centre(&mut self, shape: &Sh) {
        let r = self.vp_rect;
        let (o, d) = self.cam.ray(r.width() * 0.5, r.height() * 0.5, r.width(), r.height());
        let hit = self.mesh.as_ref().and_then(|m| m.raycast(o, d)).or_else(|| self.mesh.as_ref().and_then(|m| m.surface_at(0.0, 150.0)));
        if let Some(h) = hit {
            self.add_cut(shape, h);
        } else {
            self.toast("Could not find the dash surface to place the shape on", true);
        }
    }

    /// The outline of a cut draped on the skin, for drawing.
    pub fn drape(&self, c: &Cut) -> Vec<V3> {
        let Some(m) = &self.mesh else { return Vec::new() };
        let f = c.frame();
        let mut out = Vec::new();
        for p in c.outline() {
            let w = f.world(p[0], p[1], 0.0);
            let o = [(w[0] + f.n[0] * 300.0) as f32, (w[1] + f.n[1] * 300.0) as f32, (w[2] + f.n[2] * 300.0) as f32];
            let d = [-f.n[0] as f32, -f.n[1] as f32, -f.n[2] as f32];
            if let Some(h) = m.raycast(o, d) {
                out.push([h.p[0] + h.n[0] * 0.5, h.p[1] + h.n[1] * 0.5, h.p[2] + h.n[2] * 0.5]);
            }
        }
        out
    }

    /// A shape being dragged over the dash: its footprint under the pointer.
    pub fn set_cut_preview(&mut self, shape: Option<(&Sh, Hit)>) {
        self.cut_prev = match shape {
            Some((s, h)) => {
                let n = crate::cuts::unit([h.n[0] as f64, h.n[1] as f64, h.n[2] as f64]);
                let c = Cut::new(0, s, [h.p[0] as f64, h.p[1] as f64, h.p[2] as f64], n);
                self.drape(&c)
            }
            None => Vec::new(),
        };
    }

    pub fn pick_cut_at(&self, lx: f32, ly: f32, rect: Rect) -> Option<(u32, Hit)> {
        let (o, d) = self.cam.ray(lx, ly, rect.width(), rect.height());
        let dash = self.mesh.as_ref().and_then(|m| m.raycast(o, d));
        let dash_t = dash.as_ref().map(|h| ((h.p[0] - o[0]).powi(2) + (h.p[1] - o[1]).powi(2) + (h.p[2] - o[2]).powi(2)).sqrt());
        // 3D parts: nearest triangle along the ray
        let mut best: Option<(f64, u32)> = None;
        let (od, dd) = ([o[0] as f64, o[1] as f64, o[2] as f64], [d[0] as f64, d[1] as f64, d[2] as f64]);
        for (id, tm) in &self.cut_obj_tm {
            for t in &tm.tris {
                let (a, b, c) = (tm.ps[t[0] as usize], tm.ps[t[1] as usize], tm.ps[t[2] as usize]);
                let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
                let p = [dd[1] * e2[2] - dd[2] * e2[1], dd[2] * e2[0] - dd[0] * e2[2], dd[0] * e2[1] - dd[1] * e2[0]];
                let det = e1[0] * p[0] + e1[1] * p[1] + e1[2] * p[2];
                if det.abs() < 1e-12 {
                    continue;
                }
                let s = [od[0] - a[0], od[1] - a[1], od[2] - a[2]];
                let u = (s[0] * p[0] + s[1] * p[1] + s[2] * p[2]) / det;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let q = [s[1] * e1[2] - s[2] * e1[1], s[2] * e1[0] - s[0] * e1[2], s[0] * e1[1] - s[1] * e1[0]];
                let v = (dd[0] * q[0] + dd[1] * q[1] + dd[2] * q[2]) / det;
                if v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let t = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) / det;
                if t > 0.0 && best.map_or(true, |b| t < b.0) {
                    best = Some((t, *id));
                }
            }
        }
        if let Some((t, id)) = best {
            if dash_t.map_or(true, |dt| t <= dt as f64 + 1.0) {
                let p = [(od[0] + dd[0] * t) as f32, (od[1] + dd[1] * t) as f32, (od[2] + dd[2] * t) as f32];
                return Some((id, Hit { p, n: [0.0, 0.0, 1.0] }));
            }
        }
        let h = dash?;
        let p = [h.p[0] as f64, h.p[1] as f64, h.p[2] as f64];
        for c in self.st.proj.cuts.iter().rev().filter(|c| c.visible && !c.is3d()) {
            let l = c.frame().local(p);
            if l[2].abs() < 60.0 && inside(&c.outline(), [l[0], l[1]]) {
                return Some((c.id, h));
            }
        }
        None
    }

    /// Keep the footprints and the boolean result in step with the project.
    pub fn update_cuts(&mut self) {
        let key = (self.rev, self.st.thickness.to_bits());
        if key != self.cut_key {
            self.cut_key = key;
            self.cut_foot = crate::cuts::expanded(&self.st.proj.cuts).iter().filter(|c| !c.is3d()).map(|c| (c.id, self.drape(c))).collect();
            self.cut_pending = true;
        }
        let okey = (self.rev, self.sel_cut.unwrap_or(0));
        if okey != self.cut_obj_key {
            self.cut_obj_key = okey;
            let mut all: Vec<Vtx> = Vec::new();
            let mut tms: Vec<(u32, TriMesh)> = Vec::new();
            for c in crate::cuts::expanded(&self.st.proj.cuts).iter().filter(|c| c.is3d()) {
                if let Some(tm) = c.world_obj(true) {
                    if Some(c.id) == self.sel_cut {
                        all.extend(shade(&tm, if c.op == CutOp::Cut { [0.95, 0.4, 0.4] } else { [0.4, 0.85, 0.75] }));
                    }
                    tms.push((c.id, tm));
                }
            }
            self.cut_obj_tm = tms;
            self.skin_ver += 1;
            let mut sc = self.scene.lock().unwrap();
            sc.objs = all;
            sc.objs_ver = self.skin_ver;
        }
        // collect finished jobs
        while let Ok((k, r)) = self.cut_rx.try_recv() {
            self.cut_inflight = false;
            if k != self.cut_key {
                self.cut_pending = true;
                continue;
            }
            match r {
                Ok(res) => self.apply_cut_result(res),
                Err(e) => {
                    crate::errlog::write("CUT", &e);
                    self.cut_msg = e;
                    self.cut_ok = false;
                }
            }
        }
        // while the mouse is held (dragging a part or a slider) the cut waits; it runs once on release
        let held = self.egui_ctx.input(|i| i.pointer.any_down());
        if self.cut_pending && held && self.st.proj.cuts.iter().any(|c| c.visible && c.is3d()) {
            if self.cut_msg.is_empty() {
                self.cut_msg = "Release to update the cuts".into();
            }
            self.egui_ctx.request_repaint_after(std::time::Duration::from_millis(120));
        } else if self.cut_pending && !self.cut_inflight {
            self.cut_pending = false;
            let has = self.st.proj.cuts.iter().any(|c| c.visible) && self.st.thickness > 0.0;
            if !has {
                if self.cut_ok {
                    self.cut_ok = false;
                    let mut sc = self.scene.lock().unwrap();
                    self.skin_ver += 1;
                    sc.result = Vec::new();
                    sc.result_ver = self.skin_ver;
                    sc.plug = Vec::new();
                    sc.plug_ver = self.skin_ver;
                }
                self.cut_msg.clear();
            } else {
                let proj = self.st.proj.clone();
                let th = self.st.thickness;
                let tx = self.cut_tx.clone();
                let ctx = self.egui_ctx.clone();
                self.cut_inflight = true;
                self.cut_msg = "Working out the cuts...".into();
                std::thread::spawn(move || {
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::cuts::compute(&proj, th, false).unwrap_or_else(|| Ok(CutResult::default())))).unwrap_or_else(|_| Err("the cut engine crashed (details are in errors.log)".to_string()));
                    let _ = tx.send((key, r));
                    ctx.request_repaint();
                });
            }
        }
        if self.cut_inflight {
            self.egui_ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }

    fn apply_cut_result(&mut self, res: CutResult) {
        self.skin_ver += 1;
        let body = shade(&res.body, [1.0, 1.0, 1.0]);
        let mut plug = Vec::new();
        for (_, p) in &res.plugs {
            plug.extend(shade(p, [0.55, 0.85, 0.75]));
        }
        {
            let mut sc = self.scene.lock().unwrap();
            sc.result = body;
            sc.result_ver = self.skin_ver;
            sc.plug = plug;
            sc.plug_ver = self.skin_ver;
        }
        self.cut_ok = !res.body.tris.is_empty();
        self.cut_msg = if res.warnings.is_empty() { String::new() } else { format!("Could not apply: {}", res.warnings.join("; ")) };
        if !self.cut_msg.is_empty() && self.cut_msg != self.cut_logged {
            let parts: Vec<String> = self.st.proj.cuts.iter().map(|c| format!("[{} {:?} xyz {:.2} {:.2} {:.2} rot {:.2} {:.2} {:.2} size {:.2}x{:.2}x{:.2} wall {} mesh {:?} mirror {}]", c.name, c.op, c.x, c.y, c.z, c.rx, c.ry, c.rot, c.shape.w, c.shape.h, c.shape.depth, c.line, c.shape.mesh, c.mirror)).collect();
            crate::errlog::write("CUT", &format!("{} | piece {} | parts {}", self.cut_msg, self.st.active, parts.join(" ")));
            self.cut_logged = self.cut_msg.clone();
        } else if self.cut_msg.is_empty() {
            self.cut_logged.clear();
        }
    }

    // ---------------------------------------------------------------- left panel list
    pub fn cuts_list(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(format!("SHAPE CUTS ({})", self.st.proj.cuts.len())).size(11.0).color(MUTED).strong());
        if self.st.proj.cuts.is_empty() {
            ui.label(egui::RichText::new("Nothing cut yet. Open the Parts & shapes tab and drag a shape onto the dash.").size(12.5).color(MUTED));
        }
        let rows: Vec<(u32, String, CutOp, bool, bool)> = self.st.proj.cuts.iter().map(|c| (c.id, c.name.clone(), c.op, c.visible, c.locked)).collect();
        for (id, name, op, vis, locked) in rows {
            let sel = self.sel_cut == Some(id) && self.focus == Focus::Cut;
            let row_top = ui.cursor().min;
            let tag = if locked { format!("{} - locked", op.label()) } else { op.label().to_string() };
            let r = tree_row(ui, sel, Some(op_color(op)), &name, &tag, 0.0);
            if r.clicked() {
                self.select_cut(id);
            }
            let er = Rect::from_min_size(egui::pos2(r.rect.right() - 28.0, row_top.y + 3.0), vec2(24.0, 22.0));
            let eresp = ui.put(er, |ui: &mut egui::Ui| eye(ui, vis));
            if eresp.clicked() {
                if let Some(mut c) = self.st.proj.cuts.iter().find(|c| c.id == id).cloned() {
                    c.visible = !vis;
                    self.cut_put(&c, false);
                }
            }
        }
    }

    // ---------------------------------------------------------------- right panel
    pub fn inspect_cut(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.sel_cut else { return self.inspect_dash(ui) };
        let Some(cut) = self.st.proj.cuts.iter().find(|c| c.id == id).cloned() else { return self.inspect_dash(ui) };
        let mut c = cut.clone();
        let mut start = false;
        let mut changed = false; // a drag-value edit (no undo step per tick)
        let mut hard = false; // a click that should be its own undo step
        ui.label(egui::RichText::new(&c.name).size(18.0).strong());
        ui.add_space(4.0);
        let lock_label = if c.locked { "LOCKED  (click to unlock)" } else { "Lock in place" };
        let lock_btn = if c.locked {
            ui.add_sized([ui.available_width(), 34.0], egui::Button::new(egui::RichText::new(lock_label).strong().color(Color32::BLACK)).fill(ACCENT))
        } else {
            ui.add_sized([ui.available_width(), 34.0], egui::Button::new(lock_label))
        };
        if lock_btn.on_hover_text("Lock the part so it cannot be dragged, turned or edited by accident").clicked() {
            c.locked = !c.locked;
            self.cut_put(&c, false);
            return;
        }
        if c.locked {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("This part is locked. It stays exactly where it is until you unlock it.").size(12.5).color(MUTED));
            if c.is3d() {
                ui.label(egui::RichText::new(format!("Center  x {}  y {}  z {}", crate::units::num(c.x, 1), crate::units::num(c.y, 1), crate::units::len(c.z, 1))).size(12.5));
                ui.label(egui::RichText::new(format!("Rotation  x {:.1}  y {:.1}  z {:.1} deg", c.rx, c.ry, c.rot)).size(12.5));
            }
            if self.cut_inflight {
                ui.label(egui::RichText::new("Working out the cuts...").size(12.0).color(ACCENT));
            } else if !self.cut_msg.is_empty() {
                ui.label(egui::RichText::new(self.cut_msg.clone()).size(12.0).color(DANGER));
            }
            return;
        }
        ui.label(egui::RichText::new("Drag it on the dash to move it.").size(12.5).color(MUTED));
        ui.add_space(6.0);
        let mut nm = c.name.clone();
        if ui.add(egui::TextEdit::singleline(&mut nm).desired_width(f32::INFINITY)).changed() {
            c.name = nm.chars().take(40).collect();
            changed = true;
        }
        let is3d = c.is3d();
        label_caps(ui, "What it does");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            let ops: &[CutOp] = if is3d { &[CutOp::Cut, CutOp::Raise] } else { &CutOp::ALL };
            for &op in ops {
                let label = if is3d { if op == CutOp::Raise { "Add" } else { "Cut" } } else { op.label() };
                if pill_button(ui, label, c.op == op).on_hover_text(op.hint()).clicked() && c.op != op {
                    c.op = op;
                    if !is3d && op == CutOp::Raise && c.depth < 0.5 {
                        c.depth = 2.0;
                    }
                    hard = true;
                }
            }
        });
        if is3d {
            ui.label(egui::RichText::new(if c.op == CutOp::Cut { "The part floats where you put it. Wherever it overlaps the dash is removed." } else { "The part floats where you put it. Wherever it overlaps the dash is fused on." }).size(12.0).color(MUTED));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Handles in the view").size(12.0).color(MUTED));
                if pill_button(ui, "Move", !self.gizmo_rot).clicked() {
                    self.gizmo_rot = false;
                }
                if pill_button(ui, "Rotate", self.gizmo_rot).clicked() {
                    self.gizmo_rot = true;
                }
            });
            label_caps(ui, "Position of the part's center");
            egui::Grid::new("pos3d").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label("X (across)");
                changed |= dv(ui, &mut c.x, -1500.0..=1500.0, 0.5, " mm", &mut start);
                ui.end_row();
                ui.label("Y (front-back)");
                changed |= dv(ui, &mut c.y, -1500.0..=1500.0, 0.5, " mm", &mut start);
                ui.end_row();
                ui.label("Z (height)");
                changed |= dv(ui, &mut c.z, -1500.0..=1500.0, 0.5, " mm", &mut start);
                ui.end_row();
            });
            ui.horizontal_wrapped(|ui| {
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
                    if ui.button("Sit on surface").on_hover_text("Lower or raise the part until its underside just touches the dash under its center").clicked() {
                        c.z = s - lo;
                        hard = true;
                    }
                    if ui.button("Sink flush").on_hover_text("Push the part in until its top is level with the dash surface under its center").clicked() {
                        c.z = s - hi;
                        hard = true;
                    }
                }
            });
            label_caps(ui, "Rotate (degrees)");
            egui::Grid::new("rot3d").num_columns(4).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
                for (lab, which) in [("X", 0), ("Y", 1), ("Z", 2)] {
                    ui.label(lab);
                    let v = match which {
                        0 => &mut c.rx,
                        1 => &mut c.ry,
                        _ => &mut c.rot,
                    };
                    changed |= dv(ui, v, -360.0..=360.0, 0.5, " deg", &mut start);
                    if ui.small_button("-90").clicked() {
                        *v = (*v - 90.0).rem_euclid(360.0);
                        hard = true;
                    }
                    if ui.small_button("+90").clicked() {
                        *v = (*v + 90.0).rem_euclid(360.0);
                        hard = true;
                    }
                    ui.end_row();
                }
            });
            if ui.small_button("Reset rotation").clicked() {
                c.rx = 0.0;
                c.ry = 0.0;
                c.rot = 0.0;
                hard = true;
            }
            label_caps(ui, "Size");
            egui::Grid::new("size3d").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                let (ow, oh, od) = (c.shape.w, c.shape.h, c.shape.depth);
                let (mut w, mut h, mut dd) = (ow, oh, od);
                ui.label("Width (x)");
                let a = dv(ui, &mut w, 1.0..=1500.0, 0.5, " mm", &mut start);
                ui.end_row();
                ui.label("Length (y)");
                let b = dv(ui, &mut h, 1.0..=1500.0, 0.5, " mm", &mut start);
                ui.end_row();
                ui.label("Height (z)");
                let d3 = dv(ui, &mut dd, 0.5..=1500.0, 0.5, " mm", &mut start);
                ui.end_row();
                if a || b || d3 {
                    if c.shape.lock {
                        let k = if a { w / ow } else if b { h / oh } else { dd / od };
                        w = ow * k;
                        h = oh * k;
                        dd = od * k;
                    }
                    c.shape.set_size(w, h);
                    c.shape.depth = dd;
                    changed = true;
                }
            });
            if pill_button(ui, if c.shape.lock { "Proportions locked" } else { "Proportions free" }, c.shape.lock).clicked() {
                c.shape.lock = !c.shape.lock;
                hard = true;
            }
            if c.op == CutOp::Cut {
                if ui.checkbox(&mut c.line, "Keep a wall around the cut").on_hover_text("On: the dash keeps a wall of its own thickness around the removed part, so it is a pocket, not just a hole. Off: the part is simply removed and the dash is left with an open hole.").changed() {
                    hard = true;
                }
            }
        }
        if !is3d {
        label_caps(ui, "Depth and walls");
        }
        if !is3d { egui::Grid::new("cutprops").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            if c.op != CutOp::Cut {
                ui.label(if c.op == CutOp::Raise { "Height" } else { "Depth" });
                changed |= dv(ui, &mut c.depth, 0.2..=(if c.keep_wall { 40.0 } else { self.st.thickness.max(1.0) * if c.op == CutOp::Raise { 4.0 } else { 1.0 } }), 0.1, " mm", &mut start);
                ui.end_row();
            } else {
                ui.label("Depth");
                ui.label(egui::RichText::new(format!("through ({} wall)", crate::units::len(self.st.thickness, 1))).color(MUTED));
                ui.end_row();
            }
            ui.label("Wall bevel");
            changed |= dv(ui, &mut c.draft, -45.0..=60.0, 0.3, " deg", &mut start);
            ui.end_row();
        });
        ui.label(egui::RichText::new("0 = straight walls. A positive angle slopes them like a countersink (narrower at the bottom of a cut, narrower at the top of a raised pad).").size(11.5).color(MUTED));
        if c.op != CutOp::Cut {
            if ui.checkbox(&mut c.follow_floor, if c.op == CutOp::Raise { "Top follows the surface" } else { "Floor follows the surface" }).on_hover_text("On: the depth is the same everywhere on a curved surface. Off: the floor (or top) is a flat plane at the center of the shape.").changed() {
                hard = true;
            }
        }
        if c.op != CutOp::Cut {
            if ui.checkbox(&mut c.keep_wall, "Keep the wall thickness").on_hover_text("On: the inside of the wall is pushed in (or out) with the surface, so the wall stays as thick as the rest of the dash however deep the recess or tall the raise. Off: material is only removed or added on the outer face, so a deep recess can break through.").changed() {
                hard = true;
            }
        }
        if c.op == CutOp::Inlay {
            label_caps(ui, "Plug");
            egui::Grid::new("plug").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label("Gap around it");
                changed |= dv(ui, &mut c.gap, 0.0..=3.0, 0.02, " mm", &mut start);
                ui.end_row();
                ui.label("Stands proud");
                changed |= dv(ui, &mut c.plug_h, -3.0..=6.0, 0.05, " mm", &mut start);
                ui.end_row();
            });
            ui.label(egui::RichText::new("The plug is a separate piece (shown green). It exports as its own STL.").size(11.5).color(MUTED));
        }
        }
        if !is3d { label_caps(ui, "Size and angle");
        egui::Grid::new("cutsize").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            let (ow, oh) = (c.shape.w, c.shape.h);
            let (mut w, mut h) = (ow, oh);
            ui.label("Width");
            let a = dv(ui, &mut w, 1.0..=1500.0, 0.5, " mm", &mut start);
            ui.end_row();
            ui.label(if is3d { "Length" } else { "Height" });
            let b = dv(ui, &mut h, 1.0..=1500.0, 0.5, " mm", &mut start);
            ui.end_row();
            if a || b {
                if c.shape.lock {
                    if a {
                        h = oh * w / ow;
                    } else {
                        w = ow * h / oh;
                    }
                }
                c.shape.set_size(w, h);
                changed = true;
            }
            ui.label("Turn");
            changed |= dv(ui, &mut c.rot, -360.0..=360.0, 0.5, " deg", &mut start);
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if pill_button(ui, if c.shape.lock { "Aspect locked" } else { "Aspect free" }, c.shape.lock).clicked() {
                c.shape.lock = !c.shape.lock;
                hard = true;
            }
            if pill_button(ui, if c.follow { "Along the surface" } else { "Straight down" }, c.follow).on_hover_text("Direction of the cut. Straight down works best on the flat top. Along the surface follows the slope under the shape.").clicked() {
                c.follow = !c.follow;
                hard = true;
            }
        });
        }
        ui.add_space(4.0);
        if ui.checkbox(&mut c.mirror, "Symmetrical: also on the other side").on_hover_text("Makes the mirror image on the other side of the dash center line (x = 0), for parts that appear on both sides.").changed() {
            hard = true;
        }
        if !is3d {
            ui.label(egui::RichText::new(format!("Position  x {:.1}   y {:.1}", c.x, c.y)).size(12.5).color(MUTED));
        }
        ui.horizontal(|ui| {
            if ui.button("Center on dash").on_hover_text("Move to x = 0").clicked() {
                if is3d {
                    c.x = 0.0;
                    hard = true;
                } else if let Some(h) = self.mesh.as_ref().and_then(|m| m.surface_at(0.0, c.y as f32)) {
                    c.x = 0.0;
                    c.z = h.p[2] as f64;
                    c.n = crate::cuts::unit([h.n[0] as f64, h.n[1] as f64, h.n[2] as f64]);
                    hard = true;
                }
            }
            if c.shape.mesh.is_empty() && ui.button("Edit the outline…").on_hover_text("Open the shape editor on this cut's outline").clicked() {
                let mut sh = c.shape.clone();
                sh.name = c.name.clone();
                let mut ed = crate::ui_shapes::ShapeEd::new(sh, None);
                ed.cut_id = Some(c.id);
                self.shape_ed = Some(ed);
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Delete").clicked() {
                self.op(format!(r#"{{"op":"cut_remove","id":{}}}"#, id));
                self.sel_cut = None;
                self.focus = Focus::Dash;
            }
        });
        if self.cut_inflight {
            ui.label(egui::RichText::new("Working out the cuts...").size(12.0).color(ACCENT));
        } else if !self.cut_msg.is_empty() {
            ui.label(egui::RichText::new(self.cut_msg.clone()).size(12.0).color(DANGER));
        }
        if start {
            self.op_s(r#"{"op":"checkpoint"}"#);
        }
        if hard {
            self.cut_put(&c, false);
        } else if changed {
            self.cut_put(&c, true);
        }
    }
}
