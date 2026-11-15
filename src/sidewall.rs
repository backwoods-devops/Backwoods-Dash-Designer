//! Side walls: a flat face at each end of the dash, bounded by the back curve on one side and the
//! front profiles of the top and lower pads on the other, extruded inward by the shell thickness.
use crate::app::*;
use crate::json::{self, J};
use crate::model::{self, BezPt, Mesh, Project};
use crate::theme::*;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Shape, Stroke};

/// The back curve is stored relative to its start: (dy, dz) from the point where the top pad's
/// windshield edge starts. +y is toward the windshield. Only that start follows the dash; the end is
/// stretched down (or up) to the bottom of the lower pad.
pub fn default_back() -> Vec<BezPt> {
    let p = |co: [f64; 2], hl: [f64; 2], hr: [f64; 2]| BezPt { co, hl, hr };
    vec![
        p([0.0, 0.0], [0.0, 0.0], [-26.29, -9.86]),
        p([-81.69, -61.03], [-81.69, -28.17], [-81.69, -93.9]),
        p([-37.09, -145.54], [-55.87, -124.88], [-18.31, -166.2]),
        p([36.15, -248.83], [23.94, -225.35], [36.15, -272.3]),
        p([37.09, -319.25], [37.09, -293.43], [37.09, -319.25]),
    ]
}

#[derive(Clone, Debug)]
pub struct SideWall {
    pub on: bool,
    pub back: Vec<BezPt>,
    pub look: crate::materials::Look,
    /// draw the walls with the top pad's material and color
    pub follow: bool,
}

impl SideWall {
    pub fn new() -> SideWall {
        SideWall { on: true, back: default_back(), look: crate::materials::Look::vinyl(), follow: true }
    }
    pub fn load(dir: &std::path::Path) -> SideWall {
        let mut w = SideWall::new();
        let Some(j) = std::fs::read_to_string(dir.join("sidewall.json")).ok().and_then(|s| json::parse(&s).ok()) else {
            return w;
        };
        w.follow = j.b("follow").unwrap_or(true);
        w.on = true; // shown again at start; the layers eye only hides them while working
        if let Some(l) = crate::materials::Look::from_ids(j.s("mat").unwrap_or(""), j.s("color").unwrap_or("")) {
            w.look = l;
        }
        if let Some(a) = j.get("back").and_then(|b| b.arr()) {
            let pts: Vec<BezPt> = a
                .iter()
                .filter_map(|p| {
                    let v: Vec<f64> = p.arr()?.iter().filter_map(|x| x.num()).collect();
                    if v.len() == 6 { Some(BezPt { co: [v[0], v[1]], hl: [v[2], v[3]], hr: [v[4], v[5]] }) } else { None }
                })
                .collect();
            if pts.len() >= 2 {
                w.back = pts;
            }
        }
        w
    }
    pub fn save(&self, dir: &std::path::Path) {
        let n = |v: f64| J::Num((v * 1000.0).round() / 1000.0);
        let back: Vec<J> = self.back.iter().map(|p| J::Arr(vec![n(p.co[0]), n(p.co[1]), n(p.hl[0]), n(p.hl[1]), n(p.hr[0]), n(p.hr[1])])).collect();
        let j = J::Obj(vec![("on".into(), J::Bool(true)), ("follow".into(), J::Bool(self.follow)), ("mat".into(), J::Str(self.look.preset().id.to_string())), ("color".into(), J::Str(self.look.hex())), ("back".into(), J::Arr(back))]);
        let _ = std::fs::write(dir.join("sidewall.json"), json::to_string(&j));
    }
    /// A number that changes whenever the shape changes, to know when to rebuild.
    pub fn stamp(&self) -> u64 {
        let mut h = (self.on as u64) ^ ((self.follow as u64) << 40) ^ ((self.look.mat as u64) << 8) ^ ((self.look.col[0] as u64) << 16) ^ ((self.look.col[1] as u64) << 24) ^ ((self.look.col[2] as u64) << 32);
        for p in &self.back {
            for v in [p.co[0], p.co[1], p.hl[0], p.hl[1], p.hr[0], p.hr[1]] {
                h = h.wrapping_mul(1099511628211).wrapping_add(v.to_bits());
            }
        }
        h
    }
}

