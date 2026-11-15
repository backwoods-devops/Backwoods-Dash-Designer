// Shapes placed on a piece: cut through, recess, raise or inlay. The result is built with real mesh booleans on the printed shell.
use crate::json::J;
use crate::model::{Mesh, Project};
use crate::shapes::{self, Shape};
use boolmesh::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CutOp {
    /// Opening straight through the wall.
    Cut,
    /// Pocket that stops part way through (a sunk panel).
    Recess,
    /// Boss pulled out above the surface.
    Raise,
    /// Opening plus a separate plug of the same shape that fills it.
    Inlay,
}

impl CutOp {
    pub const ALL: [CutOp; 4] = [CutOp::Cut, CutOp::Recess, CutOp::Raise, CutOp::Inlay];
    pub fn id(self) -> &'static str {
        match self {
            CutOp::Cut => "cut",
            CutOp::Recess => "recess",
            CutOp::Raise => "raise",
            CutOp::Inlay => "inlay",
        }
    }
    pub fn from_id(s: &str) -> CutOp {
        match s {
            "cut" => CutOp::Cut,
            "raise" => CutOp::Raise,
            "inlay" => CutOp::Inlay,
            _ => CutOp::Recess,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            CutOp::Cut => "Cut through",
            CutOp::Recess => "Recess",
            CutOp::Raise => "Raise",
            CutOp::Inlay => "Inlay",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            CutOp::Cut => "Opens the shape straight through the wall (vents, speaker holes).",
            CutOp::Recess => "Sinks the shape into the surface as a pocket (an inset panel or a logo).",
            CutOp::Raise => "Pulls the shape out of the surface as a raised pad.",
            CutOp::Inlay => "Cuts the shape out and keeps it as a separate plug that fills the opening.",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cut {
    pub id: u32,
    pub name: String,
    pub shape: Shape,
    /// Where the shape sits on the surface (center), and the surface normal there.
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub n: [f64; 3],
    /// Cut along the surface normal instead of straight down.
    pub follow: bool,
    pub rot: f64,
    pub op: CutOp,
    /// Recess / inlay depth or raise height, mm.
    pub depth: f64,
    /// Wall lean in degrees (a bevel: the walls slope instead of going straight).
    pub draft: f64,
    /// Recess floor / raised top follows the surface (true) or stays flat (false).
    pub follow_floor: bool,
    /// Recess / raise / inlay: push the inside of the wall along with the surface so the wall keeps its thickness.
    pub keep_wall: bool,
    /// 3D part cut: leave a wall of the dash's own thickness around the removed part instead of a bare hole.
    pub line: bool,
    /// 3D parts: rotation about the world X and Y axes in degrees (`rot` is about Z).
    pub rx: f64,
    pub ry: f64,
    /// Also make the mirror image on the other side of the dash (x = 0).
    pub mirror: bool,
    /// Internal: this is the mirrored copy (outline / solid flipped left-right).
    pub flip: bool,
    /// Inlay: gap between the plug and the opening, mm.
    pub gap: f64,
    /// Inlay: how far the plug stands above (+) or below (-) the surface, mm.
    pub plug_h: f64,
    pub visible: bool,
    /// Goes into the exported STL (independent of being shown on screen).
    pub export: bool,
    /// Locked parts cannot be moved, turned or edited by accident.
    pub locked: bool,
}

impl Cut {
    /// An extruded shape or an imported STL: a solid part that is cut out of or added to the dash.
    pub fn is3d(&self) -> bool {
        self.shape.depth > 0.0
    }

    pub fn new(id: u32, shape: &Shape, p: [f64; 3], n: [f64; 3]) -> Cut {
        if shape.depth > 0.0 {
            return Cut { id, name: shape.name.clone(), shape: shape.clone(), x: p[0], y: p[1], z: p[2] + shape.depth / 2.0, n, follow: false, rot: 0.0, op: CutOp::Raise, depth: 1.0, draft: 0.0, follow_floor: true, keep_wall: false, line: true, rx: 0.0, ry: 0.0, mirror: false, flip: false, gap: 0.3, plug_h: 0.0, visible: true, locked: false, export: true };
        }
        let mut s = shape.clone();
        let (d, dr) = (s.depth, s.draft);
        s.depth = 0.0;
        s.bevel = 0.0;
        s.draft = 0.0;
        Cut { id, name: shape.name.clone(), shape: s, x: p[0], y: p[1], z: p[2], n, follow: false, rot: 0.0, op: CutOp::Recess, depth: if d > 0.0 { d } else { 3.0 }, draft: dr, follow_floor: true, keep_wall: true, line: true, rx: 0.0, ry: 0.0, mirror: false, flip: false, gap: 0.3, plug_h: 0.0, visible: true, locked: false, export: true }
    }

    pub fn to_json(&self) -> J {
        J::Obj(vec![
            ("id".into(), J::Num(self.id as f64)),
            ("name".into(), J::Str(self.name.clone())),
            ("shape".into(), self.shape.to_json()),
            ("x".into(), J::Num(self.x)),
            ("y".into(), J::Num(self.y)),
            ("z".into(), J::Num(self.z)),
            ("n".into(), J::Arr(self.n.iter().map(|v| J::Num(*v)).collect())),
            ("follow".into(), J::Bool(self.follow)),
            ("rot".into(), J::Num(self.rot)),
            ("op".into(), J::Str(self.op.id().into())),
            ("depth".into(), J::Num(self.depth)),
            ("draft".into(), J::Num(self.draft)),
            ("follow_floor".into(), J::Bool(self.follow_floor)),
            ("keep_wall".into(), J::Bool(self.keep_wall)),
            ("line".into(), J::Bool(self.line)),
            ("rx".into(), J::Num(self.rx)),
            ("ry".into(), J::Num(self.ry)),
            ("mirror".into(), J::Bool(self.mirror)),
            ("gap".into(), J::Num(self.gap)),
            ("plug_h".into(), J::Num(self.plug_h)),
            ("visible".into(), J::Bool(self.visible)),
            ("locked".into(), J::Bool(self.locked)),
            ("export".into(), J::Bool(self.export)),
        ])
    }

    pub fn from_json(j: &J) -> Cut {
        let n = j.get("n").and_then(|v| v.arr()).map(|a| a.iter().map(|q| q.num().unwrap_or(0.0)).collect::<Vec<f64>>()).unwrap_or_default();
        let n = if n.len() == 3 { unit([n[0], n[1], n[2]]) } else { [0.0, 0.0, 1.0] };
        Cut {
            id: j.n("id").unwrap_or(0.0) as u32,
            name: j.s("name").unwrap_or("Shape").to_string(),
            shape: j.get("shape").map(Shape::from_json).unwrap_or_else(|| Shape::new("Shape", shapes::Kind::Rect, 40.0, 40.0)),
            x: j.n("x").unwrap_or(0.0),
            y: j.n("y").unwrap_or(0.0),
            z: j.n("z").unwrap_or(0.0),
            n,
            follow: j.b("follow").unwrap_or(false),
            rot: j.n("rot").unwrap_or(0.0),
            op: CutOp::from_id(j.s("op").unwrap_or("recess")),
            depth: j.n("depth").unwrap_or(3.0).max(0.1),
            draft: j.n("draft").unwrap_or(0.0),
            follow_floor: j.b("follow_floor").unwrap_or(true),
            keep_wall: j.b("keep_wall").unwrap_or(false),
            line: j.b("line").unwrap_or(true),
            rx: j.n("rx").unwrap_or(0.0),
            ry: j.n("ry").unwrap_or(0.0),
            mirror: j.b("mirror").unwrap_or(false),
            flip: false,
            gap: j.n("gap").unwrap_or(0.3).max(0.0),
            plug_h: j.n("plug_h").unwrap_or(0.0),
            visible: j.b("visible").unwrap_or(true),
            locked: j.b("locked").unwrap_or(false),
            export: j.b("export").unwrap_or(true),
        }
    }

    /// The outline in the cutting plane (local mm), counter-clockwise.
    pub fn outline(&self) -> Vec<[f64; 2]> {
        let mut s = self.shape.clone();
        s.depth = 0.0;
        let mut o = s.outline();
        if self.flip {
            for p in o.iter_mut() {
                p[0] = -p[0];
            }
            o.reverse();
        }
        o
    }

    /// The whole flat area of the shape (main outline, added pieces and holes), turned over when `flip` is set.
    pub fn region(&self) -> shapes::Region {
        let mut s = self.shape.clone();
        s.depth = 0.0;
        let r = s.region();
        if self.flip {
            r.flipped()
        } else {
            r
        }
    }

    /// The copy on the other side of the dash (mirror plane x = 0).
    pub fn mirrored(&self) -> Cut {
        let mut c = self.clone();
        c.x = -c.x;
        c.n[0] = -c.n[0];
        c.rot = -c.rot;
        c.ry = -c.ry;
        c.flip = !c.flip;
        c.mirror = false;
        c.name = format!("{} (mirror)", c.name);
        c
    }

    pub fn rot_matrix(&self) -> [[f64; 3]; 3] {
        let (ax, ay, az) = (self.rx.to_radians(), self.ry.to_radians(), self.rot.to_radians());
        let (cx, sx, cy, sy, cz, sz) = (ax.cos(), ax.sin(), ay.cos(), ay.sin(), az.cos(), az.sin());
        let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
        let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
        let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
        let mm = |a: [[f64; 3]; 3], b: [[f64; 3]; 3]| {
            let mut o = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    for k in 0..3 {
                        o[i][j] += a[i][k] * b[k][j];
                    }
                }
            }
            o
        };
        mm(rz, mm(ry, rx))
    }

    /// A 3D part placed freely in the scene: its center at (x, y, z), rotated about X, Y and Z. `light` uses the thumbnail mesh.
    pub fn world_obj(&self, light: bool) -> Option<TriMesh> {
        let sol = if light { self.shape.solid_thumb()? } else { self.shape.solid()? };
        if sol.tris.is_empty() {
            return None;
        }
        let hz = self.shape.depth / 2.0;
        let r = self.rot_matrix();
        let ps = sol
            .pos
            .iter()
            .map(|p| {
                let q = [if self.flip { -p[0] } else { p[0] }, p[1], p[2] - hz];
                [self.x + r[0][0] * q[0] + r[0][1] * q[1] + r[0][2] * q[2], self.y + r[1][0] * q[0] + r[1][1] * q[1] + r[1][2] * q[2], self.z + r[2][0] * q[0] + r[2][1] * q[1] + r[2][2] * q[2]]
            })
            .collect();
        let tris = sol.tris.iter().map(|t| if self.flip { [t[0] as u32, t[2] as u32, t[1] as u32] } else { [t[0] as u32, t[1] as u32, t[2] as u32] }).collect();
        Some(TriMesh { ps, tris })
    }

    pub fn frame(&self) -> Frame {
        let n = if self.follow { unit(self.n) } else { [0.0, 0.0, 1.0] };
        let xa = [1.0, 0.0, 0.0];
        let mut u = sub(xa, scale(n, dot(xa, n)));
        if len(u) < 1e-6 {
            let ya = [0.0, 1.0, 0.0];
            u = sub(ya, scale(n, dot(ya, n)));
        }
        let u = unit(u);
        let v = cross(n, u);
        let (c, s) = (self.rot.to_radians().cos(), self.rot.to_radians().sin());
        let (u2, v2) = (add(scale(u, c), scale(v, s)), add(scale(u, -s), scale(v, c)));
        Frame { o: [self.x, self.y, self.z], u: u2, v: v2, n }
    }

    /// The outline draped on the surface points (world), for drawing the footprint.
    pub fn footprint_world(&self) -> Vec<[f64; 3]> {
        let f = self.frame();
        self.outline().iter().map(|p| f.world(p[0], p[1], 0.0)).collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub o: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
    pub n: [f64; 3],
}

impl Frame {
    pub fn world(&self, x: f64, y: f64, z: f64) -> [f64; 3] {
        [self.o[0] + self.u[0] * x + self.v[0] * y + self.n[0] * z, self.o[1] + self.u[1] * x + self.v[1] * y + self.n[1] * z, self.o[2] + self.u[2] * x + self.v[2] * y + self.n[2] * z]
    }
    pub fn local(&self, p: [f64; 3]) -> [f64; 3] {
        let d = sub(p, self.o);
        [dot(d, self.u), dot(d, self.v), dot(d, self.n)]
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
pub fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = len(a);
    if l < 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        scale(a, 1.0 / l)
    }
}

/// A closed triangle mesh in mm.
#[derive(Clone, Default)]
pub struct TriMesh {
    pub ps: Vec<[f64; 3]>,
    pub tris: Vec<[u32; 3]>,
}

impl TriMesh {
    pub fn from_mesh(m: &Mesh) -> TriMesh {
        // vertices at the same place are merged (shell walls carry their own vertices for shading)
        let mut ids: std::collections::HashMap<(i64, i64, i64), u32> = std::collections::HashMap::new();
        let mut ps: Vec<[f64; 3]> = Vec::new();
        let mut map: Vec<u32> = Vec::with_capacity(m.pos.len() / 3);
        for p in m.pos.chunks(3) {
            let key = ((p[0] * 1000.0).round() as i64, (p[1] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64);
            let id = *ids.entry(key).or_insert_with(|| {
                ps.push([p[0] as f64, p[1] as f64, p[2] as f64]);
                (ps.len() - 1) as u32
            });
            map.push(id);
        }
        TriMesh { ps, tris: m.idx.chunks(3).map(|t| [map[t[0] as usize], map[t[1] as usize], map[t[2] as usize]]).collect() }
    }
    pub fn to_mesh(&self) -> Mesh {
        let mut pos = Vec::with_capacity(self.ps.len() * 3);
        for p in &self.ps {
            pos.extend([p[0] as f32, p[1] as f32, p[2] as f32]);
        }
        Mesh { pos, nrm: Vec::new(), idx: self.tris.iter().flat_map(|t| [t[0], t[1], t[2]]).collect() }
    }
    /// Is this a clean closed solid the boolean engine accepts?
    pub fn check(&self) -> Result<(), String> {
        self.manifold().map(|_| ())
    }
    fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for p in &self.ps {
            for k in 0..3 {
                p[k].to_bits().hash(&mut h);
            }
        }
        self.tris.hash(&mut h);
        h.finish()
    }
    fn manifold(&self) -> Result<Manifold, String> {
        // a boolean's own output is kept, so the next step does not have to rebuild it from raw triangles
        let k = self.key();
        if let Some(m) = KEEP.with(|c| c.borrow().iter().find(|(kk, _)| *kk == k).map(|(_, m)| m.clone())) {
            return Ok(m);
        }
        let pos: Vec<f64> = self.ps.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
        let idx: Vec<usize> = self.tris.iter().flat_map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect();
        Manifold::new(&pos, &idx)
    }
    fn from_manifold(m: &Manifold) -> TriMesh {
        let t = TriMesh::raw_from_manifold(m);
        let k = t.key();
        KEEP.with(|c| {
            let mut c = c.borrow_mut();
            c.push((k, m.clone()));
            if c.len() > 12 {
                c.remove(0);
            }
        });
        t
    }
    fn raw_from_manifold(m: &Manifold) -> TriMesh {
        TriMesh { ps: m.ps.iter().map(|p| [p.x as f64, p.y as f64, p.z as f64]).collect(), tris: m.hs.chunks(3).map(|h| [h[0].tail as u32, h[1].tail as u32, h[2].tail as u32]).collect() }
    }
    pub fn volume(&self) -> f64 {
        let mut v = 0.0;
        for t in &self.tris {
            let (a, b, c) = (self.ps[t[0] as usize], self.ps[t[1] as usize], self.ps[t[2] as usize]);
            v += dot(a, cross(b, c));
        }
        v / 6.0
    }
}

/// Weld coincident corners and drop collapsed or doubled faces, so a boolean's output can feed the next boolean.
fn tidy(m: &TriMesh) -> TriMesh {
    tidy_at(m, 1e-4)
}

fn tidy_at(m: &TriMesh, q: f64) -> TriMesh {
    let mut map: std::collections::HashMap<(i64, i64, i64), u32> = std::collections::HashMap::new();
    let mut ps: Vec<[f64; 3]> = Vec::new();
    let mut remap = Vec::with_capacity(m.ps.len());
    for p in &m.ps {
        let k = ((p[0] / q).round() as i64, (p[1] / q).round() as i64, (p[2] / q).round() as i64);
        let id = *map.entry(k).or_insert_with(|| {
            ps.push(*p);
            (ps.len() - 1) as u32
        });
        remap.push(id);
    }
    let mut seen = std::collections::HashSet::new();
    let mut tris = Vec::new();
    for t in &m.tris {
        let t = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            continue;
        }
        let mut k = t;
        k.sort();
        if seen.insert(k) {
            tris.push(t);
        }
    }
    TriMesh { ps, tris }
}

