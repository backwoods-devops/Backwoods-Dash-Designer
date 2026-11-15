// Documentation screenshots: with DASHFW_DOCSHOT=<scene> the program opens straight into a ready-made scene so a
// picture can be taken for the manual (see docs/make_shots.sh). Does nothing unless that variable is set.
use crate::app::*;

impl App {
    pub fn docshot(&mut self) {
        let Ok(scene) = std::env::var("DASHFW_DOCSHOT") else { return };
        self.egui_ctx.request_repaint_after(std::time::Duration::from_millis(150));
        let f = self.frame_no;
        if f == 2 {
            // No hover tooltips in the pictures (the pointer rests somewhere on the virtual screen).
            self.egui_ctx.all_styles_mut(|st| st.interaction.tooltip_delay = 1.0e6);
        }
        if f == 1 && std::env::var("DASHFW_DOCINCH").is_ok() {
            crate::units::set(true, &self.st.dir);
        }
        if scene == "keep" {
            if f == 4 {
                if let Ok(path) = std::env::var("DASHFW_OPEN") {
                    self.file.welcome = false;
                    self.file_open(&path);
                    self.view_mat = true;
                }
            }
            if f == 10 {
                if let Ok(j) = std::env::var("DASHFW_OP") {
                    for one in j.split('|') {
                        self.op(one.to_string());
                    }
                }
            }
            return;
        }
        if f == 10 && scene != "keep" {
            if let Ok(j) = std::env::var("DASHFW_OP") {
                for one in j.split('|') {
                    self.op(one.to_string());
                }
            }
        }
        let wiz_step = ["wiz_start", "wiz_size", "wiz_wind", "wiz_front", "wiz_sections", "wiz_finish"].iter().position(|s| *s == scene);
        if f == 4 && scene != "start" && scene != "story" {
            self.file.welcome = false;
            if let Some(n) = wiz_step {
                self.open_wizard();
                if n > 0 {
                    self.op(r#"{"op":"apply_template","kind":"wrap"}"#.to_string());
                }
                if let Some(w) = &mut self.wiz {
                    w.step = n;
                    w.applied = n > 0;
                }
                self.piece_vis = [true, true, self.st.trim_on];
                return;
            }
            self.op(r#"{"op":"apply_template","kind":"wrap"}"#.to_string());
            self.piece_vis = [true, true, self.st.trim_on];
            self.op(r##"{"op":"set_look","mat":"matte","color":"#8a94a6","quiet":true}"##.to_string());
            self.view_mat = !scene.contains("design");
            self.sel_prof = 1;
            self.sel_pt = 3;
            self.dock_open = true;
            match scene.as_str() {
                "plan" => self.dock_tab = DockTab::Plan,
                "height" => self.dock_tab = DockTab::Height,
                "walls" => self.dock_tab = DockTab::Walls,
                "shapes" => self.left_tab = LeftTab::Shapes,
                "lighting" => self.light_open = true,
                "export" => self.exp_open = true,
                "size" => {
                    self.size_mode = true;
                    self.size_all = true;
                }
                "wallview" => self.wall_view = true,
                "shape_ed" => {
                    let mut s = crate::shapes::Shape::new("Gauge pod", crate::shapes::Kind::Rect, 120.0, 70.0);
                    s.radius = 14.0;
                    self.shape_ed = Some(crate::ui_shapes::ShapeEd::new(s, None));
                }
                "help" => {
                    self.help_open = true;
                    self.help_topic = std::env::var("DASHFW_HELP_TOPIC").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
                }
                "shape_combo" | "shape_3d" | "leave" => {
                    let mut e = crate::ui_shapes::ShapeEd::new(combo_shape(), None);
                    e.part_sel = Some(0);
                    if scene == "shape_3d" {
                        e.view3d = true;
                    }
                    self.shape_ed = Some(e);
                    if scene == "leave" {
                        self.leave_ask = true;
                    }
                }
                "draw_new" => self.shape_ed = Some(crate::ui_shapes::ShapeEd::drawing_new()),
                "draw" | "trace" => {
                    let mut e = crate::ui_shapes::ShapeEd::drawing_new();
                    e.canvas = [200.0, 120.0];
                    e.canvas_ask = false;
                    e.draw_pts = vec![[-70.0, -30.0], [-40.0, 30.0], [20.0, 40.0], [60.0, 10.0]];
                    if scene == "trace" {
                        let mut r = crate::refimg::RefImage::new("docs/sample_trace.png");
                        r.ensure(&self.egui_ctx);
                        r.default_scale(190.0);
                        r.opacity = 0.5;
                        r.move_mode = true;
                        e.trace = Some(r);
                    }
                    self.shape_ed = Some(e);
                }
                _ => {}
            }
        }
        if scene == "cut" || scene == "cut_panel" {
            if f == 60 {
                let mut s = crate::shapes::Shape::new("Gauge hole", crate::shapes::Kind::Ellipse, 90.0, 90.0);
                s.radius = 0.0;
                self.place_shape_at_view_centre(&s);
            }
        }
    }
}

/// A bezel with a round hole through the middle and a tab added at the bottom, for the manual's pictures.
fn combo_shape() -> crate::shapes::Shape {
    use crate::shapes::{Kind, Part, Shape};
    let mut s = Shape::new("Gauge bezel", Kind::Ellipse, 110.0, 110.0);
    s.depth = 8.0;
    let mut hole = Part::new(Kind::Ellipse, true);
    hole.sh.w = 72.0;
    hole.sh.h = 72.0;
    let mut tab = Part::new(Kind::Rect, false);
    tab.sh.w = 44.0;
    tab.sh.h = 18.0;
    tab.cy = -56.0;
    s.parts = vec![hole, tab];
    s
}