fn bez(pts: &[BezPt], per: usize) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for i in 0..pts.len().saturating_sub(1) {
        let (p0, h0, h1, p1) = (pts[i].co, pts[i].hr, pts[i + 1].hl, pts[i + 1].co);
        for j in 0..per {
            let t = j as f64 / per as f64;
            let (a, b, c, d) = ((1.0 - t).powi(3), 3.0 * t * (1.0 - t).powi(2), 3.0 * t * t * (1.0 - t), t.powi(3));
            out.push([a * p0[0] + b * h0[0] + c * h1[0] + d * p1[0], a * p0[1] + b * h0[1] + c * h1[1] + d * p1[1]]);
        }
    }
    if let Some(l) = pts.last() {
        out.push(l.co);
    }
    out
}

/// Smallest gap (mm) kept in y between the back curve and the lower pad's end profile.
pub const BACK_CLEAR: f64 = 10.0;

/// Where the lower pad's end profile sits (largest y, toward the windshield) at height z, if it reaches that height.
fn low_y_at(low: &[[f64; 2]], z: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for w in low.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (lo, hi) = (a[1].min(b[1]), a[1].max(b[1]));
        if z < lo - 1e-9 || z > hi + 1e-9 {
            continue;
        }
        let y = if (b[1] - a[1]).abs() < 1e-9 { a[0].max(b[0]) } else { a[0] + (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) };
        best = Some(best.map_or(y, |v: f64| v.max(y)));
    }
    best
}

/// Keep the back curve on the windshield side of the lower pad's end profile, at least `BACK_CLEAR` mm clear of it at every
/// height, so the two lines never cross and the wall always covers the whole end of the lower pad. Every point after the
/// start (which stays on the windshield edge) is moved toward the windshield just enough. Returns true if anything moved.
/// `thick` is the shell thickness: the wall's front edge is cut back by that much, so the gap is measured from the cut-back edge
/// (the wall's real edge), not from the pad's outer profile.
pub fn keep_clear(up: &Project, lo: &Project, back: &mut Vec<BezPt>, thick: f64) -> bool {
    if back.len() < 2 {
        return false;
    }
    let outs: Vec<Outline> = (0..2).filter_map(|e| outline(up, lo, back, e, None, thick)).collect();
    if outs.is_empty() {
        return false;
    }
    let mut moved = false;
    for _ in 0..40 {
        // the worst shortfall for each segment of the curve, over both ends
        let mut need = vec![0.0f64; back.len()];
        for o in &outs {
            let k = o.k;
            let rel = bez(back, 12);
            for (n, r) in rel.iter().enumerate().skip(1) {
                let z = o.start[1] + r[1] * k;
                let y = o.start[0] + r[0];
                if let Some(ly) = low_y_at(&o.low, z) {
                    let short = ly + BACK_CLEAR - y;
                    if short > 1e-6 {
                        // the two points that shape this stretch of the curve share the move
                        let sg = (n / 12).min(back.len() - 2);
                        need[sg] = need[sg].max(short);
                        need[sg + 1] = need[sg + 1].max(short);
                    }
                }
            }
        }
        if need.iter().all(|v| *v <= 1e-6) {
            break;
        }
        for (i, d) in need.iter().enumerate() {
            if *d > 1e-6 && i > 0 {
                let p = &mut back[i];
                p.co[0] += d + 0.01;
                p.hl[0] += d + 0.01;
                p.hr[0] += d + 0.01;
                moved = true;
            }
        }
    }
    moved
}

/// What the wall at one end is made of, in (y, z).
pub struct Outline {
    pub poly: Vec<[f64; 2]>,
    pub top: Vec<[f64; 2]>,   // top pad profile, windshield edge to front lip
    pub low: Vec<[f64; 2]>,   // lower pad profile, top to bottom
    pub back: Vec<[f64; 2]>,  // the back curve in place (start at the windshield edge, end on the floor)
    pub start: [f64; 2],
    pub k: f64, // vertical stretch applied to the back curve
    pub x: f64,
}

