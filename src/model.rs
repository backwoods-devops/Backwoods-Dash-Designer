// Dash model: profiles (Bezier cross-sections) + windshield rail -> lofted skin / solid.
// 1 unit = 1 mm. +y toward the windshield, driver side is -x, +z up.

use crate::json::J;
use crate::materials::Look;

pub const K: usize = 6; // samples per Bezier segment (anchor i sits at column i*K)
pub const M: usize = 32; // rows per span
pub const MIN_GAP: f64 = 15.0;

#[derive(Clone, Copy, Debug)]
pub struct BezPt {
    pub co: [f64; 2],
    pub hl: [f64; 2],
    pub hr: [f64; 2],
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub id: u32,
    pub x: f64,
    pub fixed: bool,
    pub crease: bool,
    pub sym: bool,
    pub pair: Option<u32>,
    pub pts: Vec<BezPt>,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    pub color: String,
    pub start: u32, // profile id where the section starts
    pub end: u32,   // profile id where it ends (covers every span between)
}

#[derive(Clone, Debug)]
pub struct Key {
    pub name: String,
    pub idx: usize, // point index on every profile
}

#[derive(Clone, Debug)]
pub struct Comp {
    pub id: u32,
    pub kind: String,  // gauge, vent, speaker, screen, switch, light ...
    pub shape: String, // round | rect
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rot: f64, // degrees about the surface normal
    pub visible: bool,
    pub locked: bool,
    pub look: Option<Look>, // None = drawn as the catalogue shows it
}

#[derive(Clone, Debug)]
pub struct Project {
    pub profiles: Vec<Profile>,
    pub comps: Vec<Comp>,
    pub next_comp: u32,
    /// Shapes placed on this piece (cut / recess / raise / inlay).
    pub cuts: Vec<crate::cuts::Cut>,
    pub next_cut: u32,
    pub rail: Vec<[f64; 3]>,
    /// Optional bottom guideline: (y, z) per rail point (same x). The last point of every profile rides on it.
    pub rail2: Vec<[f64; 2]>,
    pub look: Look,
    pub rail_sym: bool,
    pub next_id: u32,
    pub sections: Vec<Section>,
    pub keys: Vec<Key>,
    /// Top trim only: the top pad's outer surface it rides on (not saved; set by the engine).
    pub under: Option<std::sync::Arc<Under>>,
    /// Air gap between the trim underside and the pad (mm).
    pub under_gap: f64,
    /// Chamfer on the top edge of each end face (mm, 0 = none). Set by the engine.
    pub end_bevel: f64,
    /// Bevel leg measured down the end face (A); `end_bevel` is the leg measured inward along the top (B).
    pub end_down: f64,
    /// true = rounded (fillet), false = straight (chamfer)
    pub end_round: bool,
    /// sweep all the way down the end face to the underside / pad instead of only `end_down`
    pub end_full: bool,
    /// Top trim front guide mode. false = the guide moves only the end of each profile and the sides are filled down to the pad;
    /// true = the guide moves the whole profile (only the windshield end is locked) and the sides keep the shell thickness.
    pub nose_follows: bool,
    /// The trim's profile points as they were when "move the whole profile" was switched on, so switching back can restore them.
    pub follow_base: Vec<Vec<BezPt>>,
    /// Point columns whose two handles move independently (a corner or cusp); the same point number on every profile.
    pub broken: Vec<bool>,
    /// Trim only: per profile, per node, [height with no pad in the way, height actually used] (see `snap`).
    pub nat: Vec<Vec<[f64; 2]>>,
}

/// Height map of the top pad's outer surface: rows of (y,z) at increasing x.
#[derive(Debug)]
pub struct Under {
    pub xs: Vec<f64>,
    pub rows: Vec<Vec<[f64; 2]>>,
}

impl Under {
    pub fn from_grid(g: &[Vec<[f64; 3]>]) -> Under {
        Under { xs: g.iter().map(|r| r[0][0]).collect(), rows: g.iter().map(|r| r.iter().map(|p| [p[1], p[2]]).collect()).collect() }
    }
    fn z_row(row: &[[f64; 2]], y: f64) -> Option<f64> {
        const SLACK: f64 = 1.5;
        for w in row.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (lo, hi) = if a[0] < b[0] { (a[0], b[0]) } else { (b[0], a[0]) };
            if y >= lo - 1e-9 && y <= hi + 1e-9 {
                let d = b[0] - a[0];
                let t = if d.abs() < 1e-9 { 0.0 } else { (y - a[0]) / d };
                return Some(a[1] + (b[1] - a[1]) * t);
            }
        }
        let (f, l) = (row[0], row[row.len() - 1]);
        if (y - f[0]).abs() <= SLACK { return Some(f[1]); }
        if (y - l[0]).abs() <= SLACK { return Some(l[1]); }
        None
    }
    /// Pad surface height under (x, y), or None where the pad is not underneath.
    pub fn z_at(&self, x: f64, y: f64) -> Option<f64> {
        let n = self.xs.len();
        if n < 2 || x < self.xs[0] - 1e-3 || x > self.xs[n - 1] + 1e-3 {
            return None;
        }
        let mut i = self.xs.partition_point(|&v| v <= x);
        i = i.max(1).min(n - 1);
        let (x0, x1) = (self.xs[i - 1], self.xs[i]);
        let za = Self::z_row(&self.rows[i - 1], y)?;
        let zb = Self::z_row(&self.rows[i], y)?;
        let t = if x1 - x0 < 1e-9 { 0.0 } else { ((x - x0) / (x1 - x0)).max(0.0).min(1.0) };
        // smooth (cubic) between the rows when the neighbors are there, so the surface has no facets at the rows
        if i >= 2 && i + 1 < n {
            if let (Some(z0), Some(z3)) = (Self::z_row(&self.rows[i - 2], y), Self::z_row(&self.rows[i + 1], y)) {
                let (a, b) = (za, zb);
                let (m1, m2) = (0.5 * (b - z0), 0.5 * (z3 - a));
                let (t2, t3) = (t * t, t * t * t);
                return Some((2.0 * t3 - 3.0 * t2 + 1.0) * a + (t3 - 2.0 * t2 + t) * m1 + (-2.0 * t3 + 3.0 * t2) * b + (t3 - t2) * m2);
            }
        }
        Some(za + (zb - za) * t)
    }
}

/// Most the top trim may sit above the pad's surface (mm) before it is cut back to where it meets the pad.
/// How far above the pad a trim profile node stops when the front guide pulls it down (about one shell thickness).
pub const TRIM_NODE_CLEAR: f64 = 6.0;
pub const TRIM_OVERHANG: f64 = 12.0;

pub const PALETTE: [&str; 6] = ["#5b8def", "#43b88c", "#e0a43c", "#c26bd1", "#e0605e", "#4fb7c9"];

pub struct Mesh {
    pub pos: Vec<f32>,
    pub nrm: Vec<f32>,
    pub idx: Vec<u32>,
}

/// Keep only the largest connected piece of a mesh (vertices at the same spot count as joined). Triangle order is kept;
/// returns the new length of the leading `skin` part of the index buffer.
/// Cut the loops out of a 2D polyline that crosses itself: the points inside a loop are spread along the path
/// start -> crossing -> end, so the polyline keeps its point count but no longer folds over itself.
pub fn clip_loops(pts: &mut Vec<[f64; 2]>) {
    let seg_x = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| -> Option<[f64; 2]> {
        let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
        let den = r[0] * s[1] - r[1] * s[0];
        if den.abs() < 1e-12 { return None; }
        let q = [c[0] - a[0], c[1] - a[1]];
        let t = (q[0] * s[1] - q[1] * s[0]) / den;
        let u = (q[0] * r[1] - q[1] * r[0]) / den;
        if t > 1e-6 && t < 1.0 - 1e-6 && u > 1e-6 && u < 1.0 - 1e-6 { Some([a[0] + r[0] * t, a[1] + r[1] * t]) } else { None }
    };
    for _ in 0..8 {
        let n = pts.len();
        let mut found: Option<(usize, usize, [f64; 2])> = None;
        'outer: for i in 0..n.saturating_sub(3) {
            for j in (i + 2..n - 1).rev() {
                if let Some(x) = seg_x(pts[i], pts[i + 1], pts[j], pts[j + 1]) { found = Some((i, j, x)); break 'outer; }
            }
        }
        let Some((i, j, x)) = found else { break };
        let (a, b) = (pts[i], pts[j + 1]);
        let (l1, l2) = (((x[0] - a[0]).powi(2) + (x[1] - a[1]).powi(2)).sqrt(), ((b[0] - x[0]).powi(2) + (b[1] - x[1]).powi(2)).sqrt());
        let m = j - i;
        for k in 1..=m {
            let t = k as f64 / (m as f64 + 1.0) * (l1 + l2);
            pts[i + k] = if t <= l1 { let f = if l1 > 0.0 { t / l1 } else { 0.0 }; [a[0] + (x[0] - a[0]) * f, a[1] + (x[1] - a[1]) * f] } else { let f = if l2 > 0.0 { (t - l1) / l2 } else { 0.0 }; [x[0] + (b[0] - x[0]) * f, x[1] + (b[1] - x[1]) * f] };
        }
    }
}

/// True if any two non-neighboring segments of the polyline cross.
pub fn polyline_crosses(p: &[[f64; 2]]) -> bool {
    let n = p.len();
    for i in 0..n.saturating_sub(1) {
        for j in i + 2..n.saturating_sub(1) {
            let (a, b, c, d) = (p[i], p[i + 1], p[j], p[j + 1]);
            let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
            let den = r[0] * s[1] - r[1] * s[0];
            if den.abs() < 1e-12 { continue; }
            let q = [c[0] - a[0], c[1] - a[1]];
            let t = (q[0] * s[1] - q[1] * s[0]) / den;
            let u = (q[0] * r[1] - q[1] * r[0]) / den;
            if t > 1e-6 && t < 1.0 - 1e-6 && u > 1e-6 && u < 1.0 - 1e-6 { return true; }
        }
    }
    false
}

/// Smallest radius of the outside's turn at the nose, in shell thicknesses.
pub const NOSE_RADIUS: f64 = 1.25;
/// How much thinner than the set thickness the shell may get before it is marked red in the 3D view.
pub const THIN_TOL: f64 = 1.0;

/// Round the sharpest turn of each cross-section (the nose curling under) until no part of it is tighter than `rmin`.
/// Only a short window around the turn moves; its ends stay put, and the rows are treated one by one in y and z.
pub fn round_nose(g: &mut Vec<Vec<[f64; 3]>>, rmin: f64) {
    let nc = g[0].len();
    if nc < 24 { return; }
    let radius = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| -> f64 {
        let (ab, bc, ca) = (((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt(), ((b[0] - c[0]).powi(2) + (b[1] - c[1]).powi(2)).sqrt(), ((c[0] - a[0]).powi(2) + (c[1] - a[1]).powi(2)).sqrt());
        let cross = ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs();
        if cross < 1e-9 { 1e9 } else { ab * bc * ca / (2.0 * cross) }
    };
    for row in g.iter_mut() {
        let turn = |c: usize| -> f64 {
            let (a, b) = ([row[c][1] - row[c - 1][1], row[c][2] - row[c - 1][2]], [row[c + 1][1] - row[c][1], row[c + 1][2] - row[c][2]]);
            (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]).abs()
        };
        let corner = (16..nc - 3).max_by(|&x, &y| turn(x).partial_cmp(&turn(y)).unwrap()).unwrap_or(nc);
        if corner >= nc { continue; }
        let (lo, hi) = (corner.saturating_sub(9).max(2), (corner + 8).min(nc - 2));
        for _ in 0..400 {
            let mut worst = false;
            let old: Vec<[f64; 2]> = row.iter().map(|p| [p[1], p[2]]).collect();
            for c in lo..=hi {
                if radius(old[c - 1], old[c], old[c + 1]) < rmin * 0.98 {
                    worst = true;
                    row[c][1] = 0.5 * old[c][0] + 0.25 * (old[c - 1][0] + old[c + 1][0]);
                    row[c][2] = 0.5 * old[c][1] + 0.25 * (old[c - 1][1] + old[c + 1][1]);
                }
            }
            if !worst { break; }
        }
    }
}

pub fn keep_main_piece(m: &mut Mesh, skin: usize) -> usize {
    use std::collections::HashMap;
    let nv = m.pos.len() / 3;
    let mut canon: Vec<usize> = vec![0; nv];
    let mut seen: HashMap<(i64, i64, i64), usize> = HashMap::new();
    for i in 0..nv {
        let k = (
            (m.pos[i * 3] as f64 * 100.0).round() as i64,
            (m.pos[i * 3 + 1] as f64 * 100.0).round() as i64,
            (m.pos[i * 3 + 2] as f64 * 100.0).round() as i64,
        );
        canon[i] = *seen.entry(k).or_insert(i);
    }
    let mut parent: Vec<usize> = (0..nv).collect();
    fn find(p: &mut Vec<usize>, mut a: usize) -> usize {
        while p[a] != a { p[a] = p[p[a]]; a = p[a]; }
        a
    }
    for t in m.idx.chunks(3) {
        let (a, b, c) = (canon[t[0] as usize], canon[t[1] as usize], canon[t[2] as usize]);
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb { parent[ra] = rb; }
        let (rb, rc) = (find(&mut parent, b), find(&mut parent, c));
        if rb != rc { parent[rb] = rc; }
    }
    let mut count: HashMap<usize, usize> = HashMap::new();
    for t in m.idx.chunks(3) {
        let r = find(&mut parent, canon[t[0] as usize]);
        *count.entry(r).or_insert(0) += 1;
    }
    let main = match count.iter().max_by_key(|(_, n)| **n) { Some((r, _)) => *r, None => return skin };
    let mut out: Vec<u32> = Vec::with_capacity(m.idx.len());
    let mut new_skin = 0usize;
    for (ti, t) in m.idx.chunks(3).enumerate() {
        if find(&mut parent, canon[t[0] as usize]) == main {
            out.extend_from_slice(t);
            if ti * 3 < skin { new_skin += 3; }
        }
    }
    m.idx = out;
    new_skin
}

// ------------------------------------------------------------ math helpers
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l < 1e-12 { [0.0, 0.0, 1.0] } else { [a[0] / l, a[1] / l, a[2] / l] }
}

fn end_slope(h0: f64, h1: f64, d0: f64, d1: f64) -> f64 {
    let mut m = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
    if m.signum() != d0.signum() {
        m = 0.0;
    } else if d0.signum() != d1.signum() && m.abs() > 3.0 * d0.abs() {
        m = 3.0 * d0;
    }
    m
}

/// Monotone cubic (PCHIP) through (xs, ys); level at crests so the rail bows smoothly.
pub fn pchip(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 || x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let mut h = vec![0.0; n - 1];
    let mut d = vec![0.0; n - 1];
    for i in 0..n - 1 {
        h[i] = (xs[i + 1] - xs[i]).max(1e-9);
        d[i] = (ys[i + 1] - ys[i]) / h[i];
    }
    let mut m = vec![0.0; n];
    if n == 2 {
        m[0] = d[0];
        m[1] = d[0];
    } else {
        for i in 1..n - 1 {
            if d[i - 1] * d[i] > 0.0 {
                let w1 = 2.0 * h[i] + h[i - 1];
                let w2 = h[i] + 2.0 * h[i - 1];
                m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
            }
        }
        m[0] = end_slope(h[0], h[1], d[0], d[1]);
        m[n - 1] = end_slope(h[n - 2], h[n - 3], d[n - 2], d[n - 3]);
    }
    let mut i = 0;
    while i < n - 2 && x > xs[i + 1] {
        i += 1;
    }
    let t = (x - xs[i]) / h[i];
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * ys[i]
        + (t3 - 2.0 * t2 + t) * h[i] * m[i]
        + (-2.0 * t3 + 3.0 * t2) * ys[i + 1]
        + (t3 - t2) * h[i] * m[i + 1]
}

