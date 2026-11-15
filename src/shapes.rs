// Shape repository: parametric 2D shapes (rectangles, circles, polygons, stars, ...) that are edited in the
// shape editor and later dragged onto the dash to cut, imprint or emboss. Everything is in millimeters, centerd
// on the shape's own origin, y up.
use crate::json::{self, J};
use std::f64::consts::PI;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Rect,
    Ellipse,
    Poly,
    Star,
    Cross,
    /// A closed loop of hand-placed vertices.
    Free,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Rect => "Rectangle",
            Kind::Ellipse => "Ellipse",
            Kind::Poly => "Polygon",
            Kind::Star => "Star",
            Kind::Cross => "Cross",
            Kind::Free => "Free loop",
        }
    }
    pub fn id(&self) -> &'static str {
        match self {
            Kind::Rect => "rect",
            Kind::Ellipse => "ellipse",
            Kind::Poly => "poly",
            Kind::Star => "star",
            Kind::Cross => "cross",
            Kind::Free => "free",
        }
    }
    pub fn from_id(s: &str) -> Kind {
        match s {
            "ellipse" => Kind::Ellipse,
            "poly" => Kind::Poly,
            "star" => Kind::Star,
            "cross" => Kind::Cross,
            "free" => Kind::Free,
            _ => Kind::Rect,
        }
    }
    pub const ALL: [Kind; 6] = [Kind::Rect, Kind::Ellipse, Kind::Poly, Kind::Star, Kind::Cross, Kind::Free];
}

#[derive(Clone, Debug)]
pub struct Shape {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub w: f64,
    pub h: f64,
    /// Corner radius in mm (rectangles).
    pub radius: f64,
    /// Number of sides (polygons) or points (stars).
    pub sides: u32,
    /// Star: inner radius as a fraction of the outer. Cross: arm thickness as a fraction of the size.
    pub inner: f64,
    /// Rotation of the polygon / star pattern in degrees.
    pub rot: f64,
    /// Keep the width : height ratio while resizing.
    pub lock: bool,
    /// Vertices of a free loop (relative to the center of its box).
    pub pts: Vec<[f64; 2]>,
    /// Corner radius (mm) at each vertex of a free loop; 0 = sharp.
    pub rad: Vec<f64>,
    /// A smooth vertex has Bezier handles (hin / hout, offsets from the vertex); a sharp one is a plain corner.
    pub smooth: Vec<bool>,
    /// A smooth vertex whose two handles move independently (a cusp), like Illustrator's anchor point tool.
    pub broken: Vec<bool>,
    pub hin: Vec<[f64; 2]>,
    pub hout: Vec<[f64; 2]>,
    /// Extrusion height in mm (0 = a flat 2D shape).
    pub depth: f64,
    /// Wall lean in degrees: positive narrows toward the top, negative flares out.
    pub draft: f64,
    /// Size of the edge treatment on the top rim, mm (0 = sharp edge).
    pub bevel: f64,
    /// Rounded rim instead of a straight chamfer.
    pub bevel_round: bool,
    /// Imported STL part: file name in the meshes folder (empty = an ordinary outline shape).
    pub mesh: String,
    /// Extra pieces combined with the main outline: each one adds to the shape or cuts a hole in it.
    pub parts: Vec<Part>,
}

/// One extra piece of a compound shape: a standard shape (circle, rectangle, polygon, star, cross) placed
/// relative to the center of the main outline. It either adds to the shape or cuts a hole in it.
#[derive(Clone, Debug)]
pub struct Part {
    pub sh: Shape,
    pub cx: f64,
    pub cy: f64,
    /// Turn in degrees (counter-clockwise) about the piece's own center.
    pub turn: f64,
    /// True = cuts a hole, false = adds to the shape.
    pub hole: bool,
}

impl Part {
    pub fn new(kind: Kind, hole: bool) -> Part {
        let (w, h) = match kind {
            Kind::Ellipse => (20.0, 20.0),
            Kind::Rect => (30.0, 20.0),
            _ => (24.0, 24.0),
        };
        let mut sh = Shape::new("", kind, w, h);
        sh.lock = kind == Kind::Ellipse;
        Part { sh, cx: 0.0, cy: 0.0, turn: 0.0, hole }
    }

    /// Short name for lists: "Circle", "Rectangle" ...
    pub fn label(&self) -> &'static str {
        match self.sh.kind {
            Kind::Ellipse => "Circle / oval",
            Kind::Rect => "Rectangle",
            Kind::Poly => "Polygon",
            Kind::Star => "Star",
            Kind::Cross => "Cross",
            Kind::Free => "Free shape",
        }
    }

    /// The piece's outline in the main shape's coordinates, counter-clockwise.
    pub fn outline(&self) -> Vec<[f64; 2]> {
        let a = self.turn.to_radians();
        let (s, c) = a.sin_cos();
        self.sh.outline().iter().map(|p| [self.cx + p[0] * c - p[1] * s, self.cy + p[0] * s + p[1] * c]).collect()
    }

    pub fn to_json(&self) -> J {
        J::Obj(vec![("sh".into(), self.sh.to_json()), ("cx".into(), J::Num(self.cx)), ("cy".into(), J::Num(self.cy)), ("turn".into(), J::Num(self.turn)), ("hole".into(), J::Bool(self.hole))])
    }

    pub fn from_json(j: &J) -> Option<Part> {
        let sh = Shape::from_json(j.get("sh")?);
        Some(Part { sh, cx: j.n("cx").unwrap_or(0.0), cy: j.n("cy").unwrap_or(0.0), turn: j.n("turn").unwrap_or(0.0), hole: j.b("hole").unwrap_or(false) })
    }
}

/// The flat area of a shape as loops: `solid` loops are material (the main outline and every added piece),
/// `holes` are removed from it.
#[derive(Clone, Debug, Default)]
pub struct Region {
    pub solid: Vec<Vec<[f64; 2]>>,
    pub holes: Vec<Vec<[f64; 2]>>,
}

impl Region {
    pub fn single(o: Vec<[f64; 2]>) -> Region {
        Region { solid: vec![o], holes: Vec::new() }
    }
    pub fn is_simple(&self) -> bool {
        self.solid.len() == 1 && self.holes.is_empty()
    }
    pub fn main(&self) -> &[[f64; 2]] {
        self.solid.first().map(|v| v.as_slice()).unwrap_or(&[])
    }
    /// Every point of every loop (for bounding boxes).
    pub fn points(&self) -> impl Iterator<Item = &[f64; 2]> {
        self.solid.iter().chain(self.holes.iter()).flat_map(|l| l.iter())
    }
    /// Mirror across x = 0 (loops keep counter-clockwise order).
    pub fn flipped(&self) -> Region {
        let f = |l: &Vec<[f64; 2]>| -> Vec<[f64; 2]> { l.iter().rev().map(|p| [-p[0], p[1]]).collect() };
        Region { solid: self.solid.iter().map(f).collect(), holes: self.holes.iter().map(f).collect() }
    }
    /// Move the edge of the material inward by d (a hole gets bigger, material gets smaller).
    pub fn inset(&self, d: f64) -> Region {
        Region { solid: self.solid.iter().map(|l| safe_offset(l, d)).collect(), holes: self.holes.iter().map(|l| safe_offset(l, -d)).collect() }
    }
}

/// A triangle solid built from a shape (mm, z up, base on z = 0).
#[derive(Clone)]
pub struct Solid {
    pub pos: Vec<[f64; 3]>,
    pub tris: Vec<[usize; 3]>,
    /// Set when the taper / bevel was too big for the outline and had to be limited.
    pub limited: bool,
}

fn segs_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let cr = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let (d1, d2, d3, d4) = (cr(a, b, c), cr(a, b, d), cr(c, d, a), cr(c, d, b));
    d1 * d2 < -1e-12 && d3 * d4 < -1e-12
}

