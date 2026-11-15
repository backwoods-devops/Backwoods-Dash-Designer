// Shapes tab (library of basic shapes + my shapes) and the shape editor window.
use crate::app::*;
use crate::refimg::{RefImage, Stage};
use crate::shapes::{self, Kind, Shape as Sh};
use crate::theme::*;
use crate::units::MmExt;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};

#[derive(Clone, Copy, PartialEq)]
pub enum EdDrag {
    None,
    Pan,
    Handle { hx: i8, hy: i8, cx: f64, cy: f64, w: f64, h: f64 },
    Vert(usize),
    /// A Bezier handle of a smooth vertex of a free loop.
    VH { i: usize, out: bool },
    /// The center grip of an added piece or hole, with where it was grabbed.
    Part { i: usize, ox: f64, oy: f64 },
    /// A size handle of the selected piece (which side or corner, -1 / 0 / 1 across and up).
    PartSize { i: usize, hx: i8, hy: i8 },
}

pub struct ShapeEd {
    pub shape: Sh,
    pub original: Sh,
    pub orig_id: Option<u32>,
    pub cx: f64,
    pub cy: f64,
    pub scale: f32,
    pub pan: egui::Vec2,
    pub fitted: bool,
    pub snap: f64,
    pub drag: EdDrag,
    pub sel: Option<usize>,
    pub drawing: bool,
    pub draw_pts: Vec<[f64; 2]>,
    pub msg: String,
    /// What was under the pointer when the right-click menu was opened.
    pub ctx: CtxTarget,
    pub show_solid: bool,
    /// Set when the editor is changing the outline of a shape already placed on the dash.
    pub cut_id: Option<u32>,
    pub view3d: bool,
    pub rot3: [[f64; 3]; 3],
    pub pan3: egui::Vec2,
    pub zoom3: f32,
    pub export_req: bool,
    /// The "name this shape" pop-up is open.
    pub ask_name: bool,
    /// Drawing area in mm (0 = none) and whether the size pop-up is open.
    pub canvas: [f64; 2],
    pub canvas_ask: bool,
    pub measure: bool,
    pub mpts: Vec<[f64; 2]>,
    pub trace: Option<RefImage>,
    pub trace_rx: Option<std::sync::mpsc::Receiver<Option<String>>>,
    pub zoom_cmd: i8,
    /// The piece (added shape or hole) being edited in the Combine shapes card.
    pub part_sel: Option<usize>,
    /// Pen helpers while drawing a loop: lock lines to angle steps, snap to other points' lines, show angles.
    pub angle_lock: bool,
    pub angle_step: f64,
    pub align_guides: bool,
    pub show_angles: bool,
    /// Set by the "save or discard?" prompt: save the shape and close, or close without saving, then (optionally) show the Scene tab.
    pub leave_save: bool,
    pub leave_discard: bool,
    pub leave_scene: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CtxTarget {
    Empty,
    Vertex(usize),
    Edge(usize, [f64; 2]),
}

impl ShapeEd {
    pub fn new(shape: Sh, orig_id: Option<u32>) -> ShapeEd {
        ShapeEd { original: shape.clone(), shape, orig_id, cx: 0.0, cy: 0.0, scale: 3.0, pan: vec2(0.0, 0.0), fitted: false, snap: 0.0, drag: EdDrag::None, sel: None, drawing: false, draw_pts: Vec::new(), msg: String::new(), ctx: CtxTarget::Empty, show_solid: true, cut_id: None, view3d: false, rot3: view_preset(0), pan3: vec2(0.0, 0.0), zoom3: 1.0, export_req: false, ask_name: false, canvas: [0.0, 0.0], canvas_ask: false, measure: false, mpts: Vec::new(), trace: None, trace_rx: None, zoom_cmd: 0, part_sel: None, angle_lock: false, angle_step: 15.0, align_guides: true, show_angles: true, leave_save: false, leave_discard: false, leave_scene: false }
    }
    pub fn drawing_new() -> ShapeEd {
        let mut e = ShapeEd::new(Sh::new("", Kind::Free, 50.0, 50.0), None);
        e.drawing = true;
        e.canvas = [150.0, 150.0];
        e.canvas_ask = true;
        e
    }
}

/// Ask the operating system for an STL file without blocking the editor.
fn pick_stl() -> std::sync::mpsc::Receiver<Option<String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = if cfg!(target_os = "macos") {
            std::process::Command::new("osascript").args(["-e", "POSIX path of (choose file with prompt \"Choose a part (STL) or drawing (DXF, AI, EPS)\" of type {\"stl\", \"dxf\", \"ai\", \"eps\"})"]).output().ok()
        } else if cfg!(target_os = "windows") {
            crate::file::windows_dialog(false, "Choose a part (STL) or drawing (DXF, AI, EPS)", "Parts and drawings|*.stl;*.dxf;*.ai;*.eps|All files|*.*", "")
        } else {
            std::process::Command::new("zenity").args(["--file-selection", "--title=Choose an STL part", "--file-filter=Parts and drawings | *.stl *.STL *.dxf *.DXF *.ai *.AI *.eps *.EPS"]).output().ok().or_else(|| std::process::Command::new("kdialog").args(["--getopenfilename", ".", "*.stl"]).output().ok())
        };
        let path = out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
        let _ = tx.send(path);
    });
    rx
}

fn thumb(painter: &egui::Painter, rect: Rect, s: &Sh, fill: Color32, line: Color32) {
    // an extruded shape shows its solid (3D view) as the icon
    if s.depth > 0.0 {
        draw_solid3d(&painter.with_clip_rect(rect.expand(1.0)), rect, s, &view_preset(0), vec2(0.0, 0.0), 1.9);
        return;
    }
    let o = s.outline();
    if o.len() < 3 {
        return;
    }
    let k = (rect.width() / s.w as f32).min(rect.height() / s.h as f32);
    let c = rect.center();
    let pt = |p: [f64; 2]| pos2(c.x + p[0] as f32 * k, c.y - p[1] as f32 * k);
    let mut mesh = egui::Mesh::default();
    for p in &o {
        mesh.colored_vertex(pt(*p), fill);
    }
    for t in shapes::triangulate(&o) {
        mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
    }
    painter.add(egui::Shape::mesh(mesh));
    let mut pts: Vec<Pos2> = o.iter().map(|p| pt(*p)).collect();
    pts.push(pts[0]);
    painter.add(egui::Shape::line(pts, Stroke::new(1.3, line)));
    // added pieces and holes as outlines
    for p in &s.parts {
        let po = p.outline();
        if po.len() < 3 {
            continue;
        }
        let mut q: Vec<Pos2> = po.iter().map(|v| pt(*v)).collect();
        q.push(q[0]);
        painter.add(egui::Shape::line(q, Stroke::new(1.1, if p.hole { DANGER } else { line })));
    }
}