// ------------------------------------------------------------ profile
impl Profile {
    pub fn anchor(&self) -> [f64; 2] {
        self.pts[0].co
    }
    pub fn translate(&mut self, d: [f64; 2]) {
        for p in self.pts.iter_mut() {
            for v in [&mut p.co, &mut p.hl, &mut p.hr] {
                v[0] += d[0];
                v[1] += d[1];
            }
        }
    }
    /// Polyline of the Bezier chain, K samples per segment.
    pub fn curve(&self) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        let n = self.pts.len();
        if n == 0 {
            return out;
        }
        for i in 0..n.saturating_sub(1) {
            let (p0, h0, h1, p1) = (self.pts[i].co, self.pts[i].hr, self.pts[i + 1].hl, self.pts[i + 1].co);
            for j in 0..K {
                let t = j as f64 / K as f64;
                let a = (1.0 - t).powi(3);
                let b = 3.0 * t * (1.0 - t).powi(2);
                let c = 3.0 * t * t * (1.0 - t);
                let d = t.powi(3);
                out.push([
                    a * p0[0] + b * h0[0] + c * h1[0] + d * p1[0],
                    a * p0[1] + b * h0[1] + c * h1[1] + d * p1[1],
                ]);
            }
        }
        out.push(self.pts[n - 1].co);
        out
    }
}

// ------------------------------------------------------------ project
impl Project {
    pub fn from_json(j: &J) -> Result<Project, String> {
        let pa = j.get("profiles").and_then(|v| v.arr()).ok_or("no profiles")?;
        let mut profiles = Vec::new();
        for (i, p) in pa.iter().enumerate() {
            let pts_j = p.get("pts").and_then(|v| v.arr()).ok_or("no pts")?;
            let mut pts = Vec::new();
            for q in pts_j {
                let a = q.arr().ok_or("bad pt")?;
                if a.len() < 6 {
                    return Err("pt needs 6 numbers".into());
                }
                let f = |k: usize| a[k].num().unwrap_or(0.0);
                pts.push(BezPt { co: [f(0), f(1)], hl: [f(2), f(3)], hr: [f(4), f(5)] });
            }
            let id = p.n("id").map(|v| v as u32).unwrap_or(i as u32);
            let pair = match p.n("pair") {
                Some(v) if v >= 0.0 => Some(v as u32),
                _ => None,
            };
            profiles.push(Profile {
                id,
                x: p.n("x").unwrap_or(0.0),
                fixed: p.b("fixed").unwrap_or(false),
                crease: p.b("crease").unwrap_or(false),
                sym: p.b("sym").unwrap_or(true),
                pair,
                pts,
            });
        }
        let mut rail = Vec::new();
        for r in j.get("rail").and_then(|v| v.arr()).ok_or("no rail")? {
            let a = r.arr().ok_or("bad rail pt")?;
            if a.len() < 3 {
                return Err("rail pt needs 3 numbers".into());
            }
            rail.push([a[0].num().unwrap_or(0.0), a[1].num().unwrap_or(0.0), a[2].num().unwrap_or(0.0)]);
        }
        let next_id = profiles.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let mut rail2: Vec<[f64; 2]> = Vec::new();
        if let Some(ra) = j.get("rail2").and_then(|v| v.arr()) {
            for r in ra {
                if let Some(a) = r.arr() {
                    if a.len() >= 2 {
                        rail2.push([a[0].num().unwrap_or(0.0), a[1].num().unwrap_or(0.0)]);
                    }
                }
            }
        }
        if rail2.len() != rail.len() {
            rail2.clear();
        }
        let mut pr = Project { profiles, rail, rail2, look: Look::vinyl(), rail_sym: j.b("rail_sym").unwrap_or(true), next_id, sections: Vec::new(), keys: Vec::new(), comps: Vec::new(), next_comp: 1, cuts: Vec::new(), next_cut: 1, under: None, under_gap: 0.0, end_bevel: 0.0, end_down: 0.0, end_round: false, end_full: false, nose_follows: false, follow_base: Vec::new(), broken: Vec::new(), nat: Vec::new() };
        if let Some(ba) = j.get("broken").and_then(|v| v.arr()) {
            let b: Vec<bool> = ba.iter().map(|x| matches!(x, J::Bool(true))).collect();
            if b.iter().any(|&x| x) && Some(b.len()) == pr.profiles.first().map(|p| p.pts.len()) {
                pr.broken = b;
            }
        }
        if let Some(ba) = j.get("follow_base").and_then(|v| v.arr()) {
            for pa in ba {
                let mut v = Vec::new();
                for q in pa.arr().map(|v| v.as_slice()).unwrap_or(&[]) {
                    if let Some(a) = q.arr() {
                        if a.len() >= 6 {
                            let f = |i: usize| a[i].num().unwrap_or(0.0);
                            v.push(BezPt { co: [f(0), f(1)], hl: [f(2), f(3)], hr: [f(4), f(5)] });
                        }
                    }
                }
                pr.follow_base.push(v);
            }
        }
        if let Some(ca) = j.get("cuts").and_then(|v| v.arr()) {
            for c in ca {
                let cut = crate::cuts::Cut::from_json(c);
                pr.next_cut = pr.next_cut.max(cut.id + 1);
                pr.cuts.push(cut);
            }
        }
        if let Some(ca) = j.get("comps").and_then(|v| v.arr()) {
            for c in ca {
                let id = c.n("id").unwrap_or(0.0) as u32;
                pr.comps.push(Comp {
                    id,
                    kind: c.s("kind").unwrap_or("gauge").to_string(),
                    shape: c.s("shape").unwrap_or("round").to_string(),
                    name: c.s("name").unwrap_or("Component").to_string(),
                    x: c.n("x").unwrap_or(0.0),
                    y: c.n("y").unwrap_or(0.0),
                    w: c.n("w").unwrap_or(50.0),
                    h: c.n("h").unwrap_or(50.0),
                    rot: c.n("rot").unwrap_or(0.0),
                    visible: c.b("visible").unwrap_or(true),
                    locked: c.b("locked").unwrap_or(false),
                    look: match (c.s("mat"), c.s("color")) {
                        (Some(a), Some(b)) => Look::from_ids(a, b),
                        _ => None,
                    },
                });
                pr.next_comp = pr.next_comp.max(id + 1);
            }
        }
        if let Some(l) = j.get("look") {
            if let Some(v) = Look::from_ids(l.s("mat").unwrap_or("vinyl"), l.s("color").unwrap_or("")) {
                pr.look = v;
            }
        }
        if let Some(sa) = j.get("sections").and_then(|v| v.arr()) {
            for s in sa {
                pr.sections.push(Section {
                    name: s.s("name").unwrap_or("Section").to_string(),
                    color: s.s("color").unwrap_or("#5b8def").to_string(),
                    start: s.n("start").unwrap_or(0.0) as u32,
                    end: s.n("end").unwrap_or(0.0) as u32,
                });
            }
        }
        if let Some(ka) = j.get("keys").and_then(|v| v.arr()) {
            for k in ka {
                pr.keys.push(Key { name: k.s("name").unwrap_or("Key").to_string(), idx: k.n("idx").unwrap_or(0.0).max(0.0) as usize });
            }
        }
        pr.profiles.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        if pr.rail2.is_empty() {
            pr.rail.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
        } else {
            let mut z: Vec<([f64; 3], [f64; 2])> = pr.rail.iter().cloned().zip(pr.rail2.iter().cloned()).collect();
            z.sort_by(|a, b| a.0[0].partial_cmp(&b.0[0]).unwrap());
            pr.rail = z.iter().map(|q| q.0).collect();
            pr.rail2 = z.iter().map(|q| q.1).collect();
        }
        let n0 = pr.profiles.first().map(|p| p.pts.len()).unwrap_or(0);
        if pr.profiles.len() < 2 || pr.rail.len() < 2 || pr.profiles.iter().any(|p| p.pts.len() != n0 || n0 < 2) {
            return Err("need >=2 profiles, >=2 rail points, equal point counts".into());
        }
        pr.seed_front();
        pr.snap();
        if pr.sections.is_empty() {
            pr.default_sections();
        }
        if pr.keys.is_empty() {
            pr.default_keys();
        }
        pr.fix_sections();
        Ok(pr)
    }

    /// A project with no front contour gets one, traced through the current tip of every profile, so it can be edited.
    pub fn seed_front(&mut self) {
        if !self.rail2.is_empty() || self.profiles.len() < 2 {
            return;
        }
        let tips: Vec<(f64, [f64; 2])> = self.profiles.iter().map(|p| (p.x, p.pts[p.pts.len() - 1].co)).collect();
        let at = |x: f64| -> [f64; 2] {
            if x <= tips[0].0 {
                return tips[0].1;
            }
            for w in tips.windows(2) {
                if x <= w[1].0 {
                    let t = if w[1].0 > w[0].0 { (x - w[0].0) / (w[1].0 - w[0].0) } else { 0.0 };
                    return [w[0].1[0] + (w[1].1[0] - w[0].1[0]) * t, w[0].1[1] + (w[1].1[1] - w[0].1[1]) * t];
                }
            }
            tips[tips.len() - 1].1
        };
        self.rail2 = self.rail.iter().map(|r| at(r[0])).collect();
    }

    /// One section per span, named by position from the center (driver side is -x).
    pub fn default_sections(&mut self) {
        let n = self.profiles.len() - 1;
        self.sections.clear();
        for i in 0..n {
            let name = section_name(i, n);
            let ring = i.min(n - 1 - i);
            self.sections.push(Section {
                name,
                color: PALETTE[ring % PALETTE.len()].to_string(),
                start: self.profiles[i].id,
                end: self.profiles[i + 1].id,
            });
        }
    }

    /// Windshield line, the tightest bend of the middle profile (front arc) and the lip tip.
    pub fn default_keys(&mut self) {
        let mid = self.profiles.len() / 2;
        let c: Vec<[f64; 2]> = self.profiles[mid].pts.iter().map(|p| p.co).collect();
        let n = c.len();
        let mut best = n / 2;
        let mut ba = -1.0;
        for i in 1..n - 1 {
            let (a, b) = ([c[i][0] - c[i - 1][0], c[i][1] - c[i - 1][1]], [c[i + 1][0] - c[i][0], c[i + 1][1] - c[i][1]]);
            let la = (a[0] * a[0] + a[1] * a[1]).sqrt().max(1e-9);
            let lb = (b[0] * b[0] + b[1] * b[1]).sqrt().max(1e-9);
            let cosv = ((a[0] * b[0] + a[1] * b[1]) / (la * lb)).max(-1.0).min(1.0);
            let ang = cosv.acos();
            if ang > ba {
                ba = ang;
                best = i;
            }
        }
        self.keys = vec![
            Key { name: "Windshield line".into(), idx: 0 },
            Key { name: "Front arc".into(), idx: best },
            Key { name: "Lip tip".into(), idx: n - 1 },
        ];
    }

    /// Evaluate a profile at chain parameter tau (segment s plus fraction t) from the left or right side of a joint:
    /// returns (position, derivative per unit tau).
    fn eval_tau(p: &Profile, tau: f64, left: bool) -> ([f64; 2], [f64; 2]) {
        let n = p.pts.len();
        let nseg = n - 1;
        let tau = tau.max(0.0).min(nseg as f64);
        let mut s = tau.floor() as usize;
        let mut t = tau - s as f64;
        if left && t < 1e-9 && s > 0 {
            s -= 1;
            t = 1.0;
        }
        if s >= nseg {
            s = nseg - 1;
            t = 1.0;
        }
        let (p0, h0, h1, p1) = (p.pts[s].co, p.pts[s].hr, p.pts[s + 1].hl, p.pts[s + 1].co);
        let u = 1.0 - t;
        let mut pos = [0.0; 2];
        let mut d = [0.0; 2];
        for a in 0..2 {
            pos[a] = u * u * u * p0[a] + 3.0 * u * u * t * h0[a] + 3.0 * u * t * t * h1[a] + t * t * t * p1[a];
            d[a] = 3.0 * (u * u * (h0[a] - p0[a]) + 2.0 * u * t * (h1[a] - h0[a]) + t * t * (p1[a] - h1[a]));
        }
        (pos, d)
    }

