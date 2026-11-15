// Reference image for tracing: an image placed behind an editor, calibrated from two marked points.
// Everything stays on this computer: only the file path and the placement are stored (refs.json).
use crate::json::{self, J};
use eframe::egui;

#[derive(Clone, Copy, PartialEq)]
pub enum Stage {
    Idle,
    PickA,
    PickB,
}

pub struct RefImage {
    pub path: String,
    pub w: usize,
    pub h: usize,
    pub tex: Option<egui::TextureHandle>,
    pub tried: bool,
    pub err: String,
    /// mm per image pixel
    pub s: f64,
    /// where the image center sits in the editor (mm)
    pub cx: f64,
    pub cy: f64,
    pub rot: f64,
    pub flip: bool,
    pub opacity: f32,
    pub visible: bool,
    /// the two marked points, in image pixels
    pub a: Option<[f64; 2]>,
    pub b: Option<[f64; 2]>,
    pub dist: f64,
    pub stage: Stage,
    pub move_mode: bool,
    /// mouse drag in progress while moving/sizing: 0 none, 1 moving, 2 sizing from a corner
    pub drag: u8,
    pub d_anchor_px: [f64; 2],
    pub d_anchor_w: [f64; 2],
    pub d_corner_w: [f64; 2],
    pub d_s0: f64,
    /// height stretch relative to the width scale (1 = proportions as loaded); only differs when the proportions are unlocked
    pub ay: f64,
    /// keep the proportions while resizing
    pub lock: bool,
}

impl RefImage {
    pub fn new(path: &str) -> RefImage {
        RefImage { path: path.to_string(), w: 0, h: 0, tex: None, tried: false, err: String::new(), s: 1.0, cx: 0.0, cy: 0.0, rot: 0.0, flip: false, opacity: 0.6, visible: true, a: None, b: None, dist: 100.0, stage: Stage::Idle, move_mode: false, drag: 0, d_anchor_px: [0.0; 2], d_anchor_w: [0.0; 2], d_corner_w: [0.0; 2], d_s0: 1.0, ay: 1.0, lock: true }
    }

    /// Decode the file and upload it as a texture (once).
    pub fn ensure(&mut self, ctx: &egui::Context) {
        if self.tex.is_some() || self.tried {
            return;
        }
        self.tried = true;
        match image::open(&self.path) {
            Ok(img) => {
                let mut rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let m = w.max(h);
                if m > 4096 {
                    let f = 4096.0 / m as f32;
                    rgba = image::imageops::resize(&rgba, ((w as f32 * f) as u32).max(1), ((h as f32 * f) as u32).max(1), image::imageops::FilterType::Triangle);
                }
                let (w, h) = rgba.dimensions();
                self.w = w as usize;
                self.h = h as usize;
                let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
                self.tex = Some(ctx.load_texture("refimg", ci, egui::TextureOptions::LINEAR));
                self.err.clear();
            }
            Err(e) => self.err = format!("Could not open the image: {}", e),
        }
    }

    /// A freshly loaded image is sized to about `width_mm` across until it is calibrated.
    pub fn default_scale(&mut self, width_mm: f64) {
        if self.w > 0 {
            self.s = width_mm / self.w as f64;
        }
    }

    pub fn px_to_world(&self, px: f64, py: f64) -> [f64; 2] {
        let fx = if self.flip { -1.0 } else { 1.0 };
        let lx = (px - self.w as f64 / 2.0) * self.s * fx;
        let ly = -(py - self.h as f64 / 2.0) * self.s * self.ay;
        let t = self.rot.to_radians();
        [self.cx + lx * t.cos() - ly * t.sin(), self.cy + lx * t.sin() + ly * t.cos()]
    }

    pub fn world_to_px(&self, x: f64, y: f64) -> [f64; 2] {
        let (dx, dy) = (x - self.cx, y - self.cy);
        let t = -self.rot.to_radians();
        let lx = dx * t.cos() - dy * t.sin();
        let ly = dx * t.sin() + dy * t.cos();
        let fx = if self.flip { -1.0 } else { 1.0 };
        [lx / self.s.max(1e-12) * fx + self.w as f64 / 2.0, -ly / (self.s * self.ay).max(1e-12) + self.h as f64 / 2.0]
    }