/// True when the ring is a simple counter-clockwise loop.
pub fn ring_ok(p: &[[f64; 2]]) -> bool {
    let n = p.len();
    if n < 3 || poly_area(p) < 1e-3 {
        return false;
    }
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if segs_cross(a, b, p[j], p[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

/// `offset_outline`, backing off the distance until the ring stays a valid loop (corners tighter than the offset would flip).
pub fn safe_offset(p: &[[f64; 2]], d: f64) -> Vec<[f64; 2]> {
    if d.abs() < 1e-9 {
        return p.to_vec();
    }
    let mut k = 1.0;
    for _ in 0..10 {
        let r = offset_outline(p, d * k);
        if ring_ok(&r) {
            return r;
        }
        k *= 0.8;
    }
    p.to_vec()
}

/// Move every vertex of a counter-clockwise outline inward by d (outward when negative), mitred at the corners.
pub fn offset_outline(p: &[[f64; 2]], d: f64) -> Vec<[f64; 2]> {
    let n = p.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
        let nrm = |u: [f64; 2], v: [f64; 2]| {
            let (dx, dy) = (v[0] - u[0], v[1] - u[1]);
            let l = (dx * dx + dy * dy).sqrt().max(1e-12);
            [-dy / l, dx / l]
        };
        let (n1, n2) = (nrm(a, b), nrm(b, c));
        let k = (1.0 + n1[0] * n2[0] + n1[1] * n2[1]).max(0.35);
        out.push([b[0] + d * (n1[0] + n2[0]) / k, b[1] + d * (n1[1] + n2[1]) / k]);
    }
    out
}

impl Shape {
    /// Extrude the outline into a closed solid: straight or drafted walls with an optional chamfer / round on the top rim.
    /// Like `solid`, but an imported part uses its lighter preview mesh (for icons).
    pub fn solid_thumb(&self) -> Option<Solid> {
        if self.depth > 0.0 && !self.mesh.is_empty() {
            let m = mesh_preview(&self.mesh)?;
            let (sx, sy, sz) = (self.w / m.w0.max(1e-9), self.h / m.h0.max(1e-9), self.depth / m.d0.max(1e-9));
            return Some(Solid { pos: m.pos.iter().map(|p| [p[0] * sx, p[1] * sy, p[2] * sz]).collect(), tris: m.tris.iter().map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(), limited: false });
        }
        self.solid()
    }

    pub fn solid(&self) -> Option<Solid> {
        if self.depth <= 0.0 {
            return None;
        }
        if !self.mesh.is_empty() {
            let m = mesh_data(&self.mesh)?;
            let (sx, sy, sz) = (self.w / m.w0.max(1e-9), self.h / m.h0.max(1e-9), self.depth / m.d0.max(1e-9));
            return Some(Solid { pos: m.pos.iter().map(|p| [p[0] * sx, p[1] * sy, p[2] * sz]).collect(), tris: m.tris.iter().map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(), limited: false });
        }
        let base = self.outline();
        let n = base.len();
        if n < 3 {
            return None;
        }
        let depth = self.depth;
        let mut limited = false;
        let bevel = self.bevel.max(0.0).min(depth);
        let wall_h = depth - bevel;
        let tan = self.draft.max(-60.0).min(60.0).to_radians().tan();
        let max_in = base_inradius(&base) * 0.95;
        let mut off_wall = wall_h * tan;
        // layers: (z, inward offset)
        let mut layers: Vec<(f64, f64)> = vec![(0.0, 0.0)];
        if bevel > 0.0 && wall_h > 1e-9 || bevel <= 0.0 {
            if off_wall > max_in {
                off_wall = max_in;
                limited = true;
            }
            layers.push((wall_h, off_wall));
        } else {
            off_wall = 0.0;
        }
        if bevel > 0.0 {
            let steps = if self.bevel_round { 6 } else { 1 };
            for k in 1..=steps {
                let a = (k as f64 / steps as f64) * std::f64::consts::FRAC_PI_2;
                let (dz, di) = if self.bevel_round { (bevel * a.sin(), bevel * (1.0 - a.cos())) } else { (bevel * k as f64, bevel * k as f64) };
                let mut o = off_wall + di;
                if o > max_in {
                    o = max_in;
                    limited = true;
                }
                layers.push((wall_h + dz, o));
            }
        }
        let reg = self.region();
        if !reg.is_simple() {
            // the boolean is slow enough to matter when the editor asks for it every frame: remember the last result
            thread_local! {
                static LAST: std::cell::RefCell<Option<(String, Solid)>> = std::cell::RefCell::new(None);
            }
            let key = json::to_string(&self.to_json());
            if let Some(s) = LAST.with(|l| l.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, s)| s.clone())) {
                return Some(s);
            }
            let sol = crate::cuts::region_solid(&reg, &layers, limited);
            LAST.with(|l| *l.borrow_mut() = Some((key, sol.clone())));
            return Some(sol);
        }
        Some(Solid::from_layers(&base, &layers, limited))
    }
}

impl Solid {
    /// Stack of outline rings: (z, inward offset) bottom to top, closed with a floor and a roof.
    pub fn from_layers(base: &[[f64; 2]], layers: &[(f64, f64)], limited: bool) -> Solid {
        let n = base.len();
        let mut pos: Vec<[f64; 3]> = Vec::new();
        for (z, o) in layers {
            let ring = safe_offset(base, *o);
            for q in ring {
                pos.push([q[0], q[1], *z]);
            }
        }
        let mut tris: Vec<[usize; 3]> = Vec::new();
        let l = layers.len();
        for k in 0..l - 1 {
            for i in 0..n {
                let j = (i + 1) % n;
                let (a0, b0, a1, b1) = (k * n + i, k * n + j, (k + 1) * n + i, (k + 1) * n + j);
                tris.push([a0, b0, b1]);
                tris.push([a0, b1, a1]);
            }
        }
        let bottom: Vec<[f64; 2]> = (0..n).map(|i| [pos[i][0], pos[i][1]]).collect();
        for t in triangulate(&bottom) {
            tris.push([t[0], t[2], t[1]]);
        }
        let top: Vec<[f64; 2]> = (0..n).map(|i| [pos[(l - 1) * n + i][0], pos[(l - 1) * n + i][1]]).collect();
        for t in triangulate(&top) {
            tris.push([(l - 1) * n + t[0], (l - 1) * n + t[1], (l - 1) * n + t[2]]);
        }
        Solid { pos, tris, limited }
    }

    pub fn to_mesh(&self) -> crate::model::Mesh {
        let mut pos = Vec::with_capacity(self.pos.len() * 3);
        for p in &self.pos {
            pos.extend([p[0] as f32, p[1] as f32, p[2] as f32]);
        }
        let idx = self.tris.iter().flat_map(|t| [t[0] as u32, t[1] as u32, t[2] as u32]).collect();
        crate::model::Mesh { pos, nrm: Vec::new(), idx }
    }
}

/// A rough "how far can I go inward" figure: half the smaller side of the bounding box, scaled for thin shapes.
fn base_inradius(p: &[[f64; 2]]) -> f64 {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for q in p {
        x0 = x0.min(q[0]);
        x1 = x1.max(q[0]);
        y0 = y0.min(q[1]);
        y1 = y1.max(q[1]);
    }
    ((x1 - x0).min(y1 - y0)) / 2.0
}

impl Shape {
    pub fn new(name: &str, kind: Kind, w: f64, h: f64) -> Shape {
        Shape { id: 0, name: name.to_string(), kind, w, h, radius: 0.0, sides: 5, inner: 0.5, rot: 0.0, lock: false, pts: Vec::new(), rad: Vec::new(), smooth: Vec::new(), broken: Vec::new(), hin: Vec::new(), hout: Vec::new(), depth: 0.0, draft: 0.0, bevel: 0.0, bevel_round: false, mesh: String::new(), parts: Vec::new() }
    }