/// Fallback thickening: the union of copies of the part moved `t` mm in 14 directions.
fn grow_by_shifts(m: &TriMesh, t: f64) -> Result<TriMesh, String> {
    let mut dirs: Vec<[f64; 3]> = vec![[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]];
    let d = 1.0 / 3f64.sqrt();
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                dirs.push([sx * d, sy * d, sz * d]);
            }
        }
    }
    let mut acc = m.clone();
    for d in dirs {
        let sh = TriMesh { ps: m.ps.iter().map(|p| [p[0] + d[0] * t, p[1] + d[1] * t, p[2] + d[2] * t]).collect(), tris: m.tris.clone() };
        acc = boolean(&acc, &sh, OpType::Add)?;
    }
    Ok(acc)
}

/// Grow a closed mesh outwards by `t` mm (each corner moves so every face is `t` away, up to a limit).
fn grow_mesh(m: &TriMesh, t: f64) -> TriMesh {
    let (ps, tris) = (m.ps.clone(), m.tris.iter().filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]).copied().collect::<Vec<_>>());
    let mut inc: Vec<Vec<[f64; 3]>> = vec![Vec::new(); ps.len()];
    for tr in &tris {
        let (a, b, c) = (ps[tr[0] as usize], ps[tr[1] as usize], ps[tr[2] as usize]);
        let nn = cross(sub(b, a), sub(c, a));
        if len(nn) < 1e-12 {
            continue;
        }
        let nn = unit(nn);
        for k in 0..3 {
            let list = &mut inc[tr[k] as usize];
            if !list.iter().any(|q| dot(*q, nn) > 0.9995) {
                list.push(nn);
            }
        }
    }
    let mut out = ps.clone();
    for (i, ns) in inc.iter().enumerate() {
        if ns.is_empty() {
            continue;
        }
        let mut avg = [0.0; 3];
        for n in ns {
            avg = add(avg, *n);
        }
        let avg = unit(avg);
        // least squares: n_i . m = 1 for every incident face normal, pulled gently towards avg
        let lam = 0.08;
        let mut a = [[lam, 0.0, 0.0], [0.0, lam, 0.0], [0.0, 0.0, lam]];
        let mut r = scale(avg, lam);
        for n in ns {
            for x in 0..3 {
                for y in 0..3 {
                    a[x][y] += n[x] * n[y];
                }
                r[x] += n[x];
            }
        }
        let det = |m: [[f64; 3]; 3]| m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        let d = det(a);
        let mut mv = avg;
        if d.abs() > 1e-9 {
            let mut sol = [0.0; 3];
            for c in 0..3 {
                let mut b = a;
                for row in 0..3 {
                    b[row][c] = r[row];
                }
                sol[c] = det(b) / d;
            }
            mv = sol;
        }
        let l = len(mv);
        if l > 2.2 {
            mv = scale(mv, 2.2 / l);
        }
        out[i] = add(ps[i], scale(mv, t));
    }
    TriMesh { ps: out, tris }
}