fn fade(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn grid_step(scale: f32) -> f64 {
    if crate::units::inch() {
        // nice inch steps, returned in mm
        for s in [0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0] {
            if (s * crate::units::MM_PER_IN) as f32 * scale >= 18.0 {
                return s * crate::units::MM_PER_IN;
            }
        }
        return 40.0 * crate::units::MM_PER_IN;
    }
    for s in [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0] {
        if s as f32 * scale >= 18.0 {
            return s;
        }
    }
    1000.0
}

impl App {
    fn save_shapes(&mut self) {
        if let Err(e) = shapes::save(&self.st.dir, &self.shapes, self.shape_next) {
            self.toast(&format!("Could not save shapes: {}", e), true);
        }
    }

    pub fn shapes_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Your library of parts and shapes. Import a 3D part (STL) to cut out of the dash or add to it, or an outline (DXF, AI, EPS) or a basic or drawn shape to cut, recess or raise. Click a basic shape to open a copy in the editor.").size(12.0).color(MUTED));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if primary_button(ui, "Draw a closed loop").on_hover_text("Click to place vertices, click the first one to close the loop").clicked() {
                self.shape_ed = Some(ShapeEd::drawing_new());
            }
            if ui.button("Import…").on_hover_text("STL = a 3D part to cut out of or add to the dash. DXF, AI or EPS = a flat outline shape (closed paths).").clicked() && self.stl_pick.is_none() {
                self.stl_pick = Some(pick_stl());
            }
        });
        if let Some(rx) = &self.stl_pick {
            match rx.try_recv() {
                Ok(Some(path)) => {
                    self.stl_pick = None;
                    let ext = std::path::Path::new(&path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                    if ext != "stl" {
                        match crate::import::import_drawing(&path) {
                            Ok(list) => {
                                let n = list.len();
                                for mut s in list {
                                    s.id = self.shape_next;
                                    self.shape_next += 1;
                                    self.shapes.push(s);
                                }
                                self.save_shapes();
                                self.toast(&format!("Imported {} flat shape{} (extrude them in the editor if you want a 3D part)", n, if n == 1 { "" } else { "s" }), false);
                            }
                            Err(e) => self.toast(&format!("Could not import: {}", e), true),
                        }
                    } else {
                    match shapes::import_stl(&path) {
                        Ok((name, file)) => {
                            if let Some(mut s) = shapes::mesh_shape(&name, &file) {
                                s.id = self.shape_next;
                                self.shape_next += 1;
                                let chk = shapes::mesh_data(&s.mesh).map(|m| crate::cuts::TriMesh { ps: m.pos.clone(), tris: m.tris.clone() }.check());
                                match chk {
                                    Some(Err(e)) => self.toast(&format!("Imported {} but it is not a clean closed solid ({}). Cutting with it may fail; re-export it as a watertight STL.", s.name, e), true),
                                    _ => self.toast(&format!("Imported {} ({} x {} x {})", s.name, crate::units::num(s.w, 1), crate::units::num(s.h, 1), crate::units::len(s.depth, 1)), false),
                                }
                                self.shapes.push(s);
                                self.save_shapes();
                            }
                        }
                        Err(e) => self.toast(&format!("Could not import: {}", e), true),
                    }
                    }
                }
                Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => self.stl_pick = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                }
            }
        }
        label_caps(ui, "Basic shapes");
        let mut open: Option<Sh> = None;
        let mut start_drag: Option<Sh> = None;
        let mut place: Option<u32> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for s in shapes::presets() {
                let (rect, resp) = ui.allocate_exact_size(vec2(76.0, 84.0), Sense::click_and_drag());
                let hot = resp.hovered();
                let p = ui.painter();
                p.rect_filled(rect, CornerRadius::same(8), if hot { CARD_HI } else { CARD });
                p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, if hot { ACCENT } else { LINE }), StrokeKind::Inside);
                thumb(p, Rect::from_min_size(rect.min + vec2(10.0, 8.0), vec2(56.0, 46.0)), &s, fade(ACCENT, 70), ACCENT);
                p.text(pos2(rect.center().x, rect.bottom() - 16.0), Align2::CENTER_CENTER, &s.name, FontId::proportional(11.5), TEXT);
                p.text(pos2(rect.center().x, rect.bottom() - 5.0), Align2::CENTER_CENTER, format!("{:.0} x {:.0}", s.w, s.h), FontId::proportional(10.0), MUTED);
                if resp.drag_started() && self.shape_ed.is_none() {
                    start_drag = Some(s.clone());
                }
                if resp.on_hover_text("Click to open a copy in the editor. Drag onto the dash to place it.").clicked() {
                    open = Some(s.clone());
                }
            }
        });
        label_caps(ui, "My library");
        if self.shapes.is_empty() {
            ui.label(egui::RichText::new("Nothing saved yet. Open a basic shape, adjust it and press Save to my library.").size(12.5).color(MUTED));
        }
        let mut turn: Option<(u32, u8)> = None;
        let mut edit: Option<u32> = None;
        let mut dup: Option<u32> = None;
        let mut del: Option<u32> = None;
        let mut rename: Option<(u32, String)> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for s in self.shapes.clone() {
                let (rect, resp) = ui.allocate_exact_size(vec2(76.0, 84.0), Sense::click_and_drag());
                let hot = resp.hovered();
                let pt = ui.painter();
                pt.rect_filled(rect, CornerRadius::same(8), if hot { CARD_HI } else { CARD });
                pt.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, if hot { TEAL } else { LINE }), StrokeKind::Inside);
                thumb(pt, Rect::from_min_size(rect.min + vec2(10.0, 8.0), vec2(56.0, 46.0)), &s, fade(TEAL, 70), TEAL);
                let nm: String = if s.name.chars().count() > 11 { format!("{}…", s.name.chars().take(10).collect::<String>()) } else { s.name.clone() };
                pt.text(pos2(rect.center().x, rect.bottom() - 16.0), Align2::CENTER_CENTER, nm, FontId::proportional(11.5), TEXT);
                pt.text(pos2(rect.center().x, rect.bottom() - 5.0), Align2::CENTER_CENTER, format!("{:.0} x {:.0}", s.w, s.h), FontId::proportional(10.0), MUTED);
                if resp.drag_started() && self.shape_ed.is_none() {
                    start_drag = Some(s.clone());
                }
                if resp.double_clicked() {
                    place = Some(s.id);
                }
                let resp = resp.on_hover_text(format!("{} - {} - {} x {}{}\nDrag onto the dash, double-click to place it, right-click for more", s.name, s.kind.label(), crate::units::num(s.w, 0), crate::units::len(s.h, 0), if s.depth > 0.0 { format!(" x {} deep", crate::units::num(s.depth, 0)) } else { String::new() }));
                resp.context_menu(|ui| {
                    ui.label(egui::RichText::new(&s.name).strong());
                    ui.separator();
                    if ui.button("Place on the dash").clicked() {
                        place = Some(s.id);
                        ui.close();
                    }
                    if s.mesh.is_empty() && ui.button("Edit").clicked() {
                        edit = Some(s.id);
                        ui.close();
                    }
                    if ui.button("Rename…").clicked() {
                        rename = Some((s.id, s.name.clone()));
                        ui.close();
                    }
                    if ui.button("Copy").clicked() {
                        dup = Some(s.id);
                        ui.close();
                    }
                    if !s.mesh.is_empty() {
                        ui.separator();
                        for (i, l, tip) in [(0u8, "Turn X 90", "Roll it 90 degrees about the left-right axis (Y-up files)"), (1, "Turn Y 90", "Roll it 90 degrees about the front-back axis"), (2, "Flip upside down", "Turn it upside down")] {
                            if ui.button(l).on_hover_text(tip).clicked() {
                                turn = Some((s.id, i));
                                ui.close();
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Delete").clicked() {
                        del = Some(s.id);
                        ui.close();
                    }
                });
            }
        });
        if let Some(s) = start_drag {
            self.drag_shape = Some(s);
        }
        if let Some(id) = place {
            if let Some(s) = self.shapes.iter().find(|s| s.id == id).cloned() {
                self.place_shape_at_view_centre(&s);
            }
        }
        if let Some(id) = edit {
            if let Some(s) = self.shapes.iter().find(|s| s.id == id) {
                self.shape_ed = Some(ShapeEd::new(s.clone(), Some(id)));
            }
        }
        if let Some((id, how)) = turn {
            if let Some(i) = self.shapes.iter().position(|s| s.id == id) {
                let file = self.shapes[i].mesh.clone();
                match shapes::turn_mesh(&file, how) {
                    Ok(()) => {
                        let (nm, sid) = (self.shapes[i].name.clone(), self.shapes[i].id);
                        if let Some(mut n) = shapes::mesh_shape(&nm, &file) {
                            n.id = sid;
                            self.shapes[i] = n;
                            self.save_shapes();
                        }
                    }
                    Err(e) => self.toast(&format!("Could not turn it: {}", e), true),
                }
            }
        }
        if let Some(id) = dup {
            if let Some(s) = self.shapes.iter().find(|s| s.id == id).cloned() {
                let mut n = s;
                n.id = self.shape_next;
                self.shape_next += 1;
                n.name = format!("{} copy", n.name);
                self.shapes.push(n);
                self.save_shapes();
            }
        }
        if let Some(id) = del {
            self.shapes.retain(|s| s.id != id);
            self.save_shapes();
        }
        if rename.is_some() {
            self.shape_rename = rename;
        }
        if let Some((id, mut name)) = self.shape_rename.take() {
            let (mut ok, mut cancel) = (false, false);
            egui::Window::new("Rename shape").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(240.0));
                if !r.has_focus() && !ok {
                    r.request_focus();
                }
                let named = !name.trim().is_empty();
                if named && r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    ok = true;
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(named, egui::Button::new("Rename")).clicked() {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
            if ok {
                if let Some(sh) = self.shapes.iter_mut().find(|s| s.id == id) {
                    sh.name = name.trim().chars().take(40).collect();
                }
                self.save_shapes();
            } else if !cancel {
                self.shape_rename = Some((id, name));
            }
        }
        if let Some(s) = open {
            self.shape_ed = Some(ShapeEd::new(s, None));
        }
    }

    /// The editing canvas fills the main viewport area while a shape is open.
    pub fn shape_editor_center(&mut self, ui: &mut egui::Ui) {
        if let Some(ed) = self.shape_ed.as_mut() {
            Self::shape_canvas(ui, ed);
        }
    }

    /// Properties of the open shape in the right-hand panel.
    pub fn shape_editor_right(&mut self, ui: &mut egui::Ui) {
        let Some(mut ed) = self.shape_ed.take() else {
            return;
        };
        let mut close = false;
        let mut save_new = false;
        let mut save_upd = false;
        let mut done_save = false;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(if ed.drawing { "Draw a loop" } else { "Shape editor" }).size(18.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let fresh = ed.orig_id.is_none() && ed.cut_id.is_none() && !ed.drawing && ed.shape.kind == Kind::Free;
                if ui.add(egui::Button::new(egui::RichText::new("Discard").size(15.0)).min_size(vec2(84.0, 34.0))).on_hover_text("Close the editor without keeping changes").clicked() {
                    close = true;
                }
                let done_tip = if ed.drawing { "Finish the outline: closes the loop when it has 3 or more points" } else if fresh { "Save this loop to My library (it needs a name) and close" } else { "Close the editor and go back to the dash (unsaved changes are dropped)" };
                let done_btn = egui::Button::new(egui::RichText::new("Done").size(16.0).strong().color(Color32::BLACK)).fill(ACCENT).min_size(vec2(96.0, 34.0));
                if ui.add(done_btn).on_hover_text(done_tip).clicked() {
                    if ed.drawing {
                        if ed.draw_pts.len() >= 3 {
                            let pts = std::mem::take(&mut ed.draw_pts);
                            let mut sh = Sh::new(&ed.shape.name, Kind::Free, 50.0, 50.0);
                            sh.pts = pts;
                            let c = sh.sync_free();
                            ed.cx = c[0];
                            ed.cy = c[1];
                            ed.shape = sh;
                            ed.original = ed.shape.clone();
                            ed.drawing = false;
                            ed.fitted = false;
                        } else {
                            ed.msg = "Place at least 3 points, or press Discard to cancel".into();
                        }
                    } else
                    if fresh {
                        if ed.shape.name.trim().is_empty() {
                            ed.ask_name = true;
                        } else {
                            save_new = true;
                            done_save = true;
                        }
                    } else {
                        close = true;
                    }
                }
            });
        });
        ui.add_space(4.0);
        if !ed.msg.is_empty() && !ed.drawing {
            ui.label(egui::RichText::new(ed.msg.clone()).size(13.0).strong().color(ACCENT));
        }
        if ed.orig_id.is_none() && ed.cut_id.is_none() && !ed.drawing && ed.shape.kind == Kind::Free {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut ed.shape.name).hint_text("Name it, then press Done").desired_width(200.0));
            });
        }
        let keep_h = if ed.drawing { 0.0 } else { 52.0 };
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height((ui.available_height() - keep_h).max(120.0)).show(ui, |ui| {
            Self::shape_params(ui, &mut ed, &mut save_new, &mut save_upd);
        });
        let place_now = false;
        let mut apply_cut = false;
        if std::mem::take(&mut ed.leave_discard) {
            close = true;
        }
        if std::mem::take(&mut ed.leave_save) {
            if ed.drawing && ed.draw_pts.len() >= 3 {
                let pts = std::mem::take(&mut ed.draw_pts);
                let mut sh = Sh::new(&ed.shape.name, Kind::Free, 50.0, 50.0);
                sh.pts = pts;
                let c = sh.sync_free();
                ed.cx = c[0];
                ed.cy = c[1];
                ed.shape = sh;
                ed.original = ed.shape.clone();
                ed.drawing = false;
                ed.fitted = false;
            }
            if ed.drawing {
                ed.msg = "Place at least 3 points, or press Discard to cancel".into();
                ed.leave_scene = false;
            } else if ed.cut_id.is_some() {
                apply_cut = true;
            } else if ed.orig_id.is_some() {
                save_upd = true;
                done_save = true;
            } else if ed.shape.name.trim().is_empty() {
                ed.ask_name = true;
            } else {
                save_new = true;
                done_save = true;
            }
        }
        if !ed.drawing {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ed.cut_id.is_some() {
                    if primary_button(ui, "Apply to the dash").on_hover_text("Use this outline for the shape already placed on the dash").clicked() {
                        apply_cut = true;
                    }
                } else {
                    // the editor only makes shapes: name it and save it to My library, then place it on the dash from the list
                    ui.add(egui::Label::new(egui::RichText::new("Name it and press Save to my library. To use it on the dash, drag it from My library.").size(12.0).color(MUTED)).wrap());
                }
            });
        }
        if apply_cut {
            if let Some(cid) = ed.cut_id {
                if let Some(mut c) = self.st.proj.cuts.iter().find(|c| c.id == cid).cloned() {
                    let sh = ed.shape.clone();
                    c.name = sh.name.clone();
                    c.shape = sh;
                    self.cut_put(&c, false);
                }
            }
            close = true;
        }
        if place_now {
            let sh = ed.shape.clone();
            self.shape_ed = None;
            self.place_shape_at_view_centre(&sh);
            return;
        }
        if std::mem::take(&mut ed.export_req) {
            ed.msg = match export_solid(&ed.shape, &self.st.dir) {
                Ok(p) => format!("Saved {}", p),
                Err(e) => format!("Could not save: {}", e),
            };
        }
        if ed.canvas_ask {
            let mut go = false;
            egui::Window::new("Drawing canvas").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
                ui.label("How big is the thing you are drawing? Pick a size (you can change it any time).");
                ui.add_space(6.0);
                let presets: [(&str, f64, f64); 6] = [("Small  60 x 60", 60.0, 60.0), ("Switch plate  120 x 80", 120.0, 80.0), ("Medium  150 x 150", 150.0, 150.0), ("Large  300 x 300", 300.0, 300.0), ("Wide  400 x 150", 400.0, 150.0), ("Dash  700 x 300", 700.0, 300.0)];
                ui.horizontal_wrapped(|ui| {
                    for (name, w, h) in presets {
                        if pill_button(ui, name, ed.canvas == [w, h]).clicked() {
                            ed.canvas = [w, h];
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Custom");
                    let (mut w, mut h) = (ed.canvas[0], ed.canvas[1]);
                    ui.add(egui::DragValue::new(&mut w).range(10.0..=3000.0).speed(1.0).mm());
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut h).range(10.0..=3000.0).speed(1.0).mm());
                    ed.canvas = [w, h];
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Start drawing").clicked() {
                        go = true;
                    }
                    if ui.button("Load a tracing image…").clicked() && ed.trace_rx.is_none() {
                        ed.trace_rx = Some(crate::refimg::pick_file());
                        go = true;
                    }
                });
            });
            if go {
                ed.canvas_ask = false;
                ed.fitted = false;
            }
        }
        if ed.ask_name {
            let mut ok = false;
            let mut cancel = false;
            egui::Window::new("Name this shape").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
                ui.label("Give it a name to save it to My library:");
                let r = ui.add(egui::TextEdit::singleline(&mut ed.shape.name).desired_width(260.0));
                if !r.has_focus() && !ok {
                    r.request_focus();
                }
                let named = !ed.shape.name.trim().is_empty();
                if named && r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    ok = true;
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(named, egui::Button::new("Save")).clicked() {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
            if ok {
                ed.ask_name = false;
                save_new = true;
                done_save = true;
            }
            if cancel {
                ed.ask_name = false;
                ed.leave_scene = false;
            }
        }
        if save_new || save_upd {
            let name = ed.shape.name.trim().to_string();
            if name.is_empty() {
                ed.msg = "Type a name for the shape first".into();
                ed.ask_name = true;
            } else if ed.shape.kind == Kind::Free && ed.shape.pts.len() < 3 {
                ed.msg = "A loop needs at least 3 vertices".into();
            } else {
                let mut s = ed.shape.clone();
                s.name = name;
                if save_upd && ed.orig_id.is_some() {
                    let id = ed.orig_id.unwrap();
                    s.id = id;
                    if let Some(slot) = self.shapes.iter_mut().find(|x| x.id == id) {
                        *slot = s.clone();
                    }
                    ed.msg = "Updated".into();
                } else {
                    s.id = self.shape_next;
                    self.shape_next += 1;
                    ed.orig_id = Some(s.id);
                    self.shapes.push(s.clone());
                    ed.msg = "Saved to My library".into();
                }
                ed.original = s;
                self.save_shapes();
                if done_save {
                    close = true;
                }
            }
        }
        if close {
            if ed.leave_scene {
                self.left_tab = LeftTab::Scene;
            }
        } else {
            self.shape_ed = Some(ed);
        }
    }

    /// Something outside the shape editor was clicked while it is open. When the editor has nothing in it yet (the canvas
    /// size question, or a drawing with no points) it just closes and this returns false. Otherwise the "save or
    /// discard?" prompt is raised and this returns true: the click is not carried out.
    pub fn leave_click(&mut self, scene: bool) -> bool {
        let Some(ed) = self.shape_ed.as_ref() else {
            return false;
        };
        let idle = ed.orig_id.is_none() && ed.cut_id.is_none() && (ed.canvas_ask || (ed.drawing && ed.draw_pts.is_empty()));
        if idle {
            self.shape_ed = None;
            return false;
        }
        self.leave_ask = true;
        self.leave_scene_req = scene;
        true
    }

    /// "Save this shape first?" prompt, shown when something outside the editor was clicked while it is open.
    pub fn leave_window(&mut self, ctx: &egui::Context) {
        if !self.leave_ask {
            return;
        }
        let Some(ed) = self.shape_ed.as_ref() else {
            self.leave_ask = false;
            return;
        };
        let too_few = ed.drawing && ed.draw_pts.len() < 3;
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Shape editor is open").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label("You are still working on a shape, and the dash view stays hidden until you are done with it.");
            ui.add_space(4.0);
            ui.label(egui::RichText::new(if too_few { "The loop has fewer than 3 points, so it cannot be saved yet." } else { "Save it first, or discard what you did in the editor." }).size(12.5).color(MUTED));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!too_few, egui::Button::new(egui::RichText::new("Save and continue").strong().color(Color32::BLACK)).fill(ACCENT).min_size(vec2(130.0, 30.0))).on_hover_text("Keep the shape (you may be asked to name it), close the editor and go on").clicked() {
                    save = true;
                }
                if ui.add(egui::Button::new("Discard").min_size(vec2(84.0, 30.0))).on_hover_text("Close the editor and drop what has not been saved").clicked() {
                    discard = true;
                }
                if ui.add(egui::Button::new("Cancel").min_size(vec2(84.0, 30.0))).on_hover_text("Go back to the editor").clicked() {
                    cancel = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        let scene = self.leave_scene_req;
        if let Some(ed) = self.shape_ed.as_mut() {
            if save {
                ed.leave_save = true;
                ed.leave_scene = scene;
            }
            if discard {
                ed.leave_discard = true;
                ed.leave_scene = scene;
            }
        }
        if save || discard || cancel {
            self.leave_ask = false;
            self.leave_scene_req = false;
        }
    }

    fn shape_canvas(ui: &mut egui::Ui, ed: &mut ShapeEd) {
        let rect = Self::shape_canvas_body(ui, ed);
        canvas_overlays(ui, rect, ed);
    }

    fn shape_canvas_body(ui: &mut egui::Ui, ed: &mut ShapeEd) -> Rect {
        let avail = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(vec2(avail.x.max(300.0), avail.y.max(240.0)), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(8), BG);
        if ed.shape.depth > 0.0 && !ed.drawing {
            let r = Rect::from_min_size(rect.right_top() + vec2(-150.0, 40.0), vec2(120.0, 26.0));
            let id = ui.id().with("view3d_btn");
            let bresp = ui.interact(r, id, Sense::click());
            let hot = ed.view3d;
            painter.rect_filled(r, CornerRadius::same(13), if hot { ACCENT } else if bresp.hovered() { CARD_HI } else { CARD });
            painter.text(r.center(), Align2::CENTER_CENTER, if hot { "Back to editing" } else { "Rotate in 3D" }, FontId::proportional(12.5), if hot { BG } else { TEXT });
            if bresp.clicked() {
                ed.view3d = !ed.view3d;
            }
        } else {
            ed.view3d = false;
        }
        if ed.view3d {
            if resp.dragged() {
                let d = resp.drag_delta();
                if ui.input(|i| i.modifiers.shift || i.key_down(egui::Key::Space)) || ui.input(|i| i.pointer.middle_down()) {
                    ed.pan3 += d;
                } else {
                    // free tumble about the screen's own axes, no limits
                    let r = mat_mul(&rot_x(d.y as f64 * 0.01), &mat_mul(&rot_y(d.x as f64 * 0.01), &ed.rot3));
                    ed.rot3 = r;
                }
            }
            if resp.hovered() {
                let sc = ui.input(|i| i.smooth_scroll_delta.y);
                if sc != 0.0 {
                    ed.zoom3 = (ed.zoom3 * (sc * 0.0015).exp()).clamp(0.3, 8.0);
                }
            }
            draw_solid3d(&painter, rect, &ed.shape, &ed.rot3, ed.pan3, ed.zoom3);
            // view presets, like the main views
            let mut x = rect.left() + 10.0;
            for (k, name) in ["3D", "Top", "Front", "Side", "Bottom"].iter().enumerate() {
                let r = Rect::from_min_size(pos2(x, rect.top() + 36.0), vec2(if name.len() > 3 { 54.0 } else { 40.0 }, 24.0));
                let b = ui.interact(r, ui.id().with(("vp", k)), Sense::click());
                painter.rect_filled(r, CornerRadius::same(12), if b.hovered() { CARD_HI } else { CARD });
                painter.text(r.center(), Align2::CENTER_CENTER, *name, FontId::proportional(12.0), TEXT);
                if b.clicked() {
                    ed.rot3 = view_preset(k);
                    ed.pan3 = vec2(0.0, 0.0);
                    ed.zoom3 = 1.0;
                }
                x += r.width() + 4.0;
            }
            painter.text(rect.left_top() + vec2(10.0, 14.0), Align2::LEFT_CENTER, "Drag: rotate any way   Space+drag or middle-drag: pan   Scroll: zoom", FontId::proportional(11.5), MUTED);
            return rect;
        }
        if ed.zoom_cmd == 2 {
            ed.fitted = false;
        }
        if ed.zoom_cmd != 0 && ed.zoom_cmd != 2 {
            let ns = (ed.scale * if ed.zoom_cmd > 0 { 1.25 } else { 0.8 }).clamp(0.2, 60.0);
            ed.pan *= ns / ed.scale;
            ed.scale = ns;
        }
        ed.zoom_cmd = 0;
        if !ed.fitted {
            if ed.drawing && ed.canvas[0] > 0.0 {
                let m = ed.canvas[0].max(ed.canvas[1]).max(1.0) as f32;
                ed.scale = 0.8 * rect.width().min(rect.height()) / m;
                ed.pan = vec2(0.0, 0.0);
            } else {
                let m = ed.shape.w.max(ed.shape.h).max(1.0) as f32;
                ed.scale = 0.62 * rect.width().min(rect.height()) / m;
                ed.pan = vec2(-(ed.cx as f32) * ed.scale, (ed.cy as f32) * ed.scale);
            }
            ed.fitted = true;
        }
        let (scale, pan, cen) = (ed.scale, ed.pan, rect.center());
        let _to_s = move |p: [f64; 2]| pos2(cen.x + pan.x + p[0] as f32 * scale, cen.y + pan.y - p[1] as f32 * scale);
        let to_w = move |q: Pos2| -> [f64; 2] { [((q.x - cen.x - pan.x) / scale) as f64, (-(q.y - cen.y - pan.y) / scale) as f64] };
        let snap = ed.snap;
        let snapv = move |p: [f64; 2]| -> [f64; 2] {
            if snap > 0.0 {
                [(p[0] / snap).round() * snap, (p[1] / snap).round() * snap]
            } else {
                p
            }
        };
        // zoom with the wheel about the pointer
        if let Some(h) = resp.hover_pos() {
            let sc = ui.input(|i| i.smooth_scroll_delta.y);
            if sc != 0.0 {
                let w = to_w(h);
                let ns = (ed.scale * (sc * 0.0015).exp()).clamp(0.2, 60.0);
                ed.scale = ns;
                ed.pan = vec2(h.x - cen.x - w[0] as f32 * ns, h.y - cen.y + w[1] as f32 * ns);
            }
        }
        let (scale, pan) = (ed.scale, ed.pan);
        let to_s = move |p: [f64; 2]| pos2(cen.x + pan.x + p[0] as f32 * scale, cen.y + pan.y - p[1] as f32 * scale);
        let to_w = move |q: Pos2| -> [f64; 2] { [((q.x - cen.x - pan.x) / scale) as f64, (-(q.y - cen.y - pan.y) / scale) as f64] };
        // tracing image under the grid
        if let Some(r) = ed.trace.as_mut() {
            r.ensure(ui.ctx());
            if r.visible {
                if let Some(tex) = r.tex.clone() {
                    let corners = [(0.0, 0.0), (r.w as f64, 0.0), (r.w as f64, r.h as f64), (0.0, r.h as f64)];
                    let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
                    let col = Color32::from_white_alpha((r.opacity.clamp(0.05, 1.0) * 255.0) as u8);
                    let mut mesh = egui::Mesh::with_texture(tex.id());
                    for (c, uv) in corners.iter().zip(uvs.iter()) {
                        let w = r.px_to_world(c.0, c.1);
                        mesh.vertices.push(egui::epaint::Vertex { pos: to_s(w), uv: *uv, color: col });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                    painter.add(egui::Shape::mesh(mesh));
                }
            }
        }
        // grid
        let step = grid_step(scale);
        let (w0, w1) = (to_w(rect.left_top()), to_w(rect.right_bottom()));
        let mut gx = (w0[0] / step).floor() * step;
        while gx <= w1[0] {
            let major = ((gx / step).round() as i64) % 5 == 0;
            painter.line_segment([to_s([gx, w0[1]]), to_s([gx, w1[1]])], Stroke::new(1.0, if gx.abs() < 1e-9 { fade(TEAL, 120) } else if major { Color32::from_rgb(44, 49, 58) } else { Color32::from_rgb(33, 37, 44) }));
            gx += step;
        }
        let mut gy = (w1[1] / step).floor() * step;
        while gy <= w0[1] {
            let major = ((gy / step).round() as i64) % 5 == 0;
            painter.line_segment([to_s([w0[0], gy]), to_s([w1[0], gy])], Stroke::new(1.0, if gy.abs() < 1e-9 { fade(TEAL, 120) } else if major { Color32::from_rgb(44, 49, 58) } else { Color32::from_rgb(33, 37, 44) }));
            gy += step;
        }
        painter.text(rect.left_top() + vec2(36.0, 30.0), Align2::LEFT_CENTER, format!("grid {}", crate::units::len(step, if step < 1.0 { 1 } else { 0 })), FontId::proportional(11.0), MUTED);
        if ed.drawing && ed.canvas[0] > 0.0 {
            let cr = Rect::from_two_pos(to_s([-ed.canvas[0] / 2.0, -ed.canvas[1] / 2.0]), to_s([ed.canvas[0] / 2.0, ed.canvas[1] / 2.0]));
            painter.rect_stroke(cr, CornerRadius::ZERO, Stroke::new(1.5, fade(BLUE, 190)), StrokeKind::Middle);
            painter.text(cr.left_top() + vec2(4.0, -8.0), Align2::LEFT_CENTER, format!("canvas {} x {}", crate::units::num(ed.canvas[0], 0), crate::units::len(ed.canvas[1], 0)), FontId::proportional(11.0), fade(BLUE, 220));
        }
        // measuring and tracing-image tools take the clicks first
        let mut tool_busy = false;
        let mut tool_pan = false;
        if let Some(r) = ed.trace.as_mut() {
            if r.tex.is_some() {
                if r.stage != Stage::Idle {
                    tool_busy = true;
                    tool_pan = true;
                    if resp.clicked() {
                        if let Some(p) = resp.interact_pointer_pos() {
                            let w = to_w(p);
                            let px = r.world_to_px(w[0], w[1]);
                            if r.stage == Stage::PickA {
                                r.a = Some(px);
                                r.b = None;
                                r.stage = Stage::PickB;
                            } else {
                                r.b = Some(px);
                                r.stage = Stage::Idle;
                            }
                        }
                    }
                } else if r.move_mode {
                    tool_busy = true;
                    if resp.drag_started() {
                        r.drag = 1;
                        if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                            let cs = r.corners_world();
                            for k in 0..4 {
                                if to_s(cs[k]).distance(p) < 14.0 {
                                    r.size_begin(k);
                                    break;
                                }
                            }
                        }
                    }
                    if resp.dragged() {
                        if r.drag == 2 {
                            if let Some(p) = resp.interact_pointer_pos() {
                                r.size_drag(to_w(p));
                            }
                        } else {
                            let d = resp.drag_delta();
                            r.cx += d.x as f64 / scale as f64;
                            r.cy -= d.y as f64 / scale as f64;
                        }
                    }
                    if resp.drag_stopped() {
                        r.drag = 0;
                    }
                }
            }
        }
        if ed.measure {
            tool_busy = true;
            tool_pan = true;
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let w = snapv(to_w(p));
                    if ed.mpts.len() >= 2 {
                        ed.mpts.clear();
                    }
                    ed.mpts.push(w);
                }
            }
        }
        if tool_pan && resp.dragged() {
            ed.pan += resp.drag_delta();
        }
        if tool_busy && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ed.mpts.clear();
            ed.measure = false;
            if let Some(r) = ed.trace.as_mut() {
                r.stage = Stage::Idle;
                r.move_mode = false;
            }
        }

        // ---------------------------------------------------- drawing a new loop
        if ed.drawing {
            // where the next vertex would go: grid snap, then angle lock, then alignment with the other points
            let shift = ui.input(|i| i.modifiers.shift);
            let lock_on = ed.angle_lock != shift;
            let mut guide_from: Vec<[f64; 2]> = Vec::new();
            let hover: Option<[f64; 2]> = resp.hover_pos().map(|hp| {
                let mut p = snapv(to_w(hp));
                let thr = 8.0 / scale as f64;
                let last = ed.draw_pts.last().copied();
                if let (true, Some(l)) = (lock_on, last) {
                    let step = ed.angle_step.max(1.0).to_radians();
                    let (dx, dy) = (p[0] - l[0], p[1] - l[1]);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > 1e-9 {
                        let a = (dy.atan2(dx) / step).round() * step;
                        let (cs, sn) = (a.cos(), a.sin());
                        p = [l[0] + d * cs, l[1] + d * sn];
                        if ed.align_guides {
                            // slide along the locked line to meet the line of another point
                            let mut best: Option<(f64, [f64; 2], [f64; 2])> = None;
                            for q in &ed.draw_pts {
                                let mut cands: Vec<[f64; 2]> = Vec::new();
                                if cs.abs() > 1e-6 {
                                    let t = (q[0] - l[0]) / cs;
                                    if t > 0.0 {
                                        cands.push([q[0], l[1] + t * sn]);
                                    }
                                }
                                if sn.abs() > 1e-6 {
                                    let t = (q[1] - l[1]) / sn;
                                    if t > 0.0 {
                                        cands.push([l[0] + t * cs, q[1]]);
                                    }
                                }
                                for cd in cands {
                                    let dd = ((cd[0] - p[0]).powi(2) + (cd[1] - p[1]).powi(2)).sqrt();
                                    if dd < thr && best.map_or(true, |b| dd < b.0) {
                                        best = Some((dd, cd, *q));
                                    }
                                }
                            }
                            if let Some((_, cd, q)) = best {
                                p = cd;
                                guide_from.push(q);
                            }
                        }
                    }
                } else if ed.align_guides {
                    let (mut bx, mut by): (Option<(f64, [f64; 2])>, Option<(f64, [f64; 2])>) = (None, None);
                    for q in &ed.draw_pts {
                        let dx = (p[0] - q[0]).abs();
                        if dx < thr && bx.map_or(true, |b| dx < b.0) {
                            bx = Some((dx, *q));
                        }
                        let dy = (p[1] - q[1]).abs();
                        if dy < thr && by.map_or(true, |b| dy < b.0) {
                            by = Some((dy, *q));
                        }
                    }
                    if let Some((_, q)) = bx {
                        p[0] = q[0];
                        guide_from.push(q);
                    }
                    if let Some((_, q)) = by {
                        p[1] = q[1];
                        guide_from.push(q);
                    }
                }
                p
            });
            let pts: Vec<Pos2> = ed.draw_pts.iter().map(|p| to_s(*p)).collect();
            if pts.len() >= 2 {
                painter.add(egui::Shape::line(pts.clone(), Stroke::new(2.0, ACCENT)));
            }
            // alignment guides
            if let Some(h) = hover {
                for q in &guide_from {
                    painter.extend(egui::Shape::dashed_line(&[to_s(*q), to_s(h)], Stroke::new(1.2, Color32::from_rgb(255, 110, 199)), 6.0, 4.0));
                }
            }
            if let (Some(h), Some(last)) = (hover, ed.draw_pts.last()) {
                painter.line_segment([to_s(*last), to_s(h)], Stroke::new(1.2, fade(ACCENT, 140)));
            }
            for (i, p) in pts.iter().enumerate() {
                painter.rect_filled(Rect::from_center_size(*p, vec2(8.0, 8.0)), CornerRadius::same(2), if i == 0 { TEAL } else { ACCENT });
            }
            // angle readouts at the corners already placed
            let corner = |a: [f64; 2], v: [f64; 2], b: [f64; 2]| -> Option<(f64, Pos2)> {
                let (v1, v2) = ([a[0] - v[0], a[1] - v[1]], [b[0] - v[0], b[1] - v[1]]);
                let (l1, l2) = ((v1[0] * v1[0] + v1[1] * v1[1]).sqrt(), (v2[0] * v2[0] + v2[1] * v2[1]).sqrt());
                if l1 < 1e-6 || l2 < 1e-6 {
                    return None;
                }
                let ang = ((v1[0] * v2[0] + v1[1] * v2[1]) / (l1 * l2)).clamp(-1.0, 1.0).acos().to_degrees();
                let bis = [v1[0] / l1 + v2[0] / l2, v1[1] / l1 + v2[1] / l2];
                let bl = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt();
                let dir = if bl < 1e-6 { vec2(0.0, -1.0) } else { vec2((bis[0] / bl) as f32, -(bis[1] / bl) as f32) };
                Some((ang, to_s(v) + dir * 26.0))
            };
            let ang_col = |a: f64| if (a - 90.0).abs() < 0.3 || (a - 45.0).abs() < 0.3 || (a - 135.0).abs() < 0.3 || (a - 180.0).abs() < 0.3 { TEAL } else { Color32::from_rgb(160, 205, 255) };
            if ed.show_angles {
                let n = ed.draw_pts.len();
                for i in 1..n.saturating_sub(1) {
                    if let Some((a, at)) = corner(ed.draw_pts[i - 1], ed.draw_pts[i], ed.draw_pts[i + 1]) {
                        painter.text(at, Align2::CENTER_CENTER, format!("{:.1} deg", a), FontId::proportional(11.5), fade(ang_col(a), 190));
                    }
                }
                if let (Some(h), true) = (hover, n >= 2) {
                    if let Some((a, at)) = corner(ed.draw_pts[n - 2], ed.draw_pts[n - 1], h) {
                        painter.text(at, Align2::CENTER_CENTER, format!("{:.1} deg", a), FontId::proportional(12.5), ang_col(a));
                    }
                }
            }
            let hint = if ed.draw_pts.len() < 3 { "Click to place vertices" } else { "Click the green vertex (or press Enter) to close the loop" };
            let hint2 = if lock_on { "Angle locked (Shift toggles)" } else { "Hold Shift to lock the angle" };
            painter.text(rect.center_top() + vec2(0.0, 28.0), Align2::CENTER_CENTER, format!("{}   -   {}", hint, hint2), FontId::proportional(13.0), TEXT);
            let mut close = false;
            if resp.dragged() && !tool_busy {
                ed.pan += resp.drag_delta();
            }
            if let (Some(h), Some(last)) = (hover, ed.draw_pts.last()) {
                let l = ((h[0] - last[0]).powi(2) + (h[1] - last[1]).powi(2)).sqrt();
                let mid = to_s([(h[0] + last[0]) / 2.0, (h[1] + last[1]) / 2.0]);
                let ang = (h[1] - last[1]).atan2(h[0] - last[0]).to_degrees().rem_euclid(360.0);
                if !tool_busy {
                    painter.text(mid + vec2(0.0, -12.0), Align2::CENTER_CENTER, format!("{}   {:.1} deg", crate::units::len(l, 1), ang), FontId::proportional(12.0), Color32::from_rgb(160, 205, 255));
                }
            }
            if resp.clicked() && !tool_busy {
                if let Some(p) = resp.interact_pointer_pos() {
                    let near_first = ed.draw_pts.len() >= 3 && to_s(ed.draw_pts[0]).distance(p) < 12.0;
                    if near_first {
                        close = true;
                    } else if let Some(h) = hover {
                        if ed.draw_pts.last().map_or(true, |l| (l[0] - h[0]).abs() + (l[1] - h[1]).abs() > 1e-6) {
                            ed.draw_pts.push(h);
                        }
                    }
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && ed.draw_pts.len() >= 3 {
                close = true;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !tool_busy {
                ed.draw_pts.clear();
            }
            if close {
                let pts = std::mem::take(&mut ed.draw_pts);
                let mut s = Sh::new(&ed.shape.name, Kind::Free, 50.0, 50.0);
                s.pts = pts;
                let c = s.sync_free();
                ed.cx = c[0];
                ed.cy = c[1];
                ed.shape = s;
                ed.original = ed.shape.clone();
                ed.drawing = false;
                ed.fitted = false;
                ed.sel = None;
            }
            return rect;
        }

        // ---------------------------------------------------- the shape
        let o = ed.shape.outline();
        let c = [ed.cx, ed.cy];
        if ed.show_solid {
            draw_solid(&painter, &ed.shape, c, &to_s, scale);
        }
        let sp: Vec<Pos2> = o.iter().map(|p| to_s([p[0] + c[0], p[1] + c[1]])).collect();
        let mut mesh = egui::Mesh::default();
        for p in &sp {
            mesh.colored_vertex(*p, fade(ACCENT, 60));
        }
        for t in shapes::triangulate(&o) {
            mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
        }
        painter.add(egui::Shape::mesh(mesh));
        let mut ring = sp.clone();
        ring.push(sp[0]);
        painter.add(egui::Shape::line(ring, Stroke::new(2.2, ACCENT)));
        // added pieces first, then the holes on top of them
        for holes in [false, true] {
            for (k, p) in ed.shape.parts.iter().enumerate() {
                if p.hole != holes {
                    continue;
                }
                let po = p.outline();
                if po.len() < 3 {
                    continue;
                }
                let ps: Vec<Pos2> = po.iter().map(|q| to_s([q[0] + c[0], q[1] + c[1]])).collect();
                let on = ed.part_sel == Some(k);
                let col = if p.hole { DANGER } else { TEAL };
                let mut m = egui::Mesh::default();
                for q in &ps {
                    m.colored_vertex(*q, if p.hole { BG } else { fade(TEAL, 70) });
                }
                for t in shapes::triangulate(&po) {
                    m.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
                }
                painter.add(egui::Shape::mesh(m));
                let mut rg = ps.clone();
                rg.push(ps[0]);
                let st = Stroke::new(if on { 2.6 } else { 1.8 }, col);
                if p.hole {
                    painter.extend(egui::Shape::dashed_line(&rg, st, 7.0, 4.0));
                } else {
                    painter.add(egui::Shape::line(rg, st));
                }
            }
        }

        // bounding box and stretch handles
        let (hw, hh) = (ed.shape.w / 2.0, ed.shape.h / 2.0);
        let bb = Rect::from_two_pos(to_s([c[0] - hw, c[1] - hh]), to_s([c[0] + hw, c[1] + hh]));
        painter.rect_stroke(bb, CornerRadius::ZERO, Stroke::new(1.0, fade(BLUE, 170)), StrokeKind::Middle);
        let hpos = |hx: i8, hy: i8| to_s([c[0] + hx as f64 * hw, c[1] + hy as f64 * hh]);
        let handles: [(i8, i8); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
        for (hx, hy) in handles {
            painter.rect_filled(Rect::from_center_size(hpos(hx, hy), vec2(9.0, 9.0)), CornerRadius::same(2), BLUE);
        }
        painter.text(bb.center_bottom() + vec2(0.0, 14.0), Align2::CENTER_CENTER, crate::units::len(ed.shape.w, 1), FontId::proportional(12.0), Color32::from_rgb(160, 205, 255));
        painter.text(bb.right_center() + vec2(34.0, 0.0), Align2::CENTER_CENTER, format!("{:.1}", ed.shape.h), FontId::proportional(12.0), Color32::from_rgb(160, 205, 255));
        // Bezier handles of the smooth vertices of a free loop
        let hp = |ed: &ShapeEd, i: usize, out: bool| -> Pos2 {
            let p = ed.shape.pts[i];
            let h = if out { ed.shape.hout[i] } else { ed.shape.hin[i] };
            to_s([p[0] + h[0] + c[0], p[1] + h[1] + c[1]])
        };
        if ed.shape.kind == Kind::Free {
            ed.shape.fix_rad();
            for i in 0..ed.shape.pts.len() {
                if !ed.shape.smooth[i] {
                    continue;
                }
                let on = ed.sel == Some(i);
                let v = to_s([ed.shape.pts[i][0] + c[0], ed.shape.pts[i][1] + c[1]]);
                let col = if on { Color32::from_rgb(255, 217, 168) } else { fade(Color32::from_rgb(255, 217, 168), 110) };
                for out in [false, true] {
                    let q = hp(ed, i, out);
                    painter.line_segment([v, q], Stroke::new(1.0, col));
                    painter.circle_filled(q, if on { 5.0 } else { 3.5 }, col);
                }
            }
        }
        // vertices of a free loop (the control points, which may sit outside a rounded corner)
        let cps: Vec<Pos2> = if ed.shape.kind == Kind::Free { ed.shape.pts.iter().map(|p| to_s([p[0] + c[0], p[1] + c[1]])).collect() } else { Vec::new() };
        if ed.shape.kind == Kind::Free {
            for (i, p) in cps.iter().enumerate() {
                let on = ed.sel == Some(i);
                painter.rect_filled(Rect::from_center_size(*p, vec2(if on { 11.0 } else { 8.0 }, if on { 11.0 } else { 8.0 })), CornerRadius::same(2), if on { Color32::WHITE } else { TEAL });
            }
        }

        // center grips of the added pieces and holes: drag one to move its piece
        let part_grips: Vec<(Pos2, bool)> = ed.shape.parts.iter().map(|p| (to_s([p.cx + c[0], p.cy + c[1]]), p.hole)).collect();
        {
            let (mut n_add, mut n_hole) = (0, 0);
            for (k, (g, hole)) in part_grips.iter().enumerate() {
                let on = ed.part_sel == Some(k);
                let col = if *hole { DANGER } else { TEAL };
                let r = if on { 9.0 } else { 7.0 };
                let d = vec![pos2(g.x, g.y - r), pos2(g.x + r, g.y), pos2(g.x, g.y + r), pos2(g.x - r, g.y)];
                painter.add(egui::Shape::convex_polygon(d, if on { Color32::WHITE } else { col }, Stroke::new(1.5, col)));
                let name = if *hole {
                    n_hole += 1;
                    format!("Hole {}", n_hole)
                } else {
                    n_add += 1;
                    format!("Piece {}", n_add)
                };
                painter.text(*g + vec2(12.0, -12.0), Align2::LEFT_CENTER, name, FontId::proportional(11.5), col);
            }
        }

        // size handles of the selected piece, lined up with the piece's own turn
        let part_handles: Vec<(i8, i8, Pos2)> = match ed.part_sel.filter(|k| *k < ed.shape.parts.len()) {
            Some(k) => {
                let p = &ed.shape.parts[k];
                let (sn, cs) = p.turn.to_radians().sin_cos();
                let mut v = Vec::new();
                for (hx, hy) in [(-1i8, -1i8), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                    let (lx, ly) = (hx as f64 * p.sh.w / 2.0, hy as f64 * p.sh.h / 2.0);
                    v.push((hx, hy, to_s([p.cx + lx * cs - ly * sn + c[0], p.cy + lx * sn + ly * cs + c[1]])));
                }
                v
            }
            None => Vec::new(),
        };
        if let Some(k) = ed.part_sel.filter(|k| *k < ed.shape.parts.len()) {
            let col = if ed.shape.parts[k].hole { DANGER } else { TEAL };
            for (_, _, q) in &part_handles {
                painter.rect_filled(Rect::from_center_size(*q, vec2(9.0, 9.0)), CornerRadius::same(2), col);
                painter.rect_stroke(Rect::from_center_size(*q, vec2(9.0, 9.0)), CornerRadius::same(2), Stroke::new(1.0, Color32::WHITE), StrokeKind::Middle);
            }
        }

        // ---------------------------------------------------- interaction
        if tool_busy {
            return rect;
        }
        if resp.drag_started() {
            let p = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()).unwrap_or(rect.center());
            ed.drag = EdDrag::Pan;
            let mut hit = None;
            if let Some(k) = ed.part_sel.filter(|k| *k < ed.shape.parts.len()) {
                for (hx, hy, q) in &part_handles {
                    if q.distance(p) < 10.0 {
                        hit = Some(EdDrag::PartSize { i: k, hx: *hx, hy: *hy });
                        break;
                    }
                }
            }
            for (k, (g, _)) in part_grips.iter().enumerate() {
                if hit.is_some() {
                    break;
                }
                if g.distance(p) < 12.0 {
                    let wp = to_w(p);
                    hit = Some(EdDrag::Part { i: k, ox: wp[0] - c[0] - ed.shape.parts[k].cx, oy: wp[1] - c[1] - ed.shape.parts[k].cy });
                    ed.part_sel = Some(k);
                    break;
                }
            }
            if hit.is_none() && ed.shape.kind == Kind::Free {
                ed.shape.fix_rad();
                let order: Vec<usize> = ed.sel.into_iter().chain(0..cps.len()).collect();
                'hh: for i in order {
                    if i < cps.len() && ed.shape.smooth[i] {
                        for out in [true, false] {
                            if hp(ed, i, out).distance(p) < 9.0 {
                                hit = Some(EdDrag::VH { i, out });
                                ed.sel = Some(i);
                                break 'hh;
                            }
                        }
                    }
                }
            }
            if hit.is_none() && ed.shape.kind == Kind::Free {
                for (i, q) in cps.iter().enumerate() {
                    if q.distance(p) < 10.0 {
                        hit = Some(EdDrag::Vert(i));
                        ed.sel = Some(i);
                        break;
                    }
                }
            }
            if hit.is_none() {
                for (hx, hy) in handles {
                    if hpos(hx, hy).distance(p) < 10.0 {
                        hit = Some(EdDrag::Handle { hx, hy, cx: ed.cx, cy: ed.cy, w: ed.shape.w, h: ed.shape.h });
                        break;
                    }
                }
            }
            if let Some(h) = hit {
                ed.drag = h;
            }
        }
        if resp.dragged() {
            match ed.drag {
                EdDrag::Pan => ed.pan += resp.drag_delta(),
                EdDrag::Vert(i) => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let wp = snapv(to_w(p));
                        if i < ed.shape.pts.len() {
                            ed.shape.pts[i] = [wp[0] - ed.cx, wp[1] - ed.cy];
                            let sh = ed.shape.sync_free();
                            ed.cx += sh[0];
                            ed.cy += sh[1];
                        }
                    }
                }
                EdDrag::VH { i, out } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let wp = to_w(p);
                        if i < ed.shape.pts.len() {
                            let v = ed.shape.pts[i];
                            let d = [wp[0] - ed.cx - v[0], wp[1] - ed.cy - v[1]];
                            let dl = (d[0] * d[0] + d[1] * d[1]).sqrt();
                            let other = if out { ed.shape.hin[i] } else { ed.shape.hout[i] };
                            let ol = (other[0] * other[0] + other[1] * other[1]).sqrt();
                            if out {
                                ed.shape.hout[i] = d;
                            } else {
                                ed.shape.hin[i] = d;
                            }
                            if ui.input(|i| i.modifiers.alt) {
                                ed.shape.broken[i] = true;
                            }
                            if dl > 1e-6 && !ed.shape.broken[i] {
                                let m = [-d[0] / dl * ol, -d[1] / dl * ol];
                                if out {
                                    ed.shape.hin[i] = m;
                                } else {
                                    ed.shape.hout[i] = m;
                                }
                            }
                        }
                    }
                }
                EdDrag::Handle { hx, hy, cx, cy, w, h } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let alt = ui.input(|i| i.modifiers.alt);
                        let wp = to_w(p);
                        drag_resize(ed, hx, hy, (cx, cy, w, h), wp, alt);
                    }
                }
                EdDrag::Part { i, ox, oy } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let wp = snapv(to_w(p));
                        if i < ed.shape.parts.len() {
                            ed.shape.parts[i].cx = wp[0] - ed.cx - ox;
                            ed.shape.parts[i].cy = wp[1] - ed.cy - oy;
                        }
                    }
                }
                EdDrag::PartSize { i, hx, hy } => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let wp = snapv(to_w(p));
                        if i < ed.shape.parts.len() {
                            let (cx0, cy0) = (ed.cx, ed.cy);
                            let part = &mut ed.shape.parts[i];
                            let (sn, cs) = part.turn.to_radians().sin_cos();
                            let rel = [wp[0] - cx0 - part.cx, wp[1] - cy0 - part.cy];
                            let local = [rel[0] * cs + rel[1] * sn, -rel[0] * sn + rel[1] * cs];
                            let (w, h) = (part.sh.w, part.sh.h);
                            let (mut nw, mut nh) = (w, h);
                            if hx != 0 {
                                nw = (local[0] * hx as f64 + w / 2.0).max(1.0);
                            }
                            if hy != 0 {
                                nh = (local[1] * hy as f64 + h / 2.0).max(1.0);
                            }
                            if part.sh.lock {
                                if hx != 0 && hy != 0 {
                                    let k = (nw / w + nh / h) / 2.0;
                                    nw = w * k;
                                    nh = h * k;
                                } else if hx != 0 {
                                    nh = h * nw / w;
                                } else {
                                    nw = w * nh / h;
                                }
                            }
                            // the opposite side stays put: the center moves half of the change
                            let dl = [hx as f64 * (nw - w) / 2.0, hy as f64 * (nh - h) / 2.0];
                            part.cx += dl[0] * cs - dl[1] * sn;
                            part.cy += dl[0] * sn + dl[1] * cs;
                            part.sh.w = nw;
                            part.sh.h = nh;
                        }
                    }
                }
                EdDrag::None => {}
            }
        }
        if resp.drag_stopped() {
            ed.drag = EdDrag::None;
        }
        if resp.double_clicked() && ed.shape.kind == Kind::Free {
            if let Some(p) = resp.interact_pointer_pos() {
                // insert a vertex on the nearest edge
                let n = cps.len();
                let mut best = (12.0f32, None, [0.0f64; 2]);
                for i in 0..n {
                    let (a, b) = (cps[i], cps[(i + 1) % n]);
                    let d = b - a;
                    let l2 = d.length_sq().max(1e-6);
                    let t = (((p - a).dot(d)) / l2).clamp(0.0, 1.0);
                    let q = a + d * t;
                    let dist = q.distance(p);
                    if dist < best.0 {
                        let wq = to_w(q);
                        best = (dist, Some(i), [wq[0] - ed.cx, wq[1] - ed.cy]);
                    }
                }
                if let Some(i) = best.1 {
                    insert_vertex(ed, i, best.2);
                }
            }
        } else if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                ed.sel = None;
                if ed.shape.kind == Kind::Free {
                    for (i, q) in cps.iter().enumerate() {
                        if q.distance(p) < 10.0 {
                            ed.sel = Some(i);
                        }
                    }
                }
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) && ui.memory(|m| m.focused().is_none()) {
            delete_vertex(ed);
        }
        // ---------------------------------------------------- right-click menu
        if resp.secondary_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                ed.ctx = CtxTarget::Empty;
                if ed.shape.kind == Kind::Free {
                    let n = cps.len();
                    let mut found = false;
                    for (i, q) in cps.iter().enumerate() {
                        if q.distance(p) < 10.0 {
                            ed.ctx = CtxTarget::Vertex(i);
                            ed.sel = Some(i);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        let mut best = 14.0f32;
                        for i in 0..n {
                            let (a, b) = (cps[i], cps[(i + 1) % n]);
                            let d = b - a;
                            let t = (((p - a).dot(d)) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
                            let q = a + d * t;
                            if q.distance(p) < best {
                                best = q.distance(p);
                                let wq = to_w(q);
                                ed.ctx = CtxTarget::Edge(i, [wq[0] - ed.cx, wq[1] - ed.cy]);
                            }
                        }
                    }
                }
            }
        }
        resp.context_menu(|ui| {
            if ed.shape.kind != Kind::Free {
                ui.label(egui::RichText::new(ed.shape.kind.label()).size(12.0).color(MUTED));
                if ui.button("Edit points (make it a free loop)").clicked() {
                    ed.shape.to_free();
                    ed.sel = None;
                    ui.close();
                }
                return;
            }
            ed.shape.fix_rad();
            match ed.ctx {
                CtxTarget::Vertex(i) if i < ed.shape.pts.len() => {
                    let on = ed.shape.smooth[i];
                    if on {
                        if ui.button("Sharp corner").clicked() {
                            ed.shape.make_sharp(i);
                            ui.close();
                        }
                    } else if ui.button("Smooth curve (handles)").clicked() {
                        ed.shape.make_smooth(i);
                        ui.close();
                    }
                    if !on {
                        ui.horizontal(|ui| {
                            ui.label("Round corner");
                            ui.add(egui::DragValue::new(&mut ed.shape.rad[i]).range(0.0..=500.0).speed(0.2).mm());
                        });
                    } else if ui.button(if ed.shape.broken[i] { "Join handles (keep a smooth line)" } else { "Break handles (move each side alone)" }).clicked() {
                        let b = !ed.shape.broken[i];
                        ed.shape.set_broken(i, b);
                        ui.close();
                    } else if ui.button("Reset handles").clicked() {
                        ed.shape.make_smooth(i);
                        ui.close();
                    }
                    if ui.button("Add vertex after").clicked() {
                        let n = ed.shape.pts.len();
                        let (a, b) = (ed.shape.pts[i], ed.shape.pts[(i + 1) % n]);
                        insert_vertex(ed, i, [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
                        ui.close();
                    }
                    if ui.add_enabled(ed.shape.pts.len() > 3, egui::Button::new("Delete vertex")).clicked() {
                        ed.sel = Some(i);
                        delete_vertex(ed);
                        ui.close();
                    }
                    ui.separator();
                }
                CtxTarget::Edge(i, rel) => {
                    if ui.button("Add vertex here").clicked() {
                        insert_vertex(ed, i, rel);
                        ui.close();
                    }
                    ui.separator();
                }
                _ => {}
            }
            if ui.button("All smooth").clicked() {
                for i in 0..ed.shape.pts.len() {
                    ed.shape.make_smooth(i);
                }
                ui.close();
            }
            if ui.button("All sharp").clicked() {
                for i in 0..ed.shape.pts.len() {
                    ed.shape.make_sharp(i);
                }
                ui.close();
            }
        });
        painter.text(rect.left_bottom() + vec2(10.0, -12.0), Align2::LEFT_CENTER, "Drag the blue handles to stretch (Alt = from center)  -  right-click a vertex or edge for curves  -  scroll to zoom", FontId::proportional(11.0), MUTED);
        rect
    }

    fn shape_params(ui: &mut egui::Ui, ed: &mut ShapeEd, save_new: &mut bool, save_upd: &mut bool) {
        tools_ui(ui, ed);
        ui.add_space(6.0);
        if ed.drawing {
            card(ui, "Drawing a loop", |ui| {
            ui.label(egui::RichText::new("Place the vertices around the outline you want. Close the loop by clicking the first vertex or pressing Enter. Esc clears the points.").size(12.5).color(MUTED));
            ui.add_space(6.0);
            if ui.button("Close the loop").clicked() && ed.draw_pts.len() >= 3 {
                let pts = std::mem::take(&mut ed.draw_pts);
                let mut s = Sh::new(&ed.shape.name, Kind::Free, 50.0, 50.0);
                s.pts = pts;
                let c = s.sync_free();
                ed.cx = c[0];
                ed.cy = c[1];
                ed.shape = s;
                ed.original = ed.shape.clone();
                ed.drawing = false;
                ed.fitted = false;
            }
            if ui.button("Undo last point").clicked() {
                ed.draw_pts.pop();
            }
            ui.horizontal(|ui| {
                ui.label("Snap");
                ui.add(egui::DragValue::new(&mut ed.snap).range(0.0..=50.0).speed(0.2).mm());
            });
            });
            ui.add_space(6.0);
            card(ui, "Pen helpers", |ui| {
                ui.horizontal(|ui| {
                    if pill_button(ui, if ed.angle_lock { "Angle lock on" } else { "Angle lock off" }, ed.angle_lock).on_hover_text("Every new line is drawn at a multiple of the step angle (0, 15, 30, 45, 60, 75, 90 ...). Hold Shift to flip it for one line.").clicked() {
                        ed.angle_lock = !ed.angle_lock;
                    }
                    ui.label("Step");
                    ui.add(egui::DragValue::new(&mut ed.angle_step).range(1.0..=90.0).speed(0.5).suffix(" deg"));
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if pill_button(ui, if ed.align_guides { "Guides on" } else { "Guides off" }, ed.align_guides).on_hover_text("Pink dashed lines show when the pointer lines up with another point, and it snaps there.").clicked() {
                        ed.align_guides = !ed.align_guides;
                    }
                    if pill_button(ui, if ed.show_angles { "Angles shown" } else { "Angles hidden" }, ed.show_angles).on_hover_text("Show the angle at each corner and the direction of the line being drawn.").clicked() {
                        ed.show_angles = !ed.show_angles;
                    }
                });
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Hold Shift while clicking to lock the next line to the angle step. Pink guides line a new point up with an earlier one. Angles turn green at 45, 90, 135 and 180.").size(12.0).color(MUTED));
            });
            return;
        }
        label_caps(ui, "Name");
        ui.add(egui::TextEdit::singleline(&mut ed.shape.name).hint_text("Name it to save it").desired_width(240.0));
        label_caps(ui, "Type");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for k in Kind::ALL {
                if pill_button(ui, k.label(), ed.shape.kind == k).clicked() && ed.shape.kind != k {
                    if k == Kind::Free {
                        ed.shape.to_free();
                    } else {
                        if ed.shape.kind == Kind::Free {
                            ed.shape.pts.clear();
                        }
                        ed.shape.kind = k;
                        if matches!(k, Kind::Poly | Kind::Star) && ed.shape.sides < 3 {
                            ed.shape.sides = 5;
                        }
                    }
                    ed.sel = None;
                }
            }
        });
        label_caps(ui, "Size");
        let (ow, oh) = (ed.shape.w, ed.shape.h);
        let (mut w, mut h) = (ow, oh);
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(egui::DragValue::new(&mut w).range(0.5..=2000.0).speed(0.5).mm());
        });
        ui.horizontal(|ui| {
            ui.label("Height");
            ui.add(egui::DragValue::new(&mut h).range(0.5..=2000.0).speed(0.5).mm());
        });
        if (w - ow).abs() > 1e-9 || (h - oh).abs() > 1e-9 {
            if ed.shape.lock {
                if (w - ow).abs() > 1e-9 {
                    h = oh * w / ow;
                } else {
                    w = ow * h / oh;
                }
            }
            ed.shape.set_size(w, h);
        }
        if pill_button(ui, if ed.shape.lock { "Aspect ratio locked" } else { "Aspect ratio free" }, ed.shape.lock).on_hover_text("When locked, stretching keeps the width : height ratio. Turn it off to stretch freely.").clicked() {
            ed.shape.lock = !ed.shape.lock;
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Scale").size(12.0).color(MUTED));
            for (t, f) in [("50%", 0.5), ("90%", 0.9), ("110%", 1.1), ("200%", 2.0)] {
                if ui.small_button(t).clicked() {
                    let (nw, nh) = (ed.shape.w * f, ed.shape.h * f);
                    ed.shape.radius *= f;
                    for v in ed.shape.rad.iter_mut() {
                        *v *= f;
                    }
                    ed.shape.set_size(nw, nh);
                }
            }
        });
        match ed.shape.kind {
            Kind::Rect => {
                label_caps(ui, "Corners");
                ui.horizontal(|ui| {
                    ui.label("Radius");
                    let m = (ed.shape.w.min(ed.shape.h) / 2.0).max(0.5);
                    ui.add(egui::DragValue::new(&mut ed.shape.radius).range(0.0..=m).speed(0.2).mm());
                });
            }
            Kind::Poly | Kind::Star => {
                label_caps(ui, if ed.shape.kind == Kind::Poly { "Polygon" } else { "Star" });
                ui.horizontal(|ui| {
                    ui.label(if ed.shape.kind == Kind::Poly { "Sides" } else { "Points" });
                    ui.add(egui::DragValue::new(&mut ed.shape.sides).range(3..=24).speed(0.1));
                });
                if ed.shape.kind == Kind::Star {
                    ui.horizontal(|ui| {
                        ui.label("Point depth");
                        ui.add(egui::Slider::new(&mut ed.shape.inner, 0.1..=0.9).show_value(false));
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Turn");
                    ui.add(egui::DragValue::new(&mut ed.shape.rot).range(-180.0..=180.0).speed(0.5).suffix(" deg"));
                });
            }
            Kind::Ellipse => {}
            Kind::Cross => {
                label_caps(ui, "Cross");
                ui.horizontal(|ui| {
                    ui.label("Arm thickness");
                    ui.add(egui::Slider::new(&mut ed.shape.inner, 0.1..=0.9).show_value(false));
                });
            }
            Kind::Free => {
                label_caps(ui, "Vertices");
                ui.label(egui::RichText::new(format!("{} vertices. Drag one to move it, double-click an edge to add one.", ed.shape.pts.len())).size(12.0).color(MUTED));
                label_caps(ui, "Curves (handles)");
                ed.shape.fix_rad();
                ui.horizontal(|ui| {
                    if let Some(i) = ed.sel {
                        let on = ed.shape.smooth[i];
                        if pill_button(ui, if on { "Vertex: smooth (handles)" } else { "Vertex: sharp corner" }, on).on_hover_text("Turn the selected vertex into a smooth curve point with two handles, or back into a corner").clicked() {
                            if on {
                                ed.shape.make_sharp(i);
                            } else {
                                ed.shape.make_smooth(i);
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new("Click a vertex to give it handles.").size(12.0).color(MUTED));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("All smooth").on_hover_text("Convert every point to a handle point for a smooth curve all round").clicked() {
                        for i in 0..ed.shape.pts.len() {
                            ed.shape.make_smooth(i);
                        }
                    }
                    if ui.button("All sharp").clicked() {
                        for i in 0..ed.shape.pts.len() {
                            ed.shape.make_sharp(i);
                        }
                    }
                });
                if let Some(i) = ed.sel.filter(|&i| ed.shape.smooth[i]) {
                    let br = ed.shape.broken[i];
                    if pill_button(ui, if br { "Handles: broken (corner)" } else { "Handles: joined (smooth)" }, br).on_hover_text("Break the tangent so the two handles move on their own and the point can turn a sharp corner; click again to join them into one line").clicked() {
                        ed.shape.set_broken(i, !br);
                    }
                }
                if ed.sel.map(|i| ed.shape.smooth[i]).unwrap_or(false) {
                    ui.label(egui::RichText::new("Drag the handles to shape the curve (Alt-drag a handle = break it).").size(11.5).color(MUTED));
                }
                label_caps(ui, "Corner rounding");
                if let Some(i) = ed.sel {
                    ui.horizontal(|ui| {
                        ui.label("This corner");
                        ui.add_enabled(!ed.shape.smooth[i], egui::DragValue::new(&mut ed.shape.rad[i]).range(0.0..=500.0).speed(0.2).mm());
                    });
                } else {
                    ui.label(egui::RichText::new("Click a vertex to round just that corner.").size(12.0).color(MUTED));
                }
                ui.horizontal(|ui| {
                    ui.label("All corners");
                    let mut r = ed.shape.rad.iter().cloned().fold(0.0f64, f64::max);
                    if ui.add(egui::DragValue::new(&mut r).range(0.0..=500.0).speed(0.2).mm()).changed() {
                        for v in ed.shape.rad.iter_mut() {
                            *v = r;
                        }
                    }
                    if ui.small_button("Sharp").clicked() {
                        for v in ed.shape.rad.iter_mut() {
                            *v = 0.0;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(ed.sel.is_some() && ed.shape.pts.len() > 3, egui::Button::new("Delete vertex")).clicked() {
                        delete_vertex(ed);
                    }
                    if ui.button("Flip left-right").clicked() {
                        ed.shape.transform(|p| [-p[0], p[1]]);
                    }
                    if ui.button("Flip up-down").clicked() {
                        ed.shape.transform(|p| [p[0], -p[1]]);
                    }
                });
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Rotate 90").clicked() {
                if ed.shape.kind == Kind::Free {
                    ed.shape.transform(|p| [-p[1], p[0]]);
                    ed.shape.sync_free();
                } else {
                    let (w, h) = (ed.shape.w, ed.shape.h);
                    ed.shape.w = h;
                    ed.shape.h = w;
                    if matches!(ed.shape.kind, Kind::Poly | Kind::Star) {
                        ed.shape.rot += 90.0;
                    }
                }
            }
            if ed.shape.kind != Kind::Free && ui.button("Edit points").on_hover_text("Turn this into a free loop so every vertex can be moved").clicked() {
                ed.shape.to_free();
                ed.sel = None;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Snap to grid");
            ui.add(egui::DragValue::new(&mut ed.snap).range(0.0..=50.0).speed(0.2).mm());
        });
        parts_section(ui, ed);
        extrude_section(ui, ed);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("{} x {}  -  area {}", crate::units::num(ed.shape.w, 1), crate::units::len(ed.shape.h, 1), crate::units::area(ed.shape.area()))).size(12.0).color(MUTED));
        ui.separator();
        ui.horizontal(|ui| {
            if ed.orig_id.is_some() {
                if primary_button(ui, "Update").clicked() {
                    *save_upd = true;
                }
                if ui.button("Save as new").clicked() {
                    *save_new = true;
                }
            } else if primary_button(ui, "Save to my library").clicked() {
                *save_new = true;
            }
        });
        if ui.button("Reset to how it opened").clicked() {
            ed.shape = ed.original.clone();
            ed.cx = 0.0;
            ed.cy = 0.0;
            ed.sel = None;
            ed.fitted = false;
        }
        if !ed.msg.is_empty() {
            ui.label(egui::RichText::new(ed.msg.clone()).size(12.0).color(TEAL));
        }
    }
}

fn point_in_loop(p: [f64; 2], l: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = l.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (l[i], l[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The "Combine shapes" card: add standard shapes to the drawing, or cut holes in it.
fn parts_section(ui: &mut egui::Ui, ed: &mut ShapeEd) {
    if ed.shape.depth <= 0.0 && ed.shape.mesh.is_empty() {
        // still fine for a flat shape: the pieces change its outline
    }
    if !ed.shape.mesh.is_empty() {
        return;
    }
    ui.add_space(4.0);
    card(ui, "Combine shapes", |ui| {
        ui.label(egui::RichText::new("Build one shape from several. Add a circle, rectangle or other standard shape to your drawing, or cut a hole in it. Each piece gets its own size and position, and the whole thing extrudes and cuts as one shape.").size(12.0).color(MUTED));
        ui.add_space(4.0);
        let kinds = [Kind::Ellipse, Kind::Rect, Kind::Poly, Kind::Star, Kind::Cross];
        let mut added: Option<shapes::Part> = None;
        ui.horizontal(|ui| {
            ui.menu_button("Add a piece\u{2026}", |ui| {
                ui.label(egui::RichText::new("Joins the shape").size(11.5).color(MUTED));
                for k in kinds {
                    let p = shapes::Part::new(k, false);
                    if ui.button(p.label()).clicked() {
                        added = Some(p);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("Add a standard shape that becomes part of this shape");
            ui.menu_button("Cut a hole\u{2026}", |ui| {
                ui.label(egui::RichText::new("Removed from the shape").size(11.5).color(MUTED));
                for k in kinds {
                    let p = shapes::Part::new(k, true);
                    if ui.button(p.label()).clicked() {
                        added = Some(p);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("Cut a standard shape out of this shape, like a window or a center hole");
        });
        if let Some(p) = added {
            ed.shape.parts.push(p);
            ed.part_sel = Some(ed.shape.parts.len() - 1);
        }
        if ed.shape.parts.is_empty() {
            ui.add_space(2.0);
            ui.label(egui::RichText::new("No pieces yet. Use the buttons above.").size(12.0).color(MUTED));
            return;
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Click a piece to edit it. Drag its diamond handle on the drawing to move it.").size(12.0).color(MUTED));
        // list
        let mut remove: Option<usize> = None;
        let (mut n_add, mut n_hole) = (0, 0);
        for k in 0..ed.shape.parts.len() {
            let (hole, lab) = (ed.shape.parts[k].hole, ed.shape.parts[k].label());
            let name = if hole {
                n_hole += 1;
                format!("Hole {}: {}", n_hole, lab)
            } else {
                n_add += 1;
                format!("Piece {}: {}", n_add, lab)
            };
            ui.horizontal(|ui| {
                let on = ed.part_sel == Some(k);
                let col = if hole { DANGER } else { TEAL };
                ui.label(egui::RichText::new("\u{25C6}").color(col));
                if ui.selectable_label(on, name).clicked() {
                    ed.part_sel = Some(k);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Remove").on_hover_text("Delete this piece").clicked() {
                        remove = Some(k);
                    }
                });
            });
        }
        if let Some(k) = remove {
            ed.shape.parts.remove(k);
            ed.part_sel = None;
        }
        let Some(k) = ed.part_sel.filter(|k| *k < ed.shape.parts.len()) else {
            return;
        };
        ui.separator();
        let region = ed.shape.region();
        let outside_warn = {
            let p = &ed.shape.parts[k];
            let o = p.outline();
            p.hole && !o.iter().any(|q| region.solid.iter().any(|l| point_in_loop(*q, l)))
        };
        let part = &mut ed.shape.parts[k];
        label_caps(ui, "What this piece does");
        ui.horizontal(|ui| {
            if pill_button(ui, "Adds to the shape", !part.hole).on_hover_text("The piece joins the shape and makes it bigger").clicked() {
                part.hole = false;
            }
            if pill_button(ui, "Cuts a hole", part.hole).on_hover_text("The piece is removed from the shape").clicked() {
                part.hole = true;
            }
        });
        label_caps(ui, "Position (from the middle of the shape)");
        ui.horizontal(|ui| {
            ui.label("Across");
            ui.add(egui::DragValue::new(&mut part.cx).speed(0.5).mm());
            ui.label("Up");
            ui.add(egui::DragValue::new(&mut part.cy).speed(0.5).mm());
            if ui.small_button("Center").on_hover_text("Put the piece in the middle of the shape").clicked() {
                part.cx = 0.0;
                part.cy = 0.0;
            }
        });
        label_caps(ui, "Size");
        let round = part.sh.kind == Kind::Ellipse;
        let (ow, oh) = (part.sh.w, part.sh.h);
        let (mut w, mut h) = (ow, oh);
        ui.horizontal(|ui| {
            ui.label(if round { "Width" } else { "Width" });
            ui.add(egui::DragValue::new(&mut w).range(0.5..=2000.0).speed(0.5).mm());
            ui.label("Height");
            ui.add(egui::DragValue::new(&mut h).range(0.5..=2000.0).speed(0.5).mm());
        });
        if (w - ow).abs() > 1e-9 || (h - oh).abs() > 1e-9 {
            if part.sh.lock {
                if (w - ow).abs() > 1e-9 {
                    h = oh * w / ow;
                } else {
                    w = ow * h / oh;
                }
            }
            part.sh.w = w.max(0.5);
            part.sh.h = h.max(0.5);
        }
        let lock_text = if round { "Keep it round" } else { "Keep proportions" };
        ui.checkbox(&mut part.sh.lock, lock_text).on_hover_text("When on, changing the width also changes the height by the same ratio");
        match part.sh.kind {
            Kind::Rect => {
                ui.horizontal(|ui| {
                    ui.label("Corner radius");
                    let m = (part.sh.w.min(part.sh.h) / 2.0).max(0.5);
                    ui.add(egui::DragValue::new(&mut part.sh.radius).range(0.0..=m).speed(0.2).mm());
                });
            }
            Kind::Poly | Kind::Star => {
                ui.horizontal(|ui| {
                    ui.label(if part.sh.kind == Kind::Poly { "Sides" } else { "Points" });
                    ui.add(egui::DragValue::new(&mut part.sh.sides).range(3..=24).speed(0.1));
                });
                if part.sh.kind == Kind::Star {
                    ui.horizontal(|ui| {
                        ui.label("Point depth");
                        ui.add(egui::Slider::new(&mut part.sh.inner, 0.1..=0.9).show_value(false));
                    });
                }
            }
            Kind::Cross => {
                ui.horizontal(|ui| {
                    ui.label("Arm thickness");
                    ui.add(egui::Slider::new(&mut part.sh.inner, 0.1..=0.9).show_value(false));
                });
            }
            _ => {}
        }
        ui.horizontal(|ui| {
            ui.label("Turn");
            ui.add(egui::DragValue::new(&mut part.turn).range(-180.0..=180.0).speed(0.5).suffix(" deg"));
        });
        if outside_warn {
            ui.add_space(2.0);
            ui.label(egui::RichText::new("This hole is outside the shape, so it removes nothing. Move it onto the shape.").size(12.0).color(DANGER));
        }
        ui.add_space(2.0);
        ui.label(egui::RichText::new("Holes are cut straight through. Tip: a hole cut in a cut-out shape leaves an island in the middle.").size(11.5).color(MUTED));
    });
}

/// Insert a vertex after vertex i at `rel` (relative to the shape center). It gets handles when a neighbor is smooth.
fn insert_vertex(ed: &mut ShapeEd, i: usize, rel: [f64; 2]) {
    ed.shape.fix_rad();
    ed.shape.pts.insert(i + 1, rel);
    ed.shape.rad.insert(i + 1, 0.0);
    ed.shape.smooth.insert(i + 1, false);
    ed.shape.broken.insert(i + 1, false);
    ed.shape.hin.insert(i + 1, [0.0; 2]);
    ed.shape.hout.insert(i + 1, [0.0; 2]);
    let n = ed.shape.pts.len();
    if ed.shape.smooth[i] || ed.shape.smooth[(i + 2) % n] {
        ed.shape.make_smooth(i + 1);
    }
    ed.sel = Some(i + 1);
}

fn delete_vertex(ed: &mut ShapeEd) {
    if ed.shape.kind == Kind::Free && ed.shape.pts.len() > 3 {
        if let Some(i) = ed.sel {
            if i < ed.shape.pts.len() {
                ed.shape.pts.remove(i);
                if i < ed.shape.rad.len() {
                    ed.shape.rad.remove(i);
                }
                if i < ed.shape.smooth.len() {
                    ed.shape.smooth.remove(i);
                    if i < ed.shape.broken.len() {
                        ed.shape.broken.remove(i);
                    }
                    ed.shape.hin.remove(i);
                    ed.shape.hout.remove(i);
                }
                let sh = ed.shape.sync_free();
                ed.cx += sh[0];
                ed.cy += sh[1];
                ed.sel = None;
            }
        }
    }
}

/// Stretch the box from one of its handles. The opposite side stays put (Alt: scale about the center).
fn drag_resize(ed: &mut ShapeEd, hx: i8, hy: i8, st: (f64, f64, f64, f64), p: [f64; 2], alt: bool) {
    let (cx, cy, w0, h0) = st;
    let sn = |v: f64| if ed.snap > 0.0 { (v / ed.snap).round() * ed.snap } else { v };
    let (px, py) = (sn(p[0]), sn(p[1]));
    let (mut x0, mut x1, mut y0, mut y1) = (cx - w0 / 2.0, cx + w0 / 2.0, cy - h0 / 2.0, cy + h0 / 2.0);
    if hx != 0 {
        if alt {
            let hf = (px - cx).abs().max(0.5);
            x0 = cx - hf;
            x1 = cx + hf;
        } else if hx < 0 {
            x0 = px.min(x1 - 1.0);
        } else {
            x1 = px.max(x0 + 1.0);
        }
    }
    if hy != 0 {
        if alt {
            let hf = (py - cy).abs().max(0.5);
            y0 = cy - hf;
            y1 = cy + hf;
        } else if hy < 0 {
            y0 = py.min(y1 - 1.0);
        } else {
            y1 = py.max(y0 + 1.0);
        }
    }
    let (mut nw, mut nh) = (x1 - x0, y1 - y0);
    if ed.shape.lock {
        if hx != 0 && hy != 0 {
            let (sx, sy) = (nw / w0, nh / h0);
            let s = if (sx - 1.0).abs() > (sy - 1.0).abs() { sx } else { sy };
            nw = w0 * s;
            nh = h0 * s;
            if alt {
                x0 = cx - nw / 2.0;
                x1 = cx + nw / 2.0;
                y0 = cy - nh / 2.0;
                y1 = cy + nh / 2.0;
            } else {
                if hx > 0 {
                    x1 = x0 + nw;
                } else {
                    x0 = x1 - nw;
                }
                if hy > 0 {
                    y1 = y0 + nh;
                } else {
                    y0 = y1 - nh;
                }
            }
        } else if hx != 0 {
            nh = nw * h0 / w0;
            y0 = cy - nh / 2.0;
            y1 = cy + nh / 2.0;
        } else {
            nw = nh * w0 / h0;
            x0 = cx - nw / 2.0;
            x1 = cx + nw / 2.0;
        }
    }
    ed.shape.set_size(nw.max(0.5), nh.max(0.5));
    ed.cx = (x0 + x1) / 2.0;
    ed.cy = (y0 + y1) / 2.0;
}

/// Write the extruded solid to <data dir>/exports/shape_<name>.stl.
fn export_solid(sh: &Sh, dir: &std::path::Path) -> Result<String, String> {
    let sol = sh.solid().ok_or("set a depth above 0 first")?;
    let name: String = sh.name.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let d = dir.join("exports");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let path = d.join(format!("shape_{}.stl", if name.is_empty() { "solid".into() } else { name }));
    std::fs::write(&path, crate::model::stl_bytes(&sol.to_mesh())).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Depth / draft / rim controls and a small turnable 3D preview of the extruded shape.
fn extrude_section(ui: &mut egui::Ui, ed: &mut ShapeEd) {
    label_caps(ui, "Extrude (3D)");
    ui.horizontal(|ui| {
        ui.label("Depth");
        ui.add(egui::DragValue::new(&mut ed.shape.depth).range(0.0..=500.0).speed(0.2).mm());
        for d in [3.0, 6.0, 12.0] {
            if ui.small_button(format!("{}", d)).clicked() {
                ed.shape.depth = d;
            }
        }
        if ui.small_button("0").on_hover_text("Flat, no extrusion").clicked() {
            ed.shape.depth = 0.0;
        }
    });
    if ed.shape.depth <= 0.0 {
        ui.label(egui::RichText::new("Depth 0 keeps it a flat outline. Give it a depth to make a solid.").size(12.0).color(MUTED));
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Wall lean");
        ui.add(egui::DragValue::new(&mut ed.shape.draft).range(-45.0..=45.0).speed(0.2).suffix(" deg"));
        ui.label(egui::RichText::new("+ narrows up, - flares").size(11.5).color(MUTED));
    });
    ui.horizontal(|ui| {
        ui.label("Top edge");
        ui.add(egui::DragValue::new(&mut ed.shape.bevel).range(0.0..=ed.shape.depth.max(0.1)).speed(0.1).mm());
        if pill_button(ui, if ed.shape.bevel_round { "Rounded" } else { "Chamfer" }, ed.shape.bevel_round).clicked() {
            ed.shape.bevel_round = !ed.shape.bevel_round;
        }
    });
    if let Some(sol) = ed.shape.solid() {
        if sol.limited {
            ui.label(egui::RichText::new("Lean / edge was too big for this size and has been limited.").size(11.5).color(ACCENT));
        }
    }
    ui.horizontal(|ui| {
        if pill_button(ui, if ed.show_solid { "Solid shown" } else { "Solid hidden" }, ed.show_solid).on_hover_text("Show the extruded solid in the editor. The outline, handles and vertices stay editable on its base.").clicked() {
            ed.show_solid = !ed.show_solid;
        }
        if ui.button("Save STL").on_hover_text("Writes shape_<name>.stl to the exports folder").clicked() {
            ed.export_req = true;
        }
    });
    if pill_button(ui, if ed.view3d { "Back to editing" } else { "View in 3D" }, ed.view3d).on_hover_text("Turn the solid around to look at it from any side. Drag to tumble, Shift-drag or middle-drag to pan, scroll to zoom.").clicked() {
        ed.view3d = !ed.view3d;
    }
}

/// Draw the extruded solid in the editor with an oblique view: the base stays exactly under the editing handles and the
/// height rises up and to the right.
fn draw_solid(painter: &egui::Painter, sh: &Sh, c: [f64; 2], to_s: &dyn Fn([f64; 2]) -> Pos2, scale: f32) {
    let Some(sol) = sh.solid() else { return };
    let k = 0.5f64;
    let proj: Vec<Pos2> = sol.pos.iter().map(|p| to_s([p[0] + c[0], p[1] + c[1]]) + vec2((p[2] * k) as f32 * scale, -(p[2] * k) as f32 * scale)).collect();
    // direction towards the viewer in world axes
    let e = {
        let v = [-k, -k, 1.0];
        let l = (v[0] * v[0] + v[1] * v[1] + 1.0f64).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let light = [-0.3f64, 0.55, 0.78];
    let mut order: Vec<(f64, usize)> = sol.tris.iter().enumerate().map(|(i, t)| (sol_depth(&sol.pos, t, e), i)).collect();
    order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut mesh = egui::Mesh::default();
    for (_, i) in order {
        let t = sol.tris[i];
        let (a, b, d) = (sol.pos[t[0]], sol.pos[t[1]], sol.pos[t[2]]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [d[0] - a[0], d[1] - a[1], d[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
        let n = [n[0] / l, n[1] / l, n[2] / l];
        if n[0] * e[0] + n[1] * e[1] + n[2] * e[2] <= 0.0 {
            continue;
        }
        let kk = (0.3 + 0.7 * (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0)).min(1.0);
        let col = Color32::from_rgb((236.0 * kk) as u8, (156.0 * kk) as u8, (64.0 * kk) as u8);
        let (pa, pb, pd) = (proj[t[0]], proj[t[1]], proj[t[2]]);
        if ((pb.x - pa.x) * (pd.y - pa.y) - (pb.y - pa.y) * (pd.x - pa.x)).abs() < 0.02 {
            continue;
        }
        let base = mesh.vertices.len() as u32;
        for q in [pa, pb, pd] {
            mesh.vertices.push(egui::epaint::Vertex { pos: q, uv: egui::epaint::WHITE_UV, color: col });
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn sol_depth(pos: &[[f64; 3]], t: &[usize; 3], e: [f64; 3]) -> f64 {
    let m = |k: usize| pos[t[0]][k] + pos[t[1]][k] + pos[t[2]][k];
    (m(0) * e[0] + m(1) * e[1] + m(2) * e[2]) / 3.0
}

type M3 = [[f64; 3]; 3];

impl ShapeEd {
    /// Drive the 3D view of the extruded shape from a SpaceMouse. The numbers are the same amounts the main view takes
    /// (orbit and pan in mouse-drag pixels, scroll for zoom).
    pub fn sm_drive(&mut self, orbit: (f32, f32), pan: (f32, f32), scroll: f32) {
        let r = mat_mul(&rot_x(orbit.1 as f64 * 0.01), &mat_mul(&rot_y(orbit.0 as f64 * 0.01), &self.rot3));
        self.rot3 = r;
        self.pan3 += vec2(pan.0, pan.1);
        self.zoom3 = (self.zoom3 * (scroll * 0.0015).exp()).clamp(0.3, 8.0);
    }
    pub fn sm_home(&mut self, preset: usize) {
        self.rot3 = view_preset(preset);
        self.pan3 = vec2(0.0, 0.0);
        self.zoom3 = 1.0;
    }
}

fn mat_mul(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}
fn rot_x(a: f64) -> M3 {
    let (c, s) = (a.cos(), a.sin());
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}
fn rot_y(a: f64) -> M3 {
    let (c, s) = (a.cos(), a.sin());
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}
fn rot_z(a: f64) -> M3 {
    let (c, s) = (a.cos(), a.sin());
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// World -> view matrices (view x right, y up, z towards the viewer). 0 = 3D, 1 = Top, 2 = Front, 3 = Side, 4 = Bottom.
fn view_preset(k: usize) -> M3 {
    let front: M3 = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]];
    match k {
        1 => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        2 => front,
        3 => [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
        4 => [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
        _ => mat_mul(&rot_x(0.55), &mat_mul(&front, &rot_z(0.65))),
    }
}

/// Free tumble view of the extruded solid (orthographic, view only).
fn draw_solid3d(painter: &egui::Painter, rect: Rect, sh: &Sh, r: &M3, pan: egui::Vec2, zoom: f32) {
    let Some(sol) = sh.solid_thumb() else { return };
    let hz = sh.depth / 2.0;
    let rot = |p: [f64; 3]| {
        let q = [p[0], p[1], p[2] - hz];
        [0, 1, 2].map(|i| r[i][0] * q[0] + r[i][1] * q[1] + r[i][2] * q[2])
    };
    let rp: Vec<[f64; 3]> = sol.pos.iter().map(|p| rot(*p)).collect();
    let ext = sh.w.hypot(sh.h).max(sh.depth).max(1.0);
    let sc = (rect.width().min(rect.height()) as f64 * 0.7 / ext) * zoom as f64;
    let c = rect.center() + pan;
    let e = [0.0, 0.0, 1.0];
    let light = [-0.35f64, 0.5, 0.8];
    let pt = |q: [f64; 3]| pos2(c.x + (q[0] * sc) as f32, c.y - (q[1] * sc) as f32);
    let mut order: Vec<(f64, usize)> = sol.tris.iter().enumerate().map(|(i, t)| (sol_depth(&rp, t, e), i)).collect();
    order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut mesh = egui::Mesh::default();
    for (_, i) in order {
        let t = sol.tris[i];
        let (a, b, d) = (rp[t[0]], rp[t[1]], rp[t[2]]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [d[0] - a[0], d[1] - a[1], d[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
        let n = [n[0] / l, n[1] / l, n[2] / l];
        if n[2] <= 0.0 {
            continue;
        }
        let kk = (0.3 + 0.7 * (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0)).min(1.0);
        let col = Color32::from_rgb((236.0 * kk) as u8, (156.0 * kk) as u8, (64.0 * kk) as u8);
        let (pa, pb, pd) = (pt(a), pt(b), pt(d));
        if ((pb.x - pa.x) * (pd.y - pa.y) - (pb.y - pa.y) * (pd.x - pa.x)).abs() < 0.02 {
            continue;
        }
        let base = mesh.vertices.len() as u32;
        for q in [pa, pb, pd] {
            mesh.vertices.push(egui::epaint::Vertex { pos: q, uv: egui::epaint::WHITE_UV, color: col });
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn fmt_mm(v: f64, step: f64) -> String {
    if crate::units::inch() {
        let x = v / crate::units::MM_PER_IN;
        let r = (x * 100.0).round() / 100.0;
        return if (r - r.round()).abs() < 1e-9 { format!("{}", r.round() as i64) } else { format!("{}", r) };
    }
    if step >= 1.0 { format!("{}", v.round() as i64) } else { format!("{:.1}", v) }
}

/// Rulers, zoom buttons, measuring line and the pointer read-out on top of the editing canvas.
fn canvas_overlays(ui: &mut egui::Ui, rect: Rect, ed: &mut ShapeEd) {
    if ed.view3d {
        return;
    }
    let painter = ui.painter_at(rect);
    let (scale, pan, cen) = (ed.scale, ed.pan, rect.center());
    let to_s = move |p: [f64; 2]| pos2(cen.x + pan.x + p[0] as f32 * scale, cen.y + pan.y - p[1] as f32 * scale);
    let to_w = move |q: Pos2| -> [f64; 2] { [((q.x - cen.x - pan.x) / scale) as f64, (-(q.y - cen.y - pan.y) / scale) as f64] };
    let hover = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
    let pill = Color32::from_rgb(160, 205, 255);
    // calibration marks on the tracing image
    if let Some(r) = ed.trace.as_ref() {
        for (p, name) in [(r.a, "A"), (r.b, "B")] {
            if let Some(p) = p {
                let w = r.px_to_world(p[0], p[1]);
                let s = to_s(w);
                painter.line_segment([s + vec2(-9.0, 0.0), s + vec2(9.0, 0.0)], Stroke::new(2.0, TEAL));
                painter.line_segment([s + vec2(0.0, -9.0), s + vec2(0.0, 9.0)], Stroke::new(2.0, TEAL));
                painter.text(s + vec2(11.0, -11.0), Align2::LEFT_CENTER, name, FontId::proportional(13.0), TEAL);
            }
        }
        if let (Some(a), Some(b)) = (r.a, r.b) {
            let (wa, wb) = (r.px_to_world(a[0], a[1]), r.px_to_world(b[0], b[1]));
            painter.line_segment([to_s(wa), to_s(wb)], Stroke::new(1.2, TEAL));
        }
        if r.move_mode && r.visible && r.tex.is_some() {
            let cs = r.corners_world().map(|c| to_s(c));
            painter.add(egui::Shape::closed_line(cs.to_vec(), Stroke::new(1.5, TEAL)));
            for c in cs {
                painter.rect_filled(Rect::from_center_size(c, vec2(11.0, 11.0)), CornerRadius::same(2), TEAL);
                painter.rect_stroke(Rect::from_center_size(c, vec2(11.0, 11.0)), CornerRadius::same(2), Stroke::new(1.0, Color32::BLACK), StrokeKind::Middle);
            }
        }
        let msg = match r.stage {
            Stage::PickA => Some("Click point A on the image"),
            Stage::PickB => Some("Click point B on the image"),
            Stage::Idle if r.move_mode => Some("Drag the image to move it, drag a corner to resize it"),
            _ => None,
        };
        if let Some(m) = msg {
            painter.text(rect.center_top() + vec2(0.0, if ed.drawing { 58.0 } else { 40.0 }), Align2::CENTER_CENTER, m, FontId::proportional(14.0), TEAL);
        }
    }
    // measuring
    if ed.measure {
        let live = hover.map(|h| to_w(h));
        let pts: Vec<[f64; 2]> = match ed.mpts.len() {
            0 => Vec::new(),
            1 => live.map(|l| vec![ed.mpts[0], l]).unwrap_or_else(|| ed.mpts.clone()),
            _ => ed.mpts.clone(),
        };
        for p in &ed.mpts {
            painter.circle_filled(to_s(*p), 4.5, TEAL);
        }
        if pts.len() == 2 {
            let (a, b) = (pts[0], pts[1]);
            painter.line_segment([to_s(a), to_s(b)], Stroke::new(1.6, TEAL));
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = (dx * dx + dy * dy).sqrt();
            let mid = to_s([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
            let txt = format!("{}   dx {}  dy {}", crate::units::len(l, 2), crate::units::num(dx.abs(), 2), crate::units::num(dy.abs(), 2));
            let g = painter.layout_no_wrap(txt.clone(), FontId::proportional(12.5), TEXT);
            let bg = Rect::from_center_size(mid + vec2(0.0, -16.0), g.size() + vec2(12.0, 6.0));
            painter.rect_filled(bg, CornerRadius::same(5), Color32::from_rgba_unmultiplied(14, 16, 20, 230));
            painter.text(bg.center(), Align2::CENTER_CENTER, txt, FontId::proportional(12.5), TEXT);
        }
        painter.text(rect.center_top() + vec2(0.0, 58.0), Align2::CENTER_CENTER, "Measure: click two points (Esc to stop)", FontId::proportional(12.5), TEAL);
    }
    // rulers
    let (rh, rw) = (18.0f32, 30.0f32);
    let bar = Color32::from_rgba_unmultiplied(20, 23, 29, 235);
    let top = Rect::from_min_max(rect.left_top() + vec2(rw, 0.0), pos2(rect.right(), rect.top() + rh));
    let left = Rect::from_min_max(rect.left_top() + vec2(0.0, rh), pos2(rect.left() + rw, rect.bottom()));
    painter.rect_filled(top, CornerRadius::ZERO, bar);
    painter.rect_filled(left, CornerRadius::ZERO, bar);
    painter.rect_filled(Rect::from_min_size(rect.left_top(), vec2(rw, rh)), CornerRadius::ZERO, Color32::from_rgb(28, 32, 39));
    painter.text(rect.left_top() + vec2(rw / 2.0, rh / 2.0), Align2::CENTER_CENTER, crate::units::label(), FontId::proportional(9.5), MUTED);
    let step = grid_step(scale);
    let (w0, w1) = (to_w(rect.left_top()), to_w(rect.right_bottom()));
    let mut gx = (w0[0] / step).floor() * step;
    while gx <= w1[0] {
        let x = to_s([gx, 0.0]).x;
        if x > rect.left() + rw {
            let major = ((gx / step).round() as i64) % 5 == 0;
            painter.line_segment([pos2(x, rect.top() + if major { 4.0 } else { 11.0 }), pos2(x, rect.top() + rh)], Stroke::new(1.0, MUTED));
            if major {
                painter.text(pos2(x + 3.0, rect.top() + 7.0), Align2::LEFT_CENTER, fmt_mm(gx, step), FontId::proportional(9.5), MUTED);
            }
        }
        gx += step;
    }
    let mut gy = (w1[1] / step).floor() * step;
    while gy <= w0[1] {
        let y = to_s([0.0, gy]).y;
        if y > rect.top() + rh {
            let major = ((gy / step).round() as i64) % 5 == 0;
            painter.line_segment([pos2(rect.left() + if major { 4.0 } else { 19.0 }, y), pos2(rect.left() + rw, y)], Stroke::new(1.0, MUTED));
            if major {
                painter.text(pos2(rect.left() + 3.0, y - 6.0), Align2::LEFT_CENTER, fmt_mm(gy, step), FontId::proportional(9.5), MUTED);
            }
        }
        gy += step;
    }
    if let Some(h) = hover {
        painter.line_segment([pos2(h.x, rect.top()), pos2(h.x, rect.top() + rh)], Stroke::new(1.0, ACCENT));
        painter.line_segment([pos2(rect.left(), h.y), pos2(rect.left() + rw, h.y)], Stroke::new(1.0, ACCENT));
        let w = to_w(h);
        painter.text(rect.right_bottom() + vec2(-12.0, -12.0), Align2::RIGHT_CENTER, format!("x {}   y {}", crate::units::num(w[0], 1), crate::units::len(w[1], 1)), FontId::proportional(12.0), pill);
    }
    // zoom and tool buttons
    let mut x = rect.left() + rw + 8.0;
    let y = rect.bottom() - 48.0;
    let labels: [(&str, f32, bool); 4] = [("-", 30.0, false), ("+", 30.0, false), ("Fit", 40.0, false), ("Measure", 70.0, ed.measure)];
    for (k, (name, w, on)) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, y), vec2(*w, 26.0));
        let b = ui.interact(r, ui.id().with(("cvbtn", k)), Sense::click());
        painter.rect_filled(r, CornerRadius::same(13), if *on { ACCENT } else if b.hovered() { CARD_HI } else { CARD });
        painter.text(r.center(), Align2::CENTER_CENTER, *name, FontId::proportional(13.0), if *on { BG } else { TEXT });
        if b.clicked() {
            match k {
                0 => ed.zoom_cmd = -1,
                1 => ed.zoom_cmd = 1,
                2 => ed.zoom_cmd = 2,
                _ => {
                    ed.measure = !ed.measure;
                    ed.mpts.clear();
                }
            }
        }
        x += w + 5.0;
    }
}

/// Canvas size, measuring and tracing-image controls in the editor panel.
fn tools_ui(ui: &mut egui::Ui, ed: &mut ShapeEd) {
    if ed.trace_rx.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
    if let Some(rx) = &ed.trace_rx {
        if let Ok(res) = rx.try_recv() {
            ed.trace_rx = None;
            if let Some(path) = res {
                let mut r = RefImage::new(&path);
                r.ensure(ui.ctx());
                if r.tex.is_some() {
                    r.default_scale(if ed.canvas[0] > 0.0 { ed.canvas[0] } else { (ed.shape.w * 2.0).max(100.0) });
                    r.opacity = 0.5;
                    ed.trace = Some(r);
                    ed.msg = "Image loaded. Use Move / resize to place it, or Mark two points to set its real size.".into();
                } else {
                    ed.msg = if r.err.is_empty() { "Could not read that image".into() } else { r.err.clone() };
                }
            }
        }
    }
    card(ui, "View tools", |ui| {
        if ed.drawing {
            ui.horizontal_wrapped(|ui| {
                ui.label("Canvas");
                let (mut w, mut h) = (ed.canvas[0], ed.canvas[1]);
                let a = ui.add(egui::DragValue::new(&mut w).range(10.0..=3000.0).speed(1.0).mm()).changed();
                ui.label("x");
                let b = ui.add(egui::DragValue::new(&mut h).range(10.0..=3000.0).speed(1.0).mm()).changed();
                if a || b {
                    ed.canvas = [w, h];
                    ed.fitted = false;
                }
            });
        }
        ui.horizontal(|ui| {
            if pill_button(ui, "Measure", ed.measure).on_hover_text("Click two points on the canvas to read the distance").clicked() {
                ed.measure = !ed.measure;
                ed.mpts.clear();
            }
            if ui.button("Zoom to fit").clicked() {
                ed.zoom_cmd = 2;
            }
        });
        ui.label(egui::RichText::new("Scroll to zoom, drag the empty canvas to pan.").size(12.0).color(MUTED));
    });
    let mut remove = false;
    let mut msg = String::new();
    let mut load = false;
    card(ui, "Trace an image", |ui| {
        let Some(r) = ed.trace.as_mut() else {
            ui.label(egui::RichText::new("Put a photo or drawing behind the canvas to trace over. It stays on this computer.").size(12.0).color(MUTED));
            ui.add_space(4.0);
            if ui.button("Load image…").clicked() {
                load = true;
            }
            return;
        };
        // 1. what you see
        ui.label(egui::RichText::new("Appearance").size(11.5).color(MUTED));
        ui.horizontal(|ui| {
            ui.checkbox(&mut r.visible, "Show");
            ui.add(egui::Slider::new(&mut r.opacity, 0.1..=1.0).text("Opacity"));
        });
        // 2. placing it
        sub_rule(ui);
        ui.label(egui::RichText::new("Place and size").size(11.5).color(MUTED));
        ui.horizontal_wrapped(|ui| {
            if pill_button(ui, "Move / resize", r.move_mode).on_hover_text("Drag the image to move it, drag a corner to resize it").clicked() {
                r.move_mode = !r.move_mode;
                r.stage = Stage::Idle;
            }
            if ui.button("Mark two points").on_hover_text("Click two points on the image whose real distance you know").clicked() {
                r.stage = Stage::PickA;
                r.move_mode = false;
            }
        });
        if let Some(px) = r.marked_px() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(format!("Marked {:.0} px apart. Real distance", px));
                ui.add(egui::DragValue::new(&mut r.dist).range(0.1..=5000.0).speed(0.5).mm());
            });
            ui.horizontal(|ui| {
                if primary_button(ui, "Set size").clicked() {
                    match r.calibrate() {
                        Ok(s) => msg = format!("Image scaled: {} per pixel", crate::units::len(s, 3)),
                        Err(e) => msg = e,
                    }
                }
                if ui.button("Level across").clicked() {
                    r.level(false);
                }
                if ui.button("Level up").clicked() {
                    r.level(true);
                }
            });
        }
        // 3. by number
        sub_rule(ui);
        ui.label(egui::RichText::new("By number").size(11.5).color(MUTED));
        let (mut wmm, mut hmm) = r.size_mm();
        ui.horizontal(|ui| {
            ui.label("Center");
            ui.add(egui::DragValue::new(&mut r.cx).speed(0.5).prefix("x ").mm());
            ui.add(egui::DragValue::new(&mut r.cy).speed(0.5).prefix("y ").mm());
        });
        ui.horizontal(|ui| {
            ui.label("Size");
            if ui.add(egui::DragValue::new(&mut wmm).range(1.0..=20000.0).speed(1.0).prefix("w ").mm()).changed() {
                r.set_width(wmm);
            }
            if ui.add(egui::DragValue::new(&mut hmm).range(1.0..=20000.0).speed(1.0).prefix("h ").mm()).changed() {
                r.set_height(hmm);
            }
        });
        ui.checkbox(&mut r.lock, "Keep proportions").on_hover_text("On: resizing keeps the picture's shape. Off: width and height change on their own.");
        ui.horizontal(|ui| {
            ui.label("Rotate");
            ui.add(egui::DragValue::new(&mut r.rot).range(-180.0..=180.0).speed(0.2).suffix("°"));
            ui.checkbox(&mut r.flip, "Flip");
        });
        // 4. the file
        sub_rule(ui);
        ui.horizontal(|ui| {
            if ui.button("Replace…").clicked() {
                load = true;
            }
            if ui.button("Remove").clicked() {
                remove = true;
            }
        });
    });
    if load && ed.trace_rx.is_none() {
        ed.trace_rx = Some(crate::refimg::pick_file());
    }
    if !msg.is_empty() {
        ed.msg = msg;
    }
    if remove {
        ed.trace = None;
    }
}