    /// Size on the canvas in mm: (width, height).
    pub fn size_mm(&self) -> (f64, f64) {
        (self.w as f64 * self.s, self.h as f64 * self.s * self.ay)
    }

    /// Set the width in mm. Locked: the height follows; unlocked: the height stays as it is.
    pub fn set_width(&mut self, mm: f64) {
        let mm = mm.max(1.0);
        let (_, h0) = self.size_mm();
        self.s = mm / self.w.max(1) as f64;
        if !self.lock {
            self.ay = (h0 / (self.h.max(1) as f64 * self.s)).max(1e-6);
        }
    }

    /// Set the height in mm. Locked: the width follows; unlocked: the width stays as it is.
    pub fn set_height(&mut self, mm: f64) {
        let mm = mm.max(1.0);
        if self.lock {
            self.s = mm / (self.h.max(1) as f64 * self.ay);
        } else {
            self.ay = mm / (self.h.max(1) as f64 * self.s);
        }
    }

    /// The four corners of the image in editor mm, starting top left and going clockwise.
    pub fn corners_world(&self) -> [[f64; 2]; 4] {
        let (w, h) = (self.w as f64, self.h as f64);
        [self.px_to_world(0.0, 0.0), self.px_to_world(w, 0.0), self.px_to_world(w, h), self.px_to_world(0.0, h)]
    }

    /// Start resizing from corner `k` (0..4, as in corners_world): the opposite corner stays put.
    pub fn size_begin(&mut self, k: usize) {
        let (w, h) = (self.w as f64, self.h as f64);
        let px = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
        let o = px[(k + 2) % 4];
        self.drag = 2;
        self.d_anchor_px = o;
        self.d_anchor_w = self.px_to_world(o[0], o[1]);
    }

    /// While resizing: the dragged corner follows the pointer (world position `p`).
    pub fn size_drag(&mut self, p: [f64; 2]) {
        let (w, h) = (self.w as f64, self.h as f64);
        let o = self.d_anchor_px;
        // the corner opposite the fixed one, in pixels
        let c = [w - o[0], h - o[1]];
        let (dx, dy) = ((c[0] - o[0]).abs().max(1.0), (c[1] - o[1]).abs().max(1.0));
        let (vx, vy) = (p[0] - self.d_anchor_w[0], p[1] - self.d_anchor_w[1]);
        let t = -self.rot.to_radians();
        let (lx, ly) = ((vx * t.cos() - vy * t.sin()).abs(), (vx * t.sin() + vy * t.cos()).abs());
        let (nx, ny) = ((lx / dx).max(0.01), (ly / dy).max(0.01));
        if self.lock {
            let f = ((nx / self.s) + (ny / (self.s * self.ay))) / 2.0;
            self.s = (self.s * f).max(0.01);
        } else {
            self.s = nx;
            self.ay = ny / nx;
        }
        let after = self.px_to_world(o[0], o[1]);
        self.cx += self.d_anchor_w[0] - after[0];
        self.cy += self.d_anchor_w[1] - after[1];
    }

    fn keep_a_fixed(&mut self, before: [f64; 2]) {
        if let Some(a) = self.a {
            let after = self.px_to_world(a[0], a[1]);
            self.cx += before[0] - after[0];
            self.cy += before[1] - after[1];
        }
    }

    /// Pixel distance between the marked points.
    pub fn marked_px(&self) -> Option<f64> {
        match (self.a, self.b) {
            (Some(a), Some(b)) => Some(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()),
            _ => None,
        }
    }

    /// Set the scale so the marked points are `dist` mm apart; the first point stays where it is.
    pub fn calibrate(&mut self) -> Result<f64, String> {
        let px = self.marked_px().ok_or("Mark both points first")?;
        if px < 2.0 {
            return Err("The two points are too close together".into());
        }
        if self.dist <= 0.0 {
            return Err("Enter the real distance between the points".into());
        }
        let a = self.a.unwrap();
        let before = self.px_to_world(a[0], a[1]);
        self.s = self.dist / px;
        self.keep_a_fixed(before);
        Ok(self.s)
    }

