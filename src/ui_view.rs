// 3D viewport and the editor dock (profile / plan / height).
use crate::app::*;
use crate::render::*;
use crate::theme::*;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind};
use eframe::egui_glow;
use std::sync::Arc;

fn fade(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

impl App {
    // ================================================================ 3D viewport
    pub fn ui_viewport(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        self.vp_rect = rect;
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);

        // background
        let mut mesh = egui::Mesh::default();
        let (c0, c1) = (Color32::from_rgb(31, 36, 45), Color32::from_rgb(15, 16, 19));
        mesh.colored_vertex(rect.left_top(), c0);
        mesh.colored_vertex(rect.right_top(), c0);
        mesh.colored_vertex(rect.right_bottom(), c1);
        mesh.colored_vertex(rect.left_bottom(), c1);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(mesh));

        // matrices for the GL callback
        let (vp, v) = self.cam.view_proj(rect.width() / rect.height().max(1.0));
        {
            let mut sc = self.scene.lock().unwrap();
            sc.view_proj = vp;
            sc.view = v;
        }
        let scene = self.scene.clone();
        let glres = self.glres.clone();
        let cb = egui_glow::CallbackFn::new(move |_info, p| {
            let gl = p.gl();
            let mut g = glres.lock().unwrap();
            if g.is_none() {
                match GlRes::new(gl) {
                    Ok(r) => *g = Some(r),
                    Err(e) => {
                        eprintln!("shader error: {}", e);
                        return;
                    }
                }
            }
            if let Some(r) = g.as_mut() {
                let sc = scene.lock().unwrap();
                r.draw(gl, &sc);
            }
        });
        painter.add(egui::PaintCallback { rect, callback: Arc::new(cb) });

        self.viewport_input(&ctx, &resp, rect);
        self.viewport_overlay(&painter, rect, &resp);
        self.draw_gizmo(&painter, rect);
        self.viewport_chrome(&ctx, rect);
        self.cut_menu_ui(&ctx);
        self.size_overlay(&painter, rect, &resp);
        self.draw_nav(&painter, rect, resp.hover_pos());
        self.size_menu_ui(&ctx);
    }

    fn local(&self, p: Pos2, rect: Rect) -> (f32, f32) {
        (p.x - rect.left(), p.y - rect.top())
    }

    fn pick_part_at(&self, lx: f32, ly: f32, rect: Rect) -> Option<(u32, Hit)> {
        let m = self.mesh.as_ref()?;
        let (o, d) = self.cam.ray(lx, ly, rect.width(), rect.height());
        let h = m.raycast(o, d)?;
        if !self.layers.parts || !self.piece_vis[self.st.active as usize] {
            return None;
        }
        let mut best: Option<(u32, f64)> = None;
        for c in self.st.proj.comps.iter().filter(|c| c.visible) {
            let dd = (h.p[0] as f64 - c.x).hypot(h.p[1] as f64 - c.y);
            let r = crate::parts::radius(c) + 2.0;
            if dd < r && best.map_or(true, |b| dd / r < b.1) {
                best = Some((c.id, dd / r));
            }
        }
        best.map(|b| (b.0, h))
    }

    fn grip_at(&self, lx: f32, ly: f32, rect: Rect) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in self.st.proj.profiles.iter().enumerate() {
            let a = p.anchor();
            if let Some(s) = self.cam.project([p.x as f32, a[0] as f32, a[1] as f32], rect.width(), rect.height()) {
                let d = (s[0] - lx).hypot(s[1] - ly);
                if d < 12.0 && best.map_or(true, |b| d < b.1) {
                    best = Some((i, d));
                }
            }
        }
        best.map(|b| b.0)
    }

    /// Where across the dash (x) the pointer is on the windshield line or the front line, in the 3D view.
    fn pick_x_3d(&self, lx: f32, ly: f32, rect: Rect) -> Option<f64> {
        let p = &self.st.proj;
        let (x0, x1) = (p.rail.first()?[0], p.rail.last()?[0]);
        let mut best: Option<(f64, f32)> = None;
        for s in 0..=400 {
            let x = x0 + (x1 - x0) * s as f64 / 400.0;
            let (y, z) = p.rail_at(x);
            let mut pts = vec![[x as f32, y as f32, z as f32]];
            if !p.rail2.is_empty() {
                let (y2, z2) = p.rail2_at(x);
                pts.push([x as f32, y2 as f32, z2 as f32]);
            }
            for q in pts {
                if let Some(sc) = self.cam.project(q, rect.width(), rect.height()) {
                    let d = (sc[0] - lx).hypot(sc[1] - ly);
                    if d < 24.0 && best.map_or(true, |b| d < b.1) {
                        best = Some((x, d));
                    }
                }
            }
        }
        best.map(|b| (b.0 * 10.0).round() / 10.0)
    }

    fn line_at(&self, lx: f32, ly: f32, rect: Rect) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in self.st.proj.profiles.iter().enumerate() {
            for q in p.curve() {
                if let Some(s) = self.cam.project([p.x as f32, q[0] as f32, q[1] as f32], rect.width(), rect.height()) {
                    let d = (s[0] - lx).hypot(s[1] - ly);
                    if d < 9.0 && best.map_or(true, |b| d < b.1) {
                        best = Some((i, d));
                    }
                }
            }
        }
        best.map(|b| b.0)
    }

    fn viewport_input(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect) {
        // hover feedback
        if let Some(h) = resp.hover_pos() {
            let (lx, ly) = self.local(h, rect);
            if self.drag_item.is_none() && matches!(self.vp_drag, VpDrag::None) {
                if self.gizmo_hit(h, rect).is_some() || self.pick_part_at(lx, ly, rect).is_some() || self.grip_at(lx, ly, rect).is_some() {
                    ctx.set_cursor_icon(CursorIcon::Grab);
                }
            }
            let sc = ctx.input(|i| i.smooth_scroll_delta.y);
            if sc != 0.0 {
                self.cam.zoom(-sc);
            }
        }
        if resp.secondary_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let (lx, ly) = self.local(p, rect);
                if let Some((id, _)) = self.pick_cut_at(lx, ly, rect) {
                    self.select_cut(id);
                    self.cut_menu = Some((id, p));
                } else {
                    self.cut_menu = None;
                    // right-click on a piece of the dash: sizing menu
                    let (o, d) = self.cam.ray(lx, ly, rect.width(), rect.height());
                    let dist = |h: &Hit| ((h.p[0] - o[0]).powi(2) + (h.p[1] - o[1]).powi(2) + (h.p[2] - o[2]).powi(2)).sqrt();
                    let mut best: Option<(f32, usize)> = None;
                    if let Some(h) = self.mesh.as_ref().filter(|_| self.piece_vis[self.st.active as usize]).and_then(|m| m.raycast(o, d)) {
                        best = Some((dist(&h), self.st.active as usize));
                    }
                    for (k, m) in self.other_pick.iter().filter(|(k, _)| self.piece_vis[*k]) {
                        if let Some(h) = m.raycast(o, d) {
                            let dd = dist(&h);
                            if best.map_or(true, |b| dd < b.0) {
                                best = Some((dd, *k));
                            }
                        }
                    }
                    if let Some((_, k)) = best {
                        if k != self.st.active as usize {
                            let keep = self.cam;
                            self.switch_piece(k as u8);
                            self.cam = keep;
                        }
                        self.size_menu = Some(p);
                    } else {
                        self.size_menu = None;
                    }
                }
            }
        }
        // Shift or Space held while dragging pans; so does the middle or right button
        let shift = ctx.input(|i| i.modifiers.shift || i.key_down(egui::Key::Space));
        if resp.drag_started() {
            let primary = ctx.input(|i| i.pointer.button_down(PointerButton::Primary));
            self.vp_drag = VpDrag::Pan;
            let press = ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
            if primary && press.map_or(false, |p| self.nav_area(p, rect)) {
                // dragging the view gizmo turns the view, like Blender's; nothing else under it is touched
                self.vp_drag = VpDrag::Orbit;
                return;
            }
            let mut sizing = false;
            if primary && self.size_mode {
                if let Some(p) = ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()) {
                    sizing = self.size_drag_begin(p, resp.interact_pointer_pos().unwrap_or(p), rect);
                }
            }
            if sizing {
                self.vp_drag = VpDrag::Size;
            } else if primary && !shift {
                self.vp_drag = VpDrag::Orbit;
                if let Some(p) = resp.interact_pointer_pos() {
                    let (lx, ly) = self.local(p, rect);
                    if let Some(ax) = self.gizmo_hit(p, rect) {
                        self.gizmo_begin(ax, p, rect);
                    } else if let Some((id, hit)) = self.pick_part_at(lx, ly, rect) {
                        let c = self.st.proj.comps.iter().find(|c| c.id == id).cloned();
                        if let Some(c) = c {
                            self.select_part(id);
                            if c.locked {
                                self.toast("That part is locked. Unlock it in the Pieces & parts list to move it.", false);
                            } else {
                                self.op_s(r#"{"op":"checkpoint"}"#);
                                self.vp_drag = VpDrag::Part { id, ox: hit.p[0] as f64 - c.x, oy: hit.p[1] as f64 - c.y };
                            }
                        }
                    } else if let Some((id, hit)) = self.pick_cut_at(lx, ly, rect) {
                        if let Some(c) = self.st.proj.cuts.iter().find(|c| c.id == id).cloned() {
                            self.select_cut(id);
                            if c.locked {
                                self.toast("That part is locked. Unlock it in the side panel or the right-click menu to move it.", false);
                            } else {
                                self.op_s(r#"{"op":"checkpoint"}"#);
                                let (o, dir) = self.cam.ray(lx, ly, rect.width(), rect.height());
                                let [px, py] = if c.is3d() { plane_z(o, dir, c.z).unwrap_or([hit.p[0] as f64, hit.p[1] as f64]) } else { [hit.p[0] as f64, hit.p[1] as f64] };
                                self.vp_drag = VpDrag::Cut { id, ox: px - c.x, oy: py - c.y };
                            }
                        }
                    } else if let Some(i) = self.grip_at(lx, ly, rect) {
                        self.select_profile_view(i);
                        if !self.st.proj.profiles[i].fixed {
                            self.op_s(r#"{"op":"checkpoint"}"#);
                            self.vp_drag = VpDrag::Grip { i };
                        }
                    }
                }
            }
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            match self.vp_drag {
                VpDrag::Orbit => self.cam.orbit(d.x, d.y),
                VpDrag::Pan => self.cam.pan(d.x, d.y),
                VpDrag::Part { id, ox, oy } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let (lx, ly) = self.local(p, rect);
                        let (o, dir) = self.cam.ray(lx, ly, rect.width(), rect.height());
                        let dist = |h: &Hit| ((h.p[0] - o[0]).powi(2) + (h.p[1] - o[1]).powi(2) + (h.p[2] - o[2]).powi(2)).sqrt();
                        let hit = self.mesh.as_ref().and_then(|m| m.raycast(o, dir));
                        // is another piece nearer under the cursor? then letting go hands the part to it
                        let mut other: Option<(f32, usize, [f32; 3])> = None;
                        for (k, m) in self.other_pick.iter().filter(|(k, _)| self.piece_vis[*k]) {
                            if let Some(h) = m.raycast(o, dir) {
                                let dd = dist(&h);
                                if other.map_or(true, |b| dd < b.0) {
                                    other = Some((dd, *k, h.p));
                                }
                            }
                        }
                        let own = hit.as_ref().map(|h| dist(h));
                        match other {
                            Some((dd, k, hp)) if own.map_or(true, |o| dd < o) => {
                                self.drop_target = Some((k, hp[0] as f64, hp[1] as f64));
                            }
                            _ => {
                                self.drop_target = None;
                                if let Some(h) = hit {
                                    let nx = self.snap_x(h.p[0] as f64 - ox);
                                    self.op(format!(r#"{{"op":"comp_move","id":{},"x":{},"y":{}}}"#, id, nx, h.p[1] as f64 - oy));
                                }
                            }
                        }
                    }
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                }
                VpDrag::Gizmo { .. } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.gizmo_drag(p);
                    }
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                }
                VpDrag::Cut { id, ox, oy } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let (lx, ly) = self.local(p, rect);
                        let three = self.st.proj.cuts.iter().find(|c| c.id == id).map(|c| (c.is3d(), c.z));
                        if let Some((true, cz)) = three {
                            // a free part slides in the horizontal plane through its center
                            let (o, dir) = self.cam.ray(lx, ly, rect.width(), rect.height());
                            if let Some(pt) = plane_z(o, dir, cz) {
                                let nx = self.snap_x(pt[0] - ox);
                                self.op(format!(r#"{{"op":"cut_move","id":{},"x":{},"y":{},"z":{},"n":[0,0,1]}}"#, id, nx, pt[1] - oy, cz));
                            }
                            ctx.set_cursor_icon(CursorIcon::Grabbing);
                            return;
                        }
                        let hit = self.mesh.as_ref().and_then(|m| {
                            let (o, dir) = self.cam.ray(lx, ly, rect.width(), rect.height());
                            m.raycast(o, dir)
                        });
                        if let Some(h) = hit {
                            let nx = self.snap_x(h.p[0] as f64 - ox);
                            let ny = h.p[1] as f64 - oy;
                            if let Some(sh) = self.mesh.as_ref().and_then(|m| m.surface_at(nx as f32, ny as f32)) {
                                self.op(format!(r#"{{"op":"cut_move","id":{},"x":{},"y":{},"z":{},"n":[{},{},{}]}}"#, id, nx, ny, sh.p[2], sh.n[0], sh.n[1], sh.n[2]));
                            }
                        }
                    }
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                }
                VpDrag::Grip { i } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let (lx, ly) = self.local(p, rect);
                        let x = self.rail_x_near(lx, ly, rect);
                        self.op(format!(r#"{{"op":"move_profile","i":{},"x":{}}}"#, i, x));
                    }
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                VpDrag::Size => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.size_drag_update(p, shift);
                    }
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                }
                VpDrag::None => {}
            }
        }
        if resp.drag_stopped() {
            self.size_drag = None;
            if let (VpDrag::Part { id, .. }, Some((k, x, y))) = (&self.vp_drag, self.drop_target) {
                // dropped on another piece: that piece becomes the part's parent
                let id = *id;
                if let Some(nid) = self.st.move_comp(id, k, x, y) {
                    let keep = self.cam;
                    self.switch_piece(k as u8);
                    self.cam = keep;
                    self.rev += 1;
                    self.select_part(nid);
                    self.toast(&format!("Moved to the {}.", crate::engine::PIECE_NAMES[k].to_lowercase()), false);
                }
            }
            self.drop_target = None;
            self.vp_drag = VpDrag::None;
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if self.nav_area(p, rect) {
                    // a click on a ball looks along that axis (front, side, top...); elsewhere on the gizmo does nothing
                    if let Some((ax, side)) = self.nav_ball_at(p, rect) {
                        self.nav_snap(ax, side);
                    }
                    return;
                }
                let (lx, ly) = self.local(p, rect);
                if let Some(sym) = self.addp_pick {
                    // placing a new profile: the click must land on the windshield line or the front line
                    match self.pick_x_3d(lx, ly, rect) {
                        Some(x) => self.place_profile(x, sym),
                        None => self.toast("Click right on the windshield line or the front line.", false),
                    }
                    return;
                }
                if let Some((id, _)) = self.pick_part_at(lx, ly, rect) {
                    self.select_part(id);
                } else if let Some((id, _)) = self.pick_cut_at(lx, ly, rect) {
                    self.select_cut(id);
                } else if let Some(i) = self.grip_at(lx, ly, rect).or_else(|| self.line_at(lx, ly, rect)) {
                    self.select_profile_view(i);
                } else {
                    // a click on any piece of the dash: switch to that piece and show its profile at that spot
                    let (o, d) = self.cam.ray(lx, ly, rect.width(), rect.height());
                    let dist = |h: &Hit| ((h.p[0] - o[0]).powi(2) + (h.p[1] - o[1]).powi(2) + (h.p[2] - o[2]).powi(2)).sqrt();
                    let mut best: Option<(f32, usize, f64)> = None;
                    if let Some(h) = self.mesh.as_ref().filter(|_| self.piece_vis[self.st.active as usize]).and_then(|m| m.raycast(o, d)) {
                        best = Some((dist(&h), self.st.active as usize, h.p[0] as f64));
                    }
                    for (k, m) in self.other_pick.iter().filter(|(k, _)| self.piece_vis[*k]) {
                        if let Some(h) = m.raycast(o, d) {
                            let dd = dist(&h);
                            if best.map_or(true, |b| dd < b.0) {
                                best = Some((dd, *k, h.p[0] as f64));
                            }
                        }
                    }
                    // the closing side walls win when they are nearer than any piece
                    let wall_hit = if self.sw.on { self.side_pick.as_ref().and_then(|m| m.raycast(o, d)).map(|h| dist(&h)) } else { None };
                    if let Some(wd) = wall_hit.filter(|wd| best.map_or(true, |b| *wd < b.0)) {
                        let _ = wd;
                        self.select_walls();
                    } else if let Some((_, k, x)) = best {
                        if k != self.st.active as usize {
                            let keep = self.cam; // a click should not move the camera
                            self.switch_piece(k as u8);
                            self.cam = keep;
                        }
                        self.sel_part = None;
                        self.sel_cut = None;
                        self.sel_sec = None;
                        // the piece itself gets focus; the bottom editor shows the profile under the click
                        // but only a click right on a profile line or handle selects that profile
                        self.flip_dock_to_x(x);
                        self.focus = Focus::Dash;
                    } else {
                        self.focus = Focus::Dash;
                        self.sel_part = None;
                        self.sel_cut = None;
                        self.sel_sec = None;
                    }
                }
            }
        }
        // dragging a shape from the Shapes tab
        if let Some(sh) = self.drag_shape.clone() {
            let released = ctx.input(|i| i.pointer.any_released());
            let pos = ctx.pointer_latest_pos();
            let inside = pos.map_or(false, |p| rect.contains(p));
            let hit = if inside {
                let (lx, ly) = self.local(pos.unwrap(), rect);
                self.mesh.as_ref().and_then(|m| {
                    let (o, d) = self.cam.ray(lx, ly, rect.width(), rect.height());
                    m.raycast(o, d)
                })
            } else {
                None
            };
            self.set_cut_preview(hit.map(|h| (&sh, h)));
            if let Some(p) = pos {
                let lp = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("shape_ghost")));
                let r = Rect::from_min_size(p + vec2(12.0, 12.0), vec2(150.0, 24.0));
                lp.rect_filled(r, CornerRadius::same(12), fade(ACCENT, 235));
                lp.text(r.center(), Align2::CENTER_CENTER, format!("Drop {} on the dash", sh.name), FontId::proportional(12.5), Color32::from_rgb(27, 19, 8));
            }
            ctx.set_cursor_icon(if hit.is_some() { CursorIcon::Grabbing } else { CursorIcon::NotAllowed });
            if released {
                self.drag_shape = None;
                self.cut_prev.clear();
                if let Some(h) = hit {
                    self.add_cut(&sh, h);
                }
            }
        }
        // dragging a part from the library
        if let Some(it) = self.drag_item {
            let released = ctx.input(|i| i.pointer.any_released());
            let pos = ctx.pointer_latest_pos();
            let inside = pos.map_or(false, |p| rect.contains(p));
            let hit = if inside {
                let (lx, ly) = self.local(pos.unwrap(), rect);
                self.mesh.as_ref().and_then(|m| {
                    let (o, d) = self.cam.ray(lx, ly, rect.width(), rect.height());
                    m.raycast(o, d)
                })
            } else {
                None
            };
            let c = &crate::parts::CATALOG[it];
            match hit {
                Some(h) => {
                    let comp = crate::model::Comp { id: 0, kind: c.kind.into(), shape: c.shape.into(), name: c.label.into(), x: self.snap_x(h.p[0] as f64), y: h.p[1] as f64, w: c.w, h: c.h, rot: 0.0, visible: true, locked: false, look: None };
                    self.set_preview(Some(&comp));
                }
                None => self.set_preview(None),
            }
            if let Some(p) = pos {
                let lp = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("ghost")));
                let r = Rect::from_min_size(p + vec2(12.0, 12.0), vec2(120.0, 24.0));
                lp.rect_filled(r, CornerRadius::same(12), fade(ACCENT, 235));
                lp.text(r.center(), Align2::CENTER_CENTER, c.label, FontId::proportional(13.0), Color32::from_rgb(27, 19, 8));
            }
            ctx.set_cursor_icon(if hit.is_some() { CursorIcon::Grabbing } else { CursorIcon::NotAllowed });
            if released {
                self.drag_item = None;
                self.set_preview(None);
                if let Some(h) = hit {
                    let sx = self.snap_x(h.p[0] as f64);
                    self.add_part(it, sx, h.p[1] as f64);
                }
            }
        }
    }

    /// The x on the rail whose screen position is nearest the pointer.
    fn rail_x_near(&self, lx: f32, ly: f32, rect: Rect) -> f64 {
        let p = &self.st.proj;
        let (x0, x1) = (p.rail.first().unwrap()[0], p.rail.last().unwrap()[0]);
        let mut best = (f32::MAX, p.profiles[self.sel_prof].x);
        for s in 0..=240 {
            let x = x0 + (x1 - x0) * s as f64 / 240.0;
            let (y, z) = p.rail_at(x);
            if let Some(q) = self.cam.project([x as f32, y as f32, z as f32], rect.width(), rect.height()) {
                let d = (q[0] - lx).hypot(q[1] - ly);
                if d < best.0 {
                    best = (d, x);
                }
            }
        }
        best.1
    }

    fn poly(&self, pts: impl Iterator<Item = V3>, rect: Rect) -> Vec<Pos2> {
        pts.filter_map(|p| self.cam.project(p, rect.width(), rect.height())).map(|s| pos2(rect.left() + s[0], rect.top() + s[1])).collect()
    }

    fn viewport_overlay(&self, painter: &egui::Painter, rect: Rect, resp: &egui::Response) {
        if let Some((k, _, _)) = self.drop_target {
            painter.text(rect.center_top() + vec2(0.0, 26.0), Align2::CENTER_CENTER, format!("Let go to move this part to the {}", crate::engine::PIECE_NAMES[k].to_lowercase()), FontId::proportional(14.0), Color32::from_rgb(255, 217, 168));
        }
        if let (Some(sym), Some(h)) = (self.addp_pick, resp.hover_pos()) {
            let (lx, ly) = self.local(h, rect);
            if let Some(x) = self.pick_x_3d(lx, ly, rect) {
                let pr = &self.st.proj;
                for xx in if sym { vec![x, -x] } else { vec![x] } {
                    let (y, z) = pr.rail_at(xx);
                    let a = self.poly([[xx as f32, y as f32, z as f32]].into_iter(), rect);
                    let mut ends = a.clone();
                    if !pr.rail2.is_empty() {
                        let (y2, z2) = pr.rail2_at(xx);
                        ends.extend(self.poly([[xx as f32, y2 as f32, z2 as f32]].into_iter(), rect));
                    }
                    if ends.len() == 2 {
                        painter.line_segment([ends[0], ends[1]], Stroke::new(1.5, fade(ACCENT, 160)));
                    }
                    for e in ends {
                        painter.circle_filled(e, 6.0, ACCENT);
                        painter.circle_stroke(e, 6.0, Stroke::new(1.5, Color32::WHITE));
                    }
                }
                painter.text(h + vec2(14.0, -14.0), Align2::LEFT_CENTER, format!("x = {}", crate::units::num(x, 1)), FontId::proportional(12.5), Color32::from_rgb(255, 217, 168));
            }
        }
        if self.walls_sel() {
            // the side walls are the focus: no profile lines or handles of the other pieces
            return;
        }
        let p = &self.st.proj;
        let sel = self.sel_prof;
        let partner = p.locked_partner(sel);
        let show_sel = self.focus == Focus::Profile;
        // profile lines
        for (i, pr) in p.profiles.iter().enumerate() {
            let is_sel = i == sel && show_sel;
            let is_mirror = Some(i) == partner && show_sel;
            if !self.layers.lines && !is_sel {
                continue;
            }
            let pts = self.poly(pr.curve().into_iter().map(|q| [pr.x as f32, q[0] as f32, q[1] as f32]), rect);
            let stroke = if is_sel {
                Stroke::new(2.4, ACCENT)
            } else if is_mirror {
                Stroke::new(1.6, fade(TEAL, 200))
            } else {
                Stroke::new(1.1, Color32::from_rgba_unmultiplied(150, 162, 180, 120))
            };
            painter.add(Shape::line(pts, stroke));
        }
        // rail
        if self.layers.rail {
            let (x0, x1) = (p.rail.first().unwrap()[0], p.rail.last().unwrap()[0]);
            let pts = self.poly(
                (0..=150).map(|s| {
                    let x = x0 + (x1 - x0) * s as f64 / 150.0;
                    let (y, z) = p.rail_at(x);
                    [x as f32, y as f32, z as f32]
                }),
                rect,
            );
            painter.add(Shape::line(pts, Stroke::new(2.0, BLUE)));
        }
        // bottom guideline (lower pad)
        if self.layers.rail && !p.rail2.is_empty() {
            let (x0, x1) = (p.rail.first().unwrap()[0], p.rail.last().unwrap()[0]);
            let pts = self.poly(
                (0..=150).map(|s| {
                    let x = x0 + (x1 - x0) * s as f64 / 150.0;
                    let (y, z) = p.rail2_at(x);
                    [x as f32, y as f32, z as f32]
                }),
                rect,
            );
            painter.add(Shape::line(pts, Stroke::new(2.0, TEAL)));
            for (k, r) in p.rail2.iter().enumerate() {
                let x = p.rail[k][0];
                if let Some(s) = self.cam.project([x as f32, r[0] as f32, r[1] as f32], rect.width(), rect.height()) {
                    let c = pos2(rect.left() + s[0], rect.top() + s[1]);
                    painter.rect_filled(Rect::from_center_size(c, vec2(9.0, 9.0)), CornerRadius::same(2), if self.sel_rail2 == Some(k) { Color32::WHITE } else { TEAL });
                }
            }
        }
        // center line (x = 0): dashed
        if self.show_center && self.center_line.len() > 1 {
            let pts = self.poly(self.center_line.iter().cloned(), rect);
            let mut on = true;
            let mut acc = 0.0f32;
            for w in pts.windows(2) {
                let d = w[0].distance(w[1]);
                if on {
                    painter.line_segment([w[0], w[1]], Stroke::new(1.8, Color32::from_rgb(255, 224, 120)));
                }
                acc += d;
                if acc > 9.0 {
                    on = !on;
                    acc = 0.0;
                }
            }
            if let Some(l) = pts.first() {
                painter.text(*l + vec2(8.0, -12.0), Align2::LEFT_CENTER, "center", FontId::proportional(11.5), Color32::from_rgb(255, 224, 120));
            }
        }
        // part footprints (always drawn, so a part is never invisible)
        for (id, ol) in &self.part_outlines {
            let on = Some(*id) == self.sel_part;
            if !self.layers.parts || !self.piece_vis[self.st.active as usize] || (ol.len() < 3) {
                continue;
            }
            let pts = self.poly(ol.iter().cloned(), rect);
            if pts.len() > 2 {
                let mut closed = pts.clone();
                closed.push(pts[0]);
                painter.add(Shape::line(closed, Stroke::new(if on { 2.2 } else { 1.2 }, if on { ACCENT } else { Color32::from_rgba_unmultiplied(255, 255, 255, 150) })));
            }
        }
        // shape cuts: footprints colored by what they do
        for (id, ol) in &self.cut_foot {
            if ol.len() < 3 {
                continue;
            }
            let Some(c) = p.cuts.iter().find(|c| c.id == *id) else { continue };
            let on = Some(*id) == self.sel_cut && self.focus == Focus::Cut;
            let col = crate::ui_cuts::op_color(c.op);
            let pts = self.poly(ol.iter().cloned(), rect);
            if pts.len() > 2 {
                let mut closed = pts.clone();
                closed.push(pts[0]);
                painter.add(Shape::line(closed, Stroke::new(if on { 2.8 } else { 1.8 }, if on { Color32::WHITE } else { col })));
                if on {
                    let mut cl = pts.clone();
                    cl.push(pts[0]);
                    painter.add(Shape::line(cl, Stroke::new(1.2, col)));
                }
            }
        }
        if self.drag_shape.is_some() && self.cut_prev.len() > 2 {
            let pts = self.poly(self.cut_prev.iter().cloned(), rect);
            if pts.len() > 2 {
                let mut closed = pts.clone();
                closed.push(pts[0]);
                painter.add(Shape::line(closed, Stroke::new(2.4, ACCENT)));
            }
        }
        if self.drag_item.is_some() && self.preview_outline.len() > 2 {
            let pts = self.poly(self.preview_outline.iter().cloned(), rect);
            if pts.len() > 2 {
                painter.add(Shape::convex_polygon(pts.clone(), Color32::from_rgba_unmultiplied(255, 154, 60, 90), Stroke::new(2.4, ACCENT)));
                let mut cx = 0.0;
                let mut cy = 0.0;
                for q in &pts {
                    cx += q.x;
                    cy += q.y;
                }
                let c = pos2(cx / pts.len() as f32, cy / pts.len() as f32);
                painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], Stroke::new(1.5, Color32::WHITE));
                painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], Stroke::new(1.5, Color32::WHITE));
                painter.text(c + vec2(0.0, -16.0), Align2::CENTER_BOTTOM, &self.preview_name, FontId::proportional(12.0), Color32::WHITE);
            }
        }
        // grips on the windshield line
        let hover = resp.hover_pos().map(|h| self.local(h, rect)).and_then(|(lx, ly)| self.grip_at(lx, ly, rect));
        for (i, pr) in p.profiles.iter().enumerate() {
            let a = pr.anchor();
            let Some(s) = self.cam.project([pr.x as f32, a[0] as f32, a[1] as f32], rect.width(), rect.height()) else { continue };
            let c = pos2(rect.left() + s[0], rect.top() + s[1]);
            let on = i == sel && show_sel;
            let r = if on || hover == Some(i) { 9.0 } else { 7.0 };
            painter.circle_filled(c, r, if on { ACCENT } else if hover == Some(i) { CARD_HI } else { Color32::from_rgb(22, 24, 28) });
            painter.circle_stroke(c, r, Stroke::new(1.5, if on { Color32::WHITE } else { Color32::from_rgb(150, 162, 180) }));
            painter.text(c, Align2::CENTER_CENTER, format!("{}", i), FontId::proportional(10.0), if on { Color32::from_rgb(27, 19, 8) } else { TEXT });
        }
        // key point labels on the selected profile
        if show_sel {
            let pr = &p.profiles[sel];
            for (ki, k) in p.keys.iter().enumerate() {
                if let Some(q) = pr.pts.get(k.idx) {
                    if let Some(s) = self.cam.project([pr.x as f32, q.co[0] as f32, q.co[1] as f32], rect.width(), rect.height()) {
                        let c = pos2(rect.left() + s[0], rect.top() + s[1]);
                        painter.circle_filled(c, 3.0, Color32::from_rgb(255, 217, 168));
                        if ki % 2 == 1 {
                            painter.text(c + vec2(-8.0, -8.0), Align2::RIGHT_CENTER, &k.name, FontId::proportional(12.0), Color32::from_rgb(255, 217, 168));
                        } else {
                            painter.text(c + vec2(8.0, -8.0), Align2::LEFT_CENTER, &k.name, FontId::proportional(12.0), Color32::from_rgb(255, 217, 168));
                        }
                    }
                }
            }
        }
        // (the small corner axes are gone: the view gizmo in the top right shows the axes now)
    }

    fn viewport_chrome(&mut self, ctx: &egui::Context, rect: Rect) {
        if self.exp_open {
            return;
        }
        egui::Area::new(egui::Id::new("vpbar")).order(egui::Order::Foreground).fixed_pos(pos2(rect.center().x - 230.0, rect.top() + 30.0)).show(ctx, |ui| {
            egui::Frame::new().fill(fade(PANEL, 235)).corner_radius(CornerRadius::same(18)).stroke(Stroke::new(1.0, LINE)).inner_margin(egui::Margin::symmetric(8, 5)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let t = self.cam.target;
                    if pill_button(ui, if self.view_mat { "Materials" } else { "Design colors" }, self.view_mat).on_hover_text("Switch between the real material look and the editing colors").clicked() {
                        self.view_mat = !self.view_mat;
                    }
                    ui.add_space(6.0);
                    let zc = [300.0, 165.0, 340.0][self.st.active as usize];
                    if pill_button(ui, "3D", false).on_hover_text("Home view").clicked() {
                        self.cam = Camera::home();
                        self.cam.target[2] = zc + 30.0;
                        self.cam.dist = if zc < 200.0 { 1400.0 } else { 1750.0 };
                    }
                    if pill_button(ui, "Driver", false).on_hover_text("Looking toward the windshield from the seat").clicked() {
                        self.cam = Camera { yaw: 0.0, pitch: 0.12, dist: 2400.0, target: [0.0, 100.0, zc] };
                    }
                    if pill_button(ui, "Top", false).clicked() {
                        self.cam = Camera { yaw: 0.0, pitch: 1.5, dist: 2600.0, target: [t[0], 120.0, zc] };
                    }
                    if pill_button(ui, "Side", false).clicked() {
                        self.cam = Camera { yaw: -1.5708, pitch: 0.0, dist: 2000.0, target: [0.0, 120.0, zc] };
                    }
                    ui.add_space(6.0);
                    if pill_button(ui, "Wall", self.wall_view).on_hover_text("Show the real printed wall: the exact shell that gets exported, at the wall thickness set in Printing").clicked() {
                        self.wall_view = !self.wall_view;
                    }
                    if !self.st.proj.cuts.is_empty() {
                        if pill_button(ui, "Cuts", self.show_cuts).on_hover_text("Show the dash with the placed shapes cut, raised or inlaid (Materials and Wall views)").clicked() {
                            self.show_cuts = !self.show_cuts;
                        }
                    }
                    if self.wall_view {
                        if self.st.thickness <= 0.0 {
                            ui.label(egui::RichText::new("wall = 0 (surface only)").size(12.0).color(MUTED));
                        } else {
                            ui.label(egui::RichText::new(crate::units::len(self.st.thickness, 1)).size(12.0).color(MUTED));
                        }
                        ui.checkbox(&mut self.wall_cut, "Cut");
                        if self.wall_cut {
                            let (lo, hi) = self.st.proj.profiles.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.x), a.1.max(p.x)));
                            ui.add(egui::Slider::new(&mut self.wall_x, lo as f32..=hi as f32).show_value(false)).on_hover_text("The side of the cut facing away from you is removed; the orange face is the wall in section");
                        }
                    }
                });
            });
        });
    }

    pub fn lighting_window(&mut self, ctx: &egui::Context) {
        if !self.light_open {
            return;
        }
        let mut open = true;
        let mut close = false;
        let before = self.light;
        egui::Window::new("Lighting").open(&mut open).collapsible(false).resizable(false).default_width(340.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            let l = &mut self.light;
            let row = |ui: &mut egui::Ui, name: &str, v: &mut f32, lo: f32, hi: f32, tip: &str| {
                ui.horizontal(|ui| {
                    ui.add_sized([96.0, 18.0], egui::Label::new(name));
                    ui.add(egui::Slider::new(v, lo..=hi).fixed_decimals(2)).on_hover_text(tip);
                });
            };
            row(ui, "Exposure", &mut l.exposure, 0.3, 2.0, "Overall brightness. Lower it if things look washed out");
            row(ui, "Main light", &mut l.key, 0.0, 2.5, "Strength of the main light");
            row(ui, "Fill light", &mut l.fill, 0.0, 2.5, "Softer light from the other side");
            row(ui, "Ambient", &mut l.ambient, 0.0, 2.5, "Base light that reaches everything, shadows included");
            row(ui, "Reflections", &mut l.reflect, 0.0, 2.5, "How strongly the room shows in shiny and glossy surfaces");
            row(ui, "Texture", &mut l.texture, 0.0, 2.5, "Strength of grain, pebble and brushed texture");
            ui.add_space(4.0);
            row(ui, "Light turn", &mut l.yaw, -180.0, 180.0, "Direction of the main light around the scene, in degrees");
            row(ui, "Light height", &mut l.pitch, 0.0, 90.0, "How high the main light sits, in degrees");
            ui.checkbox(&mut l.fixed, "Keep the lights fixed in the scene").on_hover_text("Off: the lights follow the camera. On: the lights stay put as you orbit");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Reset lighting").clicked() {
                    *l = crate::lighting::Lighting::default();
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if self.light != before {
            self.light.save(&self.st.dir);
        }
        if !open || close {
            self.light_open = false;
        }
    }

    pub fn resample_window(&mut self, ctx: &egui::Context) {
        if !self.rs_open {
            return;
        }
        let mut open = true;
        let np = self.st.proj.profiles[0].pts.len();
        egui::Window::new("Re-space the points").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).resizable(false).default_width(340.0).show(ctx, |ui| {
            ui.label(format!("Now {} points on every profile.", np));
            ui.label(egui::RichText::new("Keeps the shape of each profile and recalculates all of them with the new count, spread evenly along the curve. Sharp corners and key points stay put.").size(12.0).color(MUTED));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("New point count");
                ui.add(egui::DragValue::new(&mut self.rs_n).range(4..=90).speed(0.2));
            });
            ui.checkbox(&mut self.rs_bend, "Put extra points on tight bends");
            ui.horizontal(|ui| {
                if primary_button(ui, "Re-space").clicked() {
                    self.op(format!(r#"{{"op":"resample","n":{},"bend":{}}}"#, self.rs_n, self.rs_bend));
                    self.sel_pt = self.sel_pt.min(self.st.proj.profiles[0].pts.len() - 1);
                    self.clamp_sel();
                    self.rs_open = false;
                }
                ui.label(egui::RichText::new("Undo brings the old points back.").size(12.0).color(MUTED));
            });
        });
        if !open {
            self.rs_open = false;
        }
    }

    // ================================================================ editor dock
    pub fn ui_dock(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 8)).stroke(Stroke::new(1.0, LINE));
        if !self.dock_open {
            let rr = egui::Panel::bottom("dock_closed").frame(frame).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if pill_button(ui, "Open editor", false).clicked() {
                        self.dock_open = true;
                    }
                    ui.label(egui::RichText::new("Profile shape, guideline plan and height").color(MUTED).size(12.0));
                });
            });
            if Self::edge_tab_h(ui.ctx(), "dock_tab_open", pos2(rr.response.rect.right() - 90.0, rr.response.rect.top() - 20.0), false, "Open the editor") {
                self.dock_open = true;
            }
            return;
        }
        if self.dock_tab == DockTab::Part && self.sel_part.is_none() {
            self.dock_tab = DockTab::Profile;
        }
        let rr = egui::Panel::bottom("dock").resizable(true).default_size(290.0).size_range(170.0..=640.0).frame(frame).show(ui, |ui| {
            // each toolbar row is a fixed, non-wrapping strip: when the window or a side panel is resized it scrolls sideways
            // instead of reflowing, so every control stays where it is
            egui::ScrollArea::horizontal().id_salt("dock_tabs").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                if pill_button(ui, "Profile shape", self.dock_tab == DockTab::Profile).clicked() {
                    self.dock_tab = DockTab::Profile;
                }
                if pill_button(ui, "Guidelines: plan", self.dock_tab == DockTab::Plan).clicked() {
                    self.dock_tab = DockTab::Plan;
                }
                if pill_button(ui, "Guidelines: height", self.dock_tab == DockTab::Height).clicked() {
                    self.dock_tab = DockTab::Height;
                }
                if pill_button(ui, "Side walls", self.dock_tab == DockTab::Walls).on_hover_text("The back curve of the closing walls at the two ends of the dash").clicked() {
                    self.dock_tab = DockTab::Walls;
                }
                if let Some(nm) = self.sel_part.and_then(|id| self.st.proj.comps.iter().find(|c| c.id == id).map(|c| c.name.clone())) {
                    if pill_button(ui, &format!("Part: {}", nm), self.dock_tab == DockTab::Part).on_hover_text("Move, size and turn the selected part from above").clicked() {
                        self.dock_tab = DockTab::Part;
                        self.plan.fitted = false;
                    }
                }
                ui.add_space(10.0);
                if ui.small_button("Fit").on_hover_text("Fit the drawing to the window").clicked() {
                    self.side.fitted = false;
                    self.plan.fitted = false;
                    self.elev.fitted = false;
                }
                // the tabs stay pinned left; the trace image controls are pinned to the right edge
                if self.dock_tab != DockTab::Walls && self.dock_tab != DockTab::Part {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.horizontal(|ui| self.trace_controls(ui));
                    });
                }
            });
            });
            if self.dock_tab != DockTab::Part {
            egui::ScrollArea::horizontal().id_salt("dock_tools").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.dock_tab == DockTab::Walls {
                    ui.add_space(6.0);
                    self.wall_toolbar(ui);
                }
                if self.dock_tab == DockTab::Plan || self.dock_tab == DockTab::Height {
                    ui.add_space(6.0);
                    let locked = self.st.active == 2;
                    if locked {
                        self.front_mode_ui(ui, true);
                    }
                    // the top trim's guidelines are fixed to the Top pad, so adding profiles and mirroring only apply to the others
                    if !locked {
                    ui.add_enabled_ui(true, |ui| {
                        if ui.button("+ Profile...").on_hover_text("Add a profile to the windshield and front lines: choose mirrored or one side, and where").clicked() {
                            self.open_add_profile();
                        }
                        let mut sym = self.st.proj.rail_sym;
                        if ui.checkbox(&mut sym, "Mirror").on_hover_text("Keep both halves of the guidelines mirrored about the center line").changed() {
                            self.op(format!(r#"{{"op":"set_rail_sym","v":{}}}"#, sym));
                        }
                    });
                    }
                }
                if self.dock_tab == DockTab::Profile {
                    ui.add_space(6.0);
                    if ui.button("+ Profile...").on_hover_text("Add a profile to the windshield and front lines: choose mirrored or one side, and where").clicked() {
                        self.open_add_profile();
                    }
                    let np = self.st.proj.profiles[0].pts.len();
                    {
                        let k = self.sel_pt;
                        let ok = k > 0 && k + 1 < np;
                        let broken = self.st.proj.is_broken(k);
                        let (txt, tip) = if broken {
                            ("Join handles", "Put the two handles of the selected point back in line, so the curve flows smoothly through it again")
                        } else {
                            ("Break handles", "Let the two handles of the selected point move independently, to make a corner or a sharp turn. Applies to this point on every profile.")
                        };
                        if ui.add_enabled(ok, egui::Button::new(txt)).on_hover_text(tip).clicked() {
                            self.op(format!(r#"{{"op":"set_broken","k":{},"on":{}}}"#, k, !broken));
                        }
                    }
                    if ui.add_enabled(self.sel_pt > 0 && self.sel_pt + 1 < np && np > 4, egui::Button::new("- Point")).on_hover_text("Remove the selected point from every profile").clicked() {
                        let k = self.sel_pt;
                        self.op(format!(r#"{{"op":"pt_remove","k":{}}}"#, k));
                        self.sel_pt = k.saturating_sub(1);
                        self.clamp_sel();
                    }
                    {
                        let i = self.sel_prof.min(self.st.proj.profiles.len() - 1);
                        let paired = self.st.proj.partner(i).is_some();
                        let mut s = self.st.proj.profiles[i].sym;
                        if paired {
                            if ui.checkbox(&mut s, "Mirror").on_hover_text("Keep this profile mirrored with its partner on the other side of the dash: moving one moves the other").changed() {
                                self.op(format!(r#"{{"op":"set_sym","i":{},"v":{}}}"#, i, s));
                            }
                        } else {
                            ui.label(egui::RichText::new("no mirror partner").size(12.0).color(MUTED)).on_hover_text("The center profile sits on the middle line, so it has no partner to mirror");
                        }
                    }
                    {
                        let i = self.sel_prof.min(self.st.proj.profiles.len() - 1);
                        let np_ = self.st.proj.profiles.len();
                        if i > 0 && i + 1 < np_ {
                            let mut sh = self.st.proj.profiles[i].crease;
                            if ui.checkbox(&mut sh, "Sharp edge").on_hover_text("On: the surface meets this profile with a sharp edge. Off: it flows smoothly through it.").changed() {
                                self.op(format!(r#"{{"op":"set_crease","i":{},"v":{}}}"#, i, sh));
                            }
                        }
                    }
                    ui.label(egui::RichText::new(format!("{} points", np)).size(12.0).color(MUTED));
                    if self.multi.len() > 1 {
                        ui.label(egui::RichText::new(format!("{} selected - drag one to move them all", self.multi.len())).size(12.0).color(Color32::from_rgb(255, 217, 168)));
                    }
                    if ui.button("Re-space all...").on_hover_text("Keep the shape but set a new point count, spread evenly on every profile").clicked() {
                        self.rs_n = np.min(24).max(4);
                        self.rs_open = true;
                    }
                }
            });
            });
            }
            let size = ui.available_size();
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, CornerRadius::same(10), Color32::from_rgb(18, 20, 24));
            painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, LINE), StrokeKind::Inside);
            match self.dock_tab {
                DockTab::Profile => self.side_editor(ui, &painter, rect, &resp),
                DockTab::Plan => self.rail_editor(ui, &painter, rect, &resp, 1),
                DockTab::Height => self.rail_editor(ui, &painter, rect, &resp, 2),
                DockTab::Walls => self.wall_editor(ui, rect, &resp),
                DockTab::Part => self.part_editor(ui, &painter, rect, &resp),
            }
            let hint = match self.dock_tab {
                        DockTab::Profile => "Drag points and handles. Right-click a point or the curve for add, remove and break handles (double-click the curve also adds). Shift-click / Shift-drag selects several.",
                        DockTab::Plan => "Seen from above. Drag circles to slide profiles, squares to bend the line. Drag a box on empty space to select several and move them together. Pan: Space+drag or middle-drag.",
                        DockTab::Height => "Seen from the side. Drag circles to slide profiles, squares to raise or lower the line. Drag a box on empty space to select several and move them together. Pan: Space+drag or middle-drag.",
                        DockTab::Part => "Seen from above. Drag the part to move it; drag the side squares to size it, the corner to scale it, the round handle to turn it (Shift = 15 degree steps).",
                        DockTab::Walls => "Side view, front to the right. Drag the squares and circles to shape the back curve; both walls use it.",
                    };
            {
                // on the top trim's guideline screens the note about the locked windshield line goes under the hint
                let note = if self.st.active == 2 && (self.dock_tab == DockTab::Plan || self.dock_tab == DockTab::Height) {
                    Some("The windshield line is locked to the Top pad. The front line (teal) can move in y and z but not below the pad. Use \"Refit trim to top pad\" to reset it.")
                } else {
                    None
                };
                let mut bottom = rect.bottom() - 10.0;
                if let Some(n) = note {
                    let g = painter.layout_no_wrap(n.to_string(), egui::FontId::proportional(12.0), ACCENT);
                    let pos = egui::pos2(rect.left() + 12.0, bottom - g.size().y);
                    painter.rect_filled(Rect::from_min_size(pos - egui::vec2(4.0, 2.0), g.size() + egui::vec2(8.0, 4.0)), CornerRadius::same(4), Color32::from_black_alpha(150));
                    bottom = pos.y - 6.0;
                    painter.galley(pos, g, ACCENT);
                }
                let g = painter.layout_no_wrap(hint.to_string(), egui::FontId::proportional(12.0), MUTED);
                let pos = egui::pos2(rect.left() + 12.0, bottom - g.size().y);
                painter.rect_filled(Rect::from_min_size(pos - egui::vec2(4.0, 2.0), g.size() + egui::vec2(8.0, 4.0)), CornerRadius::same(4), Color32::from_black_alpha(150));
                painter.galley(pos, g, MUTED);
            }
        });
        if Self::edge_tab_h(ui.ctx(), "dock_tab_close", pos2(rr.response.rect.right() - 90.0, rr.response.rect.top() - 20.0), true, "Fold the editor away") {
            self.dock_open = false;
        }
    }


    // ---------------------------------------------------------------- selected part editor (seen from above)
    fn part_editor(&mut self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect, resp: &egui::Response) {
        let p = self.st.proj.clone();
        let Some(c) = self.sel_part.and_then(|id| p.comps.iter().find(|c| c.id == id).cloned()) else {
            return self.side_editor(ui, painter, rect, resp);
        };
        // footprint of the piece seen from above
        let mut top: Vec<[f64; 2]> = Vec::new();
        let mut bot: Vec<[f64; 2]> = Vec::new();
        for pr in &p.profiles {
            let (mut a, mut b) = (f64::MAX, f64::MIN);
            for q in pr.curve() {
                a = a.min(q[0]);
                b = b.max(q[0]);
            }
            bot.push([pr.x, a]);
            top.push([pr.x, b]);
        }
        if !self.plan.fitted {
            let mut b = [1e9f32, -1e9, 1e9, -1e9];
            for q in top.iter().chain(bot.iter()) {
                b[0] = b[0].min(q[0] as f32);
                b[1] = b[1].max(q[0] as f32);
                b[2] = b[2].min(q[1] as f32);
                b[3] = b[3].max(q[1] as f32);
            }
            let r = (c.w.max(c.h) * 0.6) as f32;
            b[0] = b[0].min(c.x as f32 - r);
            b[1] = b[1].max(c.x as f32 + r);
            b[2] = b[2].min(c.y as f32 - r);
            b[3] = b[3].max(c.y as f32 + r);
            let mut v = self.plan;
            v.fit(rect, b, 30.0);
            self.plan = v;
        }
        let v = self.plan;
        self.draw_grid(painter, rect, &v);
        if self.show_center {
            let t = v.to_screen(rect, 0.0, 0.0);
            let mut y = rect.top();
            while y < rect.bottom() {
                painter.line_segment([pos2(t.x, y), pos2(t.x, (y + 6.0).min(rect.bottom()))], Stroke::new(1.2, Color32::from_rgb(255, 224, 120)));
                y += 11.0;
            }
        }
        // the dash from above
        let mut ring: Vec<Pos2> = top.iter().map(|q| v.to_screen(rect, q[0], q[1])).collect();
        ring.extend(bot.iter().rev().map(|q| v.to_screen(rect, q[0], q[1])));
        if ring.len() >= 3 {
            painter.add(Shape::convex_polygon(ring.clone(), fade(BLUE, 22), Stroke::NONE));
            let mut cl = ring.clone();
            cl.push(ring[0]);
            painter.add(Shape::line(cl, Stroke::new(1.5, fade(BLUE, 140))));
        }
        // shape outline of a part
        let outline = |cc: &crate::model::Comp, n: usize| -> Vec<Pos2> {
            let (ca, sa) = (cc.rot.to_radians().cos(), cc.rot.to_radians().sin());
            let pts: Vec<(f64, f64)> = if cc.shape == "round" {
                (0..n).map(|k| { let a = k as f64 / n as f64 * std::f64::consts::TAU; (a.cos() * cc.w / 2.0, a.sin() * cc.w / 2.0) }).collect()
            } else {
                vec![(-cc.w / 2.0, -cc.h / 2.0), (cc.w / 2.0, -cc.h / 2.0), (cc.w / 2.0, cc.h / 2.0), (-cc.w / 2.0, cc.h / 2.0)]
            };
            pts.iter().map(|(u, w)| v.to_screen(rect, cc.x + u * ca - w * sa, cc.y + u * sa + w * ca)).collect()
        };
        for o in p.comps.iter().filter(|o| o.id != c.id && o.visible) {
            let mut pl = outline(o, 40);
            if let Some(f) = pl.first().cloned() {
                pl.push(f);
            }
            painter.add(Shape::line(pl, Stroke::new(1.2, Color32::from_rgb(110, 118, 130))));
        }
        let round = c.shape == "round";
        let (ca, sa) = (c.rot.to_radians().cos(), c.rot.to_radians().sin());
        let ls = |u: f64, w: f64| v.to_screen(rect, c.x + u * ca - w * sa, c.y + u * sa + w * ca);
        let body = outline(&c, 64);
        painter.add(Shape::convex_polygon(body.clone(), fade(ACCENT, 46), Stroke::new(2.0, ACCENT)));
        let ctr = v.to_screen(rect, c.x, c.y);
        painter.circle_filled(ctr, 2.5, ACCENT);
        // handles: (mode, position)
        let mut hs: Vec<(u8, Pos2)> = vec![(2, ls(c.w / 2.0, 0.0)), (2, ls(-c.w / 2.0, 0.0))];
        if round {
            hs.push((2, ls(0.0, c.w / 2.0)));
            hs.push((2, ls(0.0, -c.w / 2.0)));
        } else {
            hs.push((3, ls(0.0, c.h / 2.0)));
            hs.push((3, ls(0.0, -c.h / 2.0)));
            hs.push((4, ls(c.w / 2.0, c.h / 2.0)));
        }
        let tm = ls(0.0, if round { c.w / 2.0 } else { c.h / 2.0 });
        let dir = {
            let d = tm - ctr;
            if d.length() > 1.0 { d / d.length() } else { vec2(0.0, -1.0) }
        };
        let rot_h = tm + dir * 30.0;
        painter.line_segment([tm, rot_h], Stroke::new(1.0, ACCENT));
        painter.circle_filled(rot_h, 7.0, Color32::from_rgb(29, 32, 36));
        painter.circle_stroke(rot_h, 7.0, Stroke::new(1.8, ACCENT));
        for (m, h) in &hs {
            let r = if *m == 4 { 6.5 } else { 5.0 };
            painter.rect_filled(Rect::from_center_size(*h, vec2(r * 2.0, r * 2.0)), CornerRadius::same(2), if *m == 4 { Color32::WHITE } else { ACCENT });
        }
        let lab = if round { format!("{} dia", crate::units::len(c.w, 1)) } else { format!("{} x {}", crate::units::num(c.w, 1), crate::units::len(c.h, 1)) };
        painter.text(ctr + vec2(0.0, body.iter().map(|q| (q.y - ctr.y).abs()).fold(0.0, f32::max) + 14.0), Align2::CENTER_CENTER, format!("{}   {:.0} deg   at {:.1}, {:.1}", lab, c.rot, c.x, c.y), FontId::proportional(11.5), Color32::from_rgb(255, 217, 168));
        painter.text(pos2(rect.left() + 10.0, rect.top() + 12.0), Align2::LEFT_CENTER, "TOP VIEW  (front of the dash up)", FontId::proportional(10.5), MUTED);

        // cursor hints
        if let Some(h) = resp.hover_pos() {
            if h.distance(rot_h) < 11.0 {
                ui.ctx().set_cursor_icon(CursorIcon::Alias);
            } else if let Some((m, _)) = hs.iter().find(|(_, q)| q.distance(h) < 11.0) {
                ui.ctx().set_cursor_icon(if *m == 4 { CursorIcon::ResizeNwSe } else { CursorIcon::ResizeHorizontal });
            } else {
                let (mx, my) = v.from_screen(rect, h);
                let (dx, dy) = (mx - c.x, my - c.y);
                let (lu, lw) = (dx * ca + dy * sa, -dx * sa + dy * ca);
                let inside = if round { (lu * lu + lw * lw).sqrt() <= c.w / 2.0 } else { lu.abs() <= c.w / 2.0 && lw.abs() <= c.h / 2.0 };
                if inside {
                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                }
            }
        }

        if resp.drag_started() {
            self.dock_drag = DockDrag::Pan;
            if let (Some(pt), true) = (resp.interact_pointer_pos(), ui.input(|i| i.pointer.button_down(PointerButton::Primary))) {
                let (mx, my) = v.from_screen(rect, pt);
                let (dx, dy) = (mx - c.x, my - c.y);
                let (lu, lw) = (dx * ca + dy * sa, -dx * sa + dy * ca);
                let inside = if round { (lu * lu + lw * lw).sqrt() <= c.w / 2.0 } else { lu.abs() <= c.w / 2.0 && lw.abs() <= c.h / 2.0 };
                let mode = if pt.distance(rot_h) < 11.0 {
                    5
                } else if let Some((m, _)) = hs.iter().find(|(_, q)| q.distance(pt) < 11.0) {
                    *m
                } else if inside {
                    1
                } else {
                    0
                };
                if mode > 0 && c.locked {
                    self.toast("That part is locked. Unlock it in the Pieces & parts list to change it.", false);
                } else if mode > 0 {
                    self.op_s(r#"{"op":"checkpoint"}"#);
                    self.dock_drag = DockDrag::Part { mode, id: c.id, ox: dx, oy: dy, w0: c.w, h0: c.h };
                }
            }
        }
        if let (DockDrag::Part { mode, id, ox, oy, w0, h0 }, true) = (&self.dock_drag, resp.dragged()) {
            if let Some(pt) = resp.interact_pointer_pos() {
                let (mode, id, ox, oy, w0, h0) = (*mode, *id, *ox, *oy, *w0, *h0);
                let (mx, my) = v.from_screen(rect, pt);
                let (dx, dy) = (mx - c.x, my - c.y);
                let (lu, lw) = (dx * ca + dy * sa, -dx * sa + dy * ca);
                let op = match mode {
                    1 => {
                        let mut nx = mx - ox;
                        if self.snap_center && nx.abs() < 3.0 {
                            nx = 0.0;
                        }
                        Some(format!(r#"{{"op":"comp_move","id":{},"x":{},"y":{}}}"#, id, nx, my - oy))
                    }
                    2 => {
                        let w = (2.0 * lu.abs().max(if round { lw.abs() } else { 0.0 })).clamp(2.0, 1000.0);
                        Some(format!(r#"{{"op":"comp_set","id":{},"quiet":true,"w":{},"h":{}}}"#, id, w, if round { w } else { c.h }))
                    }
                    3 => Some(format!(r#"{{"op":"comp_set","id":{},"quiet":true,"h":{}}}"#, id, (2.0 * lw.abs()).clamp(2.0, 1000.0))),
                    4 => {
                        let d0 = ((w0 / 2.0).powi(2) + (h0 / 2.0).powi(2)).sqrt().max(1e-3);
                        let k = (dx.hypot(dy) / d0).clamp(0.05, 50.0);
                        Some(format!(r#"{{"op":"comp_set","id":{},"quiet":true,"w":{},"h":{}}}"#, id, (w0 * k).clamp(2.0, 1000.0), (h0 * k).clamp(2.0, 1000.0)))
                    }
                    5 => {
                        let mut a = dy.atan2(dx).to_degrees() - 90.0;
                        if ui.input(|i| i.modifiers.shift) {
                            a = (a / 15.0).round() * 15.0;
                        }
                        while a > 180.0 {
                            a -= 360.0;
                        }
                        while a <= -180.0 {
                            a += 360.0;
                        }
                        Some(format!(r#"{{"op":"comp_set","id":{},"quiet":true,"rot":{}}}"#, id, (a * 10.0).round() / 10.0))
                    }
                    _ => None,
                };
                if let Some(o) = op {
                    self.st.op(&o).ok();
                    self.rev += 1;
                }
            }
        }
        if resp.clicked() && !matches!(self.dock_drag, DockDrag::Part { .. }) {
            if let Some(pt) = resp.interact_pointer_pos() {
                let (mx, my) = v.from_screen(rect, pt);
                let hit = p.comps.iter().filter(|o| o.visible && o.id != c.id).find(|o| {
                    let (ca, sa) = (o.rot.to_radians().cos(), o.rot.to_radians().sin());
                    let (dx, dy) = (mx - o.x, my - o.y);
                    let (lu, lw) = (dx * ca + dy * sa, -dx * sa + dy * ca);
                    if o.shape == "round" { (lu * lu + lw * lw).sqrt() <= o.w / 2.0 } else { lu.abs() <= o.w / 2.0 && lw.abs() <= o.h / 2.0 }
                });
                if let Some(o) = hit {
                    let id = o.id;
                    self.select_part(id);
                }
            }
        }
        self.dock_zoom_pan(ui, rect, resp, 1);
    }

    fn view_mut(&mut self, ix: usize) -> &mut View2 {
        match ix {
            0 => &mut self.side,
            1 => &mut self.plan,
            _ => &mut self.elev,
        }
    }

    fn dock_zoom_pan(&mut self, ui: &egui::Ui, rect: Rect, resp: &egui::Response, ix: usize) {
        if let Some(h) = resp.hover_pos() {
            let sc = ui.input(|i| i.smooth_scroll_delta.y);
            if sc != 0.0 {
                let v = *self.view_mut(ix);
                let (wx, wy) = v.from_screen(rect, h);
                let nv = self.view_mut(ix);
                nv.s = (nv.s * (sc * 0.0015).exp()).clamp(0.02, 200.0);
                nv.ox = h.x - rect.left() - wx as f32 * nv.s;
                nv.oy = h.y - rect.top() + wy as f32 * nv.s;
            }
        }
        if matches!(self.dock_drag, DockDrag::Pan) && resp.dragged() {
            let d = resp.drag_delta();
            let v = self.view_mut(ix);
            v.ox += d.x;
            v.oy += d.y;
        }
        if resp.double_clicked() {
            self.view_mut(ix).fitted = false;
        }
        if resp.drag_stopped() {
            self.dock_drag = DockDrag::None;
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect, v: &View2) {
        let mut step = 10.0f32;
        for s in [10.0, 20.0, 50.0, 100.0, 200.0, 500.0] {
            step = s;
            if s * v.s >= 38.0 {
                break;
            }
        }
        let (x0, _) = v.from_screen(rect, rect.left_top());
        let (x1, y0) = v.from_screen(rect, rect.right_bottom());
        let (_, y1) = v.from_screen(rect, rect.left_top());
        let mut x = (x0 as f32 / step).floor() * step;
        while (x as f64) <= x1 {
            let sx = v.to_screen(rect, x as f64, 0.0).x;
            let major = ((x / step).round() as i64) % 5 == 0;
            painter.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], Stroke::new(1.0, if major { Color32::from_rgb(43, 48, 55) } else { Color32::from_rgb(30, 33, 38) }));
            if major {
                painter.text(pos2(sx + 3.0, rect.bottom() - 11.0), Align2::LEFT_CENTER, format!("{}", x.round() as i64), FontId::proportional(10.0), MUTED);
            }
            x += step;
        }
        let mut y = (y0 as f32 / step).floor() * step;
        while (y as f64) <= y1 {
            let sy = v.to_screen(rect, 0.0, y as f64).y;
            let major = ((y / step).round() as i64) % 5 == 0;
            painter.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], Stroke::new(1.0, if major { Color32::from_rgb(43, 48, 55) } else { Color32::from_rgb(30, 33, 38) }));
            if major {
                painter.text(pos2(rect.left() + 5.0, sy - 7.0), Align2::LEFT_CENTER, format!("{}", y.round() as i64), FontId::proportional(10.0), MUTED);
            }
            y += step;
        }
    }

    fn poly2(&self, painter: &egui::Painter, rect: Rect, v: &View2, pts: &[[f64; 2]], stroke: Stroke) {
        let s: Vec<Pos2> = pts.iter().map(|q| v.to_screen(rect, q[0], q[1])).collect();
        painter.add(Shape::line(s, stroke));
    }

    // ---------------------------------------------------------------- profile shape editor
    fn side_editor(&mut self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect, resp: &egui::Response) {
        if !self.side.fitted {
            let mut b = [1e9f32, -1e9, 1e9, -1e9];
            for p in &self.st.proj.profiles {
                for q in p.curve() {
                    b[0] = b[0].min(q[0] as f32);
                    b[1] = b[1].max(q[0] as f32);
                    b[2] = b[2].min(q[1] as f32);
                    b[3] = b[3].max(q[1] as f32);
                }
            }
            self.ref_extend(0, &mut b);
            let mut v = self.side;
            v.fit(rect, b, 36.0);
            self.side = v;
        }
        let v = self.side;
        self.draw_grid(painter, rect, &v);
        let cx = ui.ctx().clone();
        self.draw_ref(&cx, painter, rect, &v, 0);
        let sel = self.sel_prof;
        let partner = self.st.proj.locked_partner(sel);
        for (i, p) in self.st.proj.profiles.iter().enumerate() {
            if i != sel && Some(i) != partner {
                self.poly2(painter, rect, &v, &p.curve(), Stroke::new(1.0, Color32::from_rgb(75, 85, 99)));
            }
        }
        if let Some(pi) = partner {
            self.poly2(painter, rect, &v, &self.st.proj.profiles[pi].curve(), Stroke::new(1.3, fade(TEAL, 190)));
        }
        let pr = self.st.proj.profiles[sel].clone();
        self.poly2(painter, rect, &v, &pr.curve(), Stroke::new(2.4, ACCENT));
        for (k, q) in pr.pts.iter().enumerate() {
            let c = v.to_screen(rect, q.co[0], q.co[1]);
            if k == 0 {
                painter.add(Shape::convex_polygon(vec![c + vec2(0.0, -7.0), c + vec2(7.0, 0.0), c + vec2(0.0, 7.0), c + vec2(-7.0, 0.0)], BLUE, Stroke::new(1.5, Color32::WHITE)));
            } else {
                let in_multi = self.multi.contains(&k);
                let r = if k == self.sel_pt || in_multi { 5.5 } else { 3.5 };
                painter.rect_filled(Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), CornerRadius::same(2), if k == self.sel_pt { Color32::WHITE } else if in_multi { Color32::from_rgb(255, 217, 168) } else { ACCENT });
            }
        }
        for (ki, k) in self.st.proj.keys.iter().enumerate() {
            if let Some(q) = pr.pts.get(k.idx) {
                let c = v.to_screen(rect, q.co[0], q.co[1]);
                // labels alternate sides (right, left, right...) so neighboring key names never overlap
                if ki % 2 == 1 {
                    painter.text(c + vec2(-16.0, -10.0), Align2::RIGHT_CENTER, &k.name, FontId::proportional(12.0), Color32::from_rgb(255, 217, 168));
                } else {
                    painter.text(c + vec2(9.0, -9.0), Align2::LEFT_CENTER, &k.name, FontId::proportional(12.0), Color32::from_rgb(255, 217, 168));
                }
            }
        }
        if let Some(q) = pr.pts.get(self.sel_pt).filter(|_| self.multi.len() < 2) {
            let c = v.to_screen(rect, q.co[0], q.co[1]);
            // broken handles (independent, a corner) are drawn in teal so they can be told from smooth ones
            let hcol = if self.st.proj.is_broken(self.sel_pt) { TEAL } else { Color32::from_rgb(255, 217, 168) };
            let hs = Stroke::new(1.0, hcol);
            if self.sel_pt > 0 {
                let h = v.to_screen(rect, q.hl[0], q.hl[1]);
                painter.line_segment([h, c], hs);
                painter.circle_filled(h, 4.5, hcol);
            }
            if self.sel_pt + 1 < pr.pts.len() {
                let h = v.to_screen(rect, q.hr[0], q.hr[1]);
                painter.line_segment([c, h], hs);
                painter.circle_filled(h, 4.5, hcol);
            }
        }
        painter.text(rect.left_top() + vec2(12.0, 14.0), Align2::LEFT_CENTER, format!("P{}   x = {}   point {}", sel, crate::units::num(pr.x, 1), self.sel_pt), FontId::proportional(12.5), MUTED);

        // interaction
        if self.ref_interact(ui, resp, rect, &v, 0) {
            self.dock_zoom_pan(ui, rect, resp, 0);
            return;
        }
        if resp.drag_started() {
            self.dock_drag = DockDrag::Pan;
            if let (Some(p), true) = (resp.interact_pointer_pos(), ui.input(|i| i.pointer.button_down(PointerButton::Primary))) {
                let mut hit: Option<DockDrag> = None;
                let shift = ui.input(|i| i.modifiers.shift || i.modifiers.command);
                let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
                let near = pr.pts.iter().enumerate().skip(1).filter(|(_, q)| v.to_screen(rect, q.co[0], q.co[1]).distance(origin) < 11.0).map(|(k, _)| k).next();
                if self.multi.len() > 1 {
                    if near.map(|k| self.multi.contains(&k)).unwrap_or(false) {
                        self.op_s(r#"{"op":"checkpoint"}"#);
                        self.multi_last = Some(v.from_screen(rect, p));
                        hit = Some(DockDrag::Multi);
                    } else if near.is_none() && shift {
                        hit = Some(DockDrag::Box { a: origin });
                    } else if near.is_none() {
                        hit = Some(DockDrag::Pan);
                    } else {
                        self.multi.clear();
                    }
                } else if near.is_none() && shift {
                    hit = Some(DockDrag::Box { a: origin });
                }
                if hit.is_none() {
                if let Some(q) = pr.pts.get(self.sel_pt) {
                    if self.sel_pt > 0 && v.to_screen(rect, q.hl[0], q.hl[1]).distance(p) < 10.0 {
                        hit = Some(DockDrag::Pt { k: self.sel_pt, which: 1 });
                    } else if self.sel_pt + 1 < pr.pts.len() && v.to_screen(rect, q.hr[0], q.hr[1]).distance(p) < 10.0 {
                        hit = Some(DockDrag::Pt { k: self.sel_pt, which: 2 });
                    }
                }
                if hit.is_none() {
                    let mut best = (11.0f32, None);
                    for (k, q) in pr.pts.iter().enumerate() {
                        let d = v.to_screen(rect, q.co[0], q.co[1]).distance(p);
                        if d < best.0 {
                            best = (d, Some(k));
                        }
                    }
                    if let Some(k) = best.1 {
                        self.sel_pt = k;
                        if k > 0 {
                            hit = Some(DockDrag::Pt { k, which: 0 });
                        } else {
                            hit = Some(DockDrag::None);
                        }
                    }
                }
                }
                if let Some(h) = hit {
                    if matches!(h, DockDrag::Pt { .. }) {
                        self.op_s(r#"{"op":"checkpoint"}"#);
                    }
                    self.dock_drag = h;
                }
            }
        }
        if resp.dragged() {
            if matches!(self.dock_drag, DockDrag::Multi) {
                if let (Some(p), Some(last)) = (resp.interact_pointer_pos(), self.multi_last) {
                    let cur = v.from_screen(rect, p);
                    let ks: Vec<String> = self.multi.iter().map(|k| k.to_string()).collect();
                    self.op(format!(r#"{{"op":"move_pts","i":{},"ks":[{}],"dy":{},"dz":{}}}"#, sel, ks.join(","), cur.0 - last.0, cur.1 - last.1));
                    self.multi_last = Some(cur);
                }
            }
            if let (DockDrag::Box { a }, Some(p)) = (&self.dock_drag, resp.interact_pointer_pos()) {
                let r = Rect::from_two_pos(*a, p);
                painter.rect_filled(r, CornerRadius::ZERO, fade(ACCENT, 40));
                painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            }
            if let (DockDrag::Pt { k, which }, Some(p)) = (&self.dock_drag, resp.interact_pointer_pos()) {
                let (wy, wz) = v.from_screen(rect, p);
                let (k, which) = (*k, *which);
                self.op(format!(r#"{{"op":"edit_pt","i":{},"k":{},"which":{},"y":{},"z":{}}}"#, sel, k, which, wy, wz));
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let mut best = (11.0f32, None);
                for (k, q) in pr.pts.iter().enumerate() {
                    let d = v.to_screen(rect, q.co[0], q.co[1]).distance(p);
                    if d < best.0 {
                        best = (d, Some(k));
                    }
                }
                let shift = ui.input(|i| i.modifiers.shift || i.modifiers.command);
                match best.1 {
                    Some(k) if shift && k > 0 => {
                        if self.multi.is_empty() && self.sel_pt > 0 {
                            self.multi.push(self.sel_pt);
                        }
                        if let Some(pos) = self.multi.iter().position(|&x| x == k) {
                            self.multi.remove(pos);
                        } else {
                            self.multi.push(k);
                        }
                        self.sel_pt = k;
                    }
                    Some(k) => {
                        if !self.multi.contains(&k) {
                            self.multi.clear();
                        }
                        self.sel_pt = k;
                    }
                    None => {
                        if !shift {
                            self.multi.clear();
                        }
                    }
                }
            }
        }
        if resp.drag_stopped() {
            if let (DockDrag::Box { a }, Some(p)) = (&self.dock_drag, resp.interact_pointer_pos()) {
                let r = Rect::from_two_pos(*a, p);
                if self.multi.is_empty() && self.sel_pt > 0 {
                    self.multi.push(self.sel_pt);
                }
                for (k, q) in pr.pts.iter().enumerate().skip(1) {
                    if r.contains(v.to_screen(rect, q.co[0], q.co[1])) && !self.multi.contains(&k) {
                        self.multi.push(k);
                    }
                }
                if let Some(&k) = self.multi.last() {
                    self.sel_pt = k;
                }
            }
        }
        self.multi.retain(|&k| k < pr.pts.len());
        if self.multi.len() < 2 && !matches!(self.dock_drag, DockDrag::Box { .. }) {
            if self.multi.len() == 1 {
                self.sel_pt = self.multi[0];
            }
            self.multi.clear();
        }
        // ---- right-click menu: on a point (break / join handles, remove) or on the line (add a point there)
        let ctx_id = egui::Id::new("profile_ctx_target");
        if resp.secondary_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let pr = &self.st.proj.profiles[sel];
                let mut target: (u8, usize, f64) = (0, 0, 0.0);
                let mut best = (11.0f32, None);
                for (k, q) in pr.pts.iter().enumerate() {
                    let d = v.to_screen(rect, q.co[0], q.co[1]).distance(p);
                    if d < best.0 {
                        best = (d, Some(k));
                    }
                }
                if let Some(k) = best.1 {
                    target = (1, k, 0.0);
                    self.sel_pt = k;
                    self.multi.clear();
                } else {
                    let cv = pr.curve();
                    let mut bl = (14.0f32, None);
                    for (si, q) in cv.iter().enumerate() {
                        let d = v.to_screen(rect, q[0], q[1]).distance(p);
                        if d < bl.0 {
                            bl = (d, Some(si));
                        }
                    }
                    if let Some(si) = bl.1 {
                        let seg = (si / crate::model::K).min(pr.pts.len() - 2);
                        let t = (si - seg * crate::model::K) as f64 / crate::model::K as f64;
                        target = (2, seg, t);
                    }
                }
                ui.data_mut(|d| d.insert_temp(ctx_id, target));
            }
        }
        let mut ctx_op: Option<String> = None;
        resp.context_menu(|ui| {
            let target: (u8, usize, f64) = ui.data(|d| d.get_temp(ctx_id)).unwrap_or((0, 0, 0.0));
            let np = self.st.proj.profiles[0].pts.len();
            match target {
                (1, k, _) if k < np => {
                    ui.label(egui::RichText::new(format!("Point {}", k)).size(12.0).color(MUTED));
                    let inner = k > 0 && k + 1 < np;
                    let broken = self.st.proj.is_broken(k);
                    if ui.add_enabled(inner, egui::Button::new(if broken { "Join handles (keep a smooth line)" } else { "Break handles (move each side alone)" })).clicked() {
                        ctx_op = Some(format!(r#"{{"op":"set_broken","k":{},"on":{}}}"#, k, !broken));
                        ui.close();
                    }
                    if ui.add_enabled(inner && np > 4, egui::Button::new("Remove point")).clicked() {
                        ctx_op = Some(format!(r#"{{"op":"pt_remove","k":{}}}"#, k));
                        ui.close();
                    }
                }
                (2, seg, t) => {
                    if ui.button("Add point here").clicked() {
                        ctx_op = Some(format!(r#"{{"op":"pt_add","k":{},"t":{}}}"#, seg, t));
                        ui.close();
                    }
                }
                _ => {
                    ui.label(egui::RichText::new("Right-click a point or the line").size(12.0).color(MUTED));
                }
            }
        });
        if let Some(o) = ctx_op {
            let t: (u8, usize, f64) = ui.data(|d| d.get_temp(ctx_id)).unwrap_or((0, 0, 0.0));
            let removed = o.contains("pt_remove");
            self.op(o);
            match t {
                (2, seg, _) => self.sel_pt = seg + 1,
                (1, k, _) if removed => self.sel_pt = k.saturating_sub(1),
                _ => {}
            }
            self.clamp_sel();
        }
        if resp.double_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let pr = &self.st.proj.profiles[sel];
                let cv = pr.curve();
                let mut best = (14.0f32, None);
                for (si, q) in cv.iter().enumerate() {
                    let d = v.to_screen(rect, q[0], q[1]).distance(p);
                    if d < best.0 {
                        best = (d, Some(si));
                    }
                }
                if let Some(si) = best.1 {
                    let seg = (si / crate::model::K).min(pr.pts.len() - 2);
                    let t = (si - seg * crate::model::K) as f64 / crate::model::K as f64;
                    self.op(format!(r#"{{"op":"pt_add","k":{},"t":{}}}"#, seg, t));
                    self.sel_pt = seg + 1;
                    self.clamp_sel();
                }
            }
        }
        self.dock_zoom_pan(ui, rect, resp, 0);
    }

    // ---------------------------------------------------------------- windshield line editors
    fn rail_editor(&mut self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect, resp: &egui::Response, ix: usize) {
        let p = self.st.proj.clone();
        let (x0, x1) = (p.rail.first().unwrap()[0], p.rail.last().unwrap()[0]);
        let curve: Vec<[f64; 2]> = (0..=148)
            .map(|s| {
                let x = x0 + (x1 - x0) * s as f64 / 148.0;
                let (y, z) = p.rail_at(x);
                [x, if ix == 1 { y } else { z }]
            })
            .collect();
        let vi = ix;
        if !self.view_mut(vi).fitted {
            let mut b = [x0 as f32, x1 as f32, 1e9, -1e9];
            for c in &curve {
                b[2] = b[2].min(c[1] as f32);
                b[3] = b[3].max(c[1] as f32);
            }
            for r in &p.rail2 {
                b[2] = b[2].min(r[ix - 1] as f32);
                b[3] = b[3].max(r[ix - 1] as f32);
            }
            let m = ((b[3] - b[2]) * 0.35).max(20.0);
            b[2] -= m;
            b[3] += m;
            self.ref_extend(vi, &mut b);
            let mut v = *self.view_mut(vi);
            v.fit(rect, b, 30.0);
            *self.view_mut(vi) = v;
        }
        let v = *self.view_mut(vi);
        self.draw_grid(painter, rect, &v);
        let cx = ui.ctx().clone();
        self.draw_ref(&cx, painter, rect, &v, ix);
        // section bands
        for (k, s) in p.sections.iter().enumerate() {
            let (Some(a), Some(b)) = (p.pid_index(s.start), p.pid_index(s.end)) else { continue };
            let xa = v.to_screen(rect, p.profiles[a].x, 0.0).x;
            let xb = v.to_screen(rect, p.profiles[b].x, 0.0).x;
            let band = Rect::from_min_max(pos2(xa, rect.top()), pos2(xb, rect.bottom())).intersect(rect);
            painter.rect_filled(band, CornerRadius::ZERO, fade(hex(&s.color), if Some(k) == self.sel_sec { 60 } else { 28 }));
        }
        // x = 0 center line
        if self.show_center {
            let top = v.to_screen(rect, 0.0, 1e5);
            let bot = v.to_screen(rect, 0.0, -1e5);
            let mut y = rect.top();
            while y < rect.bottom() {
                painter.line_segment([pos2(top.x, y), pos2(top.x, (y + 6.0).min(rect.bottom()))], Stroke::new(1.2, Color32::from_rgb(255, 224, 120)));
                y += 11.0;
            }
            let _ = bot;
            painter.text(pos2(top.x + 5.0, rect.top() + 10.0), Align2::LEFT_CENTER, "center", FontId::proportional(11.0), Color32::from_rgb(255, 224, 120));
        }
        self.poly2(painter, rect, &v, &curve, Stroke::new(2.0, BLUE));
        if !p.rail2.is_empty() {
            let c2: Vec<[f64; 2]> = (0..=148)
                .map(|s| {
                    let x = x0 + (x1 - x0) * s as f64 / 148.0;
                    let (y, z) = p.rail2_at(x);
                    [x, if ix == 1 { y } else { z }]
                })
                .collect();
            self.poly2(painter, rect, &v, &c2, Stroke::new(2.0, TEAL));
            for (k, r) in p.rail2.iter().enumerate() {
                let c = v.to_screen(rect, p.rail[k][0], r[ix - 1]);
                let on = self.sel_rail2 == Some(k);
                let rr = if on { 6.5 } else { 5.0 };
                painter.rect_filled(Rect::from_center_size(c, vec2(rr * 2.0, rr * 2.0)), CornerRadius::same(2), if on { Color32::WHITE } else { TEAL });
            }
        }
        const GRIP: f32 = 26.0;
        for (i, pr) in p.profiles.iter().enumerate() {
            let a = pr.anchor();
            let c = v.to_screen(rect, pr.x, a[ix - 1]);
            let on = i == self.sel_prof;
            let col = if on { ACCENT } else { Color32::from_rgb(123, 132, 145) };
            painter.line_segment([c, c - vec2(0.0, GRIP)], Stroke::new(1.0, col));
            let g = c - vec2(0.0, GRIP);
            painter.circle_filled(g, if on { 9.0 } else { 7.5 }, if on { ACCENT } else { Color32::from_rgb(29, 32, 36) });
            painter.circle_stroke(g, if on { 9.0 } else { 7.5 }, Stroke::new(1.5, col));
            painter.text(g, Align2::CENTER_CENTER, format!("{}", i), FontId::proportional(10.0), if on { Color32::from_rgb(27, 19, 8) } else { col });
            if pr.fixed {
                painter.text(c + vec2(0.0, 16.0), Align2::CENTER_CENTER, "fixed", FontId::proportional(10.0), MUTED);
            }
        }
        for (k, r) in p.rail.iter().enumerate() {
            let c = v.to_screen(rect, r[0], r[ix]);
            let on = self.sel_rail == Some(k);
            let rr = if on { 6.5 } else { 5.0 };
            painter.rect_filled(Rect::from_center_size(c, vec2(rr * 2.0, rr * 2.0)), CornerRadius::same(2), if on { Color32::WHITE } else { BLUE });
        }

        // extra selected items (Shift-click / drag-box)
        for &(kd, k) in &self.gsel {
            let c = match kd {
                0 => p.rail.get(k).map(|r| v.to_screen(rect, r[0], r[ix])),
                1 => p.rail2.get(k).map(|r| v.to_screen(rect, p.rail[k][0], r[ix - 1])),
                _ => p.profiles.get(k).map(|pr| v.to_screen(rect, pr.x, pr.anchor()[ix - 1]) - vec2(0.0, GRIP)),
            };
            if let Some(c) = c {
                painter.circle_stroke(c, 12.0, Stroke::new(2.0, Color32::from_rgb(255, 217, 168)));
            }
        }
        if let DockDrag::GBox { a } = &self.dock_drag {
            if let Some(b) = resp.interact_pointer_pos() {
                let r = Rect::from_two_pos(*a, b);
                painter.rect_filled(r, CornerRadius::ZERO, fade(ACCENT, 30));
                painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            }
        }

        if let Some(sym) = self.addp_pick {
            // placing a new profile: click on the windshield line or the front line
            let hover = resp.hover_pos();
            let near = |hx: f32, hy: f32| -> Option<f64> {
                let (x, _) = v.from_screen(rect, pos2(hx, hy));
                if x < x0 || x > x1 {
                    return None;
                }
                let val = |(y, z): (f64, f64)| if ix == 1 { y } else { z };
                let mut d = (v.to_screen(rect, x, val(p.rail_at(x))).y - hy).abs();
                if !p.rail2.is_empty() {
                    d = d.min((v.to_screen(rect, x, val(p.rail2_at(x))).y - hy).abs());
                }
                if d < 26.0 { Some((x * 10.0).round() / 10.0) } else { None }
            };
            if let Some(h) = hover {
                if let Some(x) = near(h.x, h.y) {
                    for xx in if sym { vec![x, -x] } else { vec![x] } {
                        let val = |(y, z): (f64, f64)| if ix == 1 { y } else { z };
                        let top = v.to_screen(rect, xx, val(p.rail_at(xx)));
                        let mut ends = vec![top];
                        if !p.rail2.is_empty() {
                            ends.push(v.to_screen(rect, xx, val(p.rail2_at(xx))));
                        }
                        painter.line_segment([pos2(top.x, rect.top()), pos2(top.x, rect.bottom())], Stroke::new(1.2, fade(ACCENT, 120)));
                        for e in ends {
                            painter.circle_filled(e, 6.0, ACCENT);
                            painter.circle_stroke(e, 6.0, Stroke::new(1.5, Color32::WHITE));
                        }
                    }
                    painter.text(h + vec2(14.0, -14.0), Align2::LEFT_CENTER, format!("x = {}", crate::units::num(x, 1)), FontId::proportional(12.5), Color32::from_rgb(255, 217, 168));
                    ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                }
            }
            if resp.clicked() {
                if let Some(pt) = resp.interact_pointer_pos() {
                    match near(pt.x, pt.y) {
                        Some(x) => self.place_profile(x, sym),
                        None => self.toast("Click right on the windshield line or the front line.", false),
                    }
                }
            }
            self.dock_zoom_pan(ui, rect, resp, ix);
            return;
        }
        if self.ref_interact(ui, resp, rect, &v, ix) {
            self.dock_zoom_pan(ui, rect, resp, ix);
            return;
        }
        // what is under the pointer: windshield points, front points and profile grips. Points that sit on top of each other
        // (a short dash puts the front line right against the windshield line) are all candidates; a click steps through them.
        let cands = |pt: Pos2, skip_fixed: bool| -> Vec<(u8, usize, f32)> {
            let mut c: Vec<(u8, usize, f32)> = Vec::new();
            for (k, r) in p.rail.iter().enumerate() {
                let d = v.to_screen(rect, r[0], r[ix]).distance(pt);
                if d < 11.0 { c.push((0, k, d)); }
            }
            for (k, r) in p.rail2.iter().enumerate() {
                let d = v.to_screen(rect, p.rail[k][0], r[ix - 1]).distance(pt);
                if d < 11.0 { c.push((1, k, d)); }
            }
            if c.is_empty() {
                for (i, pr) in p.profiles.iter().enumerate() {
                    if skip_fixed && pr.fixed { continue; }
                    let d = (v.to_screen(rect, pr.x, pr.anchor()[ix - 1]) - vec2(0.0, GRIP)).distance(pt);
                    if d < 12.0 { c.push((2, i, d)); }
                }
            }
            c
        };
        if resp.drag_started() {
            self.dock_drag = DockDrag::Pan;
            if let (Some(pt), true) = (resp.interact_pointer_pos(), ui.input(|i| i.pointer.button_down(PointerButton::Primary))) {
                let mut hit = None;
                // item under the pointer, for group moves and box selection
                let sel3 = [self.sel_rail, self.sel_rail2, Some(self.sel_prof)];
                let under: Option<(u8, usize)> = pick_dock(&cands(pt, true), sel3, false);
                let shift = ui.input(|i| i.modifiers.shift);
                if let (Some(u), true) = (under, self.gsel.len() > 1 && !shift) {
                    if self.gsel.contains(&u) {
                        let (wx, wy) = v.from_screen(rect, pt);
                        let items: Vec<(u8, usize, [f64; 3])> = self
                            .gsel
                            .iter()
                            .filter_map(|&(kd, k)| match kd {
                                0 => p.rail.get(k).map(|r| (0u8, k, *r)),
                                1 => p.rail2.get(k).map(|r| (1u8, k, [0.0, r[0], r[1]])),
                                _ => p.profiles.get(k).map(|pr| (2u8, k, [pr.x, 0.0, 0.0])),
                            })
                            .collect();
                        self.op_s(r#"{"op":"checkpoint"}"#);
                        self.dock_drag = DockDrag::Group { sx: wx, sy: wy, items };
                    }
                }
                if matches!(self.dock_drag, DockDrag::Group { .. }) {
                    hit = None;
                } else if under.is_none() && (shift || !ui.input(|i| i.key_down(egui::Key::Space))) {
                    // dragging on empty space draws a selection box; hold Space (or use the middle or right button) to pan instead
                    self.dock_drag = DockDrag::GBox { a: pt };
                    hit = None;
                }
                if matches!(self.dock_drag, DockDrag::Group { .. } | DockDrag::GBox { .. }) {
                    // nothing else to start
                } else {
                match pick_dock(&cands(pt, false), sel3, false) {
                    Some((0, k)) => {
                        hit = Some(DockDrag::Rail { k });
                        self.sel_rail = Some(k);
                        self.sel_rail2 = None;
                        self.focus = Focus::Rail;
                        self.op_s(r#"{"op":"checkpoint"}"#);
                    }
                    Some((1, k)) => {
                        hit = Some(DockDrag::Rail2 { k });
                        self.sel_rail2 = Some(k);
                        self.sel_rail = None;
                        self.focus = Focus::Rail;
                        self.op_s(r#"{"op":"checkpoint"}"#);
                    }
                    Some((_, i)) => {
                        let pr = &p.profiles[i];
                        self.select_profile(i);
                        if !pr.fixed {
                            self.op_s(r#"{"op":"checkpoint"}"#);
                            hit = Some(DockDrag::Grip { i, off: v.from_screen(rect, pt).0 - pr.x });
                        } else {
                            hit = Some(DockDrag::None);
                        }
                    }
                    None => {}
                }
                if let Some(h) = hit {
                    self.dock_drag = h;
                }
                }
            }
        }
        if resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let mut done = false;
                if ui.input(|i| i.modifiers.shift) {
                    // Shift-click adds or removes one item from the group
                    let it: Option<(u8, usize)> = pick_dock(&cands(pt, true), [self.sel_rail, self.sel_rail2, Some(self.sel_prof)], true);
                    if let Some(it) = it {
                        if self.gsel.is_empty() {
                            // the item already selected joins the group
                            if let Some(k) = self.sel_rail { self.gsel.push((0, k)); }
                            if let Some(k) = self.sel_rail2 { self.gsel.push((1, k)); }
                        }
                        if let Some(pos) = self.gsel.iter().position(|x| *x == it) { self.gsel.remove(pos); } else { self.gsel.push(it); }
                        done = true;
                    }
                } else {
                    self.gsel.clear();
                }
                if !done {
                    match pick_dock(&cands(pt, false), [self.sel_rail, self.sel_rail2, Some(self.sel_prof)], true) {
                        Some((0, k)) => { self.sel_rail = Some(k); self.sel_rail2 = None; self.focus = Focus::Rail; }
                        Some((1, k)) => { self.sel_rail2 = Some(k); self.sel_rail = None; self.focus = Focus::Rail; }
                        Some((_, i)) => { self.select_profile(i); }
                        None => {}
                    }
                }
            }
        }
        if resp.dragged() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let (wx, wy) = v.from_screen(rect, pt);
                match &self.dock_drag {
                    DockDrag::Rail { .. } if self.st.active == 2 => {
                        self.toast(crate::engine::TRIM_LOCK_MSG, false);
                    }
                    DockDrag::Rail { k } => {
                        let r = p.rail[*k];
                        let (y, z) = if ix == 1 { (wy, r[2]) } else { (r[1], wy) };
                        let k = *k;
                        self.op(format!(r#"{{"op":"move_rail","k":{},"x":{},"y":{},"z":{}}}"#, k, wx, y, z));
                    }
                    DockDrag::Rail2 { k } => {
                        let k = *k;
                        let r = p.rail2[k];
                        let (y, z) = if ix == 1 { (wy, r[1]) } else { (r[0], wy) };
                        self.op(format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y, z));
                    }
                    DockDrag::Grip { i, off } => {
                        let (i, off) = (*i, *off);
                        self.op(format!(r#"{{"op":"move_profile","i":{},"x":{}}}"#, i, wx - off));
                    }
                    DockDrag::Group { sx, sy, items } => {
                        let (dx, dv) = (wx - *sx, wy - *sy);
                        let sym = p.rail_sym;
                        let in_group = |kd: u8, k: usize| items.iter().any(|it| it.0 == kd && it.1 == k);
                        let mut cmds: Vec<String> = Vec::new();
                        for &(kd, k, o) in items.iter() {
                            if self.st.active == 2 && kd == 0 {
                                continue;
                            }
                            match kd {
                                0 => {
                                    if sym && p.rail_partner(k).map_or(false, |j| j < k && in_group(0, j)) { continue; }
                                    let (y, z) = if ix == 1 { (o[1] + dv, o[2]) } else { (o[1], o[2] + dv) };
                                    cmds.push(format!(r#"{{"op":"move_rail","k":{},"x":{},"y":{},"z":{}}}"#, k, o[0] + dx, y, z));
                                }
                                1 => {
                                    if sym && p.rail_partner(k).map_or(false, |j| j < k && in_group(1, j)) { continue; }
                                    let (y, z) = if ix == 1 { (o[1] + dv, o[2]) } else { (o[1], o[2] + dv) };
                                    cmds.push(format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y, z));
                                }
                                _ => {
                                    if p.locked_partner(k).map_or(false, |j| j < k && in_group(2, j)) { continue; }
                                    cmds.push(format!(r#"{{"op":"move_profile","i":{},"x":{}}}"#, k, o[0] + dx));
                                }
                            }
                        }
                        for c in cmds {
                            self.op(c);
                        }
                    }
                    _ => {}
                }
            }
        }
        if resp.drag_stopped() {
            if let DockDrag::GBox { a } = &self.dock_drag {
                if let Some(b) = resp.interact_pointer_pos() {
                    let r = Rect::from_two_pos(*a, b);
                    let mut sel: Vec<(u8, usize)> = Vec::new();
                    for (k, q) in p.rail.iter().enumerate() {
                        if r.contains(v.to_screen(rect, q[0], q[ix])) { sel.push((0, k)); }
                    }
                    for (k, q) in p.rail2.iter().enumerate() {
                        if r.contains(v.to_screen(rect, p.rail[k][0], q[ix - 1])) { sel.push((1, k)); }
                    }
                    for (i, pr) in p.profiles.iter().enumerate() {
                        if !pr.fixed && r.contains(v.to_screen(rect, pr.x, pr.anchor()[ix - 1]) - vec2(0.0, GRIP)) { sel.push((2, i)); }
                    }
                    if ui.input(|i| i.modifiers.shift) {
                        for s in sel { if !self.gsel.contains(&s) { self.gsel.push(s); } }
                    } else {
                        self.gsel = sel;
                    }
                }
            }
            if matches!(self.dock_drag, DockDrag::GBox { .. } | DockDrag::Group { .. }) { self.dock_drag = DockDrag::None; }
        }
        self.dock_zoom_pan(ui, rect, resp, vi);
    }
}

/// Where a view ray meets the horizontal plane z = `z`.
fn plane_z(o: [f32; 3], d: [f32; 3], z: f64) -> Option<[f64; 2]> {
    if d[2].abs() < 1e-5 {
        return None;
    }
    let t = (z - o[2] as f64) / d[2] as f64;
    if t <= 0.0 {
        return None;
    }
    Some([o[0] as f64 + d[0] as f64 * t, o[1] as f64 + d[1] as f64 * t])
}

/// Which of the things under the pointer in a guidelines editor to take. `c` holds (kind, index, distance) for kind 0 windshield
/// point, 1 front point, 2 profile grip; `sel` is what is selected now for each kind. Candidates within a few pixels of the nearest
/// count as stacked on one another. A click (`cycle`) steps to the one that is not selected, so repeated clicks go round them;
/// a drag takes the selected one if it is among them, so a point picked by clicking can then be moved.
fn pick_dock(c: &[(u8, usize, f32)], sel: [Option<usize>; 3], cycle: bool) -> Option<(u8, usize)> {
    let best = c.iter().map(|x| x.2).fold(f32::MAX, f32::min);
    if best == f32::MAX {
        return None;
    }
    let mut group: Vec<(u8, usize, f32)> = c.iter().filter(|x| x.2 <= best + 4.0).copied().collect();
    group.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    let is_sel = |x: &&(u8, usize, f32)| sel[x.0 as usize] == Some(x.1);
    let chosen = if cycle { group.iter().find(|x| !is_sel(x)).or(group.first()) } else { group.iter().find(is_sel).or(group.first()) };
    chosen.map(|x| (x.0, x.1))
}

#[cfg(test)]
mod pick_tests {
    use super::pick_dock;
    #[test]
    fn stacked_points_can_all_be_picked() {
        // a windshield point and a front point 2 px apart at the same station
        let c = [(0u8, 3usize, 3.0f32), (1u8, 3usize, 5.0f32)];
        // nothing selected: the nearer one; clicking again goes to the other, and again back
        assert_eq!(pick_dock(&c, [None, None, None], true), Some((0, 3)));
        assert_eq!(pick_dock(&c, [Some(3), None, None], true), Some((1, 3)));
        assert_eq!(pick_dock(&c, [None, Some(3), None], true), Some((0, 3)));
        // a drag takes the selected one, so a point chosen by clicking can be moved
        assert_eq!(pick_dock(&c, [None, Some(3), None], false), Some((1, 3)));
        assert_eq!(pick_dock(&c, [Some(3), None, None], false), Some((0, 3)));
        // clearly apart: the nearer one always wins
        let far = [(0u8, 3usize, 2.0f32), (1u8, 3usize, 9.0f32)];
        assert_eq!(pick_dock(&far, [Some(3), None, None], true), Some((0, 3)));
        assert_eq!(pick_dock(&[], [None, None, None], true), None);
    }
}