/// Offset an open profile (y, z) by `d` mm to its inside, where `inside` picks the side: it is the direction that the offset
/// should lean toward on average (the wall sits inside the dash pieces, so the profile is cut back by the shell thickness).
/// Points where the offset would fold back on itself (a hook tighter than the thickness) are dropped.
fn inset_profile(pts: &[[f64; 2]], d: f64, inside: [f64; 2]) -> Vec<[f64; 2]> {
    let n = pts.len();
    if n < 2 || d <= 0.0 {
        return pts.to_vec();
    }
    let tan = |i: usize| -> [f64; 2] {
        let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
        let (ty, tz) = (b[0] - a[0], b[1] - a[1]);
        let l = (ty * ty + tz * tz).sqrt().max(1e-12);
        [ty / l, tz / l]
    };
    // which side is the inside: the left normal (-tz, ty) or the right one, by the average over the profile
    let mut score = 0.0;
    for i in 0..n {
        let t = tan(i);
        score += -t[1] * inside[0] + t[0] * inside[1];
    }
    let sg = if score >= 0.0 { 1.0 } else { -1.0 };
    let off: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let t = tan(i);
            [pts[i][0] + sg * -t[1] * d, pts[i][1] + sg * t[0] * d]
        })
        .collect();
    // drop the points that run backward compared with the original profile
    let mut out: Vec<[f64; 2]> = vec![off[0]];
    for i in 1..n {
        let last = *out.last().unwrap();
        let (oy, oz) = (pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
        let (iy, iz) = (off[i][0] - last[0], off[i][1] - last[1]);
        if oy * iy + oz * iz > 0.0 || i + 1 == n {
            out.push(off[i]);
        }
    }
    out
}

/// `end` 0 = the -x end, 1 = the +x end. `start_drop` is the shell thickness of the dash pieces: with it the wall is cut back
/// to fit inside them (its edges sit on the inside surface of the top and lower pads), and it starts that far below the
/// top pad's windshield edge. With 0 the outline is the pads' outer profiles.
pub fn outline(up: &Project, lo: &Project, back: &[BezPt], end: usize, surf: Option<&model::Under>, start_drop: f64) -> Option<Outline> {
    if up.profiles.is_empty() || lo.profiles.is_empty() || back.len() < 2 {
        return None;
    }
    let (pu, pl) = if end == 0 { (&up.profiles[0], &lo.profiles[0]) } else { (up.profiles.last()?, lo.profiles.last()?) };
    let top = pu.curve();
    let low = pl.curve();
    if top.len() < 2 || low.len() < 2 {
        return None;
    }
    // inside the shells: the top pad's underside is below its surface, the lower pad's is toward the windshield (+y)
    let (top, low) = if start_drop > 0.0 { (inset_profile(&top, start_drop, [0.0, -1.0]), inset_profile(&low, start_drop, [1.0, 0.0])) } else { (top, low) };
    if top.len() < 2 || low.len() < 2 {
        return None;
    }
    let mut s = top[0];
    // where the top pad's end face really starts (its end may be rounded or chamfered): the back curve starts there
    if let Some(z) = surf.and_then(|u| u.z_at(pu.x, s[0])) {
        if z - start_drop.max(0.0) < s[1] {
            s[1] = z - start_drop.max(0.0);
        }
    }
    let foot = *low.last()?;
    let rel = bez(back, 10);
    let drop = -back.last()?.co[1];
    let k = if drop.abs() > 1e-6 { (s[1] - foot[1]) / drop } else { 1.0 };
    let abs: Vec<[f64; 2]> = rel.iter().map(|r| [s[0] + r[0], s[1] + r[1] * k]).collect();
    let mut poly: Vec<[f64; 2]> = Vec::new();
    poly.extend(top.iter().cloned());
    poly.extend(low.iter().cloned());
    // floor to the end of the back curve, then up the back curve to the start
    for p in abs.iter().rev() {
        poly.push(*p);
    }
    // drop repeats
    let mut clean: Vec<[f64; 2]> = Vec::new();
    for p in poly {
        if clean.last().map(|l| (l[0] - p[0]).abs() < 1e-6 && (l[1] - p[1]).abs() < 1e-6).unwrap_or(false) {
            continue;
        }
        clean.push(p);
    }
    if clean.len() > 2 && (clean[0][0] - clean[clean.len() - 1][0]).abs() < 1e-6 && (clean[0][1] - clean[clean.len() - 1][1]).abs() < 1e-6 {
        clean.pop();
    }
    Some(Outline { poly: clean, top, low, back: abs, start: s, k, x: pu.x })
}


