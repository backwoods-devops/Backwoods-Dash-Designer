// Generic starting dashes built from a few parameters (used by the wizard).
// The cross-section is a windshield-line top run, a front arc, a vertical drop and a small lip hook.

use crate::json::J;
use crate::model::{BezPt, Key, Profile, Project, Section, PALETTE, section_name};

#[derive(Clone, Debug)]
pub struct Params {
    pub kind: String,
    pub width: f64,      // end to end
    pub depth: f64,      // windshield line -> front edge at the center
    pub height: f64,     // windshield line -> floor at the center
    pub front_r: f64,    // front arc radius
    pub slope_deg: f64,  // fall of the top run toward the driver
    pub hook: f64,       // lip length
    pub hook_r: f64,     // lip bend radius
    pub nprof: usize,    // 3, 5 or 7
    pub bow_y: f64,      // how far the windshield line moves out at the center
    pub bow_z: f64,      // how far it rises at the center
    pub end_depth: f64,  // depth of the end profiles as a fraction of the center
    pub crease: bool,    // dip at the center of the front line
    pub centre_depth: f64, // depth of the center profile as a fraction of the full depth (used with the dip)
    pub centre_out: f64, // mm the middle of the front line is brought further out (negative: further in), in one smooth curve (0 = off; not used with the pocketed W)
    pub yw: f64,         // center windshield line position
    pub zw: f64,
    pub drv_extra: f64,  // driver side sticks out this much further (0 = symmetric dash)
    pub step_x: f64,     // where the driver-side step ends (x, mm; the driver is -x)
    pub drv_lip: f64,    // extra lip length on the driver side
}

pub fn preset(kind: &str) -> Params {
    let base = Params {
        kind: kind.to_string(),
        width: 1480.0,
        depth: 265.0,
        height: 135.0,
        front_r: 70.0,
        slope_deg: 10.0,
        hook: 25.0,
        hook_r: 8.0,
        nprof: 5,
        bow_y: 138.0,
        bow_z: 40.0,
        end_depth: 0.55,
        crease: true,
        centre_depth: 1.0,
        centre_out: 40.0,
        yw: 238.0,
        zw: 395.5,
        drv_extra: 0.0,
        step_x: 0.0,
        drv_lip: 0.0,
    };
    match kind {
        "bench" => Params { nprof: 3, bow_y: 0.0, bow_z: 0.0, end_depth: 1.0, crease: false, centre_out: 0.0, width: 1400.0, depth: 250.0, height: 120.0, ..base },
        "smooth" => Params { nprof: 5, bow_y: 90.0, bow_z: 25.0, end_depth: 0.7, crease: false, centre_out: 0.0, ..base },
        // 1978 Chevrolet C10 / K10 style: one deep, nearly flat top with a slight bow and a plain near-vertical front. Blank shell, no top trim.
        "squarebody" => Params { nprof: 5, width: 1450.0, depth: 270.0, height: 230.0, front_r: 55.0, slope_deg: 7.0, hook: 10.0, hook_r: 6.0, bow_y: 55.0, bow_z: 10.0, end_depth: 0.9, crease: false, centre_out: 0.0, drv_extra: 38.0, step_x: 70.0, drv_lip: 14.0, ..base },
        "blank" => Params { nprof: 3, bow_y: 40.0, bow_z: 10.0, end_depth: 0.8, crease: false, centre_out: 0.0, width: 1200.0, ..base },
        _ => base, // "wrap": twin-pad dash with a center crease
    }
}

