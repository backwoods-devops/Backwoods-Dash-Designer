// Parts catalogue (gauges, vents, grilles, screens...) and the geometry used to show them on the dash.
use crate::model::Comp;
use crate::render::*;

pub struct Item {
    pub kind: &'static str,
    pub shape: &'static str,
    pub label: &'static str,
    pub w: f64,
    pub h: f64,
}

pub const CATALOG: [Item; 11] = [
    Item { kind: "gauge", shape: "round", label: "Gauge 52", w: 52.0, h: 52.0 },
    Item { kind: "gauge", shape: "round", label: "Gauge 85", w: 85.0, h: 85.0 },
    Item { kind: "gauge", shape: "round", label: "Gauge 100", w: 100.0, h: 100.0 },
    Item { kind: "vent", shape: "rect", label: "Vent 120x40", w: 120.0, h: 40.0 },
    Item { kind: "vent", shape: "round", label: "Vent 60", w: 60.0, h: 60.0 },
    Item { kind: "speaker", shape: "round", label: "Speaker 100", w: 100.0, h: 100.0 },
    Item { kind: "speaker", shape: "rect", label: "Grille 150x100", w: 150.0, h: 100.0 },
    Item { kind: "screen", shape: "rect", label: "Screen 7in", w: 155.0, h: 87.0 },
    Item { kind: "screen", shape: "rect", label: "Screen 10in", w: 222.0, h: 125.0 },
    Item { kind: "switch", shape: "round", label: "Switch", w: 20.0, h: 20.0 },
    Item { kind: "light", shape: "round", label: "Indicator", w: 10.0, h: 10.0 },
];

pub fn hex(h: &str) -> V3 {
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(128) as f32 / 255.0;
    [p(1), p(3), p(5)]
}

struct B {
    out: Vec<Vtx>,
    pos: V3,
    ex: V3,
    ey: V3,
    n: V3,
}

