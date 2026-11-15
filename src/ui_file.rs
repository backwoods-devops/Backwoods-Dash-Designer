// File menu, shortcuts and the "save changes?" prompt.
use crate::app::*;
use crate::file;
use crate::theme::*;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum FileAct {
    NewDash,
    Open,
    OpenPath(String),
    Quit,
}

pub enum Pick {
    Open,
    SaveAs(Option<FileAct>),
}

pub struct FileState {
    pub path: Option<PathBuf>,
    pub sig: u64,
    pub dirty: bool,
    pub checked: Instant,
    pub rx: Option<(Pick, std::sync::mpsc::Receiver<Option<String>>)>,
    pub ask: Option<FileAct>,
    pub allow_close: bool,
    pub title: String,
    /// The start-up pop-up (create a new dash or open one) is showing.
    pub welcome: bool,
    /// The thank-you note window is open (shown at every launch, and again from the "Our story" button).
    pub story: bool,
}

impl FileState {
    pub fn new(dir: &std::path::Path) -> FileState {
        FileState { path: None, sig: file::signature(dir), dirty: false, checked: Instant::now(), rx: None, ask: None, allow_close: false, title: String::new(), welcome: true, story: std::env::var("DASHFW_DOCSHOT").map(|v| v == "story").unwrap_or(true) }
    }
}

