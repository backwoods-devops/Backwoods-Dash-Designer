// The "New dash" wizard: template cards with thumbnails, then size, windshield line, front profile, sections, parts.
use crate::app::*;
use crate::templates;
use crate::theme::*;
use crate::ui_panels::{draw_icon, json_str};
use crate::units::MmExt;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, Sense, Shape, Stroke, StrokeKind};

const STEPS: [&str; 6] = ["Start", "Size", "Windshield line", "Front profile", "Sections", "Finish"];
const KINDS: [(&str, &str, &str); 5] = [
    ("wrap", "Twin-pad wrap", "Pocketed (W style) front line, windshield line bowing out and up toward the middle. A classic 1970s two-pad look."),
    ("smooth", "Smooth bow", "Same sweep with a straight front line and a gentler bow."),
    ("bench", "Straight bench", "Flat across with three profiles. The simplest dash."),
    ("squarebody", "Squarebody truck", "Deep, nearly flat top with the driver side stepping out past the center. Chevy C10 style. No top trim."),
    ("blank", "Minimal", "Three profiles with a light bow. A clean starting point."),
];

impl App {
    pub fn open_wizard(&mut self) {
        let u0 = self.st.undo.len();
        self.op_s(r#"{"op":"checkpoint"}"#);
        self.wiz = Some(Wiz { step: 0, p: templates::preset("wrap"), inout: false, applied: false, undo_start: u0, view: DEFAULT_VIEW, focus_sec: None });
        // a new dash starts fresh: the default template is in place from the first screen (Cancel puts the old dash back)
        self.wiz_apply();
    }

    fn wiz_apply(&mut self) {
        let Some(w) = &self.wiz else { return };
        let p = &w.p;
        let s = format!(
            r#"{{"op":"apply_template","live":{},"kind":{},"width":{},"depth":{},"height":{},"front_r":{},"slope_deg":{},"hook":{},"hook_r":{},"nprof":{},"bow_y":{},"bow_z":{},"end_depth":{},"crease":{},"centre_depth":{},"centre_out":{}}}"#,
            w.applied, json_str(&p.kind), p.width, p.depth, p.height, p.front_r, p.slope_deg, p.hook, p.hook_r, p.nprof, p.bow_y, p.bow_z, p.end_depth, p.crease, p.centre_depth, p.centre_out
        );
        self.op(s);
        if let Some(w) = &mut self.wiz {
            w.applied = true;
        }
        self.sel_prof = self.st.proj.profiles.len() / 2;
        self.side.fitted = false;
        self.plan.fitted = false;
        self.elev.fitted = false;
    }

    fn wiz_close(&mut self, cancel: bool) {
        if let Some(w) = self.wiz.take() {
            if cancel {
                while self.st.undo.len() > w.undo_start {
                    self.op_s(r#"{"op":"undo"}"#);
                }
                self.side.fitted = false;
                self.plan.fitted = false;
                self.elev.fitted = false;
            }
        }
    }

    pub fn ui_wizard(&mut self, ctx: &egui::Context) {
        if self.wiz.is_none() {
            return;
        }
        let screen = ctx.content_rect();
        let mut next = false;
        let mut back = false;
        let mut cancel = false;
        let mut apply = false;
        let mut w = self.wiz.take().unwrap();
        // every step takes most of the window; steps after the first show a live 3D picture of the dash beside the controls
        let big = true;
        let body_h = if big { (screen.height() - 230.0).max(300.0) } else { 400.0 };
        egui::Window::new("wizard")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(if big { Align2::CENTER_CENTER } else { Align2::RIGHT_TOP }, if big { [0.0, 20.0] } else { [-14.0, 70.0] })
            .fixed_size([if big { screen.width() * 0.88 } else { 350.0 }, 0.0])
            .frame(egui::Frame::new().fill(PANEL).corner_radius(CornerRadius::same(14)).stroke(Stroke::new(1.0, ACCENT)).inner_margin(egui::Margin::same(18)).shadow(egui::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(140) }))
            .show(ctx, |ui| {
                // title block in the upper right, so the guided setup reads like an official step by step process
                {
                    let (right, top) = (ui.max_rect().right(), ui.min_rect().top());
                    ui.painter().text(pos2(right, top - 4.0), Align2::RIGHT_TOP, "NEW DASH SETUP", FontId::proportional(24.0), ACCENT);
                    ui.painter().text(pos2(right, top + 26.0), Align2::RIGHT_TOP, "Guided step-by-step builder", FontId::proportional(12.5), MUTED);
                }
                ui.horizontal(|ui| {
                    for (i, _) in STEPS.iter().enumerate() {
                        let (r, _) = ui.allocate_exact_size(vec2(52.0, 5.0), Sense::hover());
                        ui.painter().rect_filled(r, CornerRadius::same(3), if i <= w.step { ACCENT } else { LINE });
                    }
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new(format!("Step {} of {}", w.step + 1, STEPS.len())).size(11.5).color(MUTED));
                ui.label(egui::RichText::new(STEPS[w.step]).size(22.0).strong());
                ui.add_space(6.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(body_h).min_scrolled_height(body_h).show(ui, |ui| {
                let split = w.step > 0;
                let avail_w = ui.available_width();
                let left_w = (avail_w * 0.55).max(200.0).min(avail_w - 330.0).max(0.0);
                let top = ui.cursor().min;
                if split && left_w > 120.0 {
                    let pr = Rect::from_min_size(top, vec2(left_w, (body_h - 8.0).max(260.0)));
                    ui.painter().rect_filled(pr, CornerRadius::same(12), CARD);
                    ui.painter().rect_stroke(pr, CornerRadius::same(12), Stroke::new(1.0, LINE), StrokeKind::Inside);
                    let t = if w.step + 1 == STEPS.len() {
                        let mut ps: Vec<&crate::model::Project> = vec![self.st.piece(0), self.st.piece(1)];
                        if self.st.trim_on {
                            ps.push(self.st.piece(2));
                        }
                        full_thumb(self.rev, &ps)
                    } else {
                        live_thumb(self.rev, &self.st.proj)
                    };
                    let r = ui.interact(pr, egui::Id::new("wiz_preview"), Sense::click_and_drag());
                    if r.dragged() {
                        let d = r.drag_delta();
                        if ui.input(|i| i.modifiers.shift) {
                            w.view[3] += d.x;
                            w.view[4] += d.y;
                        } else {
                            w.view[0] += d.x * 0.01;
                            w.view[1] = (w.view[1] + d.y * 0.01).clamp(-0.4, 1.5);
                        }
                    }
                    if r.hovered() {
                        let sc = ui.input(|i| i.smooth_scroll_delta.y);
                        if sc != 0.0 {
                            w.view[2] = (w.view[2] * (1.0 + sc * 0.002)).clamp(0.3, 6.0);
                        }
                        ui.ctx().set_cursor_icon(if r.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
                    }
                    if r.double_clicked() {
                        w.view = DEFAULT_VIEW;
                    }
                    let hover = if w.step == 4 { ui.input(|i| i.pointer.hover_pos()) } else { None };
                    let hit = draw_thumb(ui, pr.shrink(10.0), &t, w.view, false, w.step == 4, hover);
                    if let Some(sec) = hit.and_then(|qi| t.qsec.get(qi).copied().flatten()) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if r.clicked() {
                            w.focus_sec = Some(sec);
                        }
                    }
                    ui.painter().text(pr.left_top() + vec2(14.0, 14.0), Align2::LEFT_CENTER, "Your dash so far", FontId::proportional(12.0), MUTED);
                    ui.painter().text(pr.left_bottom() + vec2(14.0, -14.0), Align2::LEFT_CENTER, "Drag to turn it, scroll to zoom, Shift-drag to slide, double-click to reset", FontId::proportional(11.5), fade(MUTED, 190));
                }
                let area = if split && left_w > 120.0 { Rect::from_min_max(pos2(top.x + left_w + 18.0, top.y), pos2(top.x + avail_w, top.y + 3000.0)) } else { Rect::from_min_size(top, vec2(avail_w, 3000.0)) };
                ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                match w.step {
                    0 => self.wiz_start(ui, &mut w, &mut apply),
                    1 => {
                        hint(ui, if crate::units::inch() { "Overall size in inches." } else { "Overall size in millimeters." });
                        apply |= slider(ui, "Width, end to end", &mut w.p.width, 800.0..=2200.0, 10.0, "mm");
                        apply |= slider(ui, "Depth: windshield line to front edge", &mut w.p.depth, 120.0..=500.0, 5.0, "mm");
                        apply |= slider(ui, "Height: windshield line to floor", &mut w.p.height, 60.0..=300.0, 5.0, "mm");
                        ui.horizontal(|ui| {
                            ui.label("Profiles across");
                            for n in [3usize, 5, 7] {
                                if pill_button(ui, &n.to_string(), w.p.nprof == n).clicked() {
                                    w.p.nprof = n;
                                    apply = true;
                                }
                            }
                        });
                        hint(ui, "More profiles give more control and more to edit.");
                    }
                    2 => {
                        hint(ui, "The windshield line is the rail every profile hangs from. It bows out and up toward the middle, then back down.");
                        apply |= slider(ui, "Bow out at the center", &mut w.p.bow_y, 0.0..=300.0, 5.0, "mm toward the windshield");
                        apply |= slider(ui, "Rise at the center", &mut w.p.bow_z, 0.0..=120.0, 2.0, "mm up");
                        apply |= slider(ui, "End depth", &mut w.p.end_depth, 0.3..=1.2, 0.05, "× center depth");
                        label_caps(ui, "Front line across the middle");
                        // three shapes; each has its own amount
                        let mode = if w.p.crease { 2 } else if w.inout || w.p.centre_out != 0.0 { 1 } else { 0 };
                        let mut m = mode;
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut m, 0, "Straight").on_hover_text("The front line follows the windshield line in one full curve");
                            ui.radio_value(&mut m, 1, "Center in / out").on_hover_text("The middle of the front line comes further out (or further in), in one smooth curve");
                            ui.radio_value(&mut m, 2, "Pocketed").on_hover_text("The middle of the front line dips back to a point: a W, one pocket on each side");
                        });
                        if m != mode {
                            match m {
                                0 => { w.p.crease = false; w.p.centre_out = 0.0; w.inout = false; }
                                1 => { w.p.crease = false; w.inout = true; if w.p.centre_out == 0.0 { w.p.centre_out = 40.0; } }
                                _ => { w.p.crease = true; w.inout = false; }
                            }
                            apply = true;
                        }
                        if m == 1 {
                            apply |= slider(ui, "Center in / out", &mut w.p.centre_out, -150.0..=150.0, 2.0, "mm: positive further out, negative further in");
                        }
                        if m == 2 && w.p.nprof >= 5 {
                            apply |= slider(ui, "Center depth", &mut w.p.centre_depth, 0.2..=1.2, 0.05, "× full depth");
                        }
                    }
                    3 => {
                        hint(ui, "The front profile: top run, front arc, drop, and the small lip underneath.");
                        apply |= slider(ui, "Front arc radius", &mut w.p.front_r, 10.0..=200.0, 5.0, "mm");
                        apply |= slider(ui, "Top slope", &mut w.p.slope_deg, 0.0..=30.0, 1.0, "° falling toward the front");
                        apply |= slider(ui, "Lip length", &mut w.p.hook, 0.0..=120.0, 5.0, "mm");
                        apply |= slider(ui, "Lip bend radius", &mut w.p.hook_r, 2.0..=40.0, 1.0, "mm");
                        label_caps(ui, "Key points");
                        hint(ui, "Named points that sit at the same place on every profile. Rename them here.");
                        for k in 0..self.st.proj.keys.len() {
                            let mut name = self.st.proj.keys[k].name.clone();
                            if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).changed() {
                                self.op(format!(r#"{{"op":"key_set","k":{},"name":{}}}"#, k, json_str(&name)));
                            }
                        }
                    }
                    4 => {
                        hint(ui, "The main sections of the dash. Name them and pick a color. You can split and merge sections later from the scene tree.");
                        for k in 0..self.st.proj.sections.len() {
                            let (mut name, col, a, b) = {
                                let p = &self.st.proj;
                                let s = &p.sections[k];
                                (s.name.clone(), hex(&s.color), p.pid_index(s.start).unwrap_or(0), p.pid_index(s.end).unwrap_or(0))
                            };
                            ui.horizontal(|ui| {
                                let mut c = [col.r(), col.g(), col.b()];
                                if ui.color_edit_button_srgb(&mut c).changed() {
                                    self.op(format!(r##"{{"op":"section_set","k":{},"color":"#{:02x}{:02x}{:02x}"}}"##, k, c[0], c[1], c[2]));
                                }
                                let te = ui.add(egui::TextEdit::singleline(&mut name).desired_width(210.0));
                                if w.focus_sec == Some(k) {
                                    w.focus_sec = None;
                                    te.request_focus();
                                    if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), te.id) {
                                        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(name.chars().count()))));
                                        st.store(ui.ctx(), te.id);
                                    }
                                }
                                if te.changed() {
                                    self.op(format!(r#"{{"op":"section_set","k":{},"name":{}}}"#, k, json_str(&name)));
                                }
                                ui.label(egui::RichText::new(format!("P{}–P{}", a, b)).color(MUTED).size(12.0));
                            });
                        }
                    }
                    _ => {
                        let p = &self.st.proj;
                        ui.label(egui::RichText::new("Your dash shell is ready").size(18.0).strong());
                        ui.add_space(4.0);
                        let pieces = if self.st.trim_on { "top pad, lower pad and top trim" } else { "top pad and lower pad (add the top trim any time from the Top trim button)" };
                        ui.label(format!("The picture shows the whole dash: {}. The top pad has {} profiles and {} sections.", pieces, p.profiles.len(), p.sections.len()));
                        ui.add_space(8.0);
                        hint(ui, "Press Finish to start working on it. From there you can click a profile in the 3D view to shape it in the profile editor, bend the windshield line and add parts and shapes from the Parts & shapes tab. Undo reverses each wizard change, and File > Save keeps your work.");
                    }
                }
                });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, if w.step + 1 == STEPS.len() { "Finish" } else { "Next" }).clicked() {
                            next = true;
                        }
                        if ui.add_enabled(w.step > 0, egui::Button::new("Back")).clicked() {
                            back = true;
                        }
                    });
                });
            });
        self.wiz = Some(w);
        if apply {
            self.wiz_apply();
        }
        if cancel {
            self.wiz_close(true);
        } else if next {
            let last = self.wiz.as_ref().map_or(false, |w| w.step + 1 == STEPS.len());
            if last {
                self.wiz_close(false);
            } else if let Some(w) = &mut self.wiz {
                w.step += 1;
            }
        } else if back {
            if let Some(w) = &mut self.wiz {
                w.step = w.step.saturating_sub(1);
            }
        }
    }

    fn wiz_start(&mut self, ui: &mut egui::Ui, w: &mut Wiz, apply: &mut bool) {
        hint(ui, "Pick a starting point. It starts a fresh dash in place of the current one, and Cancel puts yours back.");
        let avail = ui.available_width();
        let cols = if avail > 500.0 { ((avail / 330.0).floor() as usize).clamp(2, 5) } else { 2 };
        let gap = 12.0;
        let cw = (avail - gap * (cols as f32 - 1.0) - 4.0) / cols as f32;
        let th = if cols > 2 { (cw * 0.55).min(190.0) } else { 60.0 };
        egui::Grid::new("cards").spacing(vec2(gap, gap)).show(ui, |ui| {
            for (n, (kind, title, desc)) in KINDS.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(cw, th + 92.0), Sense::click());
                let on = w.p.kind == *kind;
                card_bg(ui, rect, on, resp.hovered());
                thumb(ui, Rect::from_min_size(rect.min + vec2(10.0, 8.0), vec2(cw - 20.0, th)), kind);
                ui.painter().text(rect.min + vec2(14.0, th + 24.0), Align2::LEFT_CENTER, *title, FontId::proportional(if cols > 2 { 16.0 } else { 14.0 }), TEXT);
                let g = ui.painter().layout(desc.to_string(), FontId::proportional(if cols > 2 { 12.5 } else { 11.0 }), MUTED, cw - 28.0);
                ui.painter().galley(rect.min + vec2(14.0, th + 38.0), g, MUTED);
                if resp.clicked() {
                    w.p = templates::preset(kind);
                    w.inout = false;
                    *apply = true;
                }
                if n % cols == cols - 1 {
                    ui.end_row();
                }
            }
        });
    }
}