    /// Resize the whole shape (a free loop has its vertices scaled about its center).
    pub fn set_size(&mut self, w: f64, h: f64) {
        let (w, h) = (w.max(0.1), h.max(0.1));
        if self.kind == Kind::Free && self.pts.len() >= 3 {
            let (cw, ch) = (self.w.max(1e-6), self.h.max(1e-6));
            let (fx, fy) = (w / cw, h / ch);
            for p in self.pts.iter_mut().chain(self.hin.iter_mut()).chain(self.hout.iter_mut()) {
                p[0] *= fx;
                p[1] *= fy;
            }
        }
        self.w = w;
        self.h = h;
    }

    /// Turn any shape into a free loop of vertices that can be edited point by point.
    pub fn to_free(&mut self) {
        if self.kind == Kind::Free {
            return;
        }
        self.pts = self.outline();
        self.rad = vec![0.0; self.pts.len()];
        self.smooth = vec![false; self.pts.len()];
        self.broken = vec![false; self.pts.len()];
        self.hin = vec![[0.0; 2]; self.pts.len()];
        self.hout = vec![[0.0; 2]; self.pts.len()];
        self.kind = Kind::Free;
    }

    /// Resize the whole shape by a factor (size, corner radii, free-loop points and handles, extrusion height).
    pub fn scale(&mut self, k: f64) {
        self.fix_rad();
        self.w *= k;
        self.h *= k;
        self.radius *= k;
        self.depth *= k;
        for v in self.pts.iter_mut().chain(self.hin.iter_mut()).chain(self.hout.iter_mut()) {
            *v = [v[0] * k, v[1] * k];
        }
        for r in self.rad.iter_mut() {
            *r *= k;
        }
        for p in self.parts.iter_mut() {
            p.cx *= k;
            p.cy *= k;
            p.sh.w *= k;
            p.sh.h *= k;
            p.sh.radius *= k;
        }
    }

    /// Keep the per-vertex radii the same length as the vertex list.
    pub fn fix_rad(&mut self) {
        let n = self.pts.len();
        self.rad.resize(n, 0.0);
        self.smooth.resize(n, false);
        self.broken.resize(n, false);
        self.hin.resize(n, [0.0; 2]);
        self.hout.resize(n, [0.0; 2]);
    }

    /// Give vertex i Bezier handles along the line between its neighbors (one third of each edge long).
    pub fn make_smooth(&mut self, i: usize) {
        self.fix_rad();
        let n = self.pts.len();
        if i >= n || n < 3 {
            return;
        }
        let (a, _b, c) = (self.pts[(i + n - 1) % n], self.pts[i], self.pts[(i + 1) % n]);
        let d = [c[0] - a[0], c[1] - a[1]];
        let dl = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
        let u = [d[0] / dl, d[1] / dl];
        // Catmull-Rom tangent: a quarter of half the chord between the neighbors (a gentle curve that hugs the corners)
        let l = dl / 8.0;
        self.hout[i] = [u[0] * l, u[1] * l];
        self.hin[i] = [-u[0] * l, -u[1] * l];
        self.smooth[i] = true;
        self.broken[i] = false;
        self.rad[i] = 0.0;
    }

    pub fn make_sharp(&mut self, i: usize) {
        self.fix_rad();
        if i < self.pts.len() {
            self.smooth[i] = false;
            self.broken[i] = false;
        }
    }

    /// Let the two handles of vertex i move independently (true) or tie them into one straight line again (false).
    pub fn set_broken(&mut self, i: usize, on: bool) {
        self.fix_rad();
        if i >= self.pts.len() || !self.smooth[i] {
            return;
        }
        self.broken[i] = on;
        if !on {
            // line the in-handle up opposite the out-handle, keeping both lengths
            let (o, n) = (self.hout[i], self.hin[i]);
            let ol = (o[0] * o[0] + o[1] * o[1]).sqrt();
            let nl = (n[0] * n[0] + n[1] * n[1]).sqrt();
            if ol > 1e-9 {
                self.hin[i] = [-o[0] / ol * nl, -o[1] / ol * nl];
            }
        }
    }

    /// Apply a linear map (flip, rotate) to the vertices and handles.
    pub fn transform(&mut self, f: impl Fn([f64; 2]) -> [f64; 2]) {
        self.fix_rad();
        for p in self.pts.iter_mut() {
            *p = f(*p);
        }
        for p in self.hin.iter_mut() {
            *p = f(*p);
        }
        for p in self.hout.iter_mut() {
            *p = f(*p);
        }
        for p in self.parts.iter_mut() {
            let q = f([p.cx, p.cy]);
            p.cx = q[0];
            p.cy = q[1];
        }
    }

    /// Outline of a free loop: corners (optionally rounded) and smooth vertices with Bezier handles.
    fn free_outline(&self) -> Vec<[f64; 2]> {
        let n = self.pts.len();
        let get = |v: &Vec<f64>, i: usize| v.get(i).copied().unwrap_or(0.0);
        let sm = |i: usize| self.smooth.get(i).copied().unwrap_or(false);
        // nodes: (points of the node, is_smooth)
        let mut nodes: Vec<Vec<[f64; 2]>> = Vec::new();
        for i in 0..n {
            let (pi, ni) = ((i + n - 1) % n, (i + 1) % n);
            let r = get(&self.rad, i);
            if !sm(i) && r >= 0.05 && !sm(pi) && !sm(ni) {
                let one = fillet_vertex(self.pts[pi], self.pts[i], self.pts[ni], r);
                nodes.push(one);
            } else {
                nodes.push(vec![self.pts[i]]);
            }
        }
        let mut out: Vec<[f64; 2]> = Vec::new();
        for i in 0..n {
            let ni = (i + 1) % n;
            out.extend_from_slice(&nodes[i]);
            if sm(i) || sm(ni) {
                let a = *nodes[i].last().unwrap();
                let b = nodes[ni][0];
                let ho = if sm(i) { self.hout.get(i).copied().unwrap_or([0.0; 2]) } else { [0.0; 2] };
                let hi = if sm(ni) { self.hin.get(ni).copied().unwrap_or([0.0; 2]) } else { [0.0; 2] };
                let (c1, c2) = ([a[0] + ho[0], a[1] + ho[1]], [b[0] + hi[0], b[1] + hi[1]]);
                let steps = 20;
                for k in 1..steps {
                    let t = k as f64 / steps as f64;
                    let u = 1.0 - t;
                    out.push([
                        u * u * u * a[0] + 3.0 * u * u * t * c1[0] + 3.0 * u * t * t * c2[0] + t * t * t * b[0],
                        u * u * u * a[1] + 3.0 * u * u * t * c1[1] + 3.0 * u * t * t * c2[1] + t * t * t * b[1],
                    ]);
                }
            }
        }
        out
    }

    /// Re-center a free loop on its box and refresh w, h. Returns the shift of the center.
    pub fn sync_free(&mut self) -> [f64; 2] {
        self.fix_rad();
        if self.pts.len() < 3 {
            return [0.0, 0.0];
        }
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in &self.pts {
            x0 = x0.min(p[0]);
            x1 = x1.max(p[0]);
            y0 = y0.min(p[1]);
            y1 = y1.max(p[1]);
        }
        let c = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
        for p in self.pts.iter_mut() {
            p[0] -= c[0];
            p[1] -= c[1];
        }
        self.w = (x1 - x0).max(0.1);
        self.h = (y1 - y0).max(0.1);
        // the pieces stay where they are on the page while the center of the loop moves
        for p in self.parts.iter_mut() {
            p.cx -= c[0];
            p.cy -= c[1];
        }
        c
    }