    /// Re-space every profile to `target` points, evenly along the curve (optionally with more points on tight bends).
    /// Sharp corners and named key points are kept exactly. All profiles use the same parameters so they stay matched.
    pub fn resample_all(&mut self, target: usize, bend: bool) -> usize {
        self.broken.clear();
        let n = self.profiles[0].pts.len();
        let nseg = n - 1;
        let target = target.max(4).min(90);
        // mass per sample interval, averaged over profiles
        let ns = nseg * K;
        let mut mass = vec![0.0f64; ns];
        let np = self.profiles.len() as f64;
        for p in self.profiles.iter() {
            let c = p.curve();
            for j in 0..ns {
                let len = ((c[j + 1][0] - c[j][0]).powi(2) + (c[j + 1][1] - c[j][1]).powi(2)).sqrt();
                let mut turn = 0.0;
                if bend && j + 2 <= ns {
                    let a = [c[j + 1][0] - c[j][0], c[j + 1][1] - c[j][1]];
                    let b = [c[j + 2][0] - c[j + 1][0], c[j + 2][1] - c[j + 1][1]];
                    let la = (a[0] * a[0] + a[1] * a[1]).sqrt().max(1e-9);
                    let lb = (b[0] * b[0] + b[1] * b[1]).sqrt().max(1e-9);
                    turn = ((a[0] * b[0] + a[1] * b[1]) / (la * lb)).max(-1.0).min(1.0).acos();
                }
                mass[j] += (len + 45.0 * turn) / np;
            }
        }
        // forced parameters: ends, corners (majority of profiles), key points
        let mut forced: Vec<usize> = vec![0, nseg];
        for k in 1..nseg {
            let mut corners = 0;
            for p in self.profiles.iter() {
                let a = [p.pts[k].co[0] - p.pts[k].hl[0], p.pts[k].co[1] - p.pts[k].hl[1]];
                let b = [p.pts[k].hr[0] - p.pts[k].co[0], p.pts[k].hr[1] - p.pts[k].co[1]];
                let la = (a[0] * a[0] + a[1] * a[1]).sqrt();
                let lb = (b[0] * b[0] + b[1] * b[1]).sqrt();
                if la > 1e-6 && lb > 1e-6 {
                    let ang = ((a[0] * b[0] + a[1] * b[1]) / (la * lb)).max(-1.0).min(1.0).acos();
                    if ang > 0.45 {
                        corners += 1;
                    }
                }
            }
            if corners * 2 >= self.profiles.len() {
                forced.push(k);
            }
        }
        for key in self.keys.iter() {
            forced.push(key.idx.min(nseg));
        }
        forced.sort();
        forced.dedup();
        let target = target.max(forced.len());
        // distribute the free points over the intervals between forced parameters by mass
        let free = target - forced.len();
        let iv_mass: Vec<f64> = forced.windows(2).map(|w| mass[w[0] * K..w[1] * K].iter().sum::<f64>()).collect();
        let total: f64 = iv_mass.iter().sum::<f64>().max(1e-9);
        let mut alloc: Vec<usize> = iv_mass.iter().map(|m| (m / total * free as f64).floor() as usize).collect();
        let mut left = free - alloc.iter().sum::<usize>();
        let mut rem: Vec<(usize, f64)> = iv_mass.iter().enumerate().map(|(i, m)| (i, m / total * free as f64 - alloc[i] as f64)).collect();
        rem.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (i, _) in rem {
            if left == 0 {
                break;
            }
            alloc[i] += 1;
            left -= 1;
        }
        let mut taus: Vec<f64> = Vec::new();
        for (iv, w) in forced.windows(2).enumerate() {
            taus.push(w[0] as f64);
            let cnt = alloc[iv];
            if cnt > 0 {
                let (a, b) = (w[0] * K, w[1] * K);
                let m: f64 = mass[a..b].iter().sum::<f64>().max(1e-9);
                let mut acc = 0.0;
                let mut j = a;
                for q in 1..=cnt {
                    let goal = m * q as f64 / (cnt + 1) as f64;
                    while j < b - 1 && acc + mass[j] < goal {
                        acc += mass[j];
                        j += 1;
                    }
                    let frac = ((goal - acc) / mass[j].max(1e-9)).max(0.0).min(1.0);
                    taus.push((j as f64 + frac) / K as f64);
                }
            }
        }
        taus.push(nseg as f64);
        // Slide the free points along the curve to where they fit it best (all profiles share the positions), so a short
        // profile follows the original shape as closely as the point count allows.
        let fixed: Vec<bool> = taus.iter().map(|t| (t - t.round()).abs() < 1e-9 && forced.contains(&(t.round() as usize))).collect();
        let build = |p: &Profile, taus: &[f64], hs: &[f64]| -> Vec<BezPt> {
            let m = taus.len();
            let mut out: Vec<BezPt> = Vec::with_capacity(m);
            for i in 0..m {
                let (co, dl) = Self::eval_tau(p, taus[i], true);
                let (_, dr) = Self::eval_tau(p, taus[i], false);
                let back = if i > 0 { taus[i] - taus[i - 1] } else { 0.0 };
                let fwd = if i + 1 < m { taus[i + 1] - taus[i] } else { 0.0 };
                let hl = if i > 0 { [co[0] - dl[0] * back / 3.0 * hs[i], co[1] - dl[1] * back / 3.0 * hs[i]] } else { co };
                let hr = if i + 1 < m { [co[0] + dr[0] * fwd / 3.0 * hs[i], co[1] + dr[1] * fwd / 3.0 * hs[i]] } else { co };
                out.push(BezPt { co, hl, hr });
            }
            out
        };
        let mut final_hs = vec![1.0f64; taus.len()];
        if target <= n && taus.len() > 3 {
            // a handful of profiles is enough to judge the fit (they all share the point positions)
            let pstep = (self.profiles.len() / 5).max(1);
            let sel: Vec<usize> = (0..self.profiles.len()).step_by(pstep).chain(std::iter::once(self.profiles.len() - 1)).collect();
            let dense: Vec<Vec<[f64; 2]>> = sel.iter().map(|&i| { let c = self.profiles[i].curve(); let l = c.len(); c.iter().enumerate().filter(|(j, _)| j % 2 == 0 || *j + 1 == l).map(|(_, q)| *q).collect() }).collect();
            let cost = |taus: &[f64], hs: &[f64]| -> f64 {
                let mut tot = 0.0f64;
                let mut worst = 0.0f64;
                for (&pi, cd) in sel.iter().zip(dense.iter()) {
                    let p = &self.profiles[pi];
                    let tmp = Profile { id: p.id, x: p.x, fixed: p.fixed, crease: p.crease, sym: p.sym, pair: p.pair, pts: build(p, taus, hs) };
                    let ca = tmp.curve();
                    let mut samp: Vec<[f64; 2]> = Vec::new();
                    // sample the candidate finely so the comparison is not limited by the preview sampling
                    for i in 0..tmp.pts.len() - 1 {
                        let (p0, h0, h1, p1) = (tmp.pts[i].co, tmp.pts[i].hr, tmp.pts[i + 1].hl, tmp.pts[i + 1].co);
                        for j in 0..8 {
                            let t = j as f64 / 8.0;
                            let u = 1.0 - t;
                            samp.push([
                                u * u * u * p0[0] + 3.0 * u * u * t * h0[0] + 3.0 * u * t * t * h1[0] + t * t * t * p1[0],
                                u * u * u * p0[1] + 3.0 * u * u * t * h0[1] + 3.0 * u * t * t * h1[1] + t * t * t * p1[1],
                            ]);
                        }
                    }
                    // no two neighboring points bunched closer than a few mm: a tiny segment with long handles makes a loop,
                    // and the inside face of the shell folds over on itself there
                    for w in tmp.pts.windows(2) {
                        let ch = ((w[1].co[0] - w[0].co[0]).powi(2) + (w[1].co[1] - w[0].co[1]).powi(2)).sqrt();
                        if ch < 8.0 { tot += (8.0 - ch).powi(2) * 6.0; }
                    }
                    samp.push(ca[ca.len() - 1]);
                    let d2p = |q: [f64; 2], poly: &[[f64; 2]]| -> f64 {
                        let mut best = f64::MAX;
                        for w in poly.windows(2) {
                            let (a, b) = (w[0], w[1]);
                            let d = [b[0] - a[0], b[1] - a[1]];
                            let l2 = (d[0] * d[0] + d[1] * d[1]).max(1e-12);
                            let t = (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / l2).max(0.0).min(1.0);
                            best = best.min((q[0] - a[0] - d[0] * t).powi(2) + (q[1] - a[1] - d[1] * t).powi(2));
                        }
                        best
                    };
                    for q in samp.iter() { let e = d2p(*q, cd); tot += e; worst = worst.max(e); }
                    for q in cd.iter() { let e = d2p(*q, &samp); tot += e; worst = worst.max(e); }
                }
                tot + 40.0 * worst
            };
            let mut hs = vec![1.0f64; taus.len()];
            let mut best = cost(&taus, &hs);
            let mut step = 0.25f64;
            let mut hstep = 0.2f64;
            for _ in 0..14 {
                let mut moved = false;
                for i in 1..taus.len() - 1 {
                    if !fixed[i] {
                        for dir in [-1.0f64, 1.0] {
                            let mut t2 = taus.clone();
                            t2[i] += dir * step;
                            // stay in order, with a little room on either side
                            if t2[i] <= t2[i - 1] + 0.05 || t2[i] >= t2[i + 1] - 0.05 { continue; }
                            let c = cost(&t2, &hs);
                            if c < best - 1e-9 { best = c; taus = t2; moved = true; }
                        }
                    }
                }
                // handle lengths (all points, ends included)
                for i in 0..taus.len() {
                    for dir in [-1.0f64, 1.0] {
                        let mut h2 = hs.clone();
                        h2[i] = (h2[i] + dir * hstep).max(0.3).min(2.2);
                        let c = cost(&taus, &h2);
                        if c < best - 1e-9 { best = c; hs = h2; moved = true; }
                    }
                }
                if !moved { step *= 0.5; hstep *= 0.5; if step < 0.01 { break; } }
            }
            final_hs = hs;
        }
        // rebuild every profile
        let m = taus.len();
        let rebuilt: Vec<Vec<BezPt>> = self.profiles.iter().map(|p| build(p, &taus, &final_hs)).collect();
        for (p, pts) in self.profiles.iter_mut().zip(rebuilt) {
            p.pts = pts;
        }
        for key in self.keys.iter_mut() {
            let kt = key.idx.min(nseg) as f64;
            let mut best = (f64::MAX, 0usize);
            for (i, t) in taus.iter().enumerate() {
                if (t - kt).abs() < best.0 {
                    best = ((t - kt).abs(), i);
                }
            }
            key.idx = best.1;
        }
        self.snap();
        m
    }

    /// Insert a point into segment k (between points k and k+1) on EVERY profile, at parameter t.
    /// The curve keeps its shape; all profiles stay in step.
    pub fn pt_add(&mut self, k: usize, t: f64) -> bool {
        let n = self.profiles[0].pts.len();
        if k + 1 >= n || n >= 90 {
            return false;
        }
        let t = t.max(0.12).min(0.88);
        let lerp = |a: [f64; 2], b: [f64; 2]| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        for p in self.profiles.iter_mut() {
            let (p0, h0, h1, p1) = (p.pts[k].co, p.pts[k].hr, p.pts[k + 1].hl, p.pts[k + 1].co);
            let (q0, q1, q2) = (lerp(p0, h0), lerp(h0, h1), lerp(h1, p1));
            let (r0, r1) = (lerp(q0, q1), lerp(q1, q2));
            let s = lerp(r0, r1);
            p.pts[k].hr = q0;
            p.pts[k + 1].hl = q2;
            p.pts.insert(k + 1, BezPt { co: s, hl: r0, hr: r1 });
        }
        if self.broken.len() == n {
            self.broken.insert(k + 1, false);
        }
        for key in self.keys.iter_mut() {
            if key.idx > k {
                key.idx += 1;
            }
        }
        true
    }

    /// Remove point k (not the two ends) from EVERY profile, joining its neighbors with one smooth segment.
    pub fn pt_remove(&mut self, k: usize) -> bool {
        let n = self.profiles[0].pts.len();
        if k == 0 || k + 1 >= n || n <= 4 {
            return false;
        }
        for p in self.profiles.iter_mut() {
            let a = p.pts[k - 1];
            let b = p.pts[k + 1];
            let gain = 1.5;
            let h0 = [a.co[0] + (a.hr[0] - a.co[0]) * gain, a.co[1] + (a.hr[1] - a.co[1]) * gain];
            let h1 = [b.co[0] + (b.hl[0] - b.co[0]) * gain, b.co[1] + (b.hl[1] - b.co[1]) * gain];
            p.pts[k - 1].hr = h0;
            p.pts[k + 1].hl = h1;
            p.pts.remove(k);
        }
        if self.broken.len() == n {
            self.broken.remove(k);
        }
        for key in self.keys.iter_mut() {
            if key.idx == k {
                key.idx = k - 1;
            } else if key.idx > k {
                key.idx -= 1;
            }
        }
        if !self.rail2.is_empty() {
            self.snap();
        }
        true
    }

    pub fn pid_index(&self, id: u32) -> Option<usize> {
        self.profiles.iter().position(|p| p.id == id)
    }

    /// Drop sections whose profiles vanished or inverted; keep indices valid.
    pub fn fix_sections(&mut self) {
        let ids: Vec<(u32, usize)> = self.profiles.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        let find = |id: u32| ids.iter().find(|(x, _)| *x == id).map(|(_, i)| *i);
        let secs = std::mem::take(&mut self.sections);
        self.sections = secs
            .into_iter()
            .filter(|s| match (find(s.start), find(s.end)) {
                (Some(a), Some(b)) => a < b,
                _ => false,
            })
            .collect();
        self.sections.sort_by_key(|s| find(s.start).unwrap_or(0));
    }

    /// Section index owning span i (profiles i -> i+1), or -1.
    pub fn span_sections(&self) -> Vec<i64> {
        let n = self.profiles.len() - 1;
        let mut out = vec![-1i64; n];
        for (k, s) in self.sections.iter().enumerate() {
            if let (Some(a), Some(b)) = (self.pid_index(s.start), self.pid_index(s.end)) {
                for i in a..b.min(n) {
                    out[i] = k as i64;
                }
            }
        }
        out
    }

    pub fn section_split(&mut self, k: usize, at_pid: u32) -> bool {
        if k >= self.sections.len() {
            return false;
        }
        let s = self.sections[k].clone();
        let (a, m, b) = match (self.pid_index(s.start), self.pid_index(at_pid), self.pid_index(s.end)) {
            (Some(a), Some(m), Some(b)) => (a, m, b),
            _ => return false,
        };
        if !(a < m && m < b) {
            return false;
        }
        let first = Section { name: s.name.clone(), color: s.color.clone(), start: s.start, end: at_pid };
        let col = PALETTE[(self.sections.len()) % PALETTE.len()].to_string();
        let second = Section { name: format!("{} 2", s.name), color: col, start: at_pid, end: s.end };
        self.sections[k] = first;
        self.sections.insert(k + 1, second);
        true
    }

    pub fn section_merge(&mut self, k: usize) -> bool {
        if k + 1 >= self.sections.len() || self.sections[k].end != self.sections[k + 1].start {
            return false;
        }
        let end = self.sections[k + 1].end;
        self.sections[k].end = end;
        self.sections.remove(k + 1);
        true
    }

    /// Move the boundary between section k and k+1 to profile id pid.
    pub fn section_boundary(&mut self, k: usize, pid: u32) -> bool {
        if k + 1 >= self.sections.len() || self.sections[k].end != self.sections[k + 1].start {
            return false;
        }
        let (a, m, b) = match (self.pid_index(self.sections[k].start), self.pid_index(pid), self.pid_index(self.sections[k + 1].end)) {
            (Some(a), Some(m), Some(b)) => (a, m, b),
            _ => return false,
        };
        if !(a < m && m < b) {
            return false;
        }
        self.sections[k].end = pid;
        self.sections[k + 1].start = pid;
        true
    }

    pub fn to_json(&self, with_curves: bool) -> J {
        let num = |v: f64| J::Num(v);
        let mut profs = Vec::new();
        for (i, p) in self.profiles.iter().enumerate() {
            let pts: Vec<J> = p
                .pts
                .iter()
                .map(|q| {
                    J::Arr(vec![num(q.co[0]), num(q.co[1]), num(q.hl[0]), num(q.hl[1]), num(q.hr[0]), num(q.hr[1])])
                })
                .collect();
            let mut o = vec![
                ("id".to_string(), num(p.id as f64)),
                ("x".to_string(), num(p.x)),
                ("fixed".to_string(), J::Bool(p.fixed)),
                ("crease".to_string(), J::Bool(p.crease)),
                ("sym".to_string(), J::Bool(p.sym)),
                ("pair".to_string(), num(p.pair.map(|v| v as f64).unwrap_or(-1.0))),
                ("pts".to_string(), J::Arr(pts)),
            ];
            if with_curves {
                let pi = self.partner(i).map(|v| v as f64).unwrap_or(-1.0);
                o.push(("pair_idx".to_string(), num(pi)));
                let c: Vec<J> = p.curve().iter().map(|q| J::Arr(vec![num(q[0]), num(q[1])])).collect();
                o.push(("curve".to_string(), J::Arr(c)));
            }
            profs.push(J::Obj(o));
        }
        let rail: Vec<J> = self.rail.iter().map(|r| J::Arr(vec![num(r[0]), num(r[1]), num(r[2])])).collect();
        let mut top = vec![
            ("profiles".to_string(), J::Arr(profs)),
            ("rail".to_string(), J::Arr(rail)),
            ("rail_sym".to_string(), J::Bool(self.rail_sym)),
        ];
        let secs: Vec<J> = self
            .sections
            .iter()
            .map(|s| {
                let mut o = vec![
                    ("name".to_string(), J::Str(s.name.clone())),
                    ("color".to_string(), J::Str(s.color.clone())),
                    ("start".to_string(), num(s.start as f64)),
                    ("end".to_string(), num(s.end as f64)),
                ];
                if with_curves {
                    o.push(("from_idx".to_string(), num(self.pid_index(s.start).map(|v| v as f64).unwrap_or(-1.0))));
                    o.push(("to_idx".to_string(), num(self.pid_index(s.end).map(|v| v as f64).unwrap_or(-1.0))));
                }
                J::Obj(o)
            })
            .collect();
        if !self.rail2.is_empty() {
            let r2: Vec<J> = self.rail2.iter().map(|r| J::Arr(vec![num(r[0]), num(r[1])])).collect();
            top.push(("rail2".to_string(), J::Arr(r2)));
        }
        if self.broken.iter().any(|&b| b) {
            top.push(("broken".to_string(), J::Arr(self.broken.iter().map(|&b| J::Bool(b)).collect())));
        }
        if !self.follow_base.is_empty() {
            let fb: Vec<J> = self.follow_base.iter().map(|p| J::Arr(p.iter().map(|q| J::Arr(vec![num(q.co[0]), num(q.co[1]), num(q.hl[0]), num(q.hl[1]), num(q.hr[0]), num(q.hr[1])])).collect())).collect();
            top.push(("follow_base".to_string(), J::Arr(fb)));
        }
        top.push(("look".to_string(), J::Obj(vec![("mat".to_string(), J::Str(self.look.preset().id.to_string())), ("color".to_string(), J::Str(self.look.hex()))])));
        top.push(("sections".to_string(), J::Arr(secs)));
        let comps: Vec<J> = self
            .comps
            .iter()
            .map(|c| {
                let mut o = vec![
                    ("id".to_string(), num(c.id as f64)),
                    ("kind".to_string(), J::Str(c.kind.clone())),
                    ("shape".to_string(), J::Str(c.shape.clone())),
                    ("name".to_string(), J::Str(c.name.clone())),
                    ("x".to_string(), num(c.x)),
                    ("y".to_string(), num(c.y)),
                    ("w".to_string(), num(c.w)),
                    ("h".to_string(), num(c.h)),
                    ("rot".to_string(), num(c.rot)),
                    ("visible".to_string(), J::Bool(c.visible)),
                    ("locked".to_string(), J::Bool(c.locked)),
                ];
                if let Some(l) = c.look {
                    o.push(("mat".to_string(), J::Str(l.preset().id.to_string())));
                    o.push(("color".to_string(), J::Str(l.hex())));
                }
                J::Obj(o)
            })
            .collect();
        top.push(("comps".to_string(), J::Arr(comps)));
        if !self.cuts.is_empty() {
            top.push(("cuts".to_string(), J::Arr(self.cuts.iter().map(|c| c.to_json()).collect())));
        }
        let keys: Vec<J> = self
            .keys
            .iter()
            .map(|k| J::Obj(vec![("name".to_string(), J::Str(k.name.clone())), ("idx".to_string(), num(k.idx as f64))]))
            .collect();
        top.push(("keys".to_string(), J::Arr(keys)));
        if with_curves {
            let ss: Vec<J> = self.span_sections().iter().map(|&v| num(v as f64)).collect();
            top.push(("span_section".to_string(), J::Arr(ss)));
            top.push(("m".to_string(), num(M as f64)));
            let x0 = self.rail.first().unwrap()[0];
            let x1 = self.rail.last().unwrap()[0];
            let steps = 148;
            let mut rc = Vec::new();
            for s in 0..=steps {
                let x = x0 + (x1 - x0) * s as f64 / steps as f64;
                let (y, z) = self.rail_at(x);
                rc.push(J::Arr(vec![num(x), num(y), num(z)]));
            }
            top.push(("rail_curve".to_string(), J::Arr(rc)));
        }
        J::Obj(top)
    }