    /// Rotate about the first point so the line through the marked points is horizontal (or vertical).
    pub fn level(&mut self, vertical: bool) {
        if let (Some(a), Some(b)) = (self.a, self.b) {
            let wa = self.px_to_world(a[0], a[1]);
            let wb = self.px_to_world(b[0], b[1]);
            let ang = (wb[1] - wa[1]).atan2(wb[0] - wa[0]).to_degrees();
            let target = if vertical { if ang >= 0.0 { 90.0 } else { -90.0 } } else if ang.abs() > 90.0 { 180.0 } else { 0.0 };
            self.rot -= ang - target;
            self.keep_a_fixed(wa);
        }
    }

    pub fn a_to_origin(&mut self) {
        if let Some(a) = self.a {
            let w = self.px_to_world(a[0], a[1]);
            self.cx -= w[0];
            self.cy -= w[1];
        }
    }

    pub fn to_json(&self) -> J {
        let pt = |p: Option<[f64; 2]>| match p {
            Some(p) => J::Arr(vec![J::Num(p[0]), J::Num(p[1])]),
            None => J::Null,
        };
        J::Obj(vec![
            ("path".into(), J::Str(self.path.clone())),
            ("s".into(), J::Num(self.s)),
            ("cx".into(), J::Num(self.cx)),
            ("cy".into(), J::Num(self.cy)),
            ("rot".into(), J::Num(self.rot)),
            ("flip".into(), J::Bool(self.flip)),
            ("opacity".into(), J::Num(self.opacity as f64)),
            ("visible".into(), J::Bool(self.visible)),
            ("a".into(), pt(self.a)),
            ("b".into(), pt(self.b)),
            ("dist".into(), J::Num(self.dist)),
            ("ay".into(), J::Num(self.ay)),
            ("lock".into(), J::Bool(self.lock)),
        ])
    }

    pub fn from_json(j: &J) -> Option<RefImage> {
        let path = j.s("path")?;
        let mut r = RefImage::new(path);
        r.s = j.n("s").unwrap_or(1.0);
        r.cx = j.n("cx").unwrap_or(0.0);
        r.cy = j.n("cy").unwrap_or(0.0);
        r.rot = j.n("rot").unwrap_or(0.0);
        r.flip = j.b("flip").unwrap_or(false);
        r.opacity = j.n("opacity").unwrap_or(0.6) as f32;
        r.visible = j.b("visible").unwrap_or(true);
        r.dist = j.n("dist").unwrap_or(100.0);
        r.ay = j.n("ay").unwrap_or(1.0).max(0.01);
        r.lock = j.b("lock").unwrap_or(true);
        let pt = |k: &str| j.get(k).and_then(|v| v.arr()).filter(|a| a.len() >= 2).map(|a| [a[0].num().unwrap_or(0.0), a[1].num().unwrap_or(0.0)]);
        r.a = pt("a");
        r.b = pt("b");
        Some(r)
    }
}

pub fn load_all(dir: &std::path::Path) -> [Option<RefImage>; 3] {
    let mut out: [Option<RefImage>; 3] = [None, None, None];
    if let Ok(txt) = std::fs::read_to_string(dir.join("refs.json")) {
        if let Ok(j) = json::parse(&txt) {
            if let Some(a) = j.arr() {
                for (i, v) in a.iter().enumerate().take(3) {
                    out[i] = RefImage::from_json(v);
                }
            }
        }
    }
    out
}

pub fn save_all(dir: &std::path::Path, refs: &[Option<RefImage>; 3]) {
    let j = J::Arr(refs.iter().map(|r| r.as_ref().map(|r| r.to_json()).unwrap_or(J::Null)).collect());
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("refs.json"), json::to_string(&j));
}