    /// The main outline plus the added pieces (as material) and the hole pieces.
    pub fn region(&self) -> Region {
        let mut r = Region::single(self.outline());
        for p in &self.parts {
            let o = p.outline();
            if o.len() >= 3 {
                if p.hole {
                    r.holes.push(o);
                } else {
                    r.solid.push(o);
                }
            }
        }
        r
    }

    pub fn area(&self) -> f64 {
        let o = self.outline();
        poly_area(&o).abs()
    }

    /// Closed outline, counter-clockwise, centerd on the origin, exactly filling w x h.
    pub fn outline(&self) -> Vec<[f64; 2]> {
        dedupe(self.outline_raw())
    }

    fn outline_raw(&self) -> Vec<[f64; 2]> {
        let (w, h) = (self.w.max(0.1), self.h.max(0.1));
        match self.kind {
            Kind::Rect => {
                let r = self.radius.max(0.0).min(w.min(h) / 2.0);
                let (hx, hy) = (w / 2.0, h / 2.0);
                if r < 0.05 {
                    return vec![[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]];
                }
                let steps = 12;
                let mut out = Vec::new();
                // corners CCW starting bottom-right: center, start angle
                let corners = [([hx - r, -hy + r], -PI / 2.0), ([hx - r, hy - r], 0.0), ([-hx + r, hy - r], PI / 2.0), ([-hx + r, -hy + r], PI)];
                for (c, a0) in corners {
                    for i in 0..=steps {
                        let a = a0 + (PI / 2.0) * i as f64 / steps as f64;
                        out.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
                    }
                }
                out
            }
            Kind::Ellipse => (0..72)
                .map(|i| {
                    let a = 2.0 * PI * i as f64 / 72.0;
                    [w / 2.0 * a.cos(), h / 2.0 * a.sin()]
                })
                .collect(),
            Kind::Poly => {
                let n = self.sides.max(3) as usize;
                let raw: Vec<[f64; 2]> = (0..n)
                    .map(|k| {
                        let a = (90.0 + self.rot).to_radians() + 2.0 * PI * k as f64 / n as f64;
                        [a.cos(), a.sin()]
                    })
                    .collect();
                fit(&raw, w, h)
            }
            Kind::Star => {
                let n = self.sides.max(3) as usize;
                let ir = self.inner.max(0.05).min(0.95);
                let raw: Vec<[f64; 2]> = (0..n * 2)
                    .map(|k| {
                        let a = (90.0 + self.rot).to_radians() + PI * k as f64 / n as f64;
                        let r = if k % 2 == 0 { 1.0 } else { ir };
                        [r * a.cos(), r * a.sin()]
                    })
                    .collect();
                fit(&raw, w, h)
            }
            Kind::Free => {
                if self.pts.len() < 3 {
                    return vec![[-w / 2.0, -h / 2.0], [w / 2.0, -h / 2.0], [0.0, h / 2.0]];
                }
                let mut p = self.free_outline();
                if poly_area(&p) < 0.0 {
                    p.reverse();
                }
                p
            }
            Kind::Cross => {
                let t = self.inner.max(0.08).min(0.92);
                let (tx, ty) = (w * t / 2.0, h * t / 2.0);
                let (hx, hy) = (w / 2.0, h / 2.0);
                vec![[tx, -hy], [tx, -ty], [hx, -ty], [hx, ty], [tx, ty], [tx, hy], [-tx, hy], [-tx, ty], [-hx, ty], [-hx, -ty], [-tx, -ty], [-tx, -hy]]
            }
        }
    }

    pub fn to_json(&self) -> J {
        J::Obj(vec![
            ("id".into(), J::Num(self.id as f64)),
            ("name".into(), J::Str(self.name.clone())),
            ("kind".into(), J::Str(self.kind.id().into())),
            ("w".into(), J::Num(self.w)),
            ("h".into(), J::Num(self.h)),
            ("radius".into(), J::Num(self.radius)),
            ("sides".into(), J::Num(self.sides as f64)),
            ("inner".into(), J::Num(self.inner)),
            ("rot".into(), J::Num(self.rot)),
            ("lock".into(), J::Bool(self.lock)),
            ("depth".into(), J::Num(self.depth)),
            ("draft".into(), J::Num(self.draft)),
            ("bevel".into(), J::Num(self.bevel)),
            ("bevel_round".into(), J::Bool(self.bevel_round)),
            ("mesh".into(), J::Str(self.mesh.clone())),
            ("rad".into(), J::Arr(self.rad.iter().map(|r| J::Num(*r)).collect())),
            ("smooth".into(), J::Arr(self.smooth.iter().map(|b| J::Bool(*b)).collect())),
            ("broken".into(), J::Arr(self.broken.iter().map(|b| J::Bool(*b)).collect())),
            ("hin".into(), J::Arr(self.hin.iter().map(|p| J::Arr(vec![J::Num(p[0]), J::Num(p[1])])).collect())),
            ("hout".into(), J::Arr(self.hout.iter().map(|p| J::Arr(vec![J::Num(p[0]), J::Num(p[1])])).collect())),
            ("pts".into(), J::Arr(self.pts.iter().map(|p| J::Arr(vec![J::Num(p[0]), J::Num(p[1])])).collect())),
            ("parts".into(), J::Arr(self.parts.iter().map(|p| p.to_json()).collect())),
        ])
    }

    pub fn from_json(j: &J) -> Shape {
        Shape {
            id: j.n("id").unwrap_or(0.0) as u32,
            name: j.s("name").unwrap_or("Shape").to_string(),
            kind: Kind::from_id(j.s("kind").unwrap_or("rect")),
            w: j.n("w").unwrap_or(50.0).max(0.1),
            h: j.n("h").unwrap_or(50.0).max(0.1),
            radius: j.n("radius").unwrap_or(0.0).max(0.0),
            sides: j.n("sides").unwrap_or(5.0).max(3.0).min(32.0) as u32,
            inner: j.n("inner").unwrap_or(0.5),
            rot: j.n("rot").unwrap_or(0.0),
            lock: j.b("lock").unwrap_or(false),
            depth: j.n("depth").unwrap_or(0.0).max(0.0),
            draft: j.n("draft").unwrap_or(0.0),
            bevel: j.n("bevel").unwrap_or(0.0).max(0.0),
            bevel_round: j.b("bevel_round").unwrap_or(false),
            mesh: j.s("mesh").unwrap_or("").to_string(),
            parts: j.get("parts").and_then(|v| v.arr()).map(|a| a.iter().filter_map(Part::from_json).collect()).unwrap_or_default(),
            smooth: j.get("smooth").and_then(|v| v.arr()).map(|a| a.iter().map(|q| q.boolean().unwrap_or(false)).collect()).unwrap_or_default(),
            broken: j.get("broken").and_then(|v| v.arr()).map(|a| a.iter().map(|q| q.boolean().unwrap_or(false)).collect()).unwrap_or_default(),
            hin: pairs(j.get("hin")),
            hout: pairs(j.get("hout")),
            rad: j.get("rad").and_then(|v| v.arr()).map(|a| a.iter().map(|q| q.num().unwrap_or(0.0)).collect()).unwrap_or_default(),
            pts: j
                .get("pts")
                .and_then(|v| v.arr())
                .map(|a| a.iter().filter_map(|q| q.arr().and_then(|c| if c.len() >= 2 { Some([c[0].num().unwrap_or(0.0), c[1].num().unwrap_or(0.0)]) } else { None })).collect())
                .unwrap_or_default(),
        }
    }
}

/// Round the corners of a closed polygon: vertex i gets a circular arc of radius rad[i] (limited so
/// neighboring corners never overlap).
pub fn fillet_vertex(a: [f64; 2], b: [f64; 2], c: [f64; 2], r: f64) -> Vec<[f64; 2]> {
    let o = fillet(&[a, b, c], &[0.0, r, 0.0]);
    // o = a, arc points around b, c
    if o.len() <= 2 {
        return vec![b];
    }
    o[1..o.len() - 1].to_vec()
}