pub fn params_from_json(j: &J) -> Params {
    let kind = j.s("kind").unwrap_or("wrap");
    let mut p = preset(kind);
    let f = |k: &str, d: f64| j.n(k).unwrap_or(d);
    p.width = f("width", p.width).max(300.0).min(3000.0);
    p.depth = f("depth", p.depth).max(60.0).min(800.0);
    p.height = f("height", p.height).max(40.0).min(500.0);
    p.front_r = f("front_r", p.front_r).max(5.0).min(300.0);
    p.slope_deg = f("slope_deg", p.slope_deg).max(0.0).min(40.0);
    p.hook = f("hook", p.hook).max(0.0).min(200.0);
    p.hook_r = f("hook_r", p.hook_r).max(2.0).min(60.0);
    let n = f("nprof", p.nprof as f64).round() as usize;
    p.nprof = if n <= 3 { 3 } else if n <= 5 { 5 } else { 7 };
    p.bow_y = f("bow_y", p.bow_y).max(0.0).min(400.0);
    p.bow_z = f("bow_z", p.bow_z).max(0.0).min(200.0);
    p.end_depth = f("end_depth", p.end_depth).max(0.3).min(1.2);
    p.crease = j.b("crease").unwrap_or(p.crease);
    p.centre_depth = f("centre_depth", p.centre_depth).max(0.2).min(1.2);
    p.centre_out = f("centre_out", p.centre_out).max(-200.0).min(200.0);
    p.yw = f("yw", p.yw);
    p.zw = f("zw", p.zw);
    p.drv_extra = f("drv_extra", p.drv_extra).max(0.0).min(200.0);
    p.step_x = f("step_x", p.step_x).max(-400.0).min(400.0);
    p.drv_lip = f("drv_lip", p.drv_lip).max(0.0).min(100.0);
    p
}

/// Dense polyline of one cross-section, from the windshield line down over the front arc to the lip.
fn section_path(yw: f64, zw: f64, depth: f64, zbot: f64, r_in: f64, alpha: f64, hook: f64, rh: f64) -> Vec<[f64; 2]> {
    let (sa, ca) = (alpha.sin(), alpha.cos());
    let mut r = r_in.min(depth * 0.8).max(2.0);
    let mut l1;
    let mut l3;
    let mut iter = 0;
    loop {
        l1 = ((depth - r * (1.0 - sa)) / ca).max(1.0);
        let z_arc_end = zw - l1 * sa - r * ca;
        l3 = (z_arc_end - rh) - zbot;
        if l3 >= 0.0 || iter > 40 {
            break;
        }
        r *= 0.9;
        iter += 1;
    }
    let l3 = l3.max(0.0);
    let mut pts: Vec<[f64; 2]> = Vec::new();
    let step = 0.5;
    // 1) top run
    let d1 = [-ca, -sa];
    let n1 = (l1 / step).ceil().max(1.0) as usize;
    for i in 0..=n1 {
        let t = l1 * i as f64 / n1 as f64;
        pts.push([yw + d1[0] * t, zw + d1[1] * t]);
    }
    let p1 = *pts.last().unwrap();
    // 2) front arc: heading rotates CCW from d1 to straight down
    let beta = std::f64::consts::FRAC_PI_2 - alpha;
    let nrm = [sa, -ca]; // left normal of d1
    let c = [p1[0] + r * nrm[0], p1[1] + r * nrm[1]];
    let n2 = ((r * beta) / step).ceil().max(2.0) as usize;
    for i in 1..=n2 {
        let th = beta * i as f64 / n2 as f64;
        let d = [d1[0] * th.cos() - d1[1] * th.sin(), d1[0] * th.sin() + d1[1] * th.cos()];
        let nn = [-d[1], d[0]];
        pts.push([c[0] - r * nn[0], c[1] - r * nn[1]]);
    }
    // 3) drop
    let p2 = *pts.last().unwrap();
    let n3 = (l3 / step).ceil() as usize;
    for i in 1..=n3 {
        pts.push([p2[0], p2[1] - l3 * i as f64 / n3 as f64]);
    }
    // 4) lip hook: down -> forward (+y)
    let p3 = *pts.last().unwrap();
    let c4 = [p3[0] + rh, p3[1]];
    let n4 = ((rh * std::f64::consts::FRAC_PI_2) / step).ceil().max(2.0) as usize;
    for i in 1..=n4 {
        let th = std::f64::consts::FRAC_PI_2 * i as f64 / n4 as f64;
        // start at angle 180 deg (left of center), sweep CCW to 270 deg (below center)
        let a = std::f64::consts::PI + th;
        pts.push([c4[0] + rh * a.cos(), c4[1] + rh * a.sin()]);
    }
    let p4 = *pts.last().unwrap();
    let n5 = (hook / step).ceil() as usize;
    for i in 1..=n5 {
        pts.push([p4[0] + hook * i as f64 / n5 as f64, p4[1]]);
    }
    pts
}