impl B {
    fn w(&self, x: f32, y: f32, z: f32) -> V3 {
        [
            self.pos[0] + self.ex[0] * x + self.ey[0] * y + self.n[0] * z,
            self.pos[1] + self.ex[1] * x + self.ey[1] * y + self.n[1] * z,
            self.pos[2] + self.ex[2] * x + self.ey[2] * y + self.n[2] * z,
        ]
    }
    fn d(&self, x: f32, y: f32, z: f32) -> V3 {
        [
            self.ex[0] * x + self.ey[0] * y + self.n[0] * z,
            self.ex[1] * x + self.ey[1] * y + self.n[1] * z,
            self.ex[2] * x + self.ey[2] * y + self.n[2] * z,
        ]
    }
    fn tri(&mut self, a: V3, b: V3, c: V3, nn: V3, col: V3) {
        let nw = self.d(nn[0], nn[1], nn[2]);
        for v in [a, b, c] {
            let p = self.w(v[0], v[1], v[2]);
            self.out.push(Vtx { p, n: nw, c: col });
        }
    }
    fn quad(&mut self, a: V3, b: V3, c: V3, d: V3, nn: V3, col: V3) {
        self.tri(a, b, c, nn, col);
        self.tri(a, c, d, nn, col);
    }
    fn bx(&mut self, cx: f32, cy: f32, z0: f32, w: f32, h: f32, d: f32, color: &str) {
        let col = hex(color);
        let (x0, x1, y0, y1, z1) = (cx - w / 2.0, cx + w / 2.0, cy - h / 2.0, cy + h / 2.0, z0 + d);
        self.quad([x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1], [0.0, 0.0, 1.0], col);
        self.quad([x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1], [0.0, -1.0, 0.0], col);
        self.quad([x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [0.0, 1.0, 0.0], col);
        self.quad([x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1], [1.0, 0.0, 0.0], col);
        self.quad([x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [-1.0, 0.0, 0.0], col);
    }
    fn cyl(&mut self, cx: f32, cy: f32, z0: f32, r: f32, hgt: f32, color: &str, seg: usize) {
        let col = hex(color);
        let z1 = z0 + hgt;
        for i in 0..seg {
            let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
            let p0 = [cx + r * a0.cos(), cy + r * a0.sin()];
            let p1 = [cx + r * a1.cos(), cy + r * a1.sin()];
            let am = (a0 + a1) / 2.0;
            let nn = [am.cos(), am.sin(), 0.0];
            self.quad([p0[0], p0[1], z0], [p1[0], p1[1], z0], [p1[0], p1[1], z1], [p0[0], p0[1], z1], nn, col);
            self.tri([cx, cy, z1], [p0[0], p0[1], z1], [p1[0], p1[1], z1], [0.0, 0.0, 1.0], col);
        }
    }
}

fn part(b: &mut B, c: &Comp, selected: bool) {
    let (w, h) = (c.w as f32, c.h as f32);
    let r = w / 2.0;
    let round = c.shape == "round";
    match c.kind.as_str() {
        "gauge" => {
            if round {
                b.cyl(0.0, 0.0, 0.0, r, 4.0, "#b8bcc4", 40);
                b.cyl(0.0, 0.0, 0.0, r * 0.86, 4.5, "#111214", 40);
                b.bx(0.0, r * 0.3, 4.5, 1.6, r * 0.6, 0.8, "#e8e8e8");
                b.cyl(0.0, 0.0, 4.5, 2.6, 1.2, "#cc3333", 12);
            } else {
                b.bx(0.0, 0.0, 0.0, w, h, 4.0, "#b8bcc4");
                b.bx(0.0, 0.0, 0.0, w * 0.9, h * 0.86, 4.5, "#111214");
            }
        }
        "vent" => {
            if round {
                b.cyl(0.0, 0.0, 0.0, r, 4.0, "#2a2d31", 36);
                for i in -2i32..=2 {
                    let y = i as f32 * r * 0.32;
                    let ch = 2.0 * (r * r - y * y).max(0.0).sqrt() * 0.88;
                    b.bx(0.0, y, 4.0, ch, 2.2, 0.5, "#08090a");
                }
            } else {
                b.bx(0.0, 0.0, 0.0, w, h, 4.0, "#2a2d31");
                let ns = ((h / 7.0).floor() as i32).max(2);
                for i in 0..ns {
                    b.bx(0.0, -h / 2.0 + (i as f32 + 0.5) * h / ns as f32, 4.0, w * 0.86, h / ns as f32 * 0.45, 0.5, "#08090a");
                }
            }
        }
        "speaker" => {
            if round {
                b.cyl(0.0, 0.0, 0.0, r, 3.0, "#1b1c1f", 40);
            } else {
                b.bx(0.0, 0.0, 0.0, w, h, 3.0, "#1b1c1f");
            }
            let area = if round { std::f32::consts::PI * r * r } else { w * h };
            let sp = (area / 420.0).sqrt().max(6.0);
            let mut y = -h / 2.0 + sp / 2.0;
            while y < h / 2.0 {
                let mut x = -w / 2.0 + sp / 2.0;
                while x < w / 2.0 {
                    let skip = if round { x.hypot(y) > r * 0.9 } else { x.abs() > w * 0.46 || y.abs() > h * 0.46 };
                    if !skip {
                        b.cyl(x, y, 3.0, (sp * 0.28).min(1.8), 0.5, "#050506", 8);
                    }
                    x += sp;
                }
                y += sp;
            }
        }
        "screen" => {
            b.bx(0.0, 0.0, 0.0, w + 10.0, h + 10.0, 5.0, "#1f2125");
            b.bx(0.0, 0.0, 5.0, w, h, 0.5, "#0a1018");
            b.bx(0.0, 0.0, 5.5, w * 0.98, h * 0.98, 0.2, "#0d1a2a");
        }
        "switch" => {
            b.cyl(0.0, 0.0, 0.0, r, 5.0, "#8d9299", 24);
            b.cyl(0.0, 0.0, 5.0, r * 0.6, 2.5, "#c9ced6", 20);
        }
        _ => b.cyl(0.0, 0.0, 0.0, r, 3.0, "#d94c3d", 20),
    }
    if selected {
        if round {
            b.cyl(0.0, 0.0, -0.4, r + 3.5, 0.5, "#ff9a3c", 48);
            b.cyl(0.0, 0.0, -0.3, r + 1.6, 0.6, "#1d1f23", 48);
        } else {
            let (ww, hh, t) = (w + 14.0, h + 14.0, 2.0);
            b.bx(0.0, hh / 2.0 - t / 2.0, -0.4, ww, t, 0.6, "#ff9a3c");
            b.bx(0.0, -hh / 2.0 + t / 2.0, -0.4, ww, t, 0.6, "#ff9a3c");
            b.bx(ww / 2.0 - t / 2.0, 0.0, -0.4, t, hh, 0.6, "#ff9a3c");
            b.bx(-ww / 2.0 + t / 2.0, 0.0, -0.4, t, hh, 0.6, "#ff9a3c");
        }
    }
}

/// Geometry for one part standing on the dash surface at its (x, y).
pub fn build(mesh: &CpuMesh, c: &Comp, selected: bool, ghost: bool, out: &mut Vec<Vtx>) {
    let Some(s) = mesh.surface_at(c.x as f32, c.y as f32) else { return };
    let n = s.n;
    let mut ex = [1.0 - n[0] * n[0], -n[0] * n[1], -n[0] * n[2]];
    if dot(ex, ex).sqrt() < 1e-3 {
        ex = [-n[1] * n[0], 1.0 - n[1] * n[1], -n[1] * n[2]];
    }
    let ex = normalize(ex);
    let cr = cross(n, ex);
    let a = (c.rot as f32).to_radians();
    let exr = [ex[0] * a.cos() + cr[0] * a.sin(), ex[1] * a.cos() + cr[1] * a.sin(), ex[2] * a.cos() + cr[2] * a.sin()];
    let ey = cross(n, exr);
    let mut b = B { out: Vec::new(), pos: [s.p[0], s.p[1], s.p[2] + 0.05], ex: exr, ey, n };
    part(&mut b, c, selected);
    if ghost {
        for v in b.out.iter_mut() {
            v.c = [(v.c[0] * 0.4 + 0.65).min(1.0), v.c[1] * 0.5 + 0.35, v.c[2] * 0.3 + 0.1];
        }
    }
    out.extend(b.out);
}

pub fn radius(c: &Comp) -> f64 {
    if c.shape == "round" { c.w / 2.0 } else { c.w.hypot(c.h) / 2.0 }
}

/// Closed outline of the part's footprint lying on the dash surface (for the always-visible overlay).
pub fn outline(mesh: &CpuMesh, c: &Comp) -> Vec<V3> {
    let mut pts = Vec::new();
    let a = (c.rot as f32).to_radians();
    let (sa, ca) = (a.sin(), a.cos());
    let local: Vec<[f32; 2]> = if c.shape == "round" {
        (0..40).map(|i| { let t = i as f32 / 40.0 * std::f32::consts::TAU; [c.w as f32 / 2.0 * t.cos(), c.w as f32 / 2.0 * t.sin()] }).collect()
    } else {
        let (hw, hh) = (c.w as f32 / 2.0, c.h as f32 / 2.0);
        // sample the edges so the outline follows the surface
        let mut v = Vec::new();
        for i in 0..8 { v.push([-hw + 2.0 * hw * i as f32 / 8.0, -hh]); }
        for i in 0..8 { v.push([hw, -hh + 2.0 * hh * i as f32 / 8.0]); }
        for i in 0..8 { v.push([hw - 2.0 * hw * i as f32 / 8.0, hh]); }
        for i in 0..8 { v.push([-hw, hh - 2.0 * hh * i as f32 / 8.0]); }
        v
    };
    for q in local {
        let (x, y) = (c.x as f32 + q[0] * ca - q[1] * sa, c.y as f32 + q[0] * sa + q[1] * ca);
        if let Some(h) = mesh.surface_at(x, y) {
            pts.push([h.p[0], h.p[1], h.p[2] + 0.4]);
        }
    }
    pts
}