pub fn fillet(p: &[[f64; 2]], rad: &[f64]) -> Vec<[f64; 2]> {
    let n = p.len();
    let mut out: Vec<[f64; 2]> = Vec::new();
    for i in 0..n {
        let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
        let r = rad.get(i).copied().unwrap_or(0.0);
        let (v1, v2) = ([a[0] - b[0], a[1] - b[1]], [c[0] - b[0], c[1] - b[1]]);
        let (l1, l2) = ((v1[0] * v1[0] + v1[1] * v1[1]).sqrt(), (v2[0] * v2[0] + v2[1] * v2[1]).sqrt());
        if r < 0.05 || l1 < 1e-9 || l2 < 1e-9 {
            out.push(b);
            continue;
        }
        let (u1, u2) = ([v1[0] / l1, v1[1] / l1], [v2[0] / l2, v2[1] / l2]);
        let th = (u1[0] * u2[0] + u1[1] * u2[1]).max(-1.0).min(1.0).acos();
        if th < 1e-3 || PI - th < 1e-3 {
            out.push(b);
            continue;
        }
        let tan_h = (th / 2.0).tan();
        let mut t = r / tan_h;
        let tmax = 0.5 * l1.min(l2);
        if t > tmax {
            t = tmax;
        }
        let rr = t * tan_h;
        let t1 = [b[0] + u1[0] * t, b[1] + u1[1] * t];
        let t2 = [b[0] + u2[0] * t, b[1] + u2[1] * t];
        let bis = [u1[0] + u2[0], u1[1] + u2[1]];
        let bl = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt().max(1e-9);
        let d = rr / (th / 2.0).sin();
        let ctr = [b[0] + bis[0] / bl * d, b[1] + bis[1] / bl * d];
        let a1 = (t1[1] - ctr[1]).atan2(t1[0] - ctr[0]);
        let a2 = (t2[1] - ctr[1]).atan2(t2[0] - ctr[0]);
        let mut da = a2 - a1;
        while da > PI {
            da -= 2.0 * PI;
        }
        while da < -PI {
            da += 2.0 * PI;
        }
        let steps = ((da.abs() / (PI / 2.0)) * 10.0).ceil().max(3.0) as usize;
        for k in 0..=steps {
            let a = a1 + da * k as f64 / steps as f64;
            out.push([ctr[0] + rr * a.cos(), ctr[1] + rr * a.sin()]);
        }
    }
    out
}

fn pairs(v: Option<&J>) -> Vec<[f64; 2]> {
    v.and_then(|v| v.arr()).map(|a| a.iter().map(|q| q.arr().filter(|c| c.len() >= 2).map(|c| [c[0].num().unwrap_or(0.0), c[1].num().unwrap_or(0.0)]).unwrap_or([0.0; 2])).collect()).unwrap_or_default()
}

fn dedupe(mut p: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-7 && (a[1] - b[1]).abs() < 1e-7);
    while p.len() > 1 && (p[0][0] - p[p.len() - 1][0]).abs() < 1e-7 && (p[0][1] - p[p.len() - 1][1]).abs() < 1e-7 {
        p.pop();
    }
    p
}

fn fit(raw: &[[f64; 2]], w: f64, h: f64) -> Vec<[f64; 2]> {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in raw {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
    }
    let (sx, sy) = (w / (x1 - x0).max(1e-9), h / (y1 - y0).max(1e-9));
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    raw.iter().map(|p| [(p[0] - cx) * sx, (p[1] - cy) * sy]).collect()
}

pub fn poly_area(p: &[[f64; 2]]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a / 2.0
}