pub const TEMPLATE_POINTS: usize = 6;

fn resample(path: &[[f64; 2]], n: usize) -> Vec<[f64; 2]> {
    let mut cum = vec![0.0];
    for i in 1..path.len() {
        let d = ((path[i][0] - path[i - 1][0]).powi(2) + (path[i][1] - path[i - 1][1]).powi(2)).sqrt();
        cum.push(cum[i - 1] + d);
    }
    let total = *cum.last().unwrap();
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for k in 0..n {
        let s = total * k as f64 / (n - 1) as f64;
        while j + 2 < path.len() && cum[j + 1] < s {
            j += 1;
        }
        let seg = (cum[j + 1] - cum[j]).max(1e-9);
        let t = ((s - cum[j]) / seg).max(0.0).min(1.0);
        out.push([path[j][0] + (path[j + 1][0] - path[j][0]) * t, path[j][1] + (path[j + 1][1] - path[j][1]) * t]);
    }
    out
}

fn to_bezier(p: &[[f64; 2]]) -> Vec<BezPt> {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (if i > 0 { p[i - 1] } else { p[i] }, if i + 1 < n { p[i + 1] } else { p[i] });
            let k = if i == 0 || i + 1 == n { 1.0 / 3.0 } else { 1.0 / 6.0 };
            let t = [(b[0] - a[0]) * k, (b[1] - a[1]) * k];
            BezPt { co: p[i], hl: [p[i][0] - t[0], p[i][1] - t[1]], hr: [p[i][0] + t[0], p[i][1] + t[1]] }
        })
        .collect()
}

/// Dash whose driver side (-x) is deeper and has a lip, with a short step to the shallower passenger side.
fn generate_stepped(p: &Params) -> Project {
    let half = p.width / 2.0;
    let zbot = p.zw - p.height;
    let alpha = p.slope_deg.to_radians();
    let ramp = 55.0;
    let xs = [-half, -0.55 * half, -0.12 * half, p.step_x, p.step_x + ramp, 0.58 * half, half];
    let n = xs.len();
    let mut profiles = Vec::new();
    let mut rail = Vec::new();
    for (i, &x) in xs.iter().enumerate() {
        let t = (x.abs() / half).min(1.0);
        let t2 = t * t;
        let yw = p.yw - p.bow_y * t2;
        let zw = p.zw - p.bow_z * t2;
        // 1 on the driver side, 0 on the passenger side, smooth through the step
        let u = ((x - p.step_x) / ramp).clamp(0.0, 1.0);
        let drv = 1.0 - u * u * (3.0 - 2.0 * u);
        let depth = p.depth * (1.0 - (1.0 - p.end_depth) * t2) + p.drv_extra * drv;
        let hook = p.hook + p.drv_lip * drv;
        let path = section_path(yw, zw, depth, zbot, p.front_r, alpha, hook, p.hook_r);
        let pts = to_bezier(&resample(&path, 36));
        profiles.push(Profile { id: i as u32, x, fixed: i == 0 || i + 1 == n, crease: false, sym: false, pair: None, pts });
        rail.push([x, yw, zw]);
    }
    let spans = n - 1;
    let sections: Vec<Section> = (0..spans)
        .map(|i| Section { name: section_name(i, spans), color: PALETTE[i % PALETTE.len()].to_string(), start: i as u32, end: (i + 1) as u32 })
        .collect();
    let mut pr = Project { profiles, rail, rail2: Vec::new(), look: crate::materials::Look::vinyl(), rail_sym: false, next_id: n as u32, sections, keys: Vec::new(), comps: Vec::new(), next_comp: 1, cuts: Vec::new(), next_cut: 1, under: None, under_gap: 0.0, end_bevel: 0.0, end_down: 0.0, end_round: false, end_full: false, nose_follows: false, follow_base: Vec::new(), broken: Vec::new(), nat: Vec::new() };
    pr.seed_front();
    pr.snap();
    pr.default_keys();
    pr.resample_all(TEMPLATE_POINTS, true);
    pr
}