/// Cut a free-standing 3D part out of the dash (optionally leaving a wall around it) or add it on.
fn apply_3d(body: &mut TriMesh, shell0: &TriMesh, c: &Cut, proj: &Project, thickness: f64, res: &mut CutResult) -> Result<(), String> {
    let obj = c.world_obj(false).ok_or("the part has no solid")?;
    if c.op == CutOp::Cut {
        let mut done = false;
        if c.line {
            let g = grow_mesh(&obj, thickness);
            // a part with fine detail can grow into a tangle; then thicken it by overlaying shifted copies instead
            let g = if g.check().is_ok() { g } else { grow_by_shifts(&obj, thickness).unwrap_or(g) };
            // how far the part reaches below the skin around it
            let (mut x0, mut x1, mut y0, mut y1, mut z0) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN, f64::MAX);
            for p in &obj.ps {
                x0 = x0.min(p[0]);
                x1 = x1.max(p[0]);
                y0 = y0.min(p[1]);
                y1 = y1.max(p[1]);
                z0 = z0.min(p[2]);
            }
            let mut top = f64::MIN;
            for p in &shell0.ps {
                if p[0] >= x0 - 8.0 && p[0] <= x1 + 8.0 && p[1] >= y0 - 8.0 && p[1] <= y1 + 8.0 {
                    top = top.max(p[2]);
                }
            }
            let reach = if top > f64::MIN { (top - z0).max(0.0) } else { 0.0 };
            if reach > 0.0 {
                // The wall is the grown part trimmed to what lies under the dash's surface. First try the solid under the skin
                // (reaches the floor of a deep pocket); if the engine refuses that, the band just under the skin.
                for attempt in 0..2 {
                    let env = if attempt == 0 {
                        TriMesh::from_mesh(&proj.under_skin(z0 - thickness - 2.0))
                    } else {
                        TriMesh::from_mesh(&proj.mesh_off(0.0, (reach + thickness + 3.0).min(160.0)))
                    };
                    let tried = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<TriMesh, String> {
                        let l = boolean(&g, &env, OpType::Intersect)?;
                        { let m = l.to_mesh(); if crate::model::signed_volume(&m.pos, &m.idx) <= 1.0 { return Err("the wall came out empty".into()); } }
                        let b = boolean(body, &l, OpType::Add)?;
                        boolean(&b, &obj, OpType::Subtract)
                    }));
                    if let Ok(Ok(w)) = tried {
                        if w.check().is_ok() {
                            *body = w;
                            done = true;
                            break;
                        }
                    }
                }
                if !done {
                    res.warnings.push(format!("{}: the wall around this cut could not be built cleanly, so it was cut without one", c.name));
                }
            }
        }
        if !done {
            *body = boolean(body, &obj, OpType::Subtract)?;
        }
    } else {
        *body = boolean(body, &obj, OpType::Add)?;
    }
    if body.check().is_err() {
        return Err("the result is not a clean solid after this step".into());
    }
    Ok(())
}