    /// Box around the surface of this piece: (min, max) in x, y, z. None when it has no profiles.
    pub fn bounds(&self) -> Option<[[f64; 3]; 2]> {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        let mut any = false;
        for p in &self.profiles {
            for q in p.curve() {
                any = true;
                for (k, v) in [p.x, q[0], q[1]].into_iter().enumerate() {
                    lo[k] = lo[k].min(v);
                    hi[k] = hi[k].max(v);
                }
            }
        }
        for r in &self.rail {
            for k in 0..3 {
                lo[k] = lo[k].min(r[k]);
                hi[k] = hi[k].max(r[k]);
            }
        }
        if any { Some([lo, hi]) } else { None }
    }

    /// Stretch the whole piece: every point goes to `a + (p - a) * s` on each axis.
    /// `parts` = also resize the parts and cut-outs themselves (only when the stretch is the same on all axes).
    pub fn scale_about(&mut self, a: [f64; 3], s: [f64; 3], parts: bool) {
        let m = |p: [f64; 2]| [a[1] + (p[0] - a[1]) * s[1], a[2] + (p[1] - a[2]) * s[2]];
        let uniform = (s[0] - s[1]).abs() < 1e-9 && (s[1] - s[2]).abs() < 1e-9;
        for p in self.profiles.iter_mut() {
            p.x = a[0] + (p.x - a[0]) * s[0];
            for q in p.pts.iter_mut() {
                q.co = m(q.co);
                q.hl = m(q.hl);
                q.hr = m(q.hr);
            }
        }
        for r in self.rail.iter_mut() {
            *r = [a[0] + (r[0] - a[0]) * s[0], a[1] + (r[1] - a[1]) * s[1], a[2] + (r[2] - a[2]) * s[2]];
        }
        for r in self.rail2.iter_mut() {
            *r = m(*r);
        }
        let k = s[0];
        for c in self.comps.iter_mut() {
            c.x = a[0] + (c.x - a[0]) * s[0];
            c.y = a[1] + (c.y - a[1]) * s[1];
            if parts && uniform {
                c.w *= k;
                c.h *= k;
            }
        }
        for c in self.cuts.iter_mut() {
            c.x = a[0] + (c.x - a[0]) * s[0];
            c.y = a[1] + (c.y - a[1]) * s[1];
            c.z = a[2] + (c.z - a[2]) * s[2];
            if parts && uniform {
                c.shape.scale(k);
                c.depth *= k;
                c.plug_h *= k;
            }
        }
    }

    pub fn rail_at(&self, x: f64) -> (f64, f64) {
        let xs: Vec<f64> = self.rail.iter().map(|r| r[0]).collect();
        let ys: Vec<f64> = self.rail.iter().map(|r| r[1]).collect();
        let zs: Vec<f64> = self.rail.iter().map(|r| r[2]).collect();
        (pchip(&xs, &ys, x), pchip(&xs, &zs, x))
    }

    pub fn rail2_at(&self, x: f64) -> (f64, f64) {
        let xs: Vec<f64> = self.rail.iter().map(|r| r[0]).collect();
        let ys: Vec<f64> = self.rail2.iter().map(|r| r[0]).collect();
        let zs: Vec<f64> = self.rail2.iter().map(|r| r[1]).collect();
        (pchip(&xs, &ys, x), pchip(&xs, &zs, x))
    }

    /// Put every profile's windshield anchor back on the rail (translating the whole profile);
    /// with a bottom guideline the last point is pulled onto it too.
    pub fn snap(&mut self) {
        for i in 0..self.profiles.len() {
            let (ry, rz) = self.rail_at(self.profiles[i].x);
            let a = self.profiles[i].anchor();
            self.profiles[i].translate([ry - a[0], rz - a[1]]);
            if !self.rail2.is_empty() {
                let (by, bz) = self.rail2_at(self.profiles[i].x);
                let n = self.profiles[i].pts.len();
                let d = [by - self.profiles[i].pts[n - 1].co[0], bz - self.profiles[i].pts[n - 1].co[1]];
                if self.nose_follows && self.under.is_some() && n > 2 {
                    // top trim: the whole nose goes with the front guide. The move fades out toward the windshield, so the
                    // top run stretches or bends to meet it instead of the tip being pulled away from the rest of the profile.
                    let px = self.profiles[i].x;
                    let nprof = self.profiles.len();
                    let mut nat = std::mem::take(&mut self.nat);
                    let pts = &mut self.profiles[i].pts;
                    let mut s = vec![0.0f64; n];
                    for k in 1..n {
                        s[k] = s[k - 1] + ((pts[k].co[0] - pts[k - 1].co[0]).powi(2) + (pts[k].co[1] - pts[k - 1].co[1]).powi(2)).sqrt();
                    }
                    let tot = s[n - 1].max(1e-9);
                    // the nose (last stretch of the profile) moves rigidly with the guide; everything before it is stretched evenly
                    // from the windshield end (no point is left behind, so the run never bunches up or kinks)
                    let s_nose = (tot - 40.0f64.min(0.4 * tot)).max(1e-9);
                    // `nat` remembers where each node would be with no pad in the way. A node pressed onto the pad stays there until
                    // its natural height comes back above the pad, so lifting the guide again does not push a bump up through it.
                    if nat.len() != nprof { nat = vec![Vec::new(); nprof]; }
                    if nat[i].len() != n { nat[i] = (0..n).map(|k| [pts[k].co[1], pts[k].co[1]]).collect(); }
                    let u = self.under.clone();
                    let gap = self.under_gap.max(0.1);
                    for k in 1..n {
                        let w = if k == n - 1 { 1.0 } else { (s[k] / s_nose).min(1.0) };
                        let q = &mut pts[k];
                        // edited by hand (or rebuilt) since the last time: forget the old natural height
                        if (q.co[1] - nat[i][k][1]).abs() > 1e-6 { nat[i][k] = [q.co[1], q.co[1]]; }
                        for v in [&mut q.co, &mut q.hl, &mut q.hr] {
                            v[0] += d[0] * w;
                        }
                        let nat_new = nat[i][k][0] + d[1] * w;
                        let mut z_new = nat_new;
                        if k < n - 1 {
                            if let Some(u) = u.as_ref() {
                                let pz = u.z_at(px, q.co[0]).or_else(|| u.z_at(px, q.co[0] - 0.5)).or_else(|| u.z_at(px, q.co[0] - 2.0));
                                if let Some(pz) = pz {
                                    // full clearance along the top; the last node before the tip only half, since the nose there is
                                    // a steep face that sits on the pad rather than a flat top above it
                                    let clear = if k + 2 >= n { 0.0 } else { TRIM_NODE_CLEAR };
                                    z_new = z_new.max(pz + gap + clear);
                                }
                            }
                        }
                        let dz = z_new - q.co[1];
                        q.co[1] += dz;
                        q.hl[1] += dz;
                        q.hr[1] += dz;
                        nat[i][k] = [nat_new, q.co[1]];
                    }
                    self.nat = nat;
                } else {
                    let q = &mut self.profiles[i].pts[n - 1];
                    for v in [&mut q.co, &mut q.hl, &mut q.hr] {
                        v[0] += d[0];
                        v[1] += d[1];
                    }
                }
            }
        }
    }

    /// Switch the top trim's front guide between moving only the profile ends and moving the whole profile. Going to the whole
    /// profile remembers the profiles as they are; coming back restores them, so the shape (and the end fill) is the same as before.
    pub fn set_follow(&mut self, follow: bool) {
        if follow == self.nose_follows {
            return;
        }
        if follow {
            self.follow_base = self.profiles.iter().map(|p| p.pts.clone()).collect();
        } else {
            let base = std::mem::take(&mut self.follow_base);
            if base.len() == self.profiles.len() && base.iter().zip(&self.profiles).all(|(b, p)| b.len() == p.pts.len()) {
                for (b, p) in base.into_iter().zip(self.profiles.iter_mut()) {
                    p.pts = b;
                }
            }
            self.nat.clear();
        }
        self.nose_follows = follow;
    }

    /// Move a point of the bottom guideline (its x follows the top guideline point).
    pub fn move_rail2(&mut self, k: usize, y: f64, z: f64) {
        if k >= self.rail2.len() {
            return;
        }
        self.rail2[k] = [y, z];
        if self.rail_sym {
            if let Some(j) = self.rail_partner(k) {
                self.rail2[j] = [y, z];
            }
        }
        self.snap();
    }

    pub fn partner(&self, i: usize) -> Option<usize> {
        let pid = self.profiles[i].pair?;
        self.profiles.iter().position(|p| p.id == pid)
    }

    /// Mirror index of profile i for span mirroring: its locked partner, or itself for the center profile.
    fn mirror_idx(&self, i: usize) -> Option<usize> {
        self.locked_partner(i).or_else(|| {
            if self.profiles[i].x.abs() < 1.0 && self.profiles[i].pair.is_none() { Some(i) } else { None }
        })
    }

    pub fn locked_partner(&self, i: usize) -> Option<usize> {
        let j = self.partner(i)?;
        if self.profiles[i].sym && self.profiles[j].sym { Some(j) } else { None }
    }

    /// Copy shape of profile i onto its locked partner (partner keeps its own anchor on the rail).
    fn sync_from(&mut self, i: usize) {
        if let Some(j) = self.locked_partner(i) {
            let ai = self.profiles[i].anchor();
            let aj = self.profiles[j].anchor();
            let src = self.profiles[i].pts.clone();
            let d = [aj[0] - ai[0], aj[1] - ai[1]];
            for (q, p) in self.profiles[j].pts.iter_mut().zip(src.iter()) {
                q.co = [p.co[0] + d[0], p.co[1] + d[1]];
                q.hl = [p.hl[0] + d[0], p.hl[1] + d[1]];
                q.hr = [p.hr[0] + d[0], p.hr[1] + d[1]];
            }
            // the copy must still hang on both guidelines: windshield end on the windshield line, front end on the front line
            self.snap();
        }
    }

    fn clamp_x(&self, i: usize, x: f64) -> f64 {
        let n = self.profiles.len();
        let lo = if i > 0 { self.profiles[i - 1].x + MIN_GAP } else { -1e9 };
        let hi = if i + 1 < n { self.profiles[i + 1].x - MIN_GAP } else { 1e9 };
        x.max(lo).min(hi.max(lo))
    }

    // ---- profile ops
    /// A profile and the guideline points it hangs on travel together: when a profile slides from `old` to `new`
    /// across the dash, the point of the windshield line (and so the front line) at that position goes with it.
    pub fn move_profile(&mut self, i: usize, x: f64) {
        if i >= self.profiles.len() || self.profiles[i].fixed {
            return;
        }
        let nx = self.clamp_x(i, x);
        self.profiles[i].x = nx;
        if let Some(j) = self.locked_partner(i) {
            if !self.profiles[j].fixed {
                let px = self.clamp_x(j, -nx);
                self.profiles[j].x = px;
            }
        }
        self.snap();
    }

    pub fn edit_pt(&mut self, i: usize, k: usize, which: u8, y: f64, z: f64) {
        if i >= self.profiles.len() || k >= self.profiles[i].pts.len() {
            return;
        }
        let n = self.profiles[i].pts.len();
        let brk = self.is_broken(k);
        {
            let p = &mut self.profiles[i].pts[k];
            match which {
                0 => {
                    if k == 0 || (k + 1 == n && !self.rail2.is_empty()) {
                        return; // end points belong to the guidelines
                    }
                    let d = [y - p.co[0], z - p.co[1]];
                    p.co = [y, z];
                    p.hl = [p.hl[0] + d[0], p.hl[1] + d[1]];
                    p.hr = [p.hr[0] + d[0], p.hr[1] + d[1]];
                    self.keep_above_pad(i, k);
                }
                1 => {
                    p.hl = [y, z];
                    if k > 0 && k + 1 < n && !brk {
                        p.hr = align(p.co, p.hl, p.hr);
                    }
                }
                _ => {
                    p.hr = [y, z];
                    if k > 0 && k + 1 < n && !brk {
                        p.hl = align(p.co, p.hr, p.hl);
                    }
                }
            }
        }
        self.sync_from(i);
        if !self.rail2.is_empty() {
            self.snap();
        }
    }

    pub fn is_broken(&self, k: usize) -> bool {
        self.profiles.first().map_or(false, |p| self.broken.len() == p.pts.len()) && self.broken.get(k).copied().unwrap_or(false)
    }

    /// Break (on) or join (off) the two handles of point k on every profile. Joining sets the handles back in line (along the
    /// direction from the left handle through the right one), keeping each handle's length.
    pub fn set_broken(&mut self, k: usize, on: bool) {
        let n = self.profiles[0].pts.len();
        if k == 0 || k + 1 >= n {
            return;
        }
        if self.broken.len() != n {
            self.broken = vec![false; n];
        }
        self.broken[k] = on;
        if !on {
            for p in self.profiles.iter_mut() {
                let q = &mut p.pts[k];
                let (dy, dz) = (q.hr[0] - q.hl[0], q.hr[1] - q.hl[1]);
                let l = (dy * dy + dz * dz).sqrt();
                if l < 1e-9 {
                    continue;
                }
                let (uy, uz) = (dy / l, dz / l);
                let (ll, lr) = (((q.co[0] - q.hl[0]).powi(2) + (q.co[1] - q.hl[1]).powi(2)).sqrt(), ((q.hr[0] - q.co[0]).powi(2) + (q.hr[1] - q.co[1]).powi(2)).sqrt());
                q.hl = [q.co[0] - uy * ll, q.co[1] - uz * ll];
                q.hr = [q.co[0] + uy * lr, q.co[1] + uz * lr];
            }
        }
        for i in 0..self.profiles.len() {
            self.sync_from(i);
        }
        if !self.rail2.is_empty() {
            self.snap();
        }
    }

    /// Top trim: a node dragged below the top pad's surface is lifted back onto it (handles go with it).
    fn keep_above_pad(&mut self, i: usize, k: usize) {
        let Some(u) = self.under.clone() else { return };
        let (px, y) = (self.profiles[i].x, self.profiles[i].pts[k].co[0]);
        let Some(pz) = u.z_at(px, y).or_else(|| u.z_at(px, y - 0.5)).or_else(|| u.z_at(px, y - 2.0)) else { return };
        let floor = pz + self.under_gap.max(0.1);
        let p = &mut self.profiles[i].pts[k];
        if p.co[1] < floor {
            let dz = floor - p.co[1];
            for v in [&mut p.co, &mut p.hl, &mut p.hr] {
                v[1] += dz;
            }
        }
    }

    /// Move several points of one profile together by (dy, dz), handles included.
    pub fn move_pts(&mut self, i: usize, ks: &[usize], dy: f64, dz: f64) {
        if i >= self.profiles.len() {
            return;
        }
        let n = self.profiles[i].pts.len();
        let mut moved = false;
        for &k in ks {
            if k == 0 || k >= n || (k + 1 == n && !self.rail2.is_empty()) {
                continue;
            }
            let p = &mut self.profiles[i].pts[k];
            for v in [&mut p.co, &mut p.hl, &mut p.hr] {
                v[0] += dy;
                v[1] += dz;
            }
            self.keep_above_pad(i, k);
            moved = true;
        }
        if moved {
            self.sync_from(i);
            if !self.rail2.is_empty() {
                self.snap();
            }
        }
    }

    fn between(&mut self, a: usize, b: usize) -> Profile {
        let (pa, pb) = (&self.profiles[a], &self.profiles[b]);
        let pts: Vec<BezPt> = pa
            .pts
            .iter()
            .zip(pb.pts.iter())
            .map(|(p, q)| BezPt {
                co: [(p.co[0] + q.co[0]) / 2.0, (p.co[1] + q.co[1]) / 2.0],
                hl: [(p.hl[0] + q.hl[0]) / 2.0, (p.hl[1] + q.hl[1]) / 2.0],
                hr: [(p.hr[0] + q.hr[0]) / 2.0, (p.hr[1] + q.hr[1]) / 2.0],
            })
            .collect();
        let id = self.next_id;
        self.next_id += 1;
        Profile { id, x: (pa.x + pb.x) / 2.0, fixed: false, crease: false, sym: true, pair: None, pts }
    }