/// Ear-clipping triangulation of a simple polygon (indices into `p`).
pub fn triangulate(p: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let n = p.len();
    if n < 3 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if poly_area(p) < 0.0 {
        idx.reverse();
    }
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < 10000 {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (a, b, c) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            if cross(p[a], p[b], p[c]) <= 1e-12 {
                continue;
            }
            let inside = idx.iter().any(|&q| {
                q != a && q != b && q != c && cross(p[a], p[b], p[q]) >= 0.0 && cross(p[b], p[c], p[q]) >= 0.0 && cross(p[c], p[a], p[q]) >= 0.0
            });
            if inside {
                continue;
            }
            out.push([a, b, c]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

/// The built-in basic shapes. They are never changed; editing one works on a copy.
pub fn presets() -> Vec<Shape> {
    let mut v = Vec::new();
    v.push(Shape::new("Rectangle", Kind::Rect, 100.0, 60.0));
    let mut s = Shape::new("Square", Kind::Rect, 60.0, 60.0);
    s.lock = true;
    v.push(s);
    let mut s = Shape::new("Rounded rect", Kind::Rect, 100.0, 60.0);
    s.radius = 12.0;
    v.push(s);
    let mut s = Shape::new("Slot", Kind::Rect, 100.0, 30.0);
    s.radius = 15.0;
    v.push(s);
    let mut s = Shape::new("Circle", Kind::Ellipse, 60.0, 60.0);
    s.lock = true;
    v.push(s);
    v.push(Shape::new("Ellipse", Kind::Ellipse, 100.0, 60.0));
    let mut s = Shape::new("Triangle", Kind::Poly, 80.0, 70.0);
    s.sides = 3;
    v.push(s);
    let mut s = Shape::new("Diamond", Kind::Poly, 60.0, 90.0);
    s.sides = 4;
    v.push(s);
    let mut s = Shape::new("Pentagon", Kind::Poly, 70.0, 67.0);
    s.sides = 5;
    v.push(s);
    let mut s = Shape::new("Hexagon", Kind::Poly, 80.0, 69.0);
    s.sides = 6;
    s.rot = 30.0;
    v.push(s);
    let mut s = Shape::new("Octagon", Kind::Poly, 70.0, 70.0);
    s.sides = 8;
    s.rot = 22.5;
    v.push(s);
    let mut s = Shape::new("Star", Kind::Star, 80.0, 76.0);
    s.sides = 5;
    s.inner = 0.45;
    v.push(s);
    let mut s = Shape::new("Cross", Kind::Cross, 80.0, 80.0);
    s.inner = 0.33;
    v.push(s);
    v
}

// ------------------------------------------------------------ imported STL parts
/// A closed triangle mesh, centerd on x/y and sitting on z = 0 (mm).
pub struct MeshData {
    pub pos: Vec<[f64; 3]>,
    pub tris: Vec<[u32; 3]>,
    pub w0: f64,
    pub h0: f64,
    pub d0: f64,
}

static MESH_DIR: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);
static MESH_CACHE: std::sync::Mutex<Vec<(String, std::sync::Arc<MeshData>)>> = std::sync::Mutex::new(Vec::new());

pub fn set_mesh_dir(dir: &std::path::Path) {
    *MESH_DIR.lock().unwrap() = Some(dir.join("meshes"));
}

pub fn mesh_clear_cache() {
    MESH_CACHE.lock().unwrap().clear();
}

pub fn mesh_forget(name: &str) {
    MESH_CACHE.lock().unwrap().retain(|(n, _)| n != name);
}

pub fn mesh_data(name: &str) -> Option<std::sync::Arc<MeshData>> {
    if let Some((_, m)) = MESH_CACHE.lock().unwrap().iter().find(|(n, _)| n == name) {
        return Some(m.clone());
    }
    let dir = MESH_DIR.lock().unwrap().clone()?;
    let bytes = std::fs::read(dir.join(name)).ok()?;
    let m = std::sync::Arc::new(mesh_from_stl(&bytes)?);
    MESH_CACHE.lock().unwrap().push((name.to_string(), m.clone()));
    Some(m)
}

/// Read a binary or ASCII STL, weld the vertices, make the winding point outwards, center it and sit it on z = 0.
pub fn mesh_from_stl(b: &[u8]) -> Option<MeshData> {
    let mut raw: Vec<[f64; 3]> = Vec::new();
    let binary_ok = b.len() >= 84 && {
        let n = u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize;
        84 + n * 50 == b.len()
    };
    if binary_ok {
        let n = u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize;
        for i in 0..n {
            let o = 84 + i * 50 + 12;
            for v in 0..3 {
                let mut p = [0.0f64; 3];
                for k in 0..3 {
                    let q = o + v * 12 + k * 4;
                    p[k] = f32::from_le_bytes([b[q], b[q + 1], b[q + 2], b[q + 3]]) as f64;
                }
                raw.push(p);
            }
        }
    } else {
        let txt = String::from_utf8_lossy(b);
        for line in txt.lines() {
            let l = line.trim();
            if let Some(r) = l.strip_prefix("vertex") {
                let v: Vec<f64> = r.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if v.len() == 3 {
                    raw.push([v[0], v[1], v[2]]);
                }
            }
        }
    }
    if raw.len() < 12 || raw.len() % 3 != 0 {
        return None;
    }
    // weld
    let mut map: std::collections::HashMap<(i64, i64, i64), u32> = std::collections::HashMap::new();
    let mut pos: Vec<[f64; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for t in raw.chunks(3) {
        let mut ids = [0u32; 3];
        for (k, p) in t.iter().enumerate() {
            let key = ((p[0] * 1e4).round() as i64, (p[1] * 1e4).round() as i64, (p[2] * 1e4).round() as i64);
            ids[k] = *map.entry(key).or_insert_with(|| {
                pos.push(*p);
                (pos.len() - 1) as u32
            });
        }
        if ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2] {
            tris.push(ids);
        }
    }
    if tris.len() < 4 {
        return None;
    }
    let (pos, tris) = repair(pos, tris);
    let (mut pos, tris) = (pos, tris);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in &pos {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let (cx, cy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    for p in pos.iter_mut() {
        *p = [p[0] - cx, p[1] - cy, p[2] - lo[2]];
    }
    Some(MeshData { pos, tris, w0: (hi[0] - lo[0]).max(1e-6), h0: (hi[1] - lo[1]).max(1e-6), d0: (hi[2] - lo[2]).max(1e-6) })
}

/// A lighter copy of a mesh for thumbnails (vertex clustering), cached per file.
pub fn mesh_preview(name: &str) -> Option<std::sync::Arc<MeshData>> {
    static CACHE: std::sync::Mutex<Vec<(String, std::sync::Arc<MeshData>)>> = std::sync::Mutex::new(Vec::new());
    if let Some((_, m)) = CACHE.lock().unwrap().iter().find(|(n, _)| n == name) {
        return Some(m.clone());
    }
    let m = mesh_data(name)?;
    let out = if m.tris.len() <= 2500 {
        m.clone()
    } else {
        let ext = m.w0.max(m.h0).max(m.d0);
        let mut cells = 20.0;
        let mut res = None;
        for _ in 0..6 {
            let cs = ext / cells;
            let mut map: std::collections::HashMap<(i64, i64, i64), usize> = std::collections::HashMap::new();
            let mut acc: Vec<([f64; 3], f64)> = Vec::new();
            let mut remap = vec![0u32; m.pos.len()];
            for (i, p) in m.pos.iter().enumerate() {
                let key = ((p[0] / cs).floor() as i64, (p[1] / cs).floor() as i64, (p[2] / cs).floor() as i64);
                let id = *map.entry(key).or_insert_with(|| {
                    acc.push(([0.0; 3], 0.0));
                    acc.len() - 1
                });
                acc[id].0 = [acc[id].0[0] + p[0], acc[id].0[1] + p[1], acc[id].0[2] + p[2]];
                acc[id].1 += 1.0;
                remap[i] = id as u32;
            }
            let pos: Vec<[f64; 3]> = acc.iter().map(|(s, n)| [s[0] / n, s[1] / n, s[2] / n]).collect();
            let mut tris: Vec<[u32; 3]> = Vec::new();
            for t in &m.tris {
                let r = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
                if r[0] != r[1] && r[1] != r[2] && r[0] != r[2] {
                    tris.push(r);
                }
            }
            let ok = tris.len() <= 3500;
            res = Some(MeshData { pos, tris, w0: m.w0, h0: m.h0, d0: m.d0 });
            if ok {
                break;
            }
            cells *= 0.6;
        }
        std::sync::Arc::new(res?)
    };
    CACHE.lock().unwrap().push((name.to_string(), out.clone()));
    Some(out)
}

/// Make a scanned / CAD mesh usable as a solid: drop doubled faces, orient every shell outwards and split vertices
/// and edges that are shared by more than two faces (shells that touch) so the result is a clean manifold.
pub fn repair(pos: Vec<[f64; 3]>, tris: Vec<[u32; 3]>) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
    use std::collections::HashMap;
    // 1. remove duplicate faces (same corners); an exact opposite pair is an internal wall: remove both
    let mut seen: HashMap<[u32; 3], Vec<(usize, bool)>> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        let mut k = *t;
        k.sort();
        // orientation parity of the permutation
        let fwd = (t[0] == k[0] && t[1] == k[1]) || (t[1] == k[0] && t[2] == k[1]) || (t[2] == k[0] && t[0] == k[1]);
        seen.entry(k).or_default().push((i, fwd));
    }
    let mut drop = vec![false; tris.len()];
    for (_, v) in seen.iter() {
        if v.len() > 1 {
            let f: Vec<usize> = v.iter().filter(|x| x.1).map(|x| x.0).collect();
            let r: Vec<usize> = v.iter().filter(|x| !x.1).map(|x| x.0).collect();
            let pairs = f.len().min(r.len());
            for k in 0..pairs {
                drop[f[k]] = true;
                drop[r[k]] = true;
            }
            // leftover duplicates of the same orientation
            for x in f.iter().skip(pairs).skip(1).chain(r.iter().skip(pairs).skip(1)) {
                drop[*x] = true;
            }
        }
    }
    let mut tris: Vec<[u32; 3]> = tris.iter().enumerate().filter(|(i, _)| !drop[*i]).map(|(_, t)| *t).collect();
    let nt = tris.len();
    // 2. orient: flood over edges used by exactly two faces, then make each shell point outwards
    let key = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        for k in 0..3 {
            edges.entry(key(t[k], t[(k + 1) % 3])).or_default().push(i);
        }
    }
    let has_dir = |t: &[u32; 3], a: u32, b: u32| (0..3).any(|k| t[k] == a && t[(k + 1) % 3] == b);
    let mut comp = vec![usize::MAX; nt];
    let mut ncomp = 0;
    for s in 0..nt {
        if comp[s] != usize::MAX {
            continue;
        }
        comp[s] = ncomp;
        let mut stack = vec![s];
        while let Some(t) = stack.pop() {
            let tv = tris[t];
            for k in 0..3 {
                let (a, b) = (tv[k], tv[(k + 1) % 3]);
                let list = &edges[&key(a, b)];
                if list.len() != 2 {
                    continue;
                }
                let n = if list[0] == t { list[1] } else { list[0] };
                if comp[n] != usize::MAX {
                    continue;
                }
                if has_dir(&tris[n], a, b) {
                    tris[n].swap(1, 2);
                }
                comp[n] = ncomp;
                stack.push(n);
            }
        }
        ncomp += 1;
    }
    let mut vol = vec![0.0f64; ncomp];
    for (i, t) in tris.iter().enumerate() {
        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        vol[comp[i]] += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    for (i, t) in tris.iter_mut().enumerate() {
        if vol[comp[i]] < 0.0 {
            t.swap(1, 2);
        }
    }
    // 3. pair every directed edge with one opposite use; vertices only merge across paired edges
    let mut edges: HashMap<(u32, u32), (Vec<usize>, Vec<usize>)> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let e = edges.entry(key(a, b)).or_default();
            if a < b {
                e.0.push(i);
            } else {
                e.1.push(i);
            }
        }
    }
    let mut uf: Vec<usize> = (0..nt * 3).collect();
    fn find(uf: &mut Vec<usize>, mut x: usize) -> usize {
        while uf[x] != x {
            uf[x] = uf[uf[x]];
            x = uf[x];
        }
        x
    }
    let corner = |tris: &Vec<[u32; 3]>, t: usize, v: u32| -> usize { t * 3 + (0..3).find(|&k| tris[t][k] == v).unwrap_or(0) };
    for ((a, b), (f, r)) in edges.iter() {
        // pair faces of the same shell first so touching shells stay apart
        let mut used = vec![false; r.len()];
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        let mut left: Vec<usize> = Vec::new();
        for &t1 in f {
            if let Some(j) = (0..r.len()).find(|&j| !used[j] && comp[r[j]] == comp[t1]) {
                used[j] = true;
                pairs.push((t1, r[j]));
            } else {
                left.push(t1);
            }
        }
        let mut free: Vec<usize> = (0..r.len()).filter(|&j| !used[j]).collect();
        for t1 in left {
            if let Some(j) = free.pop() {
                pairs.push((t1, r[j]));
            }
        }
        for (t1, t2) in pairs {
            for v in [*a, *b] {
                let (c1, c2) = (corner(&tris, t1, v), corner(&tris, t2, v));
                let (r1, r2) = (find(&mut uf, c1), find(&mut uf, c2));
                if r1 != r2 {
                    uf[r1] = r2;
                }
            }
        }
    }
    let mut ids: HashMap<usize, u32> = HashMap::new();
    let mut npos: Vec<[f64; 3]> = Vec::new();
    let mut ntris: Vec<[u32; 3]> = Vec::with_capacity(nt);
    for t in 0..nt {
        let mut o = [0u32; 3];
        for k in 0..3 {
            let r = find(&mut uf, t * 3 + k);
            o[k] = *ids.entry(r).or_insert_with(|| {
                npos.push(pos[tris[t][k] as usize]);
                (npos.len() - 1) as u32
            });
        }
        ntris.push(o);
    }
    // 4. close open holes: follow the loose edges round each hole and fan it to its center
    {
        let mut dir: HashMap<(u32, u32), bool> = HashMap::new();
        for t in &ntris {
            for k in 0..3 {
                dir.insert((t[k], t[(k + 1) % 3]), true);
            }
        }
        let mut out: HashMap<u32, Vec<u32>> = HashMap::new();
        for (&(a, b), _) in dir.iter() {
            if !dir.contains_key(&(b, a)) {
                out.entry(a).or_default().push(b);
            }
        }
        let starts: Vec<u32> = out.keys().copied().collect();
        for st in starts {
            while let Some(first) = out.get_mut(&st).and_then(|v| v.pop()) {
                let mut lp = vec![st];
                let mut cur = first;
                let mut guard = 0;
                while cur != st && guard < 100000 {
                    lp.push(cur);
                    let Some(next) = out.get_mut(&cur).and_then(|v| v.pop()) else { break };
                    cur = next;
                    guard += 1;
                }
                if cur != st || lp.len() < 3 {
                    continue;
                }
                let n = lp.len() as f64;
                let mut c = [0.0; 3];
                for &v in &lp {
                    for a in 0..3 {
                        c[a] += npos[v as usize][a] / n;
                    }
                }
                npos.push(c);
                let ci = (npos.len() - 1) as u32;
                for i in 0..lp.len() {
                    let (a, b) = (lp[i], lp[(i + 1) % lp.len()]);
                    ntris.push([b, a, ci]);
                }
            }
        }
    }
    // vertices that were split apart still sit at the same place; nudge each a hair into its own shell so
    // nothing downstream merges them back together
    let q = |p: &[f64; 3]| ((p[0] * 1e5).round() as i64, (p[1] * 1e5).round() as i64, (p[2] * 1e5).round() as i64);
    let mut count: HashMap<(i64, i64, i64), u32> = HashMap::new();
    for p in &npos {
        *count.entry(q(p)).or_default() += 1;
    }
    let mut sum = vec![[0.0f64; 3]; npos.len()];
    let mut cnt = vec![0.0f64; npos.len()];
    for t in &ntris {
        for k in 0..3 {
            for j in 0..3 {
                if j != k {
                    let o = npos[t[j] as usize];
                    for a in 0..3 {
                        sum[t[k] as usize][a] += o[a];
                    }
                    cnt[t[k] as usize] += 1.0;
                }
            }
        }
    }
    let orig = npos.clone();
    for i in 0..npos.len() {
        if count[&q(&orig[i])] > 1 && cnt[i] > 0.0 {
            let m = [sum[i][0] / cnt[i], sum[i][1] / cnt[i], sum[i][2] / cnt[i]];
            let d = [m[0] - orig[i][0], m[1] - orig[i][1], m[2] - orig[i][2]];
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if l > 0.1 {
                for a in 0..3 {
                    npos[i][a] += d[a] / l * 0.02;
                }
            }
        }
    }
    (npos, ntris)
}