/// New Euler angles (x, y, z degrees; R = Rz Ry Rx) after turning by `delta` radians about a world axis.
pub fn rotate_about_world(e: [f64; 3], axis: usize, delta: f64) -> (f64, f64, f64) {
    let probe = Cut { id: 0, name: String::new(), shape: Shape::new("p", shapes::Kind::Rect, 1.0, 1.0), x: 0.0, y: 0.0, z: 0.0, n: [0.0, 0.0, 1.0], follow: false, rot: e[2], op: CutOp::Cut, depth: 0.0, draft: 0.0, follow_floor: true, keep_wall: false, line: false, rx: e[0], ry: e[1], mirror: false, flip: false, gap: 0.0, plug_h: 0.0, visible: true, locked: false, export: true };
    let r0 = probe.rot_matrix();
    let (s, c) = (delta.sin(), delta.cos());
    let w = match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    };
    let mut m = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                m[i][j] += w[i][k] * r0[k][j];
            }
        }
    }
    let ry = (-m[2][0]).clamp(-1.0, 1.0).asin();
    let (rx, rz) = if ry.cos().abs() > 1e-6 { (m[2][1].atan2(m[2][2]), m[1][0].atan2(m[0][0])) } else { ((-m[1][2]).atan2(m[1][1]), 0.0) };
    (rx.to_degrees(), ry.to_degrees(), rz.to_degrees())
}

/// Every visible cut, plus the mirror image of those that ask for one.
pub fn expanded(cuts: &[Cut]) -> Vec<Cut> {
    expanded_for(cuts, false)
}

/// The cuts that count: the visible ones on screen, or the ones marked for export.
pub fn expanded_for(cuts: &[Cut], export: bool) -> Vec<Cut> {
    let mut v = Vec::new();
    for c in cuts.iter().filter(|c| if export { c.export } else { c.visible }) {
        v.push(c.clone());
        if c.mirror && c.x.abs() > 0.01 {
            v.push(c.mirrored());
        }
    }
    v
}