/// Where the pad's real surface sits `thick` in from the end, for each point of its end profile. The loft changes
/// shape a little along x, so a flat plate built from the end profile alone drifts off the pad's edge by a touch.
fn surface_in(pr: &Project, end: usize, thick: f64) -> Option<Vec<[f64; 2]>> {
    let g = pr.grid();
    if g.len() < 2 {
        return None;
    }
    let n = g.len();
    let xs: Vec<f64> = g.iter().map(|r| r[0][0]).collect();
    let x = if end == 0 { xs[0] + thick } else { xs[n - 1] - thick };
    let k = xs.partition_point(|&v| v <= x).max(1).min(n - 1);
    let t = ((x - xs[k - 1]) / (xs[k] - xs[k - 1]).max(1e-9)).max(0.0).min(1.0);
    Some((0..g[0].len()).map(|c| { let (a, b) = (g[k - 1][c], g[k][c]); [a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }).collect())
}

/// For each outline vertex on the top or lower pad's end profile: its (y, z) on the plane `thick` in from the end.
fn inner_edge(up: &Project, lo: &Project, o: &Outline, end: usize, thick: f64) -> Vec<Option<[f64; 2]>> {
    let (ts, ls) = (surface_in(up, end, thick), surface_in(lo, end, thick));
    let same = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6;
    o.poly
        .iter()
        .map(|q| {
            if let (Some(i), Some(ts)) = (o.top.iter().position(|t| same(*t, *q)), ts.as_ref()) {
                return ts.get(i).copied();
            }
            if let (Some(j), Some(ls)) = (o.low.iter().position(|t| same(*t, *q)), ls.as_ref()) {
                return ls.get(j).copied();
            }
            None
        })
        .collect()
}

/// A solid plate: the outline extruded in x. `end` 0 puts it from the -x end inward, 1 from the +x end inward.
/// The top pad's outer surface (with any edge treatment on its ends), so the wall's top edge can follow it.
pub fn pad_surface(up: &Project, thick: f64) -> Option<model::Under> {
    // only needed when the pad's own end is rounded or chamfered; otherwise the plain profile is exact (and the
    // height lookup can pick the wrong branch where the front lip curls under)
    if thick <= 0.0 || up.profiles.is_empty() || up.end_bevel <= 0.0 {
        return None;
    }
    let m = up.mesh(thick);
    let nc = up.profiles[0].curve().len();
    if nc < 2 {
        return None;
    }
    let nr = up.skin_idx_len(thick) / 6 / (nc - 1) + 1;
    if m.pos.len() < nr * nc * 3 {
        return None;
    }
    let g: Vec<Vec<[f64; 3]>> = (0..nr).map(|r| (0..nc).map(|c| { let k = (r * nc + c) * 3; [m.pos[k] as f64, m.pos[k + 1] as f64, m.pos[k + 2] as f64] }).collect()).collect();
    Some(model::Under::from_grid(&g))
}

pub fn plate(o: &Outline, thick: f64, end: usize, surf: Option<&model::Under>, inner: &[Option<[f64; 2]>]) -> Option<Mesh> {
    let tris = crate::shapes::triangulate(&o.poly);
    if tris.is_empty() || thick <= 0.0 {
        return None;
    }
    let (x0, x1) = if end == 0 { (o.x, o.x + thick) } else { (o.x - thick, o.x) };
    // the top edge follows the top pad's surface, so a rounded or chamfered pad end is not overshot by the flat wall
    let ntop = o.top.len();
    let x_in = if end == 0 { x1 } else { x0 };
    let hy = |i: usize, x: f64| -> f64 {
        if (x - x_in).abs() < 1e-9 {
            if let Some(q) = inner.get(i).copied().flatten() {
                return q[0];
            }
        }
        o.poly[i][0]
    };
    let hz = |i: usize, x: f64| -> f64 {
        let z = o.poly[i][1];
        if (x - x_in).abs() < 1e-9 {
            if let Some(q) = inner.get(i).copied().flatten() {
                return q[1];
            }
        }
        if i < ntop {
            if let Some(zs) = surf.and_then(|u| u.z_at(x, o.poly[i][0])) {
                return z.min(zs);
            }
        }
        z
    };
    let mut pos: Vec<f32> = Vec::new();
    let mut nrm: Vec<f32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    fn quad_ready(a: [f64; 3], b: [f64; 3], c: [f64; 3], pos: &mut Vec<f32>, nrm: &mut Vec<f32>, idx: &mut Vec<u32>) {
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
        n = [n[0] / l, n[1] / l, n[2] / l];
        let base = (pos.len() / 3) as u32;
        for p in [a, b, c] {
            pos.extend([p[0] as f32, p[1] as f32, p[2] as f32]);
            nrm.extend([n[0] as f32, n[1] as f32, n[2] as f32]);
        }
        idx.extend([base, base + 1, base + 2]);
    }
    // caps
    for t in &tris {
        let p = |i: usize, x: f64| [x, hy(i, x), hz(i, x)];
        // polygon (y, z) counter-clockwise seen with y right, z up. x axis x = y cross z, so +x faces the viewer.
        let ccw = crate::shapes::poly_area(&o.poly) >= 0.0;
        let (a, b, c) = if ccw { (t[0], t[1], t[2]) } else { (t[0], t[2], t[1]) };
        quad_ready(p(a, x1), p(b, x1), p(c, x1), &mut pos, &mut nrm, &mut idx);
        quad_ready(p(a, x0), p(c, x0), p(b, x0), &mut pos, &mut nrm, &mut idx);
    }
    // sides
    let n = o.poly.len();
    let ccw = crate::shapes::poly_area(&o.poly) >= 0.0;
    for i in 0..n {
        let (ia, ib) = (i, (i + 1) % n);
        let (ia, ib) = if ccw { (ia, ib) } else { (ib, ia) };
        let _ = (o.poly[ia], o.poly[ib]);
        let (p0, p1, p2, p3) = ([x0, hy(ia, x0), hz(ia, x0)], [x0, hy(ib, x0), hz(ib, x0)], [x1, hy(ib, x1), hz(ib, x1)], [x1, hy(ia, x1), hz(ia, x1)]);
        quad_ready(p0, p3, p2, &mut pos, &mut nrm, &mut idx);
        quad_ready(p0, p2, p1, &mut pos, &mut nrm, &mut idx);
    }
    let mut m = Mesh { pos, nrm, idx };
    if model::signed_volume(&m.pos, &m.idx) < 0.0 {
        for t in m.idx.chunks_mut(3) {
            t.swap(1, 2);
        }
        for v in m.nrm.iter_mut() {
            *v = -*v;
        }
    }
    Some(m)
}

/// Both plates as one mesh for the 3D view.
pub fn both(up: &Project, lo: &Project, back: &[BezPt], thick: f64) -> Option<Mesh> {
    let surf = pad_surface(up, thick);
    let mut all = Mesh { pos: Vec::new(), nrm: Vec::new(), idx: Vec::new() };
    for end in 0..2 {
        let o = outline(up, lo, back, end, surf.as_ref(), thick)?;
        let inner: Vec<Option<[f64; 2]>> = Vec::new();
        let m = plate(&o, thick, end, surf.as_ref(), &inner)?;
        let base = (all.pos.len() / 3) as u32;
        all.pos.extend(m.pos);
        // view shading only: lean the light toward "up" so the plate reads like the leather
        all.nrm.extend(m.nrm.chunks(3).flat_map(|n| {
            let v = [n[0] as f64 * 0.35, n[1] as f64 * 0.35, n[2] as f64 * 0.35 + 0.65];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
            [(v[0] / l) as f32, (v[1] / l) as f32, (v[2] / l) as f32]
        }));
        all.idx.extend(m.idx.iter().map(|i| i + base));
    }
    Some(all)
}

/// Write one end's wall into the exports folder under `name` (without .stl). Returns the path (with a warning if not watertight).
pub fn export_end(up: &Project, lo: &Project, back: &[BezPt], end: usize, thick: f64, dir: &std::path::Path, name: &str) -> Result<String, String> {
    let d = dir.join("exports");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let clean: String = name.trim().trim_end_matches(".stl").chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' { c } else { '_' }).collect();
    if clean.is_empty() {
        return Err("Type a file name first".into());
    }
    let surf = pad_surface(up, thick);
    let o = outline(up, lo, back, end, surf.as_ref(), thick).ok_or("could not build the wall outline")?;
    let inner: Vec<Option<[f64; 2]>> = Vec::new();
    let m = plate(&o, thick, end, surf.as_ref(), &inner).ok_or("could not build the wall (is the outline a closed shape?)")?;
    let open = model::open_edges(&m.idx);
    let path = d.join(format!("{}.stl", clean));
    std::fs::write(&path, model::stl_bytes(&m)).map_err(|e| e.to_string())?;
    Ok(format!("{}{}", path.display(), if open == 0 { "" } else { " (WARNING: not watertight)" }))
}

/// Write the walls into the exports folder: one file per end.
pub fn export(up: &Project, lo: &Project, back: &[BezPt], thick: f64, dir: &std::path::Path) -> Result<String, String> {
    let mut done = Vec::new();
    for (end, name) in [(0usize, "side_wall_driver"), (1, "side_wall_passenger")] {
        done.push(export_end(up, lo, back, end, thick, dir, name)?);
    }
    Ok(done.join("\n"))
}

// ------------------------------------------------------------------ editor window
pub struct WallUi {
    pub open: bool,
    pub last: u64,
    pub sel: Option<(usize, u8)>, // anchor index, 0 = anchor, 1 = left handle, 2 = right handle
    pub msg: String,
    pub fit_key: u64, // when the back curve was last checked against the lower pad
}

impl WallUi {
    pub fn new() -> WallUi {
        WallUi { open: false, last: 0, sel: None, msg: String::new(), fit_key: u64::MAX }
    }
}

impl App {
    /// Save the curve when it has changed.
    fn sw_sync(&mut self) {
        let st = self.sw.stamp();
        if st != self.sw_ui.last {
            self.sw_ui.last = st;
            self.sw.save(&self.st.dir);
        }
    }

    /// Buttons for the dock header when the Side walls tab is open.
    pub fn wall_toolbar(&mut self, ui: &mut egui::Ui) {
        let mut export_now = false;
        let mut lz = self.st.lock_z;
        if ui.checkbox(&mut lz, "Lock heights").on_hover_text("The lower pad's top edge follows the top pad's front line, in height and in front-to-back position, so the two meet and the side profile joins cleanly. While this is on, the top pad sets that edge. Where the lower pad is wider than the top pad, the extra width is not locked.").changed() {
            self.op(format!(r#"{{"op":"set_lock_z","v":{}}}"#, lz));
        }
        if ui.button("Reset curve").on_hover_text("Go back to the curve traced from your drawing").clicked() {
            self.sw.back = default_back();
            self.sw_ui.sel = None;
        }
        if ui.button("+ Point").on_hover_text("Split the longest piece of the curve in two").clicked() {
            split_longest(&mut self.sw.back);
        }
        let can = matches!(self.sw_ui.sel, Some((i, _)) if i > 0 && i + 1 < self.sw.back.len());
        if ui.add_enabled(can, egui::Button::new("- Point")).clicked() {
            if let Some((i, _)) = self.sw_ui.sel {
                self.sw.back.remove(i);
                self.sw_ui.sel = None;
            }
        }
        if ui.button("Export walls").on_hover_text("Write one STL per end into the exports folder").clicked() {
            export_now = true;
        }
        if export_now {
            let (up, lo) = (self.st.piece(0).clone(), self.st.piece(1).clone());
            self.sw_ui.msg = match crate::sidewall::export(&up, &lo, &self.sw.back, self.st.thickness, &self.st.dir) {
                Ok(s) => format!("Saved: {}", s.replace('\n', "  |  ")),
                Err(e) => format!("Could not export: {}", e),
            };
        }
        if !self.sw_ui.msg.is_empty() {
            ui.label(egui::RichText::new(&self.sw_ui.msg).size(12.0).color(TEAL));
        }
        self.sw_sync();
    }

    /// The side-view editor for the back curve, drawn in the dock.
    pub fn wall_editor(&mut self, ui: &mut egui::Ui, rect: Rect, resp: &egui::Response) {
        let (up, lo) = (self.st.piece(0).clone(), self.st.piece(1).clone());
        {
            let Some(o) = outline(&up, &lo, &self.sw.back, 0, None, self.st.thickness) else {
                ui.painter_at(rect).text(rect.center(), Align2::CENTER_CENTER, "Nothing to draw yet.", FontId::proportional(13.0), MUTED);
                return;
            };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, CornerRadius::same(8), Color32::from_rgb(20, 22, 27));
            // bounds of everything shown
            let (mut lo_y, mut hi_y, mut lo_z, mut hi_z) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for p in o.poly.iter().chain(o.back.iter()) {
                lo_y = lo_y.min(p[0]);
                hi_y = hi_y.max(p[0]);
                lo_z = lo_z.min(p[1]);
                hi_z = hi_z.max(p[1]);
            }
            let m = 30.0f32;
            let sc = ((rect.width() - 2.0 * m) as f64 / (hi_y - lo_y).max(1.0)).min((rect.height() - 2.0 * m) as f64 / (hi_z - lo_z).max(1.0)) as f32;
            let (cy, cz) = ((lo_y + hi_y) * 0.5, (lo_z + hi_z) * 0.5);
            // +y (windshield) is to the left so the front faces right
            let to_s = |p: [f64; 2]| pos2(rect.center().x - (p[0] - cy) as f32 * sc, rect.center().y - (p[1] - cz) as f32 * sc);
            let from_s = |q: Pos2| [cy - ((q.x - rect.center().x) / sc) as f64, cz - ((q.y - rect.center().y) / sc) as f64];
            // filled outline
            let pts: Vec<Pos2> = o.poly.iter().map(|p| to_s(*p)).collect();
            let tris = crate::shapes::triangulate(&o.poly);
            let mut mesh = egui::Mesh::default();
            for p in &pts {
                mesh.colored_vertex(*p, Color32::from_rgba_unmultiplied(67, 184, 140, 46));
            }
            for t in &tris {
                mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
            }
            painter.add(Shape::mesh(mesh));
            let line = |v: &[[f64; 2]], c: Color32, w: f32| {
                painter.add(Shape::line(v.iter().map(|p| to_s(*p)).collect(), Stroke::new(w, c)));
            };
            line(&o.top, Color32::from_rgb(170, 176, 190), 1.6);
            line(&o.low, Color32::from_rgb(170, 176, 190), 1.6);
            let floor_y = o.low.last().map(|p| p[1]).unwrap_or(0.0);
            let _ = floor_y;
            line(&o.back, Color32::from_rgb(240, 160, 60), 2.2);
            // handles and anchors, shown at the end station
            let k = o.k;
            let abs = |r: [f64; 2]| [o.start[0] + r[0], o.start[1] + r[1] * k];
            let rel = |a: [f64; 2]| [a[0] - o.start[0], (a[1] - o.start[1]) / if k.abs() > 1e-9 { k } else { 1.0 }];
            let n = self.sw.back.len();
            let mut hit: Option<(usize, u8)> = None;
            let hover = resp.hover_pos();
            for i in 0..n {
                let p = self.sw.back[i];
                let (a, hl, hr) = (to_s(abs(p.co)), to_s(abs(p.hl)), to_s(abs(p.hr)));
                if i > 0 {
                    painter.line_segment([a, hl], Stroke::new(1.0, Color32::from_rgb(120, 126, 140)));
                }
                if i + 1 < n {
                    painter.line_segment([a, hr], Stroke::new(1.0, Color32::from_rgb(120, 126, 140)));
                }
                let near = |q: Pos2| hover.map(|h| h.distance(q) < 9.0).unwrap_or(false);
                if i > 0 {
                    painter.circle_filled(hl, 4.5, if self.sw_ui.sel == Some((i, 1)) { ACCENT } else { Color32::from_rgb(200, 205, 215) });
                    if near(hl) {
                        hit = Some((i, 1));
                    }
                }
                if i + 1 < n {
                    painter.circle_filled(hr, 4.5, if self.sw_ui.sel == Some((i, 2)) { ACCENT } else { Color32::from_rgb(200, 205, 215) });
                    if near(hr) {
                        hit = Some((i, 2));
                    }
                }
                let r = Rect::from_center_size(a, vec2(10.0, 10.0));
                painter.rect_filled(r, CornerRadius::same(2), if self.sw_ui.sel == Some((i, 0)) { ACCENT } else { Color32::from_rgb(240, 160, 60) });
                if near(a) {
                    hit = Some((i, 0));
                }
            }
            painter.text(to_s([o.start[0], o.start[1]]) + vec2(8.0, -10.0), Align2::LEFT_BOTTOM, "windshield edge", FontId::proportional(11.0), MUTED);
            painter.text(rect.left_top() + vec2(10.0, 8.0), Align2::LEFT_TOP, "WINDSHIELD", FontId::proportional(10.5), MUTED);
            painter.text(rect.right_top() + vec2(-10.0, 8.0), Align2::RIGHT_TOP, "FRONT", FontId::proportional(10.5), MUTED);
            if resp.drag_started() {
                self.sw_ui.sel = hit;
            }
            if resp.clicked() && hit.is_none() {
                self.sw_ui.sel = None;
            }
            if resp.dragged() {
                if let (Some((i, w)), Some(h)) = (self.sw_ui.sel, resp.interact_pointer_pos()) {
                    let want = rel(from_s(h));
                    let last = n - 1;
                    let p = &mut self.sw.back[i];
                    match w {
                        0 => {
                            if i == 0 {
                                // the start is fixed on the windshield edge
                            } else {
                                let old = p.co;
                                let mut new = if i == last { [want[0], old[1]] } else { want };
                                // never let the point cross the lower pad's profile: keep it clear on the windshield side
                                let kk = if o.k.abs() > 1e-9 { o.k } else { 1.0 };
                                if let Some(ly) = low_y_at(&o.low, o.start[1] + new[1] * kk) {
                                    new[0] = new[0].max(ly + BACK_CLEAR - o.start[0]);
                                }
                                let d = [new[0] - old[0], new[1] - old[1]];
                                p.co = new;
                                p.hl = [p.hl[0] + d[0], p.hl[1] + d[1]];
                                p.hr = [p.hr[0] + d[0], p.hr[1] + d[1]];
                            }
                        }
                        1 => {
                            p.hl = want;
                            // keep the opposite handle in line (smooth point)
                            if i + 1 < n {
                                let v = [p.co[0] - want[0], p.co[1] - want[1]];
                                let l = (p.hr[0] - p.co[0]).hypot(p.hr[1] - p.co[1]);
                                let vl = v[0].hypot(v[1]).max(1e-9);
                                p.hr = [p.co[0] + v[0] / vl * l, p.co[1] + v[1] / vl * l];
                            }
                        }
                        _ => {
                            p.hr = want;
                            if i > 0 {
                                let v = [p.co[0] - want[0], p.co[1] - want[1]];
                                let l = (p.hl[0] - p.co[0]).hypot(p.hl[1] - p.co[1]);
                                let vl = v[0].hypot(v[1]).max(1e-9);
                                p.hl = [p.co[0] + v[0] / vl * l, p.co[1] + v[1] / vl * l];
                            }
                        }
                    }
                }
            }
            if resp.drag_stopped() {
                self.sw_ui.fit_key = u64::MAX; // check the whole curve once the drag ends
            }
            if let Some((i, w)) = self.sw_ui.sel {
                let p = self.sw.back[i];
                let v = [p.co, p.hl, p.hr][w as usize];
                painter.text(rect.left_bottom() + vec2(10.0, -8.0), Align2::LEFT_BOTTOM, format!("Selected: point {}{}   across {}   down {}", i, ["", " (left handle)", " (right handle)"][w as usize], crate::units::signed(v[0], 1), crate::units::signed(v[1], 1)), FontId::proportional(12.0), MUTED);
            }
        }
        self.sw_sync();
    }
}

fn split_longest(back: &mut Vec<BezPt>) {
    if back.len() < 2 {
        return;
    }
    let mut best = (0usize, 0.0f64);
    for i in 0..back.len() - 1 {
        let l = (back[i + 1].co[0] - back[i].co[0]).hypot(back[i + 1].co[1] - back[i].co[1]);
        if l > best.1 {
            best = (i, l);
        }
    }
    let i = best.0;
    let (p0, h0, h1, p1) = (back[i].co, back[i].hr, back[i + 1].hl, back[i + 1].co);
    let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let (a, b, c) = (mid(p0, h0), mid(h0, h1), mid(h1, p1));
    let (d, e) = (mid(a, b), mid(b, c));
    let m = mid(d, e);
    back[i].hr = a;
    back[i + 1].hl = c;
    back.insert(i + 1, BezPt { co: m, hl: d, hr: e });
}