    pub fn add_after(&mut self, i: usize) -> bool {
        if i + 1 >= self.profiles.len() {
            return false;
        }
        let mut new1 = self.between(i, i + 1);
        let mut ins: Vec<(usize, Profile)> = Vec::new();
        let (pa, pb) = (self.mirror_idx(i), self.mirror_idx(i + 1));
        let mut second: Option<(usize, Profile)> = None;
        if let (Some(a), Some(b)) = (pa, pb) {
            // mirror span runs b .. a (b is just below a); skip when it is the same span
            if b + 1 == a && !(b == i && a == i + 1) {
                let mut new2 = self.between(b, a);
                if !(a == b) {
                    new2.pair = Some(new1.id);
                    new1.pair = Some(new2.id);
                }
                second = Some((b + 1, new2));
            }
        }
        ins.push((i + 1, new1));
        if let Some(s) = second {
            ins.push(s);
        }
        ins.sort_by(|a, b| b.0.cmp(&a.0));
        for (at, p) in ins {
            self.profiles.insert(at, p);
        }
        self.snap();
        true
    }

    /// Why a new profile cannot go at x (None when it can). With `sym`, the mirror position -x must be free too.
    pub fn add_profile_problem(&self, x: f64, sym: bool) -> Option<String> {
        let (lo, hi) = (self.profiles.first().map_or(0.0, |p| p.x), self.profiles.last().map_or(0.0, |p| p.x));
        let free = |v: f64| -> Option<String> {
            if v <= lo + MIN_GAP || v >= hi - MIN_GAP {
                return Some(format!("x = {:.0} is outside the dash, or too close to its end", v));
            }
            if let Some(p) = self.profiles.iter().find(|p| (p.x - v).abs() < MIN_GAP) {
                return Some(format!("too close to the profile at x = {:.0} (keep {:.0} mm apart)", p.x, MIN_GAP));
            }
            None
        };
        if let Some(e) = free(x) {
            return Some(e);
        }
        if sym {
            if x.abs() < MIN_GAP {
                return Some("the center profile is already there".into());
            }
            if (x + x).abs() < MIN_GAP * 2.0 {
                return Some("too close to the center".into());
            }
            return free(-x);
        }
        None
    }