/// Convex outline (at most `max` points) of a mesh seen from above.
pub fn mesh_hull(m: &MeshData, max: usize) -> Vec<[f64; 2]> {
    let mut p: Vec<[f64; 2]> = m.pos.iter().map(|q| [q[0], q[1]]).collect();
    p.sort_by(|a, b| a.partial_cmp(b).unwrap());
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cr = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut h: Vec<[f64; 2]> = Vec::new();
    for &q in &p {
        while h.len() >= 2 && cr(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cr(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
    if h.len() > max {
        let n = h.len();
        h = (0..max).map(|i| h[i * n / max]).collect();
    }
    h
}

/// Build a library shape from a mesh file that is already in the meshes folder.
pub fn mesh_shape(name: &str, file: &str) -> Option<Shape> {
    let m = mesh_data(file)?;
    let mut s = Shape::new(name, Kind::Free, m.w0, m.h0);
    s.pts = mesh_hull(&m, 64);
    s.fix_rad();
    s.depth = m.d0;
    s.mesh = file.to_string();
    Some(s)
}

/// Copy an STL into the meshes folder (as binary) and return the stored file name.
pub fn import_stl(src: &str) -> Result<(String, String), String> {
    let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
    let m = mesh_from_stl(&bytes).ok_or("That does not look like a usable STL file")?;
    let dir = MESH_DIR.lock().unwrap().clone().ok_or("No data folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = std::path::Path::new(src).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("part".into());
    let safe: String = stem.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let mut n = 0;
    let mut file = format!("{}.stl", safe);
    while dir.join(&file).exists() {
        n += 1;
        file = format!("{}_{}.stl", safe, n);
    }
    write_mesh(&dir.join(&file), &m)?;
    Ok((stem, file))
}

fn write_mesh(path: &std::path::Path, m: &MeshData) -> Result<(), String> {
    let mm = crate::model::Mesh { pos: m.pos.iter().flat_map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect(), nrm: Vec::new(), idx: m.tris.iter().flat_map(|t| [t[0], t[1], t[2]]).collect() };
    std::fs::write(path, crate::model::stl_bytes(&mm)).map_err(|e| e.to_string())
}

/// Turn an imported part: axis 0 = about x, 1 = about y, 2 = flip over. The stored file is rewritten.
pub fn turn_mesh(file: &str, how: u8) -> Result<(), String> {
    let m = mesh_data(file).ok_or("part file missing")?;
    let mut pos = m.pos.clone();
    for p in pos.iter_mut() {
        *p = match how {
            0 => [p[0], -p[2], p[1]],
            1 => [p[2], p[1], -p[0]],
            _ => [p[0], -p[1], -p[2]],
        };
    }
    let mut tris = m.tris.clone();
    // a rotation keeps the winding
    let _ = &mut tris;
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in &pos {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let (cx, cy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    for p in pos.iter_mut() {
        *p = [p[0] - cx, p[1] - cy, p[2] - lo[2]];
    }
    let nm = MeshData { pos, tris, w0: (hi[0] - lo[0]).max(1e-6), h0: (hi[1] - lo[1]).max(1e-6), d0: (hi[2] - lo[2]).max(1e-6) };
    let dir = MESH_DIR.lock().unwrap().clone().ok_or("No data folder")?;
    write_mesh(&dir.join(file), &nm)?;
    mesh_forget(file);
    Ok(())
}

pub fn load(dir: &std::path::Path) -> (Vec<Shape>, u32) {
    set_mesh_dir(dir);
    let path = dir.join("shapes.json");
    let mut out = Vec::new();
    let mut next = 1u32;
    if let Ok(txt) = std::fs::read_to_string(&path) {
        if let Ok(j) = json::parse(&txt) {
            if let Some(a) = j.get("shapes").and_then(|v| v.arr()) {
                for s in a {
                    let sh = Shape::from_json(s);
                    next = next.max(sh.id + 1);
                    out.push(sh);
                }
            }
            next = next.max(j.n("next").unwrap_or(1.0) as u32);
        }
    }
    (out, next)
}

pub fn save(dir: &std::path::Path, shapes: &[Shape], next: u32) -> Result<(), String> {
    let j = J::Obj(vec![("next".into(), J::Num(next as f64)), ("shapes".into(), J::Arr(shapes.iter().map(|s| s.to_json()).collect()))]);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("shapes.json"), json::to_string(&j)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_fill_their_box_and_triangulate() {
        for s in presets() {
            let o = s.outline();
            let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for p in &o {
                x0 = x0.min(p[0]);
                x1 = x1.max(p[0]);
                y0 = y0.min(p[1]);
                y1 = y1.max(p[1]);
            }
            assert!((x1 - x0 - s.w).abs() < 0.01 && (y1 - y0 - s.h).abs() < 0.01, "{} box {} {}", s.name, x1 - x0, y1 - y0);
            let tris = triangulate(&o);
            let ta: f64 = tris.iter().map(|t| poly_area(&[o[t[0]], o[t[1]], o[t[2]]]).abs()).sum();
            assert!((ta - poly_area(&o).abs()).abs() < 1e-6 * ta.max(1.0), "{} area {} vs {}", s.name, ta, poly_area(&o));
            println!("{} {} pts {} tris area {:.0}", s.name, o.len(), tris.len(), ta);
        }
    }

    #[test]
    fn rounded_free_loop() {
        let mut sh = Shape::new("sq", Kind::Free, 100.0, 100.0);
        sh.pts = vec![[-50.0, -50.0], [50.0, -50.0], [50.0, 50.0], [-50.0, 50.0]];
        sh.rad = vec![10.0; 4];
        let o = sh.outline();
        let a = poly_area(&o).abs();
        let want = 10000.0 - (4.0 - PI) * 100.0;
        assert!((a - want).abs() < 5.0, "area {} want {}", a, want);
        let tris = triangulate(&o);
        assert!(!tris.is_empty());
        sh.rad = vec![500.0; 4];
        let o2 = sh.outline();
        assert!(poly_area(&o2).abs() > 7000.0);
    }

    #[test]
    fn smooth_free_loop() {
        let mut sh = Shape::new("sq", Kind::Free, 100.0, 100.0);
        sh.pts = vec![[-50.0, -50.0], [50.0, -50.0], [50.0, 50.0], [-50.0, 50.0]];
        sh.fix_rad();
        for i in 0..4 {
            sh.make_smooth(i);
        }
        let o = sh.outline();
        let a = poly_area(&o).abs();
        assert!(a > 6000.0 && a < 14000.0, "area {}", a);
        assert!(!triangulate(&o).is_empty());
        let j = sh.to_json();
        let back = Shape::from_json(&j);
        assert_eq!(back.smooth, sh.smooth);
        assert_eq!(back.hout.len(), 4);
        println!("smooth area {}", a);
    }
}

#[cfg(test)]
mod solid_tests {
    use super::*;
    use crate::model::{open_edges, signed_volume};

    #[test]
    fn compound_shape_hole_and_added_piece() {
        // 40 x 20 plate, 10 high: 8000 mm3
        let mut s = Shape::new("b", Kind::Rect, 40.0, 20.0);
        s.depth = 10.0;
        // a 10 mm round hole in the middle (area about 78.5 * 10 high)
        let mut hole = Part::new(Kind::Ellipse, true);
        hole.sh.w = 10.0;
        hole.sh.h = 10.0;
        s.parts.push(hole);
        let m = s.solid().unwrap().to_mesh();
        assert_eq!(open_edges(&m.idx), 0);
        let v = signed_volume(&m.pos, &m.idx);
        assert!(v < 8000.0 - 700.0 && v > 8000.0 - 850.0, "hole volume {}", v);
        // a 10 x 10 square added at the right end, half outside the plate: adds 5 x 10 x 10 = 500
        let mut add = Part::new(Kind::Rect, false);
        add.sh.w = 10.0;
        add.sh.h = 10.0;
        add.cx = 20.0;
        s.parts.push(add);
        let m = s.solid().unwrap().to_mesh();
        assert_eq!(open_edges(&m.idx), 0);
        let v2 = signed_volume(&m.pos, &m.idx);
        assert!((v2 - v - 500.0).abs() < 8.0, "added {}", v2 - v);
        // saved and loaded again it is the same shape
        let back = Shape::from_json(&s.to_json());
        assert_eq!(back.parts.len(), 2);
        assert!(back.parts[0].hole && !back.parts[1].hole);
        assert!((back.parts[1].cx - 20.0).abs() < 1e-9);
        // a drafted, beveled compound stays closed
        s.draft = 5.0;
        s.bevel = 2.0;
        s.bevel_round = true;
        let m = s.solid().unwrap().to_mesh();
        assert_eq!(open_edges(&m.idx), 0);
    }

    #[test]
    fn pieces_follow_the_loop_when_it_recenters() {
        let mut s = Shape::new("f", Kind::Rect, 40.0, 20.0);
        s.to_free();
        s.parts.push(Part::new(Kind::Ellipse, true));
        let before = s.parts[0].cx;
        s.pts[0][0] -= 10.0;
        let c = s.sync_free();
        assert!((s.parts[0].cx - (before - c[0])).abs() < 1e-9);
    }

    #[test]
    fn extruded_box_is_watertight_with_right_volume() {
        let mut s = Shape::new("b", Kind::Rect, 40.0, 20.0);
        s.depth = 10.0;
        let m = s.solid().unwrap().to_mesh();
        assert_eq!(open_edges(&m.idx), 0);
        assert!((signed_volume(&m.pos, &m.idx) - 8000.0).abs() < 1.0);
    }

    #[test]
    fn draft_and_round_bevel_stay_closed() {
        let mut s = Shape::new("c", Kind::Ellipse, 60.0, 40.0);
        s.depth = 12.0;
        s.draft = 10.0;
        s.bevel = 3.0;
        s.bevel_round = true;
        let m = s.solid().unwrap().to_mesh();
        assert_eq!(open_edges(&m.idx), 0);
        let v = signed_volume(&m.pos, &m.idx);
        assert!(v > 0.0 && v < s.area() * 12.0);
    }

    #[test]
    fn huge_taper_is_limited_and_json_roundtrips() {
        let mut s = Shape::new("star", Kind::Star, 60.0, 60.0);
        s.depth = 50.0;
        s.draft = 45.0;
        s.bevel = 2.0;
        let sol = s.solid().unwrap();
        assert!(sol.limited);
        let back = Shape::from_json(&s.to_json());
        assert_eq!((back.depth, back.draft, back.bevel), (50.0, 45.0, 2.0));
    }
}