/// Ask the operating system for an image file without blocking the editor.
pub fn pick_file() -> std::sync::mpsc::Receiver<Option<String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = if cfg!(target_os = "macos") {
            std::process::Command::new("osascript").args(["-e", "POSIX path of (choose file with prompt \"Choose a reference image\" of type {\"public.image\"})"]).output().ok()
        } else if cfg!(target_os = "windows") {
            crate::file::windows_dialog(false, "Choose a reference image", "Images|*.png;*.jpg;*.jpeg|All files|*.*", "")
        } else {
            std::process::Command::new("zenity").args(["--file-selection", "--title=Choose a reference image", "--file-filter=Images | *.png *.jpg *.jpeg"]).output().ok().or_else(|| std::process::Command::new("kdialog").args(["--getopenfilename", ".", "*.png *.jpg *.jpeg"]).output().ok())
        };
        let path = out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
        let _ = tx.send(path);
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calibrate_and_level() {
        let mut r = RefImage::new("x");
        r.w = 1000;
        r.h = 500;
        r.a = Some([100.0, 100.0]);
        r.b = Some([400.0, 140.0]);
        r.dist = 150.0;
        let wa = r.px_to_world(100.0, 100.0);
        r.calibrate().unwrap();
        let (wa2, wb2) = (r.px_to_world(100.0, 100.0), r.px_to_world(400.0, 140.0));
        assert!((wa[0] - wa2[0]).abs() < 1e-9 && (wa[1] - wa2[1]).abs() < 1e-9);
        let d = ((wa2[0] - wb2[0]).powi(2) + (wa2[1] - wb2[1]).powi(2)).sqrt();
        assert!((d - 150.0).abs() < 1e-6, "{}", d);
        r.level(false);
        let (p, q) = (r.px_to_world(100.0, 100.0), r.px_to_world(400.0, 140.0));
        assert!((p[1] - q[1]).abs() < 1e-6);
        let back = r.world_to_px(q[0], q[1]);
        assert!((back[0] - 400.0).abs() < 1e-6 && (back[1] - 140.0).abs() < 1e-6);
        r.flip = true;
        let w = r.px_to_world(250.0, 80.0);
        let b2 = r.world_to_px(w[0], w[1]);
        assert!((b2[0] - 250.0).abs() < 1e-6);
    }

    #[test]
    fn resize_keeps_or_frees_the_proportions() {
        let mut r = RefImage::new("x");
        r.w = 1000;
        r.h = 500;
        r.s = 0.5;
        r.set_width(400.0);
        assert_eq!(r.size_mm(), (400.0, 200.0), "locked: height follows the width");
        r.set_height(100.0);
        assert!((r.size_mm().0 - 200.0).abs() < 1e-6 && (r.size_mm().1 - 100.0).abs() < 1e-6);
        r.lock = false;
        r.set_width(300.0);
        let (w, h) = r.size_mm();
        assert!((w - 300.0).abs() < 1e-6 && (h - 100.0).abs() < 1e-6, "unlocked: height stays");
        // dragging the bottom-right corner keeps the top-left corner in place
        let tl = r.px_to_world(0.0, 0.0);
        r.size_begin(2);
        r.size_drag([tl[0] + 600.0, tl[1] - 150.0]);
        let tl2 = r.px_to_world(0.0, 0.0);
        let (w, h) = r.size_mm();
        assert!((tl[0] - tl2[0]).abs() < 1e-6 && (tl[1] - tl2[1]).abs() < 1e-6);
        assert!((w - 600.0).abs() < 1e-6 && (h - 150.0).abs() < 1e-6, "{} {}", w, h);
        // round trip through the saved file
        let r2 = RefImage::from_json(&r.to_json()).unwrap();
        assert!((r2.ay - r.ay).abs() < 1e-9 && !r2.lock);
        r.lock = true;
        r.size_begin(2);
        r.size_drag([tl[0] + 300.0, tl[1] - 75.0]);
        let (w, h) = r.size_mm();
        assert!((w / h - 4.0).abs() < 1e-6, "locked keeps the ratio");
    }
}