fn hint(ui: &mut egui::Ui, t: &str) {
    ui.label(egui::RichText::new(t).size(12.5).color(MUTED));
    ui.add_space(4.0);
}

fn slider(ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, step: f64, unit: &str) -> bool {
    ui.label(egui::RichText::new(format!("{}   ({})", label, crate::units::caption(unit))).size(12.5));
    ui.spacing_mut().slider_width = ui.available_width() - 90.0;
    let s = egui::Slider::new(v, range).step_by(step);
    ui.add(if unit.starts_with("mm") { s.mm() } else { s }).changed()
}

fn card_bg(ui: &egui::Ui, rect: Rect, on: bool, hover: bool) {
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(10), if on { ACCENT_DIM } else if hover { CARD_HI } else { CARD });
    p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(if on { 1.5 } else { 1.0 }, if on { ACCENT } else { LINE }), StrokeKind::Inside);
}

const DEFAULT_VIEW: [f32; 5] = [-0.28, 0.55, 1.0, 0.0, 0.0];

struct Thumb3 {
    /// surface patches around the middle of the dash (x, y, z in mm)
    quads: Vec<[[f64; 3]; 4]>,
    /// color of the section each patch belongs to
    qcol: Vec<[u8; 3]>,
    /// index (in the dash's section list) of the section each patch belongs to
    qsec: Vec<Option<usize>>,
    /// base color of each patch when the sections are not shown
    qbase: Vec<[u8; 3]>,
    radius: f64,
}