    fn profile_at(&mut self, x: f64) -> (usize, Profile) {
        let at = self.profiles.iter().filter(|p| p.x < x).count().max(1).min(self.profiles.len() - 1);
        if self.under.is_some() {
            // top trim: take the profile on each side and turn/scale it so its two ends sit exactly on the windshield line and the
            // front line at this x (as even as they can be made), then the new profile is the average of those two
            let (ra, rb) = (self.rail_at(x), if self.rail2.is_empty() { None } else { Some(self.rail2_at(x)) });
            let (pa, pb) = (&self.profiles[at - 1], &self.profiles[at]);
            let tip_of = |p: &Profile| p.pts.last().map(|q| q.co).unwrap_or([0.0, 0.0]);
            let h = ((x - pa.x) / (pb.x - pa.x)).max(0.0).min(1.0);
            let a0 = [ra.0, ra.1];
            let b0 = match rb { Some(r) => [r.0, r.1], None => { let (u, v) = (tip_of(pa), tip_of(pb)); [u[0] + (v[0] - u[0]) * h, u[1] + (v[1] - u[1]) * h] } };
            // the front line may dip under the pad between its nodes; the new profile's tip (and so the node it adds to the line)
            // rides on the pad instead, so the line, the profile and the surface all agree
            let mut b0 = b0;
            if let Some(u) = self.under.as_ref() {
                if let Some(pz) = u.z_at(x, b0[0]).or_else(|| u.z_at(x, b0[0] - 0.5)).or_else(|| u.z_at(x, b0[0] - 2.0)) {
                    b0[1] = b0[1].max(pz + self.under_gap.max(0.1));
                }
            }
            let fit = |p: &Profile| -> Vec<BezPt> {
                let (s0, s1) = (p.pts[0].co, tip_of(p));
                let (dx, dy) = (s1[0] - s0[0], s1[1] - s0[1]);
                let (ex, ey) = (b0[0] - a0[0], b0[1] - a0[1]);
                let den = (dx * dx + dy * dy).max(1e-9);
                // complex ratio (e / d): rotation and scale taking this profile's end-to-end line onto the target's
                let (cr, ci) = ((ex * dx + ey * dy) / den, (ey * dx - ex * dy) / den);
                let map = |q: [f64; 2]| { let (u, v) = (q[0] - s0[0], q[1] - s0[1]); [a0[0] + u * cr - v * ci, a0[1] + u * ci + v * cr] };
                p.pts.iter().map(|q| BezPt { co: map(q.co), hl: map(q.hl), hr: map(q.hr) }).collect()
            };
            let (fa, fb) = (fit(pa), fit(pb));
            let mix = |p: [f64; 2], q: [f64; 2]| [0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1])];
            let pts: Vec<BezPt> = fa.iter().zip(fb.iter()).map(|(p, q)| BezPt { co: mix(p.co, q.co), hl: mix(p.hl, q.hl), hr: mix(p.hr, q.hr) }).collect();
            let id = self.next_id;
            self.next_id += 1;
            return (at, Profile { id, x, fixed: false, crease: false, sym: false, pair: None, pts });
        }
        let (a, b) = (&self.profiles[at - 1], &self.profiles[at]);
        let mut t = ((x - a.x) / (b.x - a.x)).max(0.0).min(1.0);
        if self.under.is_some() {
            // top trim: an even average of the profile on each side, whatever the spacing
            t = 0.5;
        }
        let mix = |p: [f64; 2], q: [f64; 2]| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
        let pts: Vec<BezPt> = a.pts.iter().zip(b.pts.iter()).map(|(p, q)| BezPt { co: mix(p.co, q.co), hl: mix(p.hl, q.hl), hr: mix(p.hr, q.hr) }).collect();
        let id = self.next_id;
        self.next_id += 1;
        (at, Profile { id, x, fixed: false, crease: false, sym: false, pair: None, pts })
    }

    /// Add a profile at x across the dash. `sym` also adds its mirror image at -x and keeps the two linked.
    /// Its shape is blended from its neighbors; its ends hang on the windshield and front lines.
    pub fn add_profile_at(&mut self, x: f64, sym: bool) -> Option<usize> {
        if self.add_profile_problem(x, sym).is_some() {
            return None;
        }
        let (at1, mut p1) = self.profile_at(x);
        let mut ins: Vec<(usize, Profile)> = Vec::new();
        let p2_pre = if sym { Some(self.profile_at(-x)) } else { None };
        if self.under.is_some() {
            // top trim: the new profile hangs on the windshield line, so the line gets a node at that x (shape unchanged); the
            // front line gets a node where the new profile's own tip is, so snapping leaves the profile exactly as it was cut
            let tip = |p: &Profile| p.pts.last().map(|q| q.co);
            self.rail_node_at(x, tip(&p1));
            if let Some((_, p2)) = &p2_pre {
                let t2 = tip(p2);
                self.rail_node_at(-x, t2);
            }
        }
        if let Some((at2, mut p2)) = p2_pre {
            p1.sym = true;
            p2.sym = true;
            p1.pair = Some(p2.id);
            p2.pair = Some(p1.id);
            ins.push((at2, p2));
        }
        let id1 = p1.id;
        ins.push((at1, p1));
        ins.sort_by(|a, b| b.0.cmp(&a.0));
        for (at, p) in ins {
            self.profiles.insert(at, p);
        }
        self.snap();
        let i = self.profiles.iter().position(|p| p.id == id1)?;
        if sym {
            self.sync_from(i);
        }
        Some(i)
    }

    /// Insert a guideline node at x on the existing windshield/front lines (no-op when one is already there).
    fn rail_node_at(&mut self, x: f64, tip: Option<[f64; 2]>) {
        if self.rail.iter().any(|r| (r[0] - x).abs() < 1.0) {
            return;
        }
        let (y, z) = self.rail_at(x);
        let r2 = if self.rail2.is_empty() { None } else { Some(tip.map_or_else(|| self.rail2_at(x), |t| (t[0], t[1]))) };
        let at = self.rail.iter().filter(|r| r[0] < x).count();
        self.rail.insert(at, [x, y, z]);
        if let Some((a, b)) = r2 {
            self.rail2.insert(at, [a, b]);
        }
    }

    pub fn remove(&mut self, i: usize) -> bool {
        if i >= self.profiles.len() || self.profiles[i].fixed {
            return false;
        }
        let mut kill = vec![i];
        if let Some(j) = self.locked_partner(i) {
            if !self.profiles[j].fixed {
                kill.push(j);
            }
        }
        if self.profiles.len() - kill.len() < 3 {
            return false;
        }
        let ids: Vec<u32> = kill.iter().map(|&k| self.profiles[k].id).collect();
        // boundaries sitting on a removed profile slide to the next surviving one (else the previous)
        let order: Vec<u32> = self.profiles.iter().map(|p| p.id).collect();
        let repl = |id: u32| -> u32 {
            let pos = order.iter().position(|&x| x == id).unwrap_or(0);
            for &c in order[pos..].iter() {
                if !ids.contains(&c) {
                    return c;
                }
            }
            for &c in order[..pos].iter().rev() {
                if !ids.contains(&c) {
                    return c;
                }
            }
            id
        };
        for s in self.sections.iter_mut() {
            if ids.contains(&s.start) {
                s.start = repl(s.start);
            }
            if ids.contains(&s.end) {
                s.end = repl(s.end);
            }
        }
        self.profiles.retain(|p| !ids.contains(&p.id));
        self.fix_sections();
        for p in self.profiles.iter_mut() {
            if let Some(q) = p.pair {
                if ids.contains(&q) {
                    p.pair = None;
                }
            }
        }
        true
    }

    /// Make the surface cross profile `i` with a sharp edge (true) or smoothly (false); a mirrored partner follows. The two end
    /// profiles have nothing to be sharp against, so they are left alone.
    pub fn set_crease(&mut self, i: usize, v: bool) {
        let n = self.profiles.len();
        if i == 0 || i + 1 >= n {
            return;
        }
        self.profiles[i].crease = v;
        if let Some(j) = self.locked_partner(i) {
            if j != 0 && j + 1 < n {
                self.profiles[j].crease = v;
            }
        }
    }

    pub fn set_sym(&mut self, i: usize, v: bool) {
        if i >= self.profiles.len() {
            return;
        }
        self.profiles[i].sym = v;
        if let Some(j) = self.partner(i) {
            self.profiles[j].sym = v;
        }
        if v {
            // re-lock: partner takes this profile's shape and mirrored position
            let x = self.profiles[i].x;
            if let Some(j) = self.partner(i) {
                if !self.profiles[j].fixed {
                    let px = self.clamp_x(j, -x);
                    self.profiles[j].x = px;
                }
            }
            self.snap();
            self.sync_from(i);
        }
    }

    // ---- rail ops
    pub fn rail_partner(&self, k: usize) -> Option<usize> {
        let x = self.rail[k][0];
        if x.abs() < 1.0 {
            return None;
        }
        self.rail.iter().enumerate().position(|(j, r)| j != k && (r[0] + x).abs() < 1.0)
    }

    fn rail_fixed_x(&self, k: usize) -> bool {
        k == 0 || k + 1 == self.rail.len() || self.rail[k][0].abs() < 1.0
    }

    pub fn move_rail(&mut self, k: usize, x: f64, y: f64, z: f64) {
        if k >= self.rail.len() {
            return;
        }
        let partner = if self.rail_sym { self.rail_partner(k) } else { None };
        let nx = if self.rail_fixed_x(k) {
            self.rail[k][0]
        } else {
            let lo = if k > 0 { self.rail[k - 1][0] + 5.0 } else { -1e9 };
            let hi = if k + 1 < self.rail.len() { self.rail[k + 1][0] - 5.0 } else { 1e9 };
            x.max(lo).min(hi.max(lo))
        };
        self.rail[k] = [nx, y, z];
        if let Some(j) = partner {
            if !self.rail_fixed_x(j) {
                let lo = if j > 0 { self.rail[j - 1][0] + 5.0 } else { -1e9 };
                let hi = if j + 1 < self.rail.len() { self.rail[j + 1][0] - 5.0 } else { 1e9 };
                let px = (-nx).max(lo).min(hi.max(lo));
                self.rail[j] = [px, y, z];
            } else {
                self.rail[j][1] = y;
                self.rail[j][2] = z;
            }
        }
        self.snap();
    }

    pub fn rail_add(&mut self, k: usize) -> bool {
        if k + 1 >= self.rail.len() {
            return false;
        }
        let xm = (self.rail[k][0] + self.rail[k + 1][0]) / 2.0;
        let (y, z) = self.rail_at(xm);
        let r2 = if self.rail2.is_empty() { (0.0, 0.0) } else { self.rail2_at(xm) };
        let mut ins = vec![(k + 1, [xm, y, z])];
        if self.rail_sym {
            let mi = |j: usize| -> Option<usize> {
                if self.rail[j][0].abs() < 1.0 { Some(j) } else { self.rail_partner(j) }
            };
            if let (Some(a), Some(b)) = (mi(k), mi(k + 1)) {
                if b + 1 == a && !(b == k && a == k + 1) {
                    ins.push((b + 1, [-xm, y, z]));
                }
            }
        }
        ins.sort_by(|a, b| b.0.cmp(&a.0));
        for (at, p) in ins {
            self.rail.insert(at, p);
            if !self.rail2.is_empty() {
                self.rail2.insert(at, [r2.0, r2.1]);
            }
        }
        true
    }

    pub fn rail_remove(&mut self, k: usize) -> bool {
        if k >= self.rail.len() || self.rail_fixed_x(k) {
            return false;
        }
        let mut kill = vec![k];
        if self.rail_sym {
            if let Some(j) = self.rail_partner(k) {
                if !self.rail_fixed_x(j) {
                    kill.push(j);
                }
            }
        }
        if self.rail.len() - kill.len() < 3 {
            return false;
        }
        kill.sort();
        kill.reverse();
        for at in kill {
            self.rail.remove(at);
            if !self.rail2.is_empty() {
                self.rail2.remove(at);
            }
        }
        self.snap();
        true
    }

    // ---- loft
    fn tangents(&self, i: usize, p: &[Vec<[f64; 2]>], xs: &[f64]) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
        let n = xs.len();
        let np = p[i].len();
        let diff = |a: usize, b: usize| -> Vec<[f64; 2]> {
            let h = xs[b] - xs[a];
            (0..np).map(|u| [(p[b][u][0] - p[a][u][0]) / h, (p[b][u][1] - p[a][u][1]) / h]).collect()
        };
        if i == 0 {
            let m = diff(0, 1);
            return (m.clone(), m);
        }
        if i == n - 1 {
            let m = diff(n - 2, n - 1);
            return (m.clone(), m);
        }
        if self.profiles[i].crease {
            return (diff(i - 1, i), diff(i, i + 1));
        }
        let m = diff(i - 1, i + 1);
        (m.clone(), m)
    }

    /// Grid of surface points: grid[row][col]; rows run along +x, columns from windshield anchor to front tip.
    pub fn grid(&self) -> Vec<Vec<[f64; 3]>> {
        let n = self.profiles.len();
        let xs: Vec<f64> = self.profiles.iter().map(|p| p.x).collect();
        let curves: Vec<Vec<[f64; 2]>> = self.profiles.iter().map(|p| p.curve()).collect();
        let tans: Vec<(Vec<[f64; 2]>, Vec<[f64; 2]>)> = (0..n).map(|i| self.tangents(i, &curves, &xs)).collect();
        let np = curves[0].len();
        let mut rows: Vec<Vec<[f64; 3]>> = Vec::new();
        for s in 0..n - 1 {
            let h = xs[s + 1] - xs[s];
            let (p0, p1) = (&curves[s], &curves[s + 1]);
            let m0 = &tans[s].1;
            let m1 = &tans[s + 1].0;
            let r0 = if s == 0 { 0 } else { 1 };
            for r in r0..=M {
                let t = r as f64 / M as f64;
                let (t2, t3) = (t * t, t * t * t);
                let (h00, h10, h01, h11) = (2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2);
                let x = xs[s] + t * h;
                let mut row = Vec::with_capacity(np);
                for u in 0..np {
                    let y = h00 * p0[u][0] + h10 * h * m0[u][0] + h01 * p1[u][0] + h11 * h * m1[u][0];
                    let z = h00 * p0[u][1] + h10 * h * m0[u][1] + h01 * p1[u][1] + h11 * h * m1[u][1];
                    row.push([x, y, z]);
                }
                rows.push(row);
            }
        }
        rows
    }

    /// Surface normals (pointing up/outward) for the grid.
    pub fn grid_normals(g: &[Vec<[f64; 3]>]) -> Vec<Vec<[f64; 3]>> {
        let nr = g.len();
        let nc = g[0].len();
        let mut out = vec![vec![[0.0, 0.0, 1.0]; nc]; nr];
        for r in 0..nr {
            for c in 0..nc {
                let r0 = if r > 0 { r - 1 } else { r };
                let r1 = if r + 1 < nr { r + 1 } else { r };
                let c0 = if c > 0 { c - 1 } else { c };
                let c1 = if c + 1 < nc { c + 1 } else { c };
                let dr = sub(g[r1][c], g[r0][c]);
                let dc = sub(g[r][c1], g[r][c0]);
                out[r][c] = norm(cross(dc, dr));
            }
        }
        out
    }

    /// Skin only (thickness 0) or a closed solid shell (thickness > 0 inward from the top surface).
    pub fn mesh(&self, thickness: f64) -> Mesh {
        if thickness > 0.0 && (self.under.is_some() || self.end_bevel > 0.0) {
            self.mesh_trim(self.under.as_deref(), thickness)
        } else {
            self.mesh_off(0.0, thickness)
        }
    }

    /// Length of the outer-skin part of an index buffer built by `mesh(thickness)` (the rest is inner face and walls).
    pub fn skin_idx_len(&self, thickness: f64) -> usize {
        let g = self.grid();
        let nc = g[0].len();
        let mut nr = g.len();
        if thickness > 0.0 {
            let xs: Vec<f64> = g.iter().map(|r| r[0][0]).collect();
            if let Some(bp) = bevel_plan(&xs, self.end_bevel) {
                nr = 2 * bevel_profile(self.end_round).len() + (bp.k1 - bp.k0 + 1);
            }
        }
        (nr - 1) * (nc - 1) * 6
    }

    /// What the editor draws for the active piece: the plain skin, plus (for the top trim) side walls that drop to
    /// the top pad's surface so the skin does not stop short of it. Skin vertices and triangles come first, unchanged.
    pub fn mesh_view(&self, thickness: f64) -> Mesh {
        let mut m = self.mesh_off(0.0, 0.0);
        let Some(u) = self.under.as_deref() else { return m };
        let g = self.grid();
        let (nr, nc) = (g.len(), g[0].len());
        let gap = self.under_gap.max(0.0);
        let mut ring: Vec<(usize, usize)> = Vec::new();
        for c in 0..nc { ring.push((0, c)); }
        for r in 1..nr { ring.push((r, nc - 1)); }
        for c in (0..nc - 1).rev() { ring.push((nr - 1, c)); }
        for r in (1..nr - 1).rev() { ring.push((r, 0)); }
        let top: Vec<[f64; 3]> = ring.iter().map(|&(r, c)| g[r][c]).collect();
        let bot: Vec<[f64; 3]> = top
            .iter()
            .map(|p| match u.z_at(p[0], p[1]) { Some(pz) if pz + gap < p[2] => [p[0], p[1], (pz + gap).max(p[2] - thickness.max(0.5))], _ => *p })
            .collect();
        let Mesh { pos, nrm, idx } = &mut m;
        push_wall(pos, nrm, idx, &top, &bot);
        m
    }

    /// Top trim shell: the underside follows the pad's outer surface (plus the gap); the top is raised where the pad
    /// would leave it thinner than `thickness`. Edges stay straight up and down. Where no pad is underneath it falls
    /// back to the ordinary inward shell.
    pub fn mesh_trim(&self, u: Option<&Under>, thickness: f64) -> Mesh {
        let mut g = self.grid();
        let nr = g.len();
        let nc = g[0].len();
        // The turn at the nose (where the front curls under and back) is given a minimum radius: a corner tighter than the
        // shell is thick cannot be offset inward without folding over itself, and a rounder edge prints better anyway.
        if nr > 2 && u.is_some() {
            round_nose(&mut g, NOSE_RADIUS * thickness);
        }
        // never exactly zero: a trim that sits dead on the pad shows speckle where the two faces fight in the viewer
        let gap = self.under_gap.max(0.1);
        let inner_z: Vec<Vec<Option<f64>>> = vec![vec![None; nc]; nr];
        // how far the inside is pulled back along y where the underside is thinning off the pad (matches the plain offset
        // that takes over at the nose, so there is no jump)
        let inner_dy: Vec<Vec<f64>> = vec![vec![0.0; nc]; nr];
        // Only the upward-facing run rides on the pad; a trim that wraps down the front and hook keeps a plain offset.
        // Where the trim runs out past the pad (no pad under it, or the pad falls away more than TRIM_OVERHANG below it) the
        // underside stops following the pad and the trim carries on as a plain shell of the set thickness (never thicker), so it
        // hangs over toward the front as a ledge rather than a tall straight skirt down to the pad.
        let flat = Self::grid_normals(&g);
        // pad height under a point; a point right on the pad's rim may sit a hair outside it, so look a little inward too
        let pad_z0 = |x: f64, y: f64| u.and_then(|u| u.z_at(x, y).or_else(|| u.z_at(x, y - 0.5)).or_else(|| u.z_at(x, y - 2.0)));
        let gg = g.clone();
        let pad_z = |r: usize, c: usize| -> Option<f64> {
            // right on the pad's back rim the exact height can be missing on some rows and present on the next, which made the
            // inside edge alternate row by row; carry the pad's own slope out to the rim instead
            if c < 3 && u.is_some() {
                let (x, y) = (gg[r][c][0], gg[r][c][1]);
                if let Some(z) = u.and_then(|u| u.z_at(x, y)) { return Some(z); }
                if let (Some(a), Some(b)) = (u.and_then(|u| u.z_at(x, y - 6.0)), u.and_then(|u| u.z_at(x, y - 12.0))) { return Some(2.0 * a - b); }
            }
            // the first few points are right on the pad's rim: borrow the next point's pad height if this one misses
            (c..(c + 4).min(nc)).find_map(|k| pad_z0(gg[r][k][0], gg[r][k][1]).map(|z| (k, z))).filter(|&(k, _)| k == c || c < 3).map(|(_, z)| z)
        };
        // distance along each row (mm, measured along the face) to every point
        let _sdist: Vec<Vec<f64>> = (0..nr)
            .map(|r| {
                let mut acc = vec![0.0f64; nc];
                for c in 1..nc {
                    let (a, b) = (g[r][c - 1], g[r][c]);
                    acc[c] = acc[c - 1] + ((a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                }
                acc
            })
            .collect();
        // The inside of the shell is the plain offset of the top face by the thickness. Where the pad is higher than that,
        // the inside rides on the pad (floor pass below); where the pad falls away the lip is simply the shell thickness,
        // so each point depends only on its own spot and nothing can flip from one row to the next.
        let nn0 = {
            // the plain offset of the nose and hook follows the surface normal; average it over neighboring rows so a lumpy
            // front edge does not turn into spikes on the inside of the lip
            let raw = Self::grid_normals(&g);
            let mut sm = raw.clone();
            for r in 0..nr {
                let (a, b) = (r.saturating_sub(4), (r + 4).min(nr - 1));
                for c in 0..nc {
                    let mut acc = [0.0f64; 3];
                    for k in a..=b {
                        let w = 5.0 - (k as f64 - r as f64).abs();
                        for d in 0..3 { acc[d] += raw[k][c][d] * w; }
                    }
                    let l = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt().max(1e-9);
                    sm[r][c] = [acc[0] / l, acc[1] / l, acc[2] / l];
                }
            }
            sm
        };
        let mut ig: Vec<Vec<[f64; 3]>> = Vec::with_capacity(nr);
        for r in 0..nr {
            let mut row = Vec::with_capacity(nc);
            for c in 0..nc {
                let p = g[r][c];
                let n = nn0[r][c];
                let mut q = match inner_z[r][c] {
                    Some(z) => [p[0], p[1] - inner_dy[r][c], z],
                    None => {
                        // on a gently sloping top the inside is straight below the outside (so it can sit exactly on the pad);
                        // it eases over to the offset along the surface normal as the face turns steep
                        let fz = flat[r][c][2];
                        let sm = ((0.6 - fz) / 0.3).max(0.0).min(1.0);
                        let sm = sm * sm * (3.0 - 2.0 * sm);
                        let vz = p[2] - thickness;
                        [
                            if r == 0 || r + 1 == nr { p[0] } else { p[0] - n[0] * thickness * sm },
                            p[1] - n[1] * thickness * sm,
                            vz + (p[2] - n[2] * thickness - vz) * sm,
                        ]
                    }
                };
                // on an upward-facing part the inside never comes up through the top face
                if n[2] > 0.1 && q[2] > p[2] - 0.5 { q[2] = p[2] - 0.5; }
                row.push(q);
            }
            ig.push(row);
        }
        // A steep wall that turns down in front of the pad edge is offset inward by the shell thickness, which can put its
        // inside face inside the pad. Pull such an inside point forward by how deep it is in the pad (so the pull grows
        // smoothly from nothing; it never flips on or off between rows), but never more than half the thickness.
        for r in 0..nr {
            for c in 0..nc {
                let sw = { let s = ((0.6 - flat[r][c][2]) / 0.3).max(0.0).min(1.0); s * s * (3.0 - 2.0 * s) };
                if sw <= 0.0 { continue; }
                let (o, mut q) = (g[r][c], ig[r][c]);
                if let Some(z) = pad_z0(q[0], q[1]) {
                    let depth = z + gap - q[2];
                    if depth > 0.0 {
                        let pull = sw * depth.min(0.5 * thickness).min((q[1] - o[1]).max(0.0));
                        q[1] -= pull;
                        ig[r][c] = q;
                    }
                }
            }
        }
        let (g_pre, ig_pre) = (g.clone(), ig.clone());
        // Floor: wherever the pad is under the inside of the shell, the inside rides on the pad's surface (plus the gap) and
        // the top face is kept a shell thickness above it. A steep wall is never pushed up past its own outside face.
        // How "flat" a spot is changes smoothly (1 on a gentle top, 0 on a steep face), so the two rules fade into each other
        // instead of switching on and off from one row to the next.
        for r in 0..nr {
            for c in 0..nc {
                let q = ig[r][c];
                let fz = flat[r][c][2];
                let w = { let s = ((0.6 - fz) / 0.3).max(0.0).min(1.0); 1.0 - s * s * (3.0 - 2.0 * s) };
                // the pad height is read under the inside face on a steep wall and under the outside face on a gentle top; in
                // between the two readings are blended, so the height does not jump where the choice would otherwise flip
                let pz = if c < 3 { pad_z(r, c) } else {
                    let (po, pi) = (pad_z0(g[r][c][0], g[r][c][1]), pad_z0(q[0], q[1]));
                    match (po, pi) {
                        (Some(a), Some(b)) => Some(w * a + (1.0 - w) * b),
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        _ => None,
                    }
                };
                if let Some(pz) = pz {
                    let f = pz + gap;
                    if ig[r][c][2] < f {
                        let steep_v = f.min(g[r][c][2] - 0.3).max(ig[r][c][2]);
                        ig[r][c][2] = steep_v + w * (f - steep_v);
                        if g[r][c][2] < f + thickness { g[r][c][2] += w * (f + thickness - g[r][c][2]); }
                    }
                    if g[r][c][2] < f + 0.2 { g[r][c][2] += w * (f + 0.2 - g[r][c][2]); }
                }
            }
        }
        let (g_fl, ig_fl) = (g.clone(), ig.clone());
        // The floor pass starts and stops abruptly where the pad comes up under the shell; even its corrections out along x and
        // across the columns so they blend in instead of leaving ridges and kinks.
        if nr > 4 {
            for surf in 0..2 {
                let (s, pre) = if surf == 0 { (&mut g, &g_pre) } else { (&mut ig, &ig_pre) };
                let mut d: Vec<Vec<f64>> = (0..nr).map(|r| (0..nc).map(|c| s[r][c][2] - pre[r][c][2]).collect()).collect();
                let d0 = d.clone();
                for pass in 0..8 {
                    let old = d.clone();
                    for r in 0..nr {
                        for c in 0..nc {
                            let (r0, r1) = (r.saturating_sub(1), (r + 1).min(nr - 1));
                            let mut v = 0.25 * old[r0][c] + 0.5 * old[r][c] + 0.25 * old[r1][c];
                            if pass < 2 && c > 0 && c + 1 < nc { v = 0.25 * old[r][c - 1] + 0.5 * v + 0.25 * old[r][c + 1]; }
                            d[r][c] = v;
                        }
                    }
                }
                // evening out only ever adds material (the floor still holds everywhere); it softens where the correction starts
                for r in 0..nr { for c in 0..nc { if d[r][c] < d0[r][c] { d[r][c] = d0[r][c]; } } }
                for r in 0..nr {
                    // the first and last rows and the windshield-edge columns are pinned to the guides
                    if r == 0 || r + 1 == nr { continue; }
                    for c in 3..nc { s[r][c][2] = pre[r][c][2] + d[r][c]; }
                }
            }
        }
        // smoothing both faces separately can shave a little off the shell: put back at least the thickness each place had
        if nr > 4 {
            for r in 1..nr - 1 {
                for c in 3..nc {
                    if flat[r][c][2] < 0.6 { continue; }
                    let th = (g_fl[r][c][2] - ig_fl[r][c][2]).min(thickness);
                    // the last step lifts anything under the pad, so do that here first and keep the thickness above it
                    if let Some(pz) = pad_z0(ig[r][c][0], ig[r][c][1]) {
                        if ig[r][c][2] < pz + gap { ig[r][c][2] = pz + gap; }
                    }
                    if g[r][c][2] - ig[r][c][2] < th { g[r][c][2] = ig[r][c][2] + th; }
                }
            }
        }
        // The two end profiles are locked: the outer surface stays exactly on them, and the inside is the shell thickness below
        // (done above), so nothing pulls the ends down off the profile line. The end face itself runs straight down (the shell
        // extruded along x) to the pad's surface, so it meets the upper dash contour with no gap; the inside comes back up to the
        // normal underside over the next few mm.
        const END_RUN: f64 = 30.0;
        // the end plate (sides filled straight down to the pad) belongs to the mode where the front guide moves only the profile ends
        let plate = !self.nose_follows;
        if u.is_some() && nr > 2 {
            let (x0, x1) = (g[0][0][0], g[nr - 1][0][0]);
            for r in 0..nr {
                let d = (g[r][0][0] - x0).abs().min((x1 - g[r][0][0]).abs());
                if d >= thickness + END_RUN { continue; }
                let t = ((d - thickness) / END_RUN).max(0.0).min(1.0);
                let s = 1.0 - t * t * (3.0 - 2.0 * t);
                let (mut prev, mut run) = (0.0f64, 0usize);
                for c in 0..nc {
                    let q = ig[r][c];
                    let pz = if c < 3 { pad_z(r, c) } else { pad_z0(q[0], q[1]).or_else(|| pad_z0(g[r][c][0], g[r][c][1])) };
                    // only the gently sloping top comes down to the pad; over the steep nose the drop dies away within a few
                    // columns, and the lip tucked under keeps most of its shell thickness (lowering it would thin it to nothing)
                    let st = { let k = ((0.6 - flat[r][c][2]) / 0.3).max(0.0).min(1.0); k * k * (3.0 - 2.0 * k) };
                    let amt = if st < 0.5 {
                        // where the front lifts away from the pad the inside keeps its shell-thickness offset (the same profile as
                        // the rest of the lip); it only eases down to the pad once the outside is within a shell thickness or two of it
                        let a = match pz { Some(pz) if q[2] > pz + gap => (q[2] - (pz + gap)) * s * (1.0 - st) * (1.0 - lift_ease(g[r][c][2] - (pz + gap), thickness) * plate_ramp(d, thickness, plate)), _ => 0.0 };
                        prev = a;
                        run = 0;
                        a
                    } else {
                        run += 1;
                        prev * (1.0 - run as f64 / 5.0).max(0.0)
                    };
                    let mut z = q[2] - amt;
                    if q[2] > g[r][c][2] { z = z.max(g[r][c][2] + 0.5 * (q[2] - g[r][c][2])); }
                    ig[r][c][2] = z;
                }
            }
        }
        // Where the front guide runs below the top pad between two profiles, the nose dips into the pad, and the last-step cut
        // (which only fixes small overlaps) leaves steps and holes there. Lift every point by how far it is under the pad,
        // evened out along x and across the columns, so the lip rides on the pad's surface as one smooth surface at any height.
        if u.is_some() && nr > 6 {
            let floor = self.under_gap.max(0.0);
            let mut need = vec![vec![0.0f64; nc]; nr];
            for r in 0..nr {
                for c in 3..nc {
                    if let Some(pz) = pad_z0(g[r][c][0], g[r][c][1]) {
                        need[r][c] = (pz + floor - g[r][c][2]).max(0.0).min(30.0);
                    }
                }
            }
            for _ in 0..6 {
                let old = need.clone();
                for r in 1..nr - 1 {
                    for c in 3..nc {
                        let (c0, c1) = (c.saturating_sub(1).max(3), (c + 1).min(nc - 1));
                        let v = 0.25 * old[r - 1][c] + 0.5 * old[r][c] + 0.25 * old[r + 1][c];
                        need[r][c] = 0.25 * old[r][c0] * 0.0 + v * 0.5 + 0.25 * (old[r][c0] + old[r][c1]) * 0.5 + 0.25 * v;
                    }
                }
            }
            for r in 0..nr {
                for c in 3..nc {
                    if need[r][c] < 1e-6 { continue; }
                    g[r][c][2] += need[r][c];
                    ig[r][c][2] += need[r][c];
                }
            }
        }
        // Fair the underside where it leaves the pad: right beside the contact line the inside is the larger of "pad + gap" and
        // "6 mm under the top", and the two meeting leaves a ripple. Even it out along and across (never below the pad, never
        // closer than the shell thickness to the top face).
        if u.is_some() && nr > 4 {
            let gapz = self.under_gap.max(0.1);
            let mut lo = vec![vec![f64::NAN; nc]; nr];
            let (xa, xb) = (g[0][0][0], g[nr - 1][0][0]);
            for r in 0..nr {
                // the end run has its own easing; leave it alone
                if (g[r][0][0] - xa).abs().min((xb - g[r][0][0]).abs()) < thickness + END_RUN + 6.0 { continue; }
                for c in 4..nc {
                    if let Some(pz) = pad_z0(ig[r][c][0], ig[r][c][1]) {
                        let d = ig[r][c][2] - (pz + gapz);
                        if d > 0.02 && d < 4.0 { lo[r][c] = pz + gapz; }
                    }
                }
            }
            for _ in 0..6 {
                let old: Vec<Vec<f64>> = ig.iter().map(|row| row.iter().map(|q| q[2]).collect()).collect();
                for r in 1..nr - 1 {
                    for c in 4..nc - 1 {
                        if lo[r][c].is_nan() { continue; }
                        let avg = 0.25 * (old[r - 1][c] + old[r + 1][c] + old[r][c - 1] + old[r][c + 1]);
                        let mut v = (old[r][c] + 0.5 * (avg - old[r][c])).max(lo[r][c]);
                        let hi = g[r][c][2] - thickness;
                        if v > hi && v > old[r][c] { v = old[r][c]; }
                        ig[r][c][2] = v;
                    }
                }
            }
        }
        // The nose (the turn at the front, and the lip curling back from it) is a plain offset of the outside by the shell
        // thickness, done in each cross-section. The outside has been rounded to at least that radius, so the offset cannot fold
        // over itself, and the inside runs parallel to the outside the whole way round.
        if nr > 2 {
            let mut cusps: Vec<Option<usize>> = vec![None; nr];
            let mut clamped: Vec<bool> = vec![false; nr];
            for r in 0..nr {
                let turn = |c: usize| -> f64 {
                    let (a, b) = ([g[r][c][1] - g[r][c - 1][1], g[r][c][2] - g[r][c - 1][2]], [g[r][c + 1][1] - g[r][c][1], g[r][c + 1][2] - g[r][c][2]]);
                    (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]).abs()
                };
                let corner = (16..nc.saturating_sub(3)).max_by(|&x, &y| turn(x).partial_cmp(&turn(y)).unwrap()).unwrap_or(nc);
                if corner >= nc { continue; }
                let s0 = corner.saturating_sub(10).max(4);
                let off = |c: usize| -> [f64; 2] {
                    let (a, b) = (c.saturating_sub(1), (c + 1).min(nc - 1));
                    let (ty, tz) = (g[r][b][1] - g[r][a][1], g[r][b][2] - g[r][a][2]);
                    let l = (ty * ty + tz * tz).sqrt().max(1e-9);
                    // outward normal (up on the top, down on the lip curling back); the inside is the shell thickness the other way
                    let (ny, nz) = (tz / l, -ty / l);
                    [g[r][c][1] - ny * thickness, g[r][c][2] - nz * thickness]
                };
                let blend = 4usize;
                let mut first_clamp: Option<usize> = None;
                for c in s0.saturating_sub(blend)..nc {
                    let w = if c >= s0 { 1.0 } else { let k = (c + blend - s0) as f64 / blend as f64; k * k * (3.0 - 2.0 * k) };
                    let o = off(c);
                    ig[r][c][1] += (o[0] - ig[r][c][1]) * w;
                    ig[r][c][2] += (o[1] - ig[r][c][2]) * w;
                    // never below the pad; where the lip comes down onto it the shell tapers to the pad instead of being cut
                    if let Some(pz) = pad_z0(ig[r][c][0], ig[r][c][1]) {
                        let lo = pz + gap;
                        if ig[r][c][2] < lo && (c >= corner || flat[r][c][2] < 0.6) { ig[r][c][2] = lo; if first_clamp.is_none() && flat[r][c][2] < 0.6 { first_clamp = Some(c); } }
                    }
                }
                // At the ends the inside runs straight down to the pad, so the end face reaches the pad contour right round to the
                // front; the offset above would otherwise lift it off the pad. Only the top-facing columns, only where the pad is below.
                {
                    let (x, x0, x1) = (g[r][0][0], g[0][0][0], g[nr - 1][0][0]);
                    let d = (x - x0).abs().min((x1 - x).abs());
                    let t = ((d - thickness) / 30.0).clamp(0.0, 1.0);
                    let sw = 1.0 - t * t * (3.0 - 2.0 * t);
                    if sw > 0.0 {
                        for c in s0..nc {
                            if flat[r][c][2] < 0.6 { continue; }
                            if let Some(pz) = pad_z0(ig[r][c][0], ig[r][c][1]) {
                                let lo = pz + gap;
                                if ig[r][c][2] > lo { ig[r][c][2] -= (ig[r][c][2] - lo) * sw * (1.0 - lift_ease(g[r][c][2] - lo, thickness) * plate_ramp(d, thickness, plate)); }
                            }
                        }
                    }
                }
                // Where the lip hooks round so tightly that the full-thickness inside cannot follow it, the shell is closed off with a
                // straight line: from the last inside point that still runs along the outside to the tip of the lip.
                let mut cusp = None;
                for c in (s0 + 1)..nc {
                    let (oy, oz) = (g[r][c][1] - g[r][c - 1][1], g[r][c][2] - g[r][c - 1][2]);
                    let (iy, iz) = (ig[r][c][1] - ig[r][c - 1][1], ig[r][c][2] - ig[r][c - 1][2]);
                    if oy * iy + oz * iz < 0.0 { cusp = Some(c - 1); break; }
                }
                if let Some(fc) = first_clamp { cusp = Some(cusp.map_or(fc.saturating_sub(1), |k| k.min(fc.saturating_sub(1)))); }
                cusps[r] = cusp;
                clamped[r] = first_clamp.is_some();
            }
            // the hook's closing line should start at about the same column on neighboring rows, or the insides twist
            // across one another: every row starts it no later than its neighbors do
            let sm: Vec<Option<usize>> = (0..nr).map(|r| {
                let mut k: Option<usize> = cusps[r];
                for q in r.saturating_sub(2)..(r + 3).min(nr) {
                    if let Some(kq) = cusps[q] { k = Some(k.map_or(kq, |x| x.min(kq))); }
                }
                k
            }).collect();
            for r in 0..nr {
                let mut cusp = sm[r];
                // The end plate: for the first shell thickness at each end the space under the lip is filled straight down to the pad,
                // from where the underside leaves the pad out to the front. The fill closes with a vertical face under the tip.
                if u.is_some() && plate {
                    let (x, x0, x1) = (g[r][0][0], g[0][0][0], g[nr - 1][0][0]);
                    if (x - x0).abs().min((x1 - x).abs()) <= thickness + 1e-6 {
                        if let Some(c1) = (4..nc).find(|&c| flat[r][c][2] < 0.45) {
                            let kk = c1 - 1;
                            let t = g[r][nc - 1];
                            if let Some(pz) = pad_z0(x, t[1]).or_else(|| pad_z0(x, t[1] + 2.0)).or_else(|| pad_z0(x, t[1] + 6.0)).filter(|&pz| t[2] - (pz + gap) > thickness + 1.0) {
                                for c in 3..kk {
                                    if let Some(pc) = pad_z0(g[r][c][0], g[r][c][1]) {
                                        let z = pc + gap;
                                        if g[r][c][2] - z > 0.2 && ig[r][c][2] > z { ig[r][c] = [g[r][c][0], g[r][c][1], z]; }
                                    }
                                }
                                ig[r][kk] = [x, t[1], (pz + gap).min(t[2] - 0.5)];
                                cusp = Some(kk);
                            }
                        }
                    }
                }
                if let Some(k) = cusp {
                    let a = ig[r][k];
                    // the closing line ends a hair inside the tip so the end of the lip is a thin edge, not a zero-thickness one
                    let b = {
                        let t = g[r][nc - 1];
                        let (dy, dz) = (a[1] - t[1], a[2] - t[2]);
                        let l = (dy * dy + dz * dz).sqrt().max(1e-9);
                        let e = 0.3_f64.min(l * 0.5);
                        [t[0], t[1] + dy / l * e, t[2] + dz / l * e]
                    };
                    // A tight corner folds the inside only locally. When the lip carries on beyond it (a long overhang pulled well out
                    // past the pad) the plain offset is fine again a few columns on: close the corner with the line only up to there
                    // and keep the full shell thickness along the rest of the lip, instead of tapering the whole lip to a wedge.
                    let resume = {
                        let ok = |c: usize| -> bool {
                            let (oy, oz) = (g[r][c][1] - g[r][c - 1][1], g[r][c][2] - g[r][c - 1][2]);
                            let (iy, iz) = (ig[r][c][1] - ig[r][c - 1][1], ig[r][c][2] - ig[r][c - 1][2]);
                            let (dy, dz) = (g[r][c][1] - ig[r][c][1], g[r][c][2] - ig[r][c][2]);
                            oy * iy + oz * iz > 0.0 && (dy * dy + dz * dz).sqrt() >= 0.8 * thickness
                        };
                        let m = (k + 2..nc - 1).rev().take_while(|&c| ok(c)).last();
                        m.filter(|&m| m + 2 < nc)
                    };
                    if let Some(m) = resume {
                        let b = ig[r][m];
                        let n = (m - k) as f64;
                        for c in k + 1..m {
                            let t = (c - k) as f64 / n;
                            ig[r][c] = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
                        }
                        continue;
                    }
                    let n = (nc - 1 - k) as f64;
                    for c in k + 1..nc {
                        let t = (c - k) as f64 / n;
                        ig[r][c] = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
                        if let Some(pz) = pad_z0(ig[r][c][0], ig[r][c][1]) {
                            let lo = pz + gap;
                            if ig[r][c][2] < lo && g[r][c][2] - lo > 0.1 { ig[r][c][2] = lo; }
                        }
                    }
                }
            }
        }
        // chamfer the top edge of each end face: add a row at the end plane lowered by the bevel, and slide the
        // first real row inward along the surface
        let xs: Vec<f64> = g.iter().map(|r| r[0][0]).collect();
        if let Some(bp) = bevel_plan(&xs, self.end_bevel) {
            let lerp = |a: [f64; 3], b: [f64; 3], t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
            let prof = bevel_profile(self.end_round);
            // surface and inner face at distance x between the loft rows
            let at = |g: &Vec<Vec<[f64; 3]>>, x: f64| -> Vec<[f64; 3]> {
                let k = xs.partition_point(|&v| v <= x).max(1).min(nr - 1);
                let t = ((x - xs[k - 1]) / (xs[k] - xs[k - 1]).max(1e-9)).max(0.0).min(1.0);
                (0..nc).map(|c| lerp(g[k - 1][c], g[k][c], t)).collect()
            };
            let mut ng: Vec<Vec<[f64; 3]>> = Vec::new();
            let mut ni: Vec<Vec<[f64; 3]>> = Vec::new();
            let end_rows = |from_start: bool, ng: &mut Vec<Vec<[f64; 3]>>, ni: &mut Vec<Vec<[f64; 3]>>| {
                let mut rows: Vec<(Vec<[f64; 3]>, Vec<[f64; 3]>)> = Vec::new();
                for &(fx, drop) in &prof {
                    let x = if from_start { xs[0] + bp.d * fx } else { xs[nr - 1] - bp.d * fx };
                    let (mut o, i2) = if fx == 0.0 {
                        let e = if from_start { 0 } else { nr - 1 };
                        (g[e].clone(), ig[e].clone())
                    } else {
                        (at(&g, x), at(&ig, x))
                    };
                    let e = if from_start { 0 } else { nr - 1 };
                    for c in 0..nc {
                        // The edge is a true corner cut, as seen from the front: it starts `end_down` mm down the end face and comes
                        // back to the top surface `end_bevel` mm in along the top. Only the outside is cut; the inside (and the end
                        // face under the cut, down to the pad or the underside) stays where it is. A thin end face is never cut
                        // all the way through: half a millimeter is always left.
                        let h_end = g[e][c][2] - ig[e][c][2];
                        let a = self.end_down.max(0.0).min((h_end - 0.5).max(0.0));
                        o[c][2] -= a * drop;
                    }
                    rows.push((o, i2));
                }
                if !from_start { rows.reverse(); }
                for (o, i2) in rows { ng.push(o); ni.push(i2); }
            };
            end_rows(true, &mut ng, &mut ni);
            for r in bp.k0..=bp.k1 {
                ng.push(g[r].clone());
                ni.push(ig[r].clone());
            }
            end_rows(false, &mut ng, &mut ni);
            g = ng;
            ig = ni;
        }
        let nr = g.len();
        let nn = Self::grid_normals(&g);
        let mut pos: Vec<f32> = Vec::with_capacity(nr * nc * 6);
        let mut nrm: Vec<f32> = Vec::new();
        for r in 0..nr {
            for c in 0..nc {
                let p = g[r][c];
                pos.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32]);
                let n = nn[r][c];
                nrm.extend_from_slice(&[n[0] as f32, n[1] as f32, n[2] as f32]);
            }
        }
        let vcount = nr * nc;
        for r in 0..nr {
            for c in 0..nc {
                let q = ig[r][c];
                let n = nn[r][c];
                pos.extend_from_slice(&[q[0] as f32, q[1] as f32, q[2] as f32]);
                nrm.extend_from_slice(&[-n[0] as f32, -n[1] as f32, -n[2] as f32]);
            }
        }
        let id = |r: usize, c: usize| (r * nc + c) as u32;
        let off = vcount as u32;
        let mut idx: Vec<u32> = Vec::new();
        // same layout as the plain shell: every outer triangle first, then the inner ones, then the edge walls
        for r in 0..nr - 1 {
            for c in 0..nc - 1 {
                let (a, b, cc, d) = (id(r, c), id(r, c + 1), id(r + 1, c + 1), id(r + 1, c));
                idx.extend_from_slice(&[a, b, cc, a, cc, d]);
            }
        }
        for r in 0..nr - 1 {
            for c in 0..nc - 1 {
                let (a, b, cc, d) = (id(r, c) + off, id(r, c + 1) + off, id(r + 1, c + 1) + off, id(r + 1, c) + off);
                idx.extend_from_slice(&[a, cc, b, a, d, cc]);
            }
        }
        let mut ring: Vec<(usize, usize)> = Vec::new();
        for c in 0..nc { ring.push((0, c)); }
        for r in 1..nr { ring.push((r, nc - 1)); }
        for c in (0..nc - 1).rev() { ring.push((nr - 1, c)); }
        for r in (1..nr - 1).rev() { ring.push((r, 0)); }
        let top: Vec<[f64; 3]> = ring.iter().map(|&(r, c)| g[r][c]).collect();
        let bot: Vec<[f64; 3]> = ring.iter().map(|&(r, c)| ig[r][c]).collect();
        let wall0 = pos.len() / 3;
        push_wall(&mut pos, &mut nrm, &mut idx, &top, &bot);
        if signed_volume(&pos, &idx) < 0.0 {
            for t in idx.chunks_mut(3) { t.swap(1, 2); }
            for v in nrm[wall0 * 3..].iter_mut() { *v = -*v; }
        }
        // Last step: cut the shell with the top surface of the pad. Anything of the trim that ended up below the pad's top
        // surface is brought up onto it, so wherever the two meet the trim's underside is the pad's surface.
        if let Some(u) = u {
            let floor = self.under_gap.max(0.0);
            let before = pos.clone();
            for v in pos.chunks_mut(3) {
                if let Some(pz) = u.z_at(v[0] as f64, v[1] as f64) {
                    let f = (pz + floor) as f32;
                    // small corrections only: where a steep nose wraps round the pad's edge it overlaps the pad by more
                    if v[2] < f && f - v[2] < 3.0 { v[2] = f; }
                }
            }
            // The cut moves the outside and the inside by different amounts; never let it squash the shell flat or turn it
            // through itself: each inside point keeps (at least) a small distance on its own side of the outside point.
            const MIN: f32 = 0.2;
            let nv_all = pos.len() / 3;
            let mut fix = |o: usize, i: usize| {
                let (oz0, iz0) = (before[o * 3 + 2], before[i * 3 + 2]);
                let (oz, iz) = (pos[o * 3 + 2], pos[i * 3 + 2]);
                if oz0 - iz0 > MIN {
                    if oz - iz < MIN { pos[i * 3 + 2] = oz - MIN; }
                } else if iz0 - oz0 > MIN && iz - oz < MIN {
                    pos[i * 3 + 2] = oz + MIN;
                }
            };
            for k in 0..vcount { fix(k, vcount + k); }
            let mut w = wall0;
            while w + 3 < nv_all {
                fix(w, w + 2);
                fix(w + 1, w + 3);
                w += 4;
            }
        }
        Mesh { pos, nrm, idx }
    }

    /// The mesh to draw or export: for the top trim, after the cut with the pad, every loose piece other than the main
    /// one is thrown away. Returns the mesh and the length of its outer-skin part (the first `skin` indices are skin).
    pub fn mesh_final(&self, thickness: f64) -> (Mesh, usize) {
        let mut m = self.mesh(thickness);
        let skin = self.skin_idx_len(thickness);
        if thickness > 0.0 && self.under.is_some() {
            let k = keep_main_piece(&mut m, skin);
            return (m, k);
        }
        (m, skin)
    }

    /// Which vertices of `mesh(thickness)` sit where the shell is thinner than the set thickness by more than `THIN_TOL` mm
    /// (outside and inside vertex of a pair are both marked). Empty when the shell is not solid.
    pub fn thin_mask(&self, m: &Mesh, thickness: f64) -> Vec<bool> {
        let nv = m.pos.len() / 3;
        let mut out = vec![false; nv];
        if thickness <= 0.0 || self.under.is_none() { return out; }
        let nc = self.grid()[0].len();
        let skin = self.skin_idx_len(thickness);
        let v = (skin / (6 * (nc - 1)) + 1) * nc;
        if 2 * v > nv { return out; }
        for k in 0..v {
            let d = ((0..3).map(|a| (m.pos[k * 3 + a] - m.pos[(v + k) * 3 + a]) as f64).map(|x| x * x).sum::<f64>()).sqrt();
            if d < thickness - THIN_TOL { out[k] = true; out[v + k] = true; }
        }
        out
    }

    /// Closed solid under the skin: the skin from the windshield edge to its front-most point, dropped straight down to `zb`.
    /// Used to trim a wall-and-floor liner so it stays beneath the surface.
    pub fn under_skin(&self, zb: f64) -> Mesh {
        let g = self.grid();
        let nr = g.len();
        let nc0 = g[0].len();
        let mid = &g[nr / 2];
        let mut k = 0usize;
        for c in 0..nc0 {
            if mid[c][1] < mid[k][1] {
                k = c;
            }
        }
        let nc = k + 1;
        let mut pos: Vec<f32> = Vec::new();
        for r in 0..nr {
            for c in 0..nc {
                let p = g[r][c];
                pos.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32]);
            }
        }
        for r in 0..nr {
            for c in 0..nc {
                let p = g[r][c];
                pos.extend_from_slice(&[p[0] as f32, p[1] as f32, zb as f32]);
            }
        }
        let off = (nr * nc) as u32;
        let id = |r: usize, c: usize| (r * nc + c) as u32;
        let mut idx: Vec<u32> = Vec::new();
        for r in 0..nr - 1 {
            for c in 0..nc - 1 {
                let (a, b, cc, d) = (id(r, c), id(r, c + 1), id(r + 1, c + 1), id(r + 1, c));
                idx.extend_from_slice(&[a, b, cc, a, cc, d]);
                idx.extend_from_slice(&[a + off, cc + off, b + off, a + off, d + off, cc + off]);
            }
        }
        let mut ring: Vec<(usize, usize)> = Vec::new();
        for c in 0..nc {
            ring.push((0, c));
        }
        for r in 1..nr {
            ring.push((r, nc - 1));
        }
        for c in (0..nc - 1).rev() {
            ring.push((nr - 1, c));
        }
        for r in (1..nr - 1).rev() {
            ring.push((r, 0));
        }
        for i in 0..ring.len() {
            let (r0, c0) = ring[i];
            let (r1, c1) = ring[(i + 1) % ring.len()];
            let (a, b) = (id(r0, c0), id(r1, c1));
            idx.extend_from_slice(&[b, a, a + off, b, a + off, b + off]);
        }
        if signed_volume(&pos, &idx) < 0.0 {
            for t in idx.chunks_mut(3) {
                t.swap(1, 2);
            }
        }
        let nrm = vec![0.0f32; pos.len()];
        Mesh { pos, nrm, idx }
    }

    /// Shell whose outer face floats `off` mm above the skin (used for the top cover).
    pub fn mesh_off(&self, off: f64, thickness: f64) -> Mesh {
        let mut g = self.grid();
        let nr = g.len();
        let nc = g[0].len();
        let nn = Self::grid_normals(&g);
        if off != 0.0 {
            for r in 0..nr {
                for c in 0..nc {
                    for a in 0..3 {
                        g[r][c][a] += nn[r][c][a] * off;
                    }
                }
            }
        }
        let vcount = nr * nc;
        let solid = thickness > 0.0;
        let mut pos: Vec<f32> = Vec::with_capacity(vcount * 3 * if solid { 2 } else { 1 });
        let mut nrm: Vec<f32> = Vec::new();
        for r in 0..nr {
            for c in 0..nc {
                let p = g[r][c];
                pos.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32]);
                let n = nn[r][c];
                nrm.extend_from_slice(&[n[0] as f32, n[1] as f32, n[2] as f32]);
            }
        }
        let mut idx: Vec<u32> = Vec::new();
        let id = |r: usize, c: usize| (r * nc + c) as u32;
        for r in 0..nr - 1 {
            for c in 0..nc - 1 {
                let (a, b, cc, d) = (id(r, c), id(r, c + 1), id(r + 1, c + 1), id(r + 1, c));
                idx.extend_from_slice(&[a, b, cc, a, cc, d]);
            }
        }
        if solid {
            for r in 0..nr {
                for c in 0..nc {
                    let p = g[r][c];
                    let n = nn[r][c];
                    let sx = if r == 0 || r + 1 == nr { 0.0 } else { n[0] * thickness }; // square (90 degree) cut at the ends
                    pos.extend_from_slice(&[
                        (p[0] - sx) as f32,
                        (p[1] - n[1] * thickness) as f32,
                        (p[2] - n[2] * thickness) as f32,
                    ]);
                    nrm.extend_from_slice(&[-n[0] as f32, -n[1] as f32, -n[2] as f32]);
                }
            }
            let off = vcount as u32;
            // inner surface, reversed winding
            for r in 0..nr - 1 {
                for c in 0..nc - 1 {
                    let (a, b, cc, d) = (id(r, c) + off, id(r, c + 1) + off, id(r + 1, c + 1) + off, id(r + 1, c) + off);
                    idx.extend_from_slice(&[a, cc, b, a, d, cc]);
                }
            }
            // boundary walls: outer ring walked in a fixed direction, inner ring below it
            let mut ring: Vec<(usize, usize)> = Vec::new();
            for c in 0..nc {
                ring.push((0, c));
            }
            for r in 1..nr {
                ring.push((r, nc - 1));
            }
            for c in (0..nc - 1).rev() {
                ring.push((nr - 1, c));
            }
            for r in (1..nr - 1).rev() {
                ring.push((r, 0));
            }
            for i in 0..ring.len() {
                let (r0, c0) = ring[i];
                let (r1, c1) = ring[(i + 1) % ring.len()];
                let (a, b) = (id(r0, c0), id(r1, c1));
                // outer faces walk this edge a->b, so the wall must walk it b->a
                idx.extend_from_slice(&[b, a, a + off, b, a + off, b + off]);
            }
            // make sure the solid is outward-facing
            if signed_volume(&pos, &idx) < 0.0 {
                for t in idx.chunks_mut(3) {
                    t.swap(1, 2);
                }
            }
        }
        Mesh { pos, nrm, idx }
    }
}