impl App {
    pub fn file_name(&self) -> String {
        self.file.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into())
    }

    /// Write everything the working folder needs to hold right now.
    fn file_flush(&mut self) {
        let d = self.st.dir.clone();
        self.st.save();
        self.st.dirty = false;
        self.sw.save(&d);
        self.light.save(&d);
        let _ = crate::shapes::save(&d, &self.shapes, self.shape_next);
        crate::refimg::save_all(&d, &self.refs);
    }

    pub fn file_request(&mut self, act: FileAct) {
        if self.file.dirty && act != FileAct::Open {
            self.file.ask = Some(act);
        } else if self.file.dirty && act == FileAct::Open {
            self.file.ask = Some(act);
        } else {
            self.file_do(act);
        }
    }

    fn file_do(&mut self, act: FileAct) {
        match act {
            FileAct::NewDash => {
                self.file.welcome = false;
                file::clear(&self.st.dir);
                self.reload_from_dir();
                self.file.path = None;
                self.file.sig = file::signature(&self.st.dir);
                self.file.dirty = false;
                self.open_wizard();
            }
            FileAct::Open => {
                let start = self.file.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string());
                self.file.rx = Some((Pick::Open, file::pick(false, start, String::new())));
            }
            FileAct::OpenPath(p) => self.file_open(&p),
            FileAct::Quit => {
                self.file.allow_close = true;
                self.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn file_open(&mut self, p: &str) {
        let path = PathBuf::from(p);
        match file::read_file(&path, &self.st.dir) {
            Ok(()) => {
                self.file.welcome = false;
                self.reload_from_dir();
                self.file.path = Some(path.clone());
                self.file.sig = file::signature(&self.st.dir);
                self.file.dirty = false;
                file::push_recent(&self.st.dir, &path);
                self.toast(&format!("Opened {}", self.file_name()), false);
            }
            Err(e) => self.toast(&e, true),
        }
    }

    pub fn file_save(&mut self, then: Option<FileAct>) {
        let Some(path) = self.file.path.clone() else {
            return self.file_save_as(then);
        };
        self.file_flush();
        match file::write_file(&self.st.dir, &path) {
            Ok(()) => {
                self.file.sig = file::signature(&self.st.dir);
                self.file.dirty = false;
                file::push_recent(&self.st.dir, &path);
                self.toast(&format!("Saved {}", self.file_name()), false);
                if let Some(a) = then {
                    self.file_do(a);
                }
            }
            Err(e) => self.toast(&e, true),
        }
    }

    pub fn file_save_as(&mut self, then: Option<FileAct>) {
        let start = self.file.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string());
        let name = format!("{}.{}", self.file_name(), file::EXT);
        self.file.rx = Some((Pick::SaveAs(then), file::pick(true, start, name)));
    }

    /// Re-read everything from the working folder (after New or Open).
    pub fn reload_from_dir(&mut self) {
        let dir = self.st.dir.clone();
        self.light = crate::lighting::Lighting::load(&dir);
        self.sw = crate::sidewall::SideWall::load(&dir);
        self.st = crate::engine::State::new(dir.clone());
        let (shapes, next) = crate::shapes::load(&dir);
        self.shapes = shapes;
        self.shape_next = next;
        self.refs = crate::refimg::load_all(&dir);
        crate::shapes::mesh_clear_cache();
        self.size_orig = None;
        self.piece_vis = [true, true, self.st.trim_on];
        self.wiz = None;
        self.shape_ed = None;
        self.sel_prof = self.st.proj.profiles.len() / 2;
        self.sel_pt = 1;
        self.multi.clear();
        self.sel_rail = None;
        self.sel_sec = None;
        self.sel_part = None;
        self.sel_cut = None;
        self.focus = Focus::Dash;
        self.parts_key = (0, None);
        self.mesh = None;
        self.cut_key = (u64::MAX, 0);
        self.cut_obj_key = (u64::MAX, 0);
        self.wall_key = (u64::MAX, false, 0);
        self.other_key = (u64::MAX, 0, 0);
        self.rev += 1;
        self.mesh_rev += 1;
        self.last_save = Instant::now();
    }

    /// Per-frame: dialog results, shortcuts, window close, change tracking, title, prompt.
    pub fn file_frame(&mut self, ctx: &egui::Context) {
        if self.file.rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        if let Some((_, rx)) = &self.file.rx {
            if let Ok(res) = rx.try_recv() {
                let (kind, _) = self.file.rx.take().unwrap();
                if let Some(p) = res {
                    match kind {
                        Pick::Open => self.file_open(&p),
                        Pick::SaveAs(then) => {
                            let path = file::with_ext(&p);
                            self.file.path = Some(path);
                            self.file_save(then);
                        }
                    }
                }
            }
        }
        if !self.file.allow_close && ctx.input(|i| i.viewport().close_requested()) {
            if self.file.dirty {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.file.ask = Some(FileAct::Quit);
            }
        }
        let busy = ctx.egui_wants_keyboard_input() && false;
        if !busy {
            let (s, sa, o) = ctx.input_mut(|i| {
                let c = egui::Modifiers::COMMAND;
                let cs = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
                (i.consume_key(c, egui::Key::S), i.consume_key(cs, egui::Key::S), i.consume_key(c, egui::Key::O))
            });
            if sa {
                self.file_save_as(None);
            } else if s {
                self.file_save(None);
            }
            if o {
                self.file_request(FileAct::Open);
            }
        }
        if self.file.checked.elapsed().as_millis() > 1000 && !ctx.input(|i| i.pointer.any_down()) && !self.st.dirty {
            self.file.checked = Instant::now();
            self.file.dirty = file::signature(&self.st.dir) != self.file.sig;
        }
        let title = format!("{}{} — Backwoods Dash Designer", self.file_name(), if self.file.dirty { " •" } else { "" });
        if title != self.file.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.file.title = title;
        }
        if let Some(act) = self.file.ask.clone() {
            let mut choice: Option<u8> = None;
            egui::Window::new("Save changes?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                ui.label(format!("Save changes to \"{}\" first?", self.file_name()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Save").clicked() { choice = Some(0); }
                    if ui.button("Don't save").clicked() { choice = Some(1); }
                    if ui.button("Cancel").clicked() { choice = Some(2); }
                });
            });
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { choice = Some(2); }
            match choice {
                Some(0) => { self.file.ask = None; self.file_save(Some(act)); }
                Some(1) => { self.file.ask = None; self.file_do(act); }
                Some(2) => self.file.ask = None,
                _ => {}
            }
        }
    }

    /// Shortcut text with plain words: the symbol glyphs are not in the app font.
    fn mod_key(k: &str, shift: bool) -> String {
        format!("{}{}+{}", if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" }, if shift { "+Shift" } else { "" }, k)
    }

    pub fn file_menu(&mut self, ui: &mut egui::Ui) {
        let name = format!("{}{}", self.file_name(), if self.file.dirty { " •" } else { "" });
        ui.menu_button("File", |ui| {
            if ui.add(egui::Button::new("Open…").shortcut_text(Self::mod_key("O", false))).clicked() { ui.close(); self.file_request(FileAct::Open); }
            let recent = file::recent(&self.st.dir);
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button("Open recent", |ui| {
                    for r in recent.iter() {
                        let label = std::path::Path::new(r).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| r.clone());
                        if ui.button(label).on_hover_text(r).clicked() { ui.close(); self.file_request(FileAct::OpenPath(r.clone())); }
                    }
                });
            });
            ui.separator();
            if ui.add(egui::Button::new("Save").shortcut_text(Self::mod_key("S", false))).clicked() { ui.close(); self.file_save(None); }
            if ui.add(egui::Button::new("Save as…").shortcut_text(Self::mod_key("S", true))).clicked() { ui.close(); self.file_save_as(None); }
            ui.separator();
            if ui.button("Open last session (autosave)").on_hover_text("Each launch starts on a fresh dash; the previous one is kept here").clicked() {
                ui.close();
                let ad = self.st.dir.join("autosave");
                let mut all: Vec<PathBuf> = std::fs::read_dir(&ad).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
                all.sort();
                match all.pop() {
                    Some(p) => self.file_request(FileAct::OpenPath(p.to_string_lossy().to_string())),
                    None => self.toast("No earlier session saved yet", false),
                }
            }
        });
        ui.label(egui::RichText::new(name).size(12.0).color(MUTED));
    }
}