thread_local! {
    static THUMBS: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<Thumb3>>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Points of a curve at `n` evenly spaced fractions of its length.
fn along(c: &[[f64; 2]], n: usize) -> Vec<[f64; 2]> {
    let mut cum = vec![0.0f64];
    for w in c.windows(2) {
        cum.push(cum.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let total = cum.last().copied().unwrap_or(0.0).max(1e-9);
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for i in 0..n {
        let d = total * i as f64 / (n - 1) as f64;
        while j + 2 < cum.len() && cum[j + 1] < d {
            j += 1;
        }
        let span = (cum[j + 1] - cum[j]).max(1e-9);
        let t = ((d - cum[j]) / span).clamp(0.0, 1.0);
        out.push([c[j][0] + (c[j + 1][0] - c[j][0]) * t, c[j][1] + (c[j + 1][1] - c[j][1]) * t]);
    }
    out
}

fn build_thumb(kind: &str) -> Thumb3 {
    build_thumb_proj(&templates::generate(&templates::preset(kind)))
}

fn build_thumb_proj(pr: &crate::model::Project) -> Thumb3 {
    build_thumb_at(pr, None)
}

/// `center`: the point the picture is centerd on (default: the middle of this piece), so several pieces can share one.
fn build_thumb_at(pr: &crate::model::Project, center: Option<[f64; 3]>) -> Thumb3 {
    let mut profs: Vec<&crate::model::Profile> = pr.profiles.iter().collect();
    profs.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    const R: usize = 56;
    const SUB: usize = 10;
    let curves: Vec<(f64, Vec<[f64; 2]>)> = profs.iter().map(|p| (p.x, along(&p.curve(), R))).collect();
    // grid of surface points: columns across the dash (with extra columns between profiles), rows down the section
    let mut grid: Vec<Vec<[f64; 3]>> = Vec::new();
    let mut span_of_col: Vec<usize> = Vec::new();
    // slope of one coordinate along x at profile `i` (Fritsch-Carlson: flat where the direction changes, so creases stay sharp)
    let slope = |i: usize, r: usize, k: usize| -> f64 {
        let v = |j: usize| curves[j].1[r][k];
        let n = curves.len();
        let d = |a: usize, b: usize| {
            let dx = curves[b].0 - curves[a].0;
            if dx.abs() < 1e-9 { 0.0 } else { (v(b) - v(a)) / dx }
        };
        if i == 0 || i + 1 == n {
            return if i == 0 { d(0, 1) } else { d(n - 2, n - 1) };
        }
        let (d0, d1) = (d(i - 1, i), d(i, i + 1));
        if d0 * d1 <= 0.0 { 0.0 } else { 2.0 * d0 * d1 / (d0 + d1) }
    };
    for (si, w) in curves.windows(2).enumerate() {
        let h = w[1].0 - w[0].0;
        for s in 0..SUB {
            span_of_col.push(si);
            let t = s as f64 / SUB as f64;
            let (h00, h10, h01, h11) = (2.0 * t * t * t - 3.0 * t * t + 1.0, t * t * t - 2.0 * t * t + t, -2.0 * t * t * t + 3.0 * t * t, t * t * t - t * t);
            grid.push((0..R).map(|r| {
                let (a, b) = (w[0].1[r], w[1].1[r]);
                let f = |k: usize| h00 * a[k] + h10 * h * slope(si, r, k) + h01 * b[k] + h11 * h * slope(si + 1, r, k);
                [w[0].0 + h * t, f(0), f(1)]
            }).collect());
        }
    }
    if let Some(l) = curves.last() {
        grid.push(l.1.iter().map(|q| [l.0, q[0], q[1]]).collect());
    }
    // center the dash on the origin so it turns about its middle
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for col in &grid {
        for q in col {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
    }
    let c = center.unwrap_or([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0]);
    let radius = (((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt() / 2.0).max(1.0);
    // section color for each span between neighboring profiles
    let pos = |id: u32| profs.iter().position(|p| p.id == id);
    let span_col: Vec<[u8; 3]> = (0..profs.len().saturating_sub(1))
        .map(|si| {
            pr.sections
                .iter()
                .find(|sec| match (pos(sec.start), pos(sec.end)) {
                    (Some(a), Some(b)) => a.min(b) <= si && si < a.max(b),
                    _ => false,
                })
                .map(|sec| {
                    let h = sec.color.trim_start_matches('#');
                    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("80"), 16).unwrap_or(128);
                    [v(0), v(2), v(4)]
                })
                .unwrap_or([160, 168, 180])
        })
        .collect();
    let span_sec: Vec<Option<usize>> = (0..profs.len().saturating_sub(1))
        .map(|si| {
            pr.sections.iter().position(|sec| match (pos(sec.start), pos(sec.end)) {
                (Some(a), Some(b)) => a.min(b) <= si && si < a.max(b),
                _ => false,
            })
        })
        .collect();
    let mut qcol = Vec::new();
    let mut qsec = Vec::new();
    let mut quads = Vec::new();
    for ci in 0..grid.len() - 1 {
        let sc = span_of_col.get(ci).copied().unwrap_or(0);
        for r in 0..R - 1 {
            qcol.push(span_col.get(sc).copied().unwrap_or([160, 168, 180]));
            qsec.push(span_sec.get(sc).copied().flatten());
            quads.push([grid[ci][r], grid[ci + 1][r], grid[ci + 1][r + 1], grid[ci][r + 1]].map(|q| [q[0] - c[0], q[1] - c[1], q[2] - c[2]]));
        }
    }
    let qbase = vec![[176u8, 184, 198]; quads.len()];
    Thumb3 { quads, qcol, qsec, qbase, radius }
}

/// Draw the dash as shaded patches. view = [turn, tilt, zoom, slide x, slide y]. `fit` scales it to fill the rect (cards);
/// otherwise the size is fixed so it does not jump while you turn it.
fn draw_thumb(ui: &egui::Ui, rect: Rect, t: &Thumb3, view: [f32; 5], fit: bool, tint: bool, hover: Option<egui::Pos2>) -> Option<usize> {
    let (yaw, pitch) = (view[0] as f64, view[1] as f64);
    let (cy, sy_) = (yaw.cos(), yaw.sin());
    let (cp, sp) = (pitch.cos(), pitch.sin());
    let light = {
        let l = [-0.45f64, -0.5, 0.75];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };
    let mut quads: Vec<([[f64; 2]; 4], f64, f64, usize)> = Vec::with_capacity(t.quads.len());
    for (qi, q) in t.quads.iter().enumerate() {
        let ps = q.map(|p| [p[0] * cy - p[1] * sy_, p[0] * sy_ + p[1] * cy, p[2]]);
        let e1 = [ps[1][0] - ps[0][0], ps[1][1] - ps[0][1], ps[1][2] - ps[0][2]];
        let e2 = [ps[3][0] - ps[0][0], ps[3][1] - ps[0][1], ps[3][2] - ps[0][2]];
        let mut n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
        n = [n[0] / l, n[1] / l, n[2] / l];
        // a surface faces the viewer: flip the patch normal toward the camera
        let toward = [0.0, -cp, sp];
        if n[0] * toward[0] + n[1] * toward[1] + n[2] * toward[2] < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let shade = 0.32 + 0.68 * (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
        let sc = ps.map(|p| ([p[0], p[2] * cp + p[1] * sp], p[1] * cp - p[2] * sp));
        let depth = (sc[0].1 + sc[1].1 + sc[2].1 + sc[3].1) / 4.0;
        quads.push(([sc[0].0, sc[1].0, sc[2].0, sc[3].0], shade, depth, qi));
    }
    quads.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let (k, ox, oy) = if fit {
        let (mut x0, mut x1, mut y0, mut y1) = (1e18f64, -1e18f64, 1e18f64, -1e18f64);
        for q in &quads {
            for p in &q.0 {
                x0 = x0.min(p[0]);
                x1 = x1.max(p[0]);
                y0 = y0.min(p[1]);
                y1 = y1.max(p[1]);
            }
        }
        let k = ((rect.width() as f64) / (x1 - x0).max(1.0)).min(rect.height() as f64 / (y1 - y0).max(1.0)) * 0.96;
        (k, rect.center().x as f64 - (x0 + x1) / 2.0 * k, rect.center().y as f64 + (y0 + y1) / 2.0 * k)
    } else {
        // one fixed scale (not tied to the dash's own size), so changing the width does not make everything shrink or grow
        let k = (rect.width().min(rect.height()) as f64) / 2000.0 * 1.15 * view[2] as f64;
        (k, rect.center().x as f64 + view[3] as f64, rect.center().y as f64 + view[4] as f64)
    };
    let clip = ui.painter().with_clip_rect(rect);
    let mut hit: Option<usize> = None;
    let mut mesh = egui::Mesh::default();
    for (q, shade, _, qi) in &quads {
        let pts: Vec<egui::Pos2> = q.iter().map(|p| pos2((ox + p[0] * k) as f32, (oy - p[1] * k) as f32)).collect();
        let c = if tint {
            let b = t.qcol[*qi];
            let l = 0.45 + 0.75 * (*shade as f32 - 0.32).max(0.0) / 0.68;
            Color32::from_rgb((b[0] as f32 * l).min(255.0) as u8, (b[1] as f32 * l).min(255.0) as u8, (b[2] as f32 * l).min(255.0) as u8)
        } else {
            let b = t.qbase.get(*qi).copied().unwrap_or([176, 184, 198]);
            Color32::from_rgb((b[0] as f64 * shade) as u8, (b[1] as f64 * shade) as u8, (b[2] as f64 * shade) as u8)
        };
        if let Some(h) = hover {
            if rect.contains(h) && point_in_quad(&pts, h) {
                hit = Some(*qi);
            }
        }
        // plain triangles, no outline: an outline on a squashed patch (the trim thins to nothing) shoots out long spikes
        if pts.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            continue;
        }
        let i0 = mesh.vertices.len() as u32;
        for p in &pts {
            mesh.colored_vertex(*p, c);
        }
        mesh.add_triangle(i0, i0 + 1, i0 + 2);
        mesh.add_triangle(i0, i0 + 2, i0 + 3);
    }
    clip.add(Shape::mesh(mesh));
    hit
}

/// Is the point inside the (convex) four-corner patch?
fn point_in_quad(p: &[egui::Pos2], h: egui::Pos2) -> bool {
    let (mut pos, mut neg) = (false, false);
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let cr = (b.x - a.x) * (h.y - a.y) - (b.y - a.y) * (h.x - a.x);
        if cr > 0.0 { pos = true; }
        if cr < 0.0 { neg = true; }
    }
    !(pos && neg)
}

/// Little 3D picture of a template (cached; drawn as shaded quads).
pub(crate) fn thumb(ui: &egui::Ui, rect: Rect, kind: &str) {
    thumb_view(ui, rect, kind, DEFAULT_VIEW);
}

/// Same, seen from a chosen angle: view = [turn, tilt, zoom, slide x, slide y].
pub(crate) fn thumb_view(ui: &egui::Ui, rect: Rect, kind: &str, view: [f32; 5]) {
    let t = THUMBS.with(|c| c.borrow_mut().entry(kind.to_string()).or_insert_with(|| std::rc::Rc::new(build_thumb(kind))).clone());
    draw_thumb(ui, rect, &t, view, true, false, None);
    let _ = draw_icon;
}

thread_local! {
    static FULL: std::cell::RefCell<(u64, Option<std::rc::Rc<Thumb3>>)> = std::cell::RefCell::new((u64::MAX, None));
    static LIVE: std::cell::RefCell<(u64, Option<std::rc::Rc<Thumb3>>)> = std::cell::RefCell::new((u64::MAX, None));
}

/// 3D picture of the dash as it is right now (rebuilt only when it changes).
/// The whole dash: top pad, lower pad and (when it is part of the dash) the top trim, all in the same tone as the earlier steps.
fn full_thumb(rev: u64, pieces: &[&crate::model::Project]) -> std::rc::Rc<Thumb3> {
    FULL.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != rev || c.1.is_none() {
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for p in pieces {
                if let Some(b) = p.bounds() {
                    for k in 0..3 {
                        lo[k] = lo[k].min(b[0][k]);
                        hi[k] = hi[k].max(b[1][k]);
                    }
                }
            }
            let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
            let tones: [[u8; 3]; 3] = [[176, 184, 198]; 3];
            let mut all = Thumb3 { quads: Vec::new(), qcol: Vec::new(), qsec: Vec::new(), qbase: Vec::new(), radius: 1.0 };
            for (i, p) in pieces.iter().enumerate() {
                let t = build_thumb_at(p, Some(center));
                all.radius = all.radius.max(t.radius);
                let n = t.quads.len();
                all.quads.extend(t.quads);
                all.qcol.extend(t.qcol);
                all.qsec.extend(t.qsec);
                all.qbase.extend(std::iter::repeat(tones[i.min(2)]).take(n));
            }
            *c = (rev, Some(std::rc::Rc::new(all)));
        }
        c.1.clone().unwrap()
    })
}

fn live_thumb(rev: u64, pr: &crate::model::Project) -> std::rc::Rc<Thumb3> {
    LIVE.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != rev || c.1.is_none() {
            *c = (rev, Some(std::rc::Rc::new(build_thumb_proj(pr))));
        }
        c.1.clone().unwrap()
    })
}