/// A copy with non-manifold edges split and holes closed, for inputs a boolean refuses as they are.
fn healed(m: &TriMesh) -> TriMesh {
    let (ps, tris) = shapes::repair(m.ps.clone(), m.tris.clone());
    TriMesh { ps, tris }
}

thread_local! {
    static KEEP: std::cell::RefCell<Vec<(u64, Manifold)>> = std::cell::RefCell::new(Vec::new());
}

fn boolean(a: &TriMesh, b: &TriMesh, op: OpType) -> Result<TriMesh, String> {
    let ma = a.manifold().or_else(|_| healed(a).manifold()).map_err(|e| format!("dash: {}", e))?;
    let mb = b.manifold().or_else(|_| healed(b).manifold()).map_err(|e| format!("the part is not a clean closed solid ({})", e))?;
    let r = compute_boolean(&ma, &mb, op).map_err(|e| format!("boolean: {}", e))?;
    Ok(TriMesh::from_manifold(&r))
}

#[derive(Default)]
pub struct CutResult {
    pub body: TriMesh,
    pub plugs: Vec<(String, TriMesh)>,
    pub warnings: Vec<String>,
}

/// The prism of a region through (height, inward offset) layers, in the region's own coordinates. Added pieces are
/// merged in; holes are cut out with the offset turned around so a hole grows where the material shrinks.
pub fn region_prism(r: &shapes::Region, layers: &[(f64, f64)]) -> TriMesh {
    let mut ls: Vec<(f64, f64)> = Vec::new();
    for l in layers {
        if ls.last().map_or(true, |p| l.0 > p.0 + 1e-6) {
            ls.push(*l);
        }
    }
    let mk = |o: &[[f64; 2]], sign: f64, ext: f64| -> TriMesh {
        let mut l2: Vec<(f64, f64)> = ls.iter().map(|l| (l.0, l.1 * sign)).collect();
        if ext > 0.0 && !l2.is_empty() {
            let (first, last) = (l2[0], l2[l2.len() - 1]);
            l2.insert(0, (first.0 - ext, first.1));
            l2.push((last.0 + ext, last.1));
        }
        let sol = shapes::Solid::from_layers(o, &l2, false);
        TriMesh { ps: sol.pos.clone(), tris: sol.tris.iter().map(|t| [t[0] as u32, t[1] as u32, t[2] as u32]).collect() }
    };
    let mut body = mk(r.main(), 1.0, 0.0);
    for o in r.solid.iter().skip(1) {
        if let Ok(m) = boolean(&body, &mk(o, 1.0, 0.0), OpType::Add) {
            body = m;
        }
    }
    for o in &r.holes {
        if let Ok(m) = boolean(&body, &mk(o, -1.0, 1.0), OpType::Subtract) {
            body = m;
        }
    }
    body
}

/// A compound shape extruded for the editor and for export.
pub fn region_solid(r: &shapes::Region, layers: &[(f64, f64)], limited: bool) -> shapes::Solid {
    let m = region_prism(r, layers);
    shapes::Solid { pos: m.ps, tris: m.tris.iter().map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(), limited }
}

/// An extruded copy of a region through the given (local height, inward offset) layers, placed in the frame.
fn prism_layers(f: &Frame, region: &shapes::Region, layers: &[(f64, f64)]) -> TriMesh {
    let m = if region.is_simple() {
        let mut ls: Vec<(f64, f64)> = Vec::new();
        for l in layers {
            if ls.last().map_or(true, |p| l.0 > p.0 + 1e-6) {
                ls.push(*l);
            }
        }
        let sol = shapes::Solid::from_layers(region.main(), &ls, false);
        TriMesh { ps: sol.pos.clone(), tris: sol.tris.iter().map(|t| [t[0] as u32, t[1] as u32, t[2] as u32]).collect() }
    } else {
        region_prism(region, layers)
    };
    TriMesh { ps: m.ps.iter().map(|p| f.world(p[0], p[1], p[2])).collect(), tris: m.tris }
}

/// A cutter: vertical walls above the surface, sloping (a bevel) over the depth `d` below it, then straight again.
fn cutter(f: &Frame, outline: &shapes::Region, z0: f64, z1: f64, d: f64, lean: f64) -> TriMesh {
    let t = lean.max(-60.0).min(60.0).to_radians().tan();
    let o = d * t;
    prism_layers(f, outline, &[(z0, o), (-d, o), (0.0, 0.0), (z1, 0.0)])
}

/// A boss: vertical below the surface, narrowing (or widening) over its height `h`, then straight up to z1.
fn boss(f: &Frame, outline: &shapes::Region, z0: f64, z1: f64, h: f64, lean: f64) -> TriMesh {
    let t = lean.max(-60.0).min(60.0).to_radians().tan();
    let o = h * t;
    prism_layers(f, outline, &[(z0, 0.0), (0.0, 0.0), (h, o), (z1.max(h + 0.01), o)])
}

/// The range of local heights the shell occupies under the shape's footprint.
fn shell_span(shell: &TriMesh, f: &Frame, outline: &shapes::Region) -> (f64, f64) {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in outline.points() {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
    }
    let m = 14.0;
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in &shell.ps {
        let l = f.local(*p);
        if l[0] >= x0 - m && l[0] <= x1 + m && l[1] >= y0 - m && l[1] <= y1 + m {
            lo = lo.min(l[2]);
            hi = hi.max(l[2]);
        }
    }
    if lo > hi {
        (-10.0, 10.0)
    } else {
        (lo, hi)
    }
}

/// Apply every visible cut of the piece in order. None when there is nothing to do.
pub fn compute(proj: &Project, thickness: f64, export: bool) -> Option<Result<CutResult, String>> {
    if !proj.cuts.iter().any(|c| if export { c.export } else { c.visible }) || thickness <= 0.0 {
        return None;
    }
    Some(compute_inner(proj, thickness, export))
}