/// 0 while the outside is no more than a shell thickness above the pad, rising smoothly to 1 at three thicknesses: how far
/// the inside of a lifted front stays on its own offset instead of dropping to the pad.
/// 0 inside the end plate (the first shell thickness from each end, which is filled straight down to the pad), rising to 1
/// a millimeter or so further in. Without the plate it is always 1.
fn plate_ramp(d: f64, thickness: f64, plate: bool) -> f64 {
    if !plate { return 1.0; }
    let t = ((d - thickness) / 1.5).max(0.0).min(1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lift_ease(h_out: f64, thickness: f64) -> f64 {
    let t = ((h_out - thickness) / (2.0 * thickness.max(0.5))).max(0.0).min(1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where the end bevel goes: its inward reach `d` (capped so the two ends never meet) and the first/last original
/// rows that stay.
struct BevelPlan { d: f64, k0: usize, k1: usize }
fn bevel_plan(xs: &[f64], bv: f64) -> Option<BevelPlan> {
    let n = xs.len();
    if bv <= 0.0 || n < 4 {
        return None;
    }
    let len = xs[n - 1] - xs[0];
    let d = bv.min(len * 0.45);
    if d <= 1e-6 {
        return None;
    }
    let k0 = xs.partition_point(|&x| x <= xs[0] + d).max(1).min(n - 2);
    let k1 = (xs.partition_point(|&x| x < xs[n - 1] - d).max(1) - 1).max(k0).min(n - 2);
    Some(BevelPlan { d, k0, k1 })
}

const FILLET_SEG: usize = 8;
/// Bevel cross-section as (fraction of the inward reach, fraction of the end-face drop still to go): starts on the end
/// face and finishes on the top surface.
fn bevel_profile(round: bool) -> Vec<(f64, f64)> {
    if !round {
        return vec![(0.0, 1.0), (1.0, 0.0)];
    }
    (0..=FILLET_SEG)
        .map(|i| {
            let th = std::f64::consts::FRAC_PI_2 * i as f64 / FILLET_SEG as f64;
            (1.0 - th.cos(), 1.0 - th.sin())
        })
        .collect()
}

/// Side wall around a closed ring: top[i] over bot[i]. Each wall quad gets its own vertices and a true face normal
/// (neighboring quads share a normal only on smooth stretches), so tall walls shade as walls, not as the surface.
fn push_wall(pos: &mut Vec<f32>, nrm: &mut Vec<f32>, idx: &mut Vec<u32>, top: &[[f64; 3]], bot: &[[f64; 3]]) {
    let k = top.len();
    let face = |i: usize| -> [f64; 3] {
        let (a, b) = (i, (i + 1) % k);
        // triangle (b, a, bot[a]) as listed in idx
        norm(cross(sub(top[a], top[b]), sub(bot[a], top[b])))
    };
    let faces: Vec<[f64; 3]> = (0..k).map(face).collect();
    for i in 0..k {
        let j = (i + 1) % k;
        let pick = |own: [f64; 3], other: [f64; 3]| {
            let d = own[0] * other[0] + own[1] * other[1] + own[2] * other[2];
            if d > 0.8 { norm([own[0] + other[0], own[1] + other[1], own[2] + other[2]]) } else { own }
        };
        let n_a = pick(faces[i], faces[(i + k - 1) % k]);
        let n_b = pick(faces[i], faces[j]);
        // shading only: lean toward "up" so edge bands match the leather on top
        let lean = |n: [f64; 3]| norm([n[0] * 0.35, n[1] * 0.35, n[2] * 0.35 + 0.65]);
        let (n_a, n_b) = (lean(n_a), lean(n_b));
        let base = (pos.len() / 3) as u32;
        // order: a_top, b_top, a_bot, b_bot
        for (p, n) in [(top[i], n_a), (top[j], n_b), (bot[i], n_a), (bot[j], n_b)] {
            pos.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32]);
            nrm.extend_from_slice(&[n[0] as f32, n[1] as f32, n[2] as f32]);
        }
        let (at, bt, ab, bb) = (base, base + 1, base + 2, base + 3);
        idx.extend_from_slice(&[bt, at, ab, bt, ab, bb]);
    }
}

fn align(co: [f64; 2], moved: [f64; 2], opp: [f64; 2]) -> [f64; 2] {
    let dx = co[0] - moved[0];
    let dy = co[1] - moved[1];
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-9 {
        return opp;
    }
    let ol = ((opp[0] - co[0]).powi(2) + (opp[1] - co[1]).powi(2)).sqrt();
    [co[0] + dx / l * ol, co[1] + dy / l * ol]
}

pub fn signed_volume(pos: &[f32], idx: &[u32]) -> f64 {
    let mut v = 0.0f64;
    for t in idx.chunks(3) {
        let p = |i: u32| {
            let k = i as usize * 3;
            [pos[k] as f64, pos[k + 1] as f64, pos[k + 2] as f64]
        };
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        v += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    v / 6.0
}

/// Open edges after welding vertices that sit at the same place (walls carry their own vertices).
pub fn open_edges_welded(pos: &[f32], idx: &[u32]) -> usize {
    use std::collections::HashMap;
    let mut ids: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let mut map: Vec<u32> = Vec::with_capacity(pos.len() / 3);
    for p in pos.chunks(3) {
        let key = ((p[0] * 1000.0).round() as i64, (p[1] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64);
        let n = ids.len() as u32;
        map.push(*ids.entry(key).or_insert(n));
    }
    let w: Vec<u32> = idx.iter().map(|&i| map[i as usize]).collect();
    open_edges(&w)
}

/// Number of edges not shared by exactly two triangles (0 = watertight).
pub fn open_edges(idx: &[u32]) -> usize {
    use std::collections::HashMap;
    let mut m: HashMap<(u32, u32), i32> = HashMap::new();
    for t in idx.chunks(3) {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            *m.entry(key).or_insert(0) += 1;
        }
    }
    m.values().filter(|&&c| c != 2).count()
}

pub fn stl_bytes(mesh: &Mesh) -> Vec<u8> {
    let tris = mesh.idx.len() / 3;
    let mut out = Vec::with_capacity(84 + tris * 50);
    let mut header = [0u8; 80];
    let tag = b"Backwoods Dash Designer binary STL (mm)";
    header[..tag.len()].copy_from_slice(tag);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(tris as u32).to_le_bytes());
    for t in mesh.idx.chunks(3) {
        let p = |i: u32| {
            let k = i as usize * 3;
            [mesh.pos[k] as f64, mesh.pos[k + 1] as f64, mesh.pos[k + 2] as f64]
        };
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let n = norm(cross(sub(b, a), sub(c, a)));
        for v in [n, a, b, c] {
            for x in v {
                out.extend_from_slice(&(x as f32).to_le_bytes());
            }
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

pub fn section_name(i: usize, n: usize) -> String {
    // n spans, i = 0.. from the driver (-x) side
    match n {
        2 => ["Driver half", "Passenger half"][i].to_string(),
        4 => ["Driver end", "Driver pad", "Passenger pad", "Passenger end"][i].to_string(),
        6 => ["Driver end", "Driver outer", "Driver pad", "Passenger pad", "Passenger outer", "Passenger end"][i].to_string(),
        _ => format!("Section {}", i + 1),
    }
}