fn fade(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

#[cfg(test)]
mod thumb_tests {
    use super::*;
    #[test]
    fn finish_picture_holds_every_piece() {
        let st = crate::engine::State::new(std::env::temp_dir().join("dashfw_fin_test"));
        let one = build_thumb_proj(st.piece(0)).quads.len();
        let all = full_thumb(1, &[st.piece(0), st.piece(1), st.piece(2)]);
        assert!(all.quads.len() > one * 2, "{} vs {}", all.quads.len(), one);
        assert_eq!(all.qbase.len(), all.quads.len());
        let tones: std::collections::HashSet<[u8; 3]> = all.qbase.iter().copied().collect();
        assert_eq!(tones.len(), 1, "every piece uses the same tone as the earlier steps");
    }

    #[test]
    fn sections_colour_each_span() {
        let pr = templates::generate(&templates::preset("squarebody"));
        let t = build_thumb_proj(&pr);
        let mut cols: Vec<[u8; 3]> = Vec::new();
        for c in &t.qcol {
            if !cols.contains(c) {
                cols.push(*c);
            }
        }
        assert_eq!(cols.len(), pr.sections.len(), "one color per section");
        // first patches (driver end, -x) take the first section's color
        let h = pr.sections[0].color.trim_start_matches('#').to_string();
        let first = [u8::from_str_radix(&h[0..2], 16).unwrap(), u8::from_str_radix(&h[2..4], 16).unwrap(), u8::from_str_radix(&h[4..6], 16).unwrap()];
        assert_eq!(t.qcol[0], first);
        // every patch knows its section, so a click in the picture can find its name box
        assert_eq!(t.qsec.len(), t.quads.len());
        let mut secs: Vec<usize> = t.qsec.iter().flatten().copied().collect();
        secs.sort();
        secs.dedup();
        assert_eq!(secs.len(), pr.sections.len());
        let sq = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0), pos2(0.0, 10.0)];
        assert!(point_in_quad(&sq, pos2(5.0, 5.0)) && !point_in_quad(&sq, pos2(15.0, 5.0)));
    }
}