/// The Backwoods Fabrications logo (kept inside the program, never fetched from anywhere).
fn logo_tex(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("backwoods_logo_tex");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let img = image::load_from_memory(include_bytes!("logo.png")).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::new(1, 1));
    let (w, h) = img.dimensions();
    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let t = ctx.load_texture("backwoods_logo", ci, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

fn dim(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// The logo mark: a dash cross-section (top run, front arc, drop and lip hook), like the profiles in the editor.
fn logo(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let st = Stroke::new((r * 0.16).max(2.0), col);
    let mut pts: Vec<Pos2> = Vec::new();
    let at = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    pts.push(at(-0.95, -0.55));
    pts.push(at(0.1, -0.7));
    for i in 0..=8 {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2 * 1.15 * i as f32 / 8.0;
        pts.push(at(0.1 + 0.62 * a.cos(), -0.08 + 0.62 * a.sin()));
    }
    pts.push(at(0.72, 0.55));
    for i in 1..=5 {
        let a = std::f32::consts::FRAC_PI_2 * i as f32 / 5.0;
        pts.push(at(0.5 + 0.22 * a.cos(), 0.55 + 0.22 * a.sin()));
    }
    pts.push(at(-0.1, 0.77));
    p.add(egui::Shape::line(pts.clone(), st));
    for q in [pts[0], pts[pts.len() - 1]] {
        p.circle_filled(q, st.width * 0.7, col);
    }
}

/// Icons drawn with the painter so the screen needs no image files.
fn card_icon(p: &egui::Painter, kind: u8, c: Pos2, r: f32, col: Color32) {
    let st = Stroke::new(3.0, col);
    match kind {
        0 => {
            // clock with a return arrow: open the last one
            p.circle_stroke(c, r * 0.62, st);
            p.line_segment([c, c + vec2(0.0, -r * 0.40)], st);
            p.line_segment([c, c + vec2(r * 0.28, r * 0.14)], st);
            let a = c + vec2(-r * 0.62, -r * 0.1);
            p.line_segment([a, a + vec2(-r * 0.2, -r * 0.22)], st);
            p.line_segment([a, a + vec2(r * 0.22, -r * 0.2)], st);
        }
        1 => {
            // folder
            let (w, h) = (r * 1.5, r * 1.05);
            let body = Rect::from_center_size(c + vec2(0.0, r * 0.1), vec2(w, h));
            p.rect_stroke(body, CornerRadius::same(5), st, StrokeKind::Middle);
            let tab = Rect::from_min_size(body.left_top() + vec2(0.0, -r * 0.22), vec2(r * 0.6, r * 0.24));
            p.rect_stroke(tab, CornerRadius::same(3), st, StrokeKind::Middle);
            p.line_segment([body.left_top() + vec2(r * 0.1, r * 0.3), body.right_top() + vec2(-r * 0.1, r * 0.3)], Stroke::new(2.0, dim(col, 150)));
        }
        _ => {
            // a dash with a plus: start a new one
            logo(p, c + vec2(-r * 0.12, r * 0.05), r * 0.82, col);
            let q = c + vec2(r * 0.62, -r * 0.55);
            p.circle_filled(q, r * 0.3, col);
            p.line_segment([q + vec2(-r * 0.15, 0.0), q + vec2(r * 0.15, 0.0)], Stroke::new(2.5, BG));
            p.line_segment([q + vec2(0.0, -r * 0.15), q + vec2(0.0, r * 0.15)], Stroke::new(2.5, BG));
        }
    }
}

/// Small vector icons for the support buttons (drawn, no image files): 0 PayPal, 1 YouTube, 2 website globe, 3 Facebook. `k` scales them.
fn support_icon(p: &egui::Painter, kind: u8, c: Pos2, k: f32) {
    match kind {
        0 => {
            p.text(c + vec2(1.0, 0.0) * k, Align2::CENTER_CENTER, "P", FontId::proportional(18.0 * k), Color32::from_rgb(0, 156, 222));
            p.text(c + vec2(-1.0, 1.0) * k, Align2::CENTER_CENTER, "P", FontId::proportional(18.0 * k), Color32::from_rgb(0, 48, 135));
        }
        1 => {
            p.rect_filled(Rect::from_center_size(c, vec2(20.0, 14.0) * k), CornerRadius::same((4.0 * k) as u8), Color32::from_rgb(255, 0, 0));
            p.add(egui::Shape::convex_polygon(vec![c + vec2(-2.5, -3.5) * k, c + vec2(-2.5, 3.5) * k, c + vec2(3.5, 0.0) * k], Color32::WHITE, Stroke::NONE));
        }
        2 => {
            let st = Stroke::new(1.4 * k, Color32::from_rgb(150, 190, 240));
            p.circle_stroke(c, 8.5 * k, st);
            p.line_segment([c + vec2(-8.5, 0.0) * k, c + vec2(8.5, 0.0) * k], st);
            let n = 24;
            let pts: Vec<Pos2> = (0..=n).map(|i| { let a = i as f32 / n as f32 * std::f32::consts::TAU; c + vec2(a.cos() * 4.0, a.sin() * 8.5) * k }).collect();
            p.add(egui::Shape::line(pts, st));
        }
        4 => {
            let col = Color32::from_rgb(255, 122, 26);
            p.circle_filled(c + vec2(-4.4, -2.6) * k, 4.8 * k, col);
            p.circle_filled(c + vec2(4.4, -2.6) * k, 4.8 * k, col);
            p.add(egui::Shape::convex_polygon(vec![c + vec2(-8.9, -0.9) * k, c + vec2(8.9, -0.9) * k, c + vec2(0.0, 9.0) * k], col, Stroke::NONE));
        }
        _ => {
            p.circle_filled(c, 9.0 * k, Color32::from_rgb(24, 119, 242));
            p.text(c + vec2(1.5, 2.5) * k, Align2::CENTER_CENTER, "f", FontId::proportional(15.0 * k), Color32::WHITE);
        }
    }
}

impl App {
    /// The first screen: three big choices side by side, then recent files.
    /// A "made by / support" strip between the title and the three cards: the credit centered on top, then link buttons shaped like the cards below (big icon over the label).
    fn support_card(&mut self, ui: &mut egui::Ui, r: Rect) {
        let p = ui.painter().clone();
        p.rect_filled(r, CornerRadius::same(14), dim(Color32::WHITE, 6));
        p.rect_stroke(r, CornerRadius::same(14), Stroke::new(1.0, LINE), StrokeKind::Inside);
        p.text(pos2(r.center().x, r.top() + 24.0), Align2::CENTER_CENTER, "Free to use, made by", FontId::proportional(14.0), MUTED);
        p.text(pos2(r.center().x, r.top() + 48.0), Align2::CENTER_CENTER, "Chris Weston", FontId::proportional(20.0), TEXT);
        p.text(pos2(r.center().x, r.top() + 70.0), Align2::CENTER_CENTER, "Backwoods Fabrications", FontId::proportional(14.0), ACCENT);
        let items = [("Say thanks (PayPal)", crate::links::DONATE_URL, 0u8), ("YouTube", crate::links::YOUTUBE_URL, 1), ("backwoodsfab.com", crate::links::WEBSITE_URL, 2), ("Facebook", crate::links::FACEBOOK_URL, 3), ("Our story", "", 4)];
        let (bw, bh, gap) = (150.0f32, 84.0f32, 14.0f32);
        let mut x = r.center().x - (bw * 5.0 + gap * 4.0) / 2.0;
        for (label, url, kind) in items {
            let b = Rect::from_min_size(pos2(x, r.top() + 86.0), vec2(bw, bh));
            let resp = ui.interact(b, egui::Id::new(("support", label)), egui::Sense::click());
            let hot = resp.hovered();
            p.rect_filled(b, CornerRadius::same(12), dim(Color32::WHITE, if hot { 16 } else { 8 }));
            support_icon(&p, kind, pos2(b.center().x, b.top() + 28.0), 1.5);
            p.text(pos2(b.center().x, b.top() + 64.0), Align2::CENTER_CENTER, label, FontId::proportional(13.0), TEXT);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                if kind == 4 { self.file.story = true; } else { crate::links::open(url); }
            }
            x += bw + gap;
        }
    }

    pub fn welcome_ui(&mut self, ui: &mut egui::Ui) {
        let outer = ui.max_rect();
        let view = Rect::from_min_max(outer.min, pos2(outer.right(), outer.bottom() - 44.0));
        let recent: Vec<String> = file::recent(&self.st.dir).into_iter().filter(|q| std::path::Path::new(q).exists()).collect();
        let last: Option<(String, String, bool)> = recent.first().map(|q| (q.clone(), std::path::Path::new(q).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), false)).or_else(|| {
            let mut all: Vec<PathBuf> = std::fs::read_dir(self.st.dir.join("autosave")).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
            all.sort();
            all.pop().map(|q| (q.to_string_lossy().to_string(), "Last session".to_string(), true))
        });
        let extra = recent.len().min(4).saturating_sub(if recent.is_empty() { 0 } else { 1 });
        let content_h = 267.0 + 30.0 + 250.0 + 202.0 + if extra > 0 { 40.0 + extra as f32 * 34.0 } else { 0.0 };
        // everything above the footer scrolls when the window is shorter than the content
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
        let act: Option<FileAct> = egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut child, |ui| {
        let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), (content_h + 60.0).max(view.height())), Sense::hover());
        let p = ui.painter().clone();
        let top = (area.center().y - content_h / 2.0).max(area.top() + 24.0);
        let cx = area.center().x;
        // the Backwoods Fabrications logo, then the app name with a 3D picture of a dash beside it
        let tex = logo_tex(ui.ctx());
        let ls = tex.size_vec2();
        let lw = 460.0f32.min(area.width() - 40.0);
        let lrect = Rect::from_center_size(pos2(cx, top + lw * ls.y / ls.x / 2.0), vec2(lw, lw * ls.y / ls.x));
        p.image(tex.id(), lrect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let ty = lrect.bottom() + 40.0;
        let gname = p.layout_no_wrap("Dash Designer".to_string(), FontId::proportional(32.0), TEXT);
        let tw = 120.0 + 14.0 + gname.size().x;
        let tx = cx - tw / 2.0;
        crate::ui_wiz::thumb_view(ui, Rect::from_center_size(pos2(tx + 60.0, ty), vec2(120.0, 64.0)), "wrap", [-0.38, 0.5, 1.0, 0.0, 0.0]);
        p.galley(pos2(tx + 134.0, ty - gname.size().y / 2.0), gname, TEXT);
        p.text(pos2(cx, ty + 52.0), Align2::CENTER_CENTER, "Design 3D-printable dashes, panels and trim", FontId::proportional(15.0), MUTED);
        p.line_segment([pos2(cx - 60.0, ty + 78.0), pos2(cx + 60.0, ty + 78.0)], Stroke::new(2.0, dim(ACCENT, 160)));
        // three cards
        let (cw, ch, gap) = (300.0f32, 250.0f32, 22.0f32);
        let total = cw * 3.0 + gap * 2.0;
        let x0 = cx - total / 2.0;
        let strip = Rect::from_min_size(pos2(x0, lrect.bottom() + 40.0 + 78.0 + 20.0), vec2(total, 184.0));
        self.support_card(ui, strip);
        let y0 = strip.bottom() + 18.0;
        let lastname = last.as_ref().map(|l| l.1.clone());
        let cards: [(u8, &str, String, bool); 3] = [
            (0, "Open last", match (&last, &lastname) { (Some((_, n, auto)), _) => if *auto { "Pick up where you left off: the dash you were working on last time.".to_string() } else { format!("Back to \"{}\", the dash you saved most recently.", n) }, _ => "Nothing to reopen yet. Your last dash shows up here.".to_string() }, last.is_some()),
            (1, "Open existing", "Browse for a saved .bwd file on this computer.".to_string(), true),
            (2, "Create new", "Build a dash from a style with the step-by-step wizard: size, windshield line, front shape and sections.".to_string(), true),
        ];
        let primary = if last.is_some() { 0 } else { 2 };
        let mut act: Option<FileAct> = None;
        for (i, (kind, title, desc, enabled)) in cards.iter().enumerate() {
            let mut r = Rect::from_min_size(pos2(x0 + i as f32 * (cw + gap), y0), vec2(cw, ch));
            let resp = ui.interact(r, egui::Id::new(("welcome_card", i)), if *enabled { Sense::click() } else { Sense::hover() });
            let hot = *enabled && resp.hovered();
            if hot {
                r = r.translate(vec2(0.0, -4.0));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let is_primary = i == primary;
            p.rect_filled(r.translate(vec2(0.0, 6.0)).expand(1.0), CornerRadius::same(18), Color32::from_black_alpha(if hot { 90 } else { 50 }));
            p.rect_filled(r, CornerRadius::same(16), if hot { CARD_HI } else { CARD });
            p.rect_stroke(r, CornerRadius::same(16), Stroke::new(if is_primary || hot { 2.0 } else { 1.0 }, if hot || (is_primary && *enabled) { ACCENT } else { LINE }), StrokeKind::Inside);
            let col = if !*enabled { dim(MUTED, 120) } else if hot || is_primary { ACCENT } else { TEXT };
            let ic = pos2(r.center().x, r.top() + 62.0);
            p.circle_filled(ic, 40.0, if hot || (is_primary && *enabled) { dim(ACCENT, 34) } else { dim(Color32::WHITE, 8) });
            card_icon(&p, *kind, ic, 30.0, col);
            p.text(pos2(r.center().x, r.top() + 128.0), Align2::CENTER_CENTER, *title, FontId::proportional(21.0), if *enabled { TEXT } else { MUTED });
            let g = p.layout(desc.clone(), FontId::proportional(13.0), MUTED, cw - 44.0);
            p.galley(pos2(r.left() + 22.0, r.top() + 150.0), g, MUTED);
            if *enabled {
                p.text(pos2(r.center().x, r.bottom() - 20.0), Align2::CENTER_CENTER, if hot { "Click to start" } else { "" }, FontId::proportional(13.0), ACCENT);
            }
            if resp.clicked() {
                act = Some(match kind {
                    0 => FileAct::OpenPath(last.as_ref().map(|l| l.0.clone()).unwrap_or_default()),
                    1 => FileAct::Open,
                    _ => FileAct::NewDash,
                });
            }
        }
        // recent files
        let mut y = y0 + ch + 34.0;
        if extra > 0 {
            p.text(pos2(x0, y), Align2::LEFT_CENTER, "RECENT", FontId::proportional(11.5), MUTED);
            y += 18.0;
            for q in recent.iter().skip(1).take(4) {
                let row = Rect::from_min_size(pos2(x0, y), vec2(total, 30.0));
                let resp = ui.interact(row, egui::Id::new(("welcome_recent", q.as_str())), Sense::click());
                if resp.hovered() {
                    p.rect_filled(row, CornerRadius::same(8), CARD);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let nm = std::path::Path::new(q).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                p.text(pos2(row.left() + 12.0, row.center().y), Align2::LEFT_CENTER, nm, FontId::proportional(14.0), TEXT);
                let dir = std::path::Path::new(q).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                p.text(pos2(row.right() - 12.0, row.center().y), Align2::RIGHT_CENTER, dir, FontId::proportional(12.0), MUTED);
                if resp.clicked() {
                    act = Some(FileAct::OpenPath(q.clone()));
                }
                y += 34.0;
            }
        }
        if self.file.rx.is_some() {
            p.text(pos2(cx, y0 + ch + 12.0), Align2::CENTER_CENTER, "Choose a file in the window that just opened\u{2026}", FontId::proportional(13.0), ACCENT);
        }
        act
        }).inner;
        let area = outer;
        let cx = area.center().x;
        let p = ui.painter().clone();
        // units: millimeters or inches (metric is the default)
        {
            let inch = crate::units::inch();
            p.text(pos2(area.left() + 28.0, area.bottom() - 22.0), Align2::LEFT_CENTER, "Units", FontId::proportional(12.5), MUTED);
            for (k, (name, on)) in [("mm", !inch), ("in", inch)].iter().enumerate() {
                let r = Rect::from_center_size(pos2(area.left() + 98.0 + k as f32 * 46.0, area.bottom() - 22.0), vec2(42.0, 26.0));
                let resp = ui.interact(r, egui::Id::new(("welcome_unit", k)), Sense::click());
                p.rect_filled(r, CornerRadius::same(13), if *on { dim(ACCENT, 60) } else if resp.hovered() { CARD_HI } else { CARD });
                p.rect_stroke(r, CornerRadius::same(13), Stroke::new(if *on { 1.5 } else { 1.0 }, if *on { ACCENT } else { LINE }), StrokeKind::Inside);
                p.text(r.center(), Align2::CENTER_CENTER, *name, FontId::proportional(12.5), if *on { ACCENT } else { MUTED });
                if resp.clicked() && *on == false {
                    crate::units::set(k == 1, &self.st.dir);
                }
            }
        }
        let hr = Rect::from_center_size(pos2(area.right() - 56.0, area.bottom() - 22.0), vec2(80.0, 26.0));
        let hresp = ui.interact(hr, egui::Id::new("welcome_help"), Sense::click());
        p.rect_filled(hr, CornerRadius::same(13), if hresp.hovered() { CARD_HI } else { CARD });
        p.rect_stroke(hr, CornerRadius::same(13), Stroke::new(1.0, if hresp.hovered() { ACCENT } else { LINE }), StrokeKind::Inside);
        p.text(hr.center(), Align2::CENTER_CENTER, "Help (F1)", FontId::proportional(12.5), if hresp.hovered() { ACCENT } else { MUTED });
        if hresp.clicked() {
            self.help_open = true;
        }
        p.text(pos2(cx, area.bottom() - 22.0), Align2::CENTER_CENTER, "Backwoods Fabrications  \u{00B7}  your dashes are saved as .bwd files", FontId::proportional(12.0), dim(MUTED, 170));
        if let Some(a) = act {
            self.file_do(a);
        }
        self.story_window(&ui.ctx().clone());
    }

    /// The thank-you note: a centered window over a dimmed start screen. 
    fn story_window(&mut self, ctx: &egui::Context) {
        if !self.file.story { return; }
        let screen = ctx.content_rect();
        // dim and block everything behind it
        egui::Area::new(egui::Id::new("story_dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(165));
        });
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let w = 660.0f32.min(screen.width() - 40.0);
        let h = (screen.height() - 70.0).min(700.0);
        egui::Window::new("story_note")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .fixed_size(vec2(w, h))
            .frame(egui::Frame::new().fill(CARD).stroke(Stroke::new(1.5, dim(ACCENT, 170))).corner_radius(CornerRadius::same(18)).inner_margin(egui::Margin::same(26)))
            .show(ctx, |ui| {
                let tex = logo_tex(ui.ctx());
                let ls = tex.size_vec2();
                let lw = 240.0;
                ui.vertical_centered(|ui| { ui.add(egui::Image::new((tex.id(), vec2(lw, lw * ls.y / ls.x)))); });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(h - 52.0 - 150.0).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(crate::links::STORY_TITLE).size(23.0).strong().color(ACCENT)).wrap());
                    ui.add_space(12.0);
                    for para in crate::links::STORY {
                        ui.add(egui::Label::new(egui::RichText::new(*para).size(15.0).color(TEXT)).wrap());
                        ui.add_space(10.0);
                    }
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(egui::RichText::new(crate::links::STORY_SIGN).size(17.0).strong().color(TEXT)));
                    ui.add(egui::Label::new(egui::RichText::new("Backwoods Fabrications").size(14.0).color(ACCENT)));
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(egui::RichText::new(crate::links::STORY_TAG).size(13.5).italics().color(MUTED)).wrap());
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    for (label, url) in [("Donate (PayPal)", crate::links::DONATE_URL), ("YouTube", crate::links::YOUTUBE_URL), ("backwoodsfab.com", crate::links::WEBSITE_URL), ("Facebook", crate::links::FACEBOOK_URL)] {
                        if ui.add(egui::Button::new(egui::RichText::new(label).size(13.0))).clicked() {
                            crate::links::open(url);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Close").size(14.0).color(ACCENT)).min_size(vec2(90.0, 28.0))).clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close {
            self.file.story = false;
        }
    }
}