pub fn generate(p: &Params) -> Project {
    if p.drv_extra > 0.0 {
        return generate_stepped(p);
    }
    let n = p.nprof;
    let half = p.width / 2.0;
    let zbot = p.zw - p.height;
    let alpha = p.slope_deg.to_radians();
    let mut profiles = Vec::new();
    let mut rail = Vec::new();
    for i in 0..n {
        let x = -half + p.width * i as f64 / (n - 1) as f64;
        let t = (x.abs() / half).min(1.0);
        let t2 = t * t;
        let yw = p.yw - p.bow_y * t2;
        let zw = p.zw - p.bow_z * t2;
        // crease on: the front line is a W (each side a hump, the center dipping back to a point) while the windshield
        // line stays one U; off: the front follows the windshield in one full U
        let hump = if p.crease && n >= 5 { 0.25 * p.depth * (1.0 - (2.0 * t - 1.0).powi(2)) } else { 0.0 };
        // the center profile's own depth: pulls the middle of the front line back to a point that eases out over the middle half
        let dip = if p.crease && n >= 5 { (1.0 - p.centre_depth) * p.depth * (1.0 - 2.0 * t).max(0.0) } else { 0.0 };
        // center out: the middle of the front line comes further out in one smooth curve that eases to nothing at the ends
        let out = if p.crease { 0.0 } else { let u = 1.0 - t; p.centre_out * u * u * (3.0 - 2.0 * u) };
        let depth = p.depth * (1.0 - (1.0 - p.end_depth) * t2) - hump - dip + out;
        let path = section_path(yw, zw, depth, zbot, p.front_r, alpha, p.hook, p.hook_r);
        let pts = to_bezier(&resample(&path, 36));
        let center = (n % 2 == 1) && i == n / 2;
        let end = i == 0 || i + 1 == n;
        let pair = if center { None } else { Some((n - 1 - i) as u32) };
        profiles.push(Profile { id: i as u32, x: if center { 0.0 } else { x }, fixed: center || end, crease: center && p.crease, sym: true, pair, pts });
        rail.push([if center { 0.0 } else { x }, yw, zw]);
    }
    let spans = n - 1;
    let sections: Vec<Section> = (0..spans)
        .map(|i| Section {
            name: section_name(i, spans),
            color: PALETTE[i.min(spans - 1 - i) % PALETTE.len()].to_string(),
            start: i as u32,
            end: (i + 1) as u32,
        })
        .collect();
    let mut pr = Project { profiles, rail, rail2: Vec::new(), look: crate::materials::Look::vinyl(), rail_sym: true, next_id: n as u32, sections, keys: Vec::new(), comps: Vec::new(), next_comp: 1, cuts: Vec::new(), next_cut: 1, under: None, under_gap: 0.0, end_bevel: 0.0, end_down: 0.0, end_round: false, end_full: false, nose_follows: false, follow_base: Vec::new(), broken: Vec::new(), nat: Vec::new() };
    pr.seed_front();
    pr.snap();
    pr.default_keys();
    pr.resample_all(TEMPLATE_POINTS, true);
    pr
}

/// A top trim that follows the given top pad: same windshield line and front profile, sitting a few mm above it
/// and covering the top run (about three quarters of the way to the front).
pub fn trim_from(top: &Project, look: crate::materials::Look) -> Project {
    trim_from_n(top, look, TEMPLATE_POINTS)
}