fn compute_inner(proj: &Project, thickness: f64, export: bool) -> Result<CutResult, String> {
    let shell0 = TriMesh::from_mesh(&proj.mesh_final(thickness).0);
    let mut body = shell0.clone();
    let mut res = CutResult::default();
    for c in expanded_for(&proj.cuts, export).iter() {
        let f = c.frame();
        let outline = c.region();
        if outline.main().len() < 3 {
            continue;
        }
        if c.is3d() {
            let v0 = body.volume();
            let applied = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply_3d(&mut body, &shell0, c, proj, thickness, &mut res)))
                .unwrap_or_else(|_| Err("the geometry engine crashed on this part (details are in errors.log); try moving or turning it slightly".to_string()));
            if let Err(e) = applied {
                res.warnings.push(format!("{}: {}", c.name, e));
            } else if (body.volume() - v0).abs() < 1.0 {
                let what = if c.op == CutOp::Cut { "cuts nothing" } else { "adds nothing" };
                res.warnings.push(format!("{} {}: it does not overlap the {} (move it so it passes through the surface)", c.name, what, if c.op == CutOp::Cut { "pad" } else { "pad or is already inside it" }));
            }
            continue;
        }
        let (lo, hi) = shell_span(&shell0, &f, &outline);
        let keep = c.keep_wall && c.op != CutOp::Cut;
        let d = if keep { c.depth.max(0.1).min(60.0) } else { c.depth.max(0.1).min(thickness) };
        let up = 6.0;
        let r: Result<(), String> = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            match c.op {
                CutOp::Cut => {
                    let cu = cutter(&f, &outline, lo - 3.0, hi + up, thickness, c.draft);
                    body = boolean(&body, &cu, OpType::Subtract)?;
                }
                CutOp::Recess | CutOp::Inlay => {
                    let cu = if c.follow_floor {
                        let wide = cutter(&f, &outline, lo - 3.0, hi + up, d, c.draft);
                        let layer = TriMesh::from_mesh(&proj.mesh_off(0.4, d + 0.4));
                        boolean(&wide, &layer, OpType::Intersect)?
                    } else {
                        cutter(&f, &outline, -d, hi + up, d, c.draft)
                    };
                    body = boolean(&body, &cu, OpType::Subtract)?;
                    if keep {
                        // push the inside of the wall in by the same amount so the wall stays `thickness` thick
                        let top = thickness.max(d);
                        let t = thickness.min(d);
                        let lean = c.draft.max(-60.0).min(60.0).to_radians().tan();
                        let o = d * lean;
                        let z0 = lo - d - thickness - 4.0;
                        let off0 = o + (-d - z0) * lean.max(0.0);
                        let pad_prism = if c.follow_floor {
                            prism_layers(&f, &outline, &[(z0, off0), (-d, o), (hi + up, o)])
                        } else {
                            prism_layers(&f, &outline, &[(-d - thickness, o + thickness * lean.max(0.0)), (-d, o)])
                        };
                        let slab = if c.follow_floor {
                            TriMesh::from_mesh(&proj.mesh_off(-top + 0.3, t + 0.3))
                        } else {
                            TriMesh::from_mesh(&proj.mesh_off(0.0, thickness + d + 4.0))
                        };
                        let pad = boolean(&pad_prism, &slab, OpType::Intersect)?;
                        body = boolean(&body, &pad, OpType::Add)?;
                    }
                    if c.op == CutOp::Inlay {
                        let inner = if c.gap > 1e-6 { outline.inset(c.gap) } else { outline.clone() };
                        let wide = cutter(&f, &inner, lo - 3.0, hi + up + c.plug_h.max(0.0), d, c.draft);
                        let layer = TriMesh::from_mesh(&proj.mesh_off(c.plug_h, d + c.plug_h));
                        let plug = boolean(&wide, &layer, OpType::Intersect)?;
                        res.plugs.push((c.name.clone(), plug));
                    }
                }
                CutOp::Raise => {
                    let h = c.depth.max(0.1);
                    let b = if c.follow_floor {
                        let wide = boss(&f, &outline, lo - 3.0, hi + h + up, h, c.draft);
                        let layer = TriMesh::from_mesh(&proj.mesh_off(h, h + 2.0));
                        boolean(&wide, &layer, OpType::Intersect)?
                    } else {
                        boss(&f, &outline, lo - 2.0, h, h, c.draft)
                    };
                    body = boolean(&body, &b, OpType::Add)?;
                    if keep {
                        // hollow the underside of the pad so it stays a wall `thickness` thick
                        let inset = outline.inset(thickness);
                        if inset.main().len() >= 3 {
                            let cav = prism_layers(&f, &inset, &[(lo - 4.0, 0.0), (h - thickness, 0.0)]);
                            let slab = TriMesh::from_mesh(&proj.mesh_off(h - thickness, h + 3.0));
                            let cav = boolean(&cav, &slab, OpType::Intersect)?;
                            body = boolean(&body, &cav, OpType::Subtract)?;
                        }
                    }
                }
            }
            Ok(())
        }))
        .unwrap_or_else(|_| Err("the geometry engine crashed on this shape (details are in errors.log)".to_string()));
        if let Err(e) = r {
            res.warnings.push(format!("{}: {}", c.name, e));
        }
    }
    res.body = body;
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine;
    use crate::model::{open_edges, signed_volume};

    fn proj() -> Project {
        engine::default_project()
    }

    fn cut(op: CutOp, depth: f64) -> Cut {
        let mut sh = Shape::new("slot", shapes::Kind::Rect, 60.0, 20.0);
        sh.radius = 6.0;
        let p = proj();
        // a point on the top surface near the center
        let m = p.mesh(0.0);
        let mut best = (f64::MAX, [0.0; 3]);
        for q in m.pos.chunks(3) {
            let d = (q[0] as f64).abs() + (q[1] as f64 - 150.0).abs();
            if d < best.0 {
                best = (d, [q[0] as f64, q[1] as f64, q[2] as f64]);
            }
        }
        let mut c = Cut::new(1, &sh, best.1, [0.0, 0.0, 1.0]);
        c.op = op;
        c.depth = depth;
        c.keep_wall = false;
        c
    }

    fn run(op: CutOp, depth: f64) -> (f64, f64, CutResult) {
        let mut p = proj();
        let base = p.mesh(6.0);
        let v0 = signed_volume(&base.pos, &base.idx);
        p.cuts.push(cut(op, depth));
        let t = std::time::Instant::now();
        let r = compute(&p, 6.0, false).unwrap().unwrap();
        println!("{:?} took {:?}, warnings {:?}", op, t.elapsed(), r.warnings);
        let m = r.body.to_mesh();
        println!("{:?}: tris {} open edges {} volume {:.0} -> {:.0}", op, m.idx.len() / 3, open_edges(&m.idx), v0, signed_volume(&m.pos, &m.idx));
        assert!(r.warnings.is_empty());
        assert_eq!(open_edges(&m.idx), 0);
        (v0, signed_volume(&m.pos, &m.idx), r)
    }

    #[test]
    fn cut_with_a_hole_piece_leaves_the_island() {
        let mut p = proj();
        let base = p.mesh(6.0);
        let v0 = signed_volume(&base.pos, &base.idx);
        let mut plain = cut(CutOp::Cut, 6.0);
        plain.shape = Shape::new("slot", shapes::Kind::Rect, 60.0, 30.0);
        p.cuts.push(plain.clone());
        let r = compute(&p, 6.0, false).unwrap().unwrap();
        let m = r.body.to_mesh();
        let v_plain = signed_volume(&m.pos, &m.idx);
        // the same cut with a round hole piece in it: the island in the middle stays
        let mut ring = plain.clone();
        let mut hole = shapes::Part::new(shapes::Kind::Ellipse, true);
        hole.sh.w = 16.0;
        hole.sh.h = 16.0;
        ring.shape.parts.push(hole);
        p.cuts.clear();
        p.cuts.push(ring);
        let r = compute(&p, 6.0, false).unwrap().unwrap();
        println!("warnings {:?}", r.warnings);
        assert!(r.warnings.is_empty());
        let m = r.body.to_mesh();
        assert_eq!(open_edges(&m.idx), 0);
        let v_ring = signed_volume(&m.pos, &m.idx);
        println!("dash {:.0}, plain cut {:.0}, ring cut {:.0}", v0, v_plain, v_ring);
        assert!(v_ring > v_plain + 300.0, "the island is missing");
    }

    #[test]
    fn cut_through_removes_material() {
        let (v0, v1, _) = run(CutOp::Cut, 6.0);
        assert!(v1 < v0 - 3000.0);
    }

    fn part3d(op: CutOp, sink: f64, line: bool) -> (f64, f64) {
        let mut sh = Shape::new("box", shapes::Kind::Rect, 50.0, 25.0);
        sh.radius = 5.0;
        sh.depth = 14.0;
        let mut p = proj();
        let base = p.mesh(6.0);
        let v0 = signed_volume(&base.pos, &base.idx);
        let mut c = cut(CutOp::Cut, 1.0);
        c.shape = sh;
        c.op = op;
        // c.z is the surface height at the click; the part's center goes where the test wants it
        c.z = if op == CutOp::Cut { c.z - 7.0 } else { c.z + 6.0 };
        let _ = sink;
        c.line = line;
        p.cuts.push(c);
        let r = compute(&p, 6.0, false).unwrap().unwrap();
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        let m = r.body.to_mesh();
        println!("3d {:?} sink {} line {}: open {} vol {:.0} -> {:.0}", op, sink, line, open_edges(&m.idx), v0, signed_volume(&m.pos, &m.idx));
        assert_eq!(open_edges(&m.idx), 0);
        (v0, signed_volume(&m.pos, &m.idx))
    }

    #[test]
    fn open_stl_gets_its_holes_closed() {
        // a box missing its top
        let v = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 20.0, 0.0], [0.0, 20.0, 0.0], [0.0, 0.0, 30.0], [10.0, 0.0, 30.0], [10.0, 20.0, 30.0], [0.0, 20.0, 30.0]];
        let f = [[0, 2, 1], [0, 3, 2], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
        let mut txt = String::from("solid t\n");
        for t in f {
            txt.push_str("facet normal 0 0 0\nouter loop\n");
            for k in t {
                txt.push_str(&format!("vertex {} {} {}\n", v[k][0], v[k][1], v[k][2]));
            }
            txt.push_str("endloop\nendfacet\n");
        }
        let m = shapes::mesh_from_stl(txt.as_bytes()).unwrap();
        let tm = TriMesh { ps: m.pos.clone(), tris: m.tris.clone() };
        assert!(tm.check().is_ok(), "{:?}", tm.check().err());
        assert!(tm.volume() > 5000.0);
    }

    #[test]
    fn gizmo_rotation_round_trips() {
        let (rx, ry, rz) = rotate_about_world([20.0, 30.0, 40.0], 2, 90f64.to_radians());
        // turning 90 about world Z leaves rx, ry alone only for a pure-Z start; here check the composed matrix directly
        let mk = |a: f64, b: f64, c: f64| {
            let mut s = Shape::new("p", shapes::Kind::Rect, 1.0, 1.0);
            s.depth = 1.0;
            let mut cu = cut(CutOp::Cut, 1.0);
            cu.shape = s;
            cu.rx = a;
            cu.ry = b;
            cu.rot = c;
            cu.rot_matrix()
        };
        let got = mk(rx, ry, rz);
        let r0 = mk(20.0, 30.0, 40.0);
        let (s, c) = (1f64, 0f64);
        let w = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
        for i in 0..3 {
            for j in 0..3 {
                let mut e = 0.0;
                for k in 0..3 {
                    e += w[i][k] * r0[k][j];
                }
                assert!((e - got[i][j]).abs() < 1e-9, "{} {} {} {}", i, j, e, got[i][j]);
            }
        }
    }

    #[test]
    fn mirror_makes_a_second_cut() {
        let run = |mirror: bool| {
            let mut p = proj();
            let mut c = cut(CutOp::Cut, 1.0);
            let mut sh = Shape::new("box", shapes::Kind::Rect, 40.0, 20.0);
            sh.depth = 14.0;
            c.shape = sh;
            c.x = 120.0;
            c.z -= 7.0;
            c.line = false;
            c.mirror = mirror;
            p.cuts.push(c);
            let r = compute(&p, 6.0, false).unwrap().unwrap();
            assert!(r.warnings.is_empty(), "{:?}", r.warnings);
            let m = r.body.to_mesh();
            assert_eq!(open_edges(&m.idx), 0);
            signed_volume(&m.pos, &m.idx)
        };
        let (one, two) = (run(false), run(true));
        println!("mirror: {:.0} vs {:.0}", one, two);
        assert!(two < one - 1000.0);
    }

    #[test]
    fn solid_parts_cut_and_add() {
        let (v0, bare) = part3d(CutOp::Cut, 14.0, false);
        let (_, lined) = part3d(CutOp::Cut, 14.0, true);
        assert!(bare < v0);
        assert!(lined > bare + 500.0, "the wall around the cut should add material");
        let (_, added) = part3d(CutOp::Raise, 1.0, true);
        assert!(added > v0);
    }

    #[test]
    fn touching_shells_and_doubled_faces_are_repaired() {
        // two unit-ish cubes that touch along one edge, one of them with reversed winding, plus a doubled face
        let cube = |o: [f64; 3], flip: bool| {
            let v = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0, 10.0], [10.0, 0.0, 10.0], [10.0, 10.0, 10.0], [0.0, 10.0, 10.0]];
            let f = [[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
            let mut out = Vec::new();
            for t in f {
                let mut t = t;
                if flip {
                    t.swap(1, 2);
                }
                for k in t {
                    out.push([v[k][0] + o[0], v[k][1] + o[1], v[k][2] + o[2]]);
                }
            }
            out
        };
        let mut all = cube([0.0, 0.0, 0.0], false);
        all.extend(cube([10.0, 10.0, 0.0], true));
        let dup: Vec<[f64; 3]> = all[0..3].to_vec();
        all.extend(dup);
        let mut txt = String::from("solid t\n");
        for t in all.chunks(3) {
            txt.push_str("facet normal 0 0 0\nouter loop\n");
            for p in t {
                txt.push_str(&format!("vertex {} {} {}\n", p[0], p[1], p[2]));
            }
            txt.push_str("endloop\nendfacet\n");
        }
        let m = shapes::mesh_from_stl(txt.as_bytes()).unwrap();
        let tm = TriMesh { ps: m.pos.iter().copied().collect(), tris: m.tris.clone() };
        let mut ec: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        for t in &tm.tris {
            for k in 0..3 {
                *ec.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        println!("verts {} tris {} max directed use {}", tm.ps.len(), tm.tris.len(), ec.values().max().unwrap());
        assert!(tm.manifold().is_ok(), "{:?}", tm.manifold().err());
        assert!(tm.volume() > 1900.0 && tm.volume() < 2100.0, "{}", tm.volume());
    }

    #[test]
    fn stl_import_round_trip() {
        // a 10 x 20 x 30 box as ASCII STL
        let v = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 20.0, 0.0], [0.0, 20.0, 0.0], [0.0, 0.0, 30.0], [10.0, 0.0, 30.0], [10.0, 20.0, 30.0], [0.0, 20.0, 30.0]];
        let f = [[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
        let mut txt = String::from("solid t\n");
        for t in f {
            txt.push_str("facet normal 0 0 0\nouter loop\n");
            for k in t {
                txt.push_str(&format!("vertex {} {} {}\n", v[k][0], v[k][1], v[k][2]));
            }
            txt.push_str("endloop\nendfacet\n");
        }
        let m = shapes::mesh_from_stl(txt.as_bytes()).unwrap();
        assert_eq!(m.pos.len(), 8);
        assert!((m.w0 - 10.0).abs() < 1e-9 && (m.h0 - 20.0).abs() < 1e-9 && (m.d0 - 30.0).abs() < 1e-9);
        let hull = shapes::mesh_hull(&m, 64);
        assert_eq!(hull.len(), 4);
    }

    #[test]
    fn keep_wall_recess_does_not_break_through() {
        for (op, d) in [(CutOp::Recess, 8.0), (CutOp::Recess, 3.0), (CutOp::Raise, 5.0)] {
            let mut p = proj();
            let base = p.mesh(6.0);
            let v0 = signed_volume(&base.pos, &base.idx);
            let mut c = cut(op, d);
            c.keep_wall = true;
            p.cuts.push(c);
            let r = compute(&p, 6.0, false).unwrap().unwrap();
            assert!(r.warnings.is_empty(), "{:?}", r.warnings);
            let m = r.body.to_mesh();
            let v1 = signed_volume(&m.pos, &m.idx);
            println!("keep {:?} {}: open {} vol {:.0} -> {:.0}", op, d, open_edges(&m.idx), v0, v1);
            assert_eq!(open_edges(&m.idx), 0);
        }
    }

    #[test]
    fn recess_removes_less_than_cut() {
        let (v0, v1, _) = run(CutOp::Recess, 2.0);
        let (_, vc, _) = run(CutOp::Cut, 6.0);
        assert!(v1 < v0 && v1 > vc);
    }

    #[test]
    fn raise_adds_material() {
        let (v0, v1, _) = run(CutOp::Raise, 3.0);
        assert!(v1 > v0 + 1000.0);
    }

    #[test]
    fn inlay_makes_a_plug_that_fills_the_pocket() {
        let (v0, v1, r) = run(CutOp::Inlay, 3.0);
        assert_eq!(r.plugs.len(), 1);
        let pm = r.plugs[0].1.to_mesh();
        assert_eq!(open_edges(&pm.idx), 0);
        let pv = signed_volume(&pm.pos, &pm.idx);
        println!("plug volume {:.0}, removed {:.0}", pv, v0 - v1);
        assert!(pv > 500.0 && (pv - (v0 - v1)).abs() < 0.25 * (v0 - v1));
    }

    #[test]
    fn bevelled_walls_work() {
        let mut p = proj();
        let mut c = cut(CutOp::Recess, 3.0);
        c.draft = 30.0;
        p.cuts.push(c);
        let r = compute(&p, 6.0, false).unwrap().unwrap();
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        let m = r.body.to_mesh();
        assert_eq!(open_edges(&m.idx), 0);
    }
}

#[cfg(test)]
mod export_flag_tests {
    use super::*;
    #[test]
    fn hidden_cut_still_exports_and_vice_versa() {
        let mut a = Cut::from_json(&J::Obj(vec![]));
        a.visible = false;
        a.export = true;
        let mut b = Cut::from_json(&J::Obj(vec![]));
        b.visible = true;
        b.export = false;
        let v = vec![a, b];
        assert_eq!(expanded_for(&v, true).len(), 1);
        assert!(expanded_for(&v, true)[0].export);
        assert_eq!(expanded_for(&v, false).len(), 1);
        assert!(expanded_for(&v, false)[0].visible);
    }
}