/// Same, with `npts` points on every profile.
pub fn trim_from_n(top: &Project, look: crate::materials::Look, npts: usize) -> Project {
    const BACK: f64 = 3.5;
    const FRONT: f64 = 11.0;
    const COVER: f64 = 0.73;
    let grid = top.grid();
    let step = (crate::model::M / 2).max(1);
    let mut rows: Vec<&Vec<[f64; 3]>> = grid.iter().step_by(step).collect();
    if let (Some(last), Some(l2)) = (grid.last(), rows.last()) {
        if (l2[0][0] - last[0][0]).abs() > 1e-6 {
            rows.push(last);
        }
    }
    let n = rows.len();
    let mut profiles = Vec::new();
    let mut rail = Vec::new();
    let mut rail2 = Vec::new();
    let mut xs = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let x = row[0][0];
        // top run only: thin at the windshield, thicker toward the front, ending in a rounded nose that tucks into the pad
        let (yw, ymin) = (row[0][1], row.iter().map(|q| q[1]).fold(f64::MAX, f64::min));
        let cut = yw - COVER * (yw - ymin);
        let span = (yw - cut).max(1e-9);
        let raise = |y: f64| {
            let f = ((yw - y) / span).clamp(0.0, 1.0);
            BACK + (FRONT - BACK) * f * f * (3.0 - 2.0 * f)
        };
        let mut path: Vec<[f64; 2]> = Vec::new();
        let mut zc = 0.0;
        let mut last_i = 0usize;
        for (qi, q) in row.iter().enumerate() {
            if q[1] >= cut || path.len() < 2 {
                path.push([q[1], q[2] + raise(q[1])]);
                zc = q[2];
                last_i = qi;
            } else {
                // the last point of the top run, not a later point of the lip that hooks back past the cut (pocketed fronts)
                let a = row[last_i];
                let t = ((a[1] - cut) / (a[1] - q[1]).abs().max(1e-9)).clamp(0.0, 1.0);
                zc = a[2] + (q[2] - a[2]) * t;
                let l = path.len();
                path[l - 1] = [a[1], a[2] + raise(a[1])];
                path.push([cut, zc + FRONT]);
                break;
            }
        }
        let rn = FRONT;
        // pad surface height at y along this row (row runs from the windshield toward the front, y falling)
        let pad_at = |y: f64| -> f64 {
            for w in row.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (y <= a[1] && y >= b[1]) || (y >= a[1] && y <= b[1]) {
                    let d = b[1] - a[1];
                    let t = if d.abs() < 1e-9 { 0.0 } else { (y - a[1]) / d };
                    return a[2] + (b[2] - a[2]) * t;
                }
            }
            row[row.len() - 1][2]
        };
        // the nose is a round bulge that goes over the top, out past the front and tucks back under (about 150 degrees of a
        // circle), unless the pad is met first
        const SWEEP: f64 = 2.62;
        const STEPS: usize = 16;
        for k in 1..=STEPS {
            let th = SWEEP * k as f64 / STEPS as f64;
            let pt = [cut - rn * th.sin(), zc + FRONT - rn + rn * th.cos()];
            let pz = pad_at(pt[0]);
            if pt[1] <= pz - 0.4 && th > 0.5 {
                // met the pad on the way round: stop there
                path.push([pt[0], pz - 0.4]);
                break;
            }
            path.push(pt);
        }
        let pts = to_bezier(&resample(&path, 60));
        let first = path[0];
        let last = *path.last().unwrap();
        rail.push([x, first[0], first[1]]);
        rail2.push([last[0], last[1]]);
        xs.push(x);
        profiles.push(Profile { id: i as u32, x, fixed: i == 0 || i + 1 == n, crease: false, sym: false, pair: None, pts });
    }
    let sym = top.rail_sym && (0..n).all(|i| (xs[i] + xs[n - 1 - i]).abs() < 0.5);
    if sym {
        for i in 0..n {
            let center = n % 2 == 1 && i == n / 2;
            profiles[i].sym = true;
            profiles[i].pair = if center { None } else { Some((n - 1 - i) as u32) };
            if center {
                profiles[i].x = 0.0;
                rail[i][0] = 0.0;
            }
        }
    }
    let spans = n - 1;
    let sections: Vec<Section> = (0..spans)
        .map(|i| Section { name: section_name(i, spans), color: PALETTE[i.min(spans - 1 - i) % PALETTE.len()].to_string(), start: i as u32, end: (i + 1) as u32 })
        .collect();
    let mut pr = Project { profiles, rail, rail2, look, rail_sym: sym, next_id: n as u32, sections, keys: Vec::new(), comps: Vec::new(), next_comp: 1, cuts: Vec::new(), next_cut: 1, under: None, under_gap: 0.0, end_bevel: 0.0, end_down: 0.0, end_round: false, end_full: false, nose_follows: false, follow_base: Vec::new(), broken: Vec::new(), nat: Vec::new() };
    pr.snap();
    pr.default_keys();
    pr.resample_all(npts, true);
    pr
}

#[allow(dead_code)]
pub fn keys_of(pr: &Project) -> &Vec<Key> {
    &pr.keys
}
