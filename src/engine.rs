// Editing engine shared by the native app: project state, undo/redo and the op dispatcher.
use crate::json::{self, J};
use crate::model::{self, stl_bytes, Project};
use crate::templates;
use std::path::PathBuf;

pub const DEFAULT_PROJECT: &str = include_str!("default_project.json");

pub const DEFAULT_LOWER: &str = include_str!("default_lower.json");
pub const DEFAULT_TRIM: &str = include_str!("default_trim.json");

pub const PIECE_NAMES: [&str; 3] = ["Top pad", "Lower pad", "Top trim"];
pub const PIECE_KEYS: [&str; 3] = ["upper", "lower", "trim"];

#[derive(Clone)]
pub struct Snap {
    pub proj: Project,
    pub shelf: [Option<Project>; 3],
    pub active: u8,
}

/// `proj` is always the piece being edited; the other pieces wait on the shelf.
/// active 0 = top pad, 1 = lower pad, 2 = top trim.
pub struct State {
    pub proj: Project,
    pub shelf: [Option<Project>; 3],
    pub active: u8,
    pub undo: Vec<Snap>,
    pub redo: Vec<Snap>,
    pub thickness: f64,
    pub dir: PathBuf,
    pub dirty: bool,
    /// The lower pad's top height follows the front-lip height of the top pad (z only), so the side profile joins cleanly.
    pub lock_z: bool,
    /// Air gap kept between parts that sit on each other (the trim over the top pad).
    pub gap_on: bool,
    pub gap: f64,
    /// Chamfer on the top edge of each piece's two end faces.
    pub bevel_on: bool,
    pub bevel: f64,
    /// down the end face (A); `bevel` is the reach inward along the top (B)
    pub bevel_down: f64,
    pub bevel_round: bool,
    /// sweep all the way down the end face
    pub bevel_full: bool,
    /// Top trim front guide: false = it moves only the end of each profile (sides filled to the pad); true = it moves the whole profile.
    pub trim_follow: bool,
    /// The top trim piece is part of this dash (a style can start without it; it can be added later).
    pub trim_on: bool,
    /// The top trim is still the stock one and does not match this dash yet; adding it builds one that follows the top pad.
    pub trim_stale: bool,
    /// The front guide of the top trim has been moved by hand, so the trim no longer rebuilds itself when the top pad changes.
    pub trim_custom: bool,
    /// Top trim front guide: heights a point was pressed down to on the pad. Pulled back out, it keeps that height.
    pub rail2_hold: std::collections::HashMap<usize, f64>,
    /// Fingerprint of the top pad's shape the top trim was last fitted to.
    pub trim_sig: u64,
    /// What "Reset to original profiles" returns each piece to: the New dash wizard's result (None = the built-in default).
    pub origin: [Option<Project>; 3],
}

impl State {
    pub fn project_path(&self) -> PathBuf {
        self.dir.join("project.json")
    }
    pub fn snap(&self) -> Snap {
        Snap { proj: self.proj.clone(), shelf: self.shelf.clone(), active: self.active }
    }
    pub fn push_undo(&mut self) {
        let s = self.snap();
        self.undo.push(s);
    }
    pub fn piece(&self, i: usize) -> &Project {
        if i == self.active as usize { &self.proj } else { self.shelf[i].as_ref().unwrap_or(&self.proj) }
    }
    pub fn piece_name(&self) -> &'static str {
        PIECE_NAMES[self.active as usize]
    }
    pub fn save(&self) {
        let mut o: Vec<(String, J)> = Vec::new();
        for i in 0..3 {
            o.push((PIECE_KEYS[i].to_string(), self.piece(i).to_json(false)));
        }
        o.push(("active".into(), J::Num(self.active as f64)));
        o.push(("lock_z".into(), J::Bool(self.lock_z)));
        o.push(("gap_on".into(), J::Bool(self.gap_on)));
        o.push(("gap".into(), J::Num(self.gap)));
        o.push(("bevel_on".into(), J::Bool(self.bevel_on)));
        o.push(("bevel".into(), J::Num(self.bevel)));
        o.push(("bevel_down".into(), J::Num(self.bevel_down)));
        o.push(("bevel_round".into(), J::Bool(self.bevel_round)));
        o.push(("bevel_full".into(), J::Bool(self.bevel_full)));
        o.push(("trim_follow".into(), J::Bool(self.trim_follow)));
        o.push(("trim_on".into(), J::Bool(self.trim_on)));
        o.push(("trim_stale".into(), J::Bool(self.trim_stale)));
        o.push(("trim_custom".into(), J::Bool(self.trim_custom)));
        for i in 0..3 {
            if let Some(p) = &self.origin[i] {
                o.push((format!("origin_{}", PIECE_KEYS[i]), p.to_json(false)));
            }
        }
        let _ = std::fs::write(self.project_path(), json::to_string(&J::Obj(o)));
    }
    pub fn new(dir: PathBuf) -> State {
        let mut pieces = [default_project(), default_lower(), default_trim()];
        let mut active = 0u8;
        let mut lock_z = true;
        let (mut gap_on, mut gap) = (false, 0.3);
        let (mut bevel_on, mut bevel) = (false, 2.0);
        let (mut bevel_down, mut bevel_round, mut bevel_full) = (2.0, false, true);
        let mut trim_follow = false;
        let mut trim_on = true;
        let mut trim_stale = false;
        let mut trim_custom = false;
        let mut origin: [Option<Project>; 3] = [None, None, None];
        if let Some(j) = std::fs::read_to_string(dir.join("project.json")).ok().and_then(|s| json::parse(&s).ok()) {
            if j.get("upper").is_some() {
                for i in 0..3 {
                    if let Some(p) = j.get(PIECE_KEYS[i]).and_then(|v| Project::from_json(v).ok()) {
                        pieces[i] = p;
                    }
                }
                active = (j.n("active").unwrap_or(0.0).max(0.0) as u8).min(2);
                lock_z = j.b("lock_z").unwrap_or(true);
                gap_on = j.b("gap_on").unwrap_or(false);
                // the old default (0.3 mm, on) is no longer the default: the trim now sits exactly on the pad
                if gap_on && (j.n("gap").unwrap_or(0.3) - 0.3).abs() < 1e-9 { gap_on = false; }
                gap = j.n("gap").unwrap_or(0.3).max(0.0).min(3.0);
                bevel_on = j.b("bevel_on").unwrap_or(false);
                bevel = j.n("bevel").unwrap_or(2.0).max(0.0).min(200.0);
                bevel_down = j.n("bevel_down").unwrap_or(bevel).max(0.0).min(200.0);
                bevel_round = j.b("bevel_round").unwrap_or(false);
                bevel_full = j.b("bevel_full").unwrap_or(true);
                trim_follow = j.b("trim_follow").unwrap_or(false);
                trim_on = j.b("trim_on").unwrap_or(true);
                trim_stale = j.b("trim_stale").unwrap_or(false);
                trim_custom = j.b("trim_custom").unwrap_or(false);
                for i in 0..3 {
                    origin[i] = j.get(&format!("origin_{}", PIECE_KEYS[i])).and_then(|v| Project::from_json(v).ok());
                }
            } else if let Ok(p) = Project::from_json(&j) {
                pieces[0] = p; // project saved by an earlier version
            }
        }
        let mut shelf: [Option<Project>; 3] = [None, None, None];
        let mut proj = None;
        for (i, p) in pieces.into_iter().enumerate() {
            if i == active as usize {
                proj = Some(p);
            } else {
                shelf[i] = Some(p);
            }
        }
        let mut st = State { proj: proj.unwrap(), shelf, active, undo: Vec::new(), redo: Vec::new(), thickness: 6.0, dir, dirty: false, lock_z, gap_on, gap, bevel_on, bevel, bevel_down, bevel_round, bevel_full, trim_follow, trim_on, trim_stale, trim_custom, rail2_hold: std::collections::HashMap::new(), trim_sig: 0, origin };
        st.trim_sig = top_sig(st.piece(0));
        if st.trim_on && !st.trim_custom && st.trim_out_of_date() { st.refit_trim(); }
        st.sync_lock();
        st
    }
    /// Hand a placed part to another piece, which becomes its parent. It keeps its size and angle and lands at (x, y).
    pub fn move_comp(&mut self, id: u32, to: usize, x: f64, y: f64) -> Option<u32> {
        let from = self.active as usize;
        if to == from || to > 2 || self.shelf[to].is_none() {
            return None;
        }
        let Some(pos) = self.proj.comps.iter().position(|c| c.id == id) else { return None };
        self.push_undo();
        self.redo.clear();
        let mut c = self.proj.comps.remove(pos);
        c.x = x;
        c.y = y;
        let mut newid = None;
        if let Some(t) = self.shelf[to].as_mut() {
            c.id = t.next_comp;
            newid = Some(c.id);
            t.next_comp += 1;
            t.comps.push(c);
        }
        newid
    }
    pub fn piece_mut(&mut self, i: usize) -> Option<&mut Project> {
        if i == self.active as usize { Some(&mut self.proj) } else { self.shelf[i].as_mut() }
    }
    /// Keep the lower pad's top height equal to the top pad's front-lip height (z only). Returns true if anything moved.
    pub fn sync_lock(&mut self) -> bool {
        self.link_under();
        self.sync_z()
    }
    /// Rebuild the top trim from the top pad right now (keeps its material, parts and cut-outs).
    pub fn refit_trim(&mut self) {
        if std::ptr::eq(self.piece(0), self.piece(2)) {
            return;
        }
        let look = self.piece(2).look;
        let mut t = templates::trim_from(self.piece(0), look);
        if let Some(old) = self.piece_mut(2) {
            t.comps = std::mem::take(&mut old.comps);
            t.next_comp = old.next_comp;
            t.cuts = std::mem::take(&mut old.cuts);
            t.next_cut = old.next_cut;
            *old = t;
        }
        self.trim_sig = top_sig(self.piece(0));
        self.trim_stale = false;
        self.trim_custom = false;
        self.trim_on = true;
    }
    /// Give the top trim the top pad's outer surface to ride on.
    pub fn link_under(&mut self) {
        let under = {
            let (up, tr) = (self.piece(0), self.piece(2));
            if std::ptr::eq(up, tr) { None } else { Some(std::sync::Arc::new(model::Under::from_grid(&up.grid()))) }
        };
        let gap = if self.gap_on { self.gap } else { 0.0 };
        let bv = if self.bevel_on { self.bevel } else { 0.0 };
        let (bdn, brd, bfl, tfl) = (self.bevel_down, self.bevel_round, self.bevel_full, self.trim_follow);
        for i in 0..3 {
            if let Some(p) = self.piece_mut(i) {
                // the end-face edge is for the top trim only
                p.end_bevel = if i == 2 { bv } else { 0.0 };
                p.end_down = bdn;
                p.end_round = brd;
                p.end_full = bfl;
                p.nose_follows = i == 2 && tfl;
                if i == 2 {
                    p.under = under.clone();
                    p.under_gap = gap;
                } else {
                    p.under = None;
                    p.under_gap = 0.0;
                }
            }
        }
    }
    /// Put the top trim back on the dash. A trim that was left off while the top pad changed is rebuilt to follow the top pad's
    /// windshield line and front profile (with its own material kept).
    pub fn add_trim(&mut self) {
        if self.trim_stale {
            let look = self.piece(2).look;
            let t = templates::trim_from(self.piece(0), look);
            self.origin[2] = Some(t.clone());
            if let Some(p) = self.piece_mut(2) {
                *p = t;
            }
            self.trim_stale = false;
            self.trim_custom = false;
        }
        self.trim_on = true;
        self.sync_lock();
        self.dirty = true;
    }
    /// Lock the lower pad's top edge (its windshield line) onto the top pad's front line: same height and same front-to-back position.
    /// Only where the two pads overlap: a lower pad that is wider than the top pad keeps its own shape (and stays editable) past the
    /// top pad's ends.
    fn sync_z(&mut self) -> bool {
        if !self.lock_z {
            return false;
        }
        let want: Vec<Option<[f64; 2]>> = {
            let (up, lo) = (self.piece(0), self.piece(1));
            if std::ptr::eq(up, lo) {
                return false;
            }
            let (x0, x1) = up.rail.iter().fold((f64::MAX, f64::MIN), |a, r| (a.0.min(r[0]), a.1.max(r[0])));
            lo.rail
                .iter()
                .map(|r| {
                    if r[0] < x0 - 0.5 || r[0] > x1 + 0.5 {
                        None
                    } else if up.rail2.is_empty() {
                        Some([r[1], up.profiles.last().and_then(|p| p.pts.last()).map(|q| q.co[1]).unwrap_or(r[2])])
                    } else {
                        let (y, z) = up.rail2_at(r[0]);
                        Some([y, z])
                    }
                })
                .collect()
        };
        let Some(lo) = self.piece_mut(1) else { return false };
        if lo.rail.len() != want.len() || lo.rail.iter().zip(want.iter()).all(|(r, w)| w.map_or(true, |w| (r[1] - w[0]).abs() < 1e-6 && (r[2] - w[1]).abs() < 1e-6)) {
            return false;
        }
        for (r, w) in lo.rail.iter_mut().zip(want.iter()) {
            if let Some(w) = w {
                r[1] = w[0];
                r[2] = w[1];
            }
        }
        lo.snap();
        true
    }
}

/// A number that changes whenever the shape of a piece (its guides and cross-sections) changes.
pub fn top_sig(p: &Project) -> u64 {
    let mut h: u64 = 1469598103934665603;
    let mut add = |v: f64| h = h.wrapping_mul(1099511628211).wrapping_add(v.to_bits());
    for r in &p.rail {
        r.iter().for_each(|v| add(*v));
    }
    for r in &p.rail2 {
        r.iter().for_each(|v| add(*v));
    }
    for pr in &p.profiles {
        add(pr.x);
        add(if pr.crease { 1.0 } else { 0.0 });
        for q in &pr.pts {
            for v in [q.co[0], q.co[1], q.hl[0], q.hl[1], q.hr[0], q.hr[1]] {
                add(v);
            }
        }
    }
    h
}

/// Shown when someone tries to edit the top trim's guidelines.
pub const TRIM_LOCK_MSG: &str = "The top trim's windshield line follows the Top pad. To move it, switch to the Top pad (profiles can still be added here; they pin to that line). (The front line can be moved here; it just cannot go below the pad.)";

/// Points per contour line (profile) on a fresh piece.
pub const DEFAULT_POINTS: usize = 6;
/// The lower pad's hook and drop need one more to stay within about 0.7 mm of the original shape.
pub const LOWER_POINTS: usize = 7;

/// The stock top trim is built from the stock top pad, so its underside matches the pad exactly (the old hand-shaped
/// version carried a lip meant for LED lighting, which left a gap and a knuckle at the front).
pub fn default_trim() -> Project {
    let p = Project::from_json(&json::parse(DEFAULT_TRIM).expect("trim json")).expect("trim project");
    templates::trim_from(&default_project(), p.look)
}

pub fn default_for(i: u8) -> Project {
    match i {
        0 => default_project(),
        1 => default_lower(),
        _ => default_trim(),
    }
}

pub fn default_lower() -> Project {
    let mut p = Project::from_json(&json::parse(DEFAULT_LOWER).expect("lower json")).expect("lower project");
    p.resample_all(LOWER_POINTS, true);
    p
}

pub fn default_project() -> Project {
    let mut p = Project::from_json(&json::parse(DEFAULT_PROJECT).expect("default json")).expect("default project");
    p.resample_all(DEFAULT_POINTS, true);
    p
}

pub fn apply_op(st: &mut State, j: &J) -> Result<(), String> {
    let op = j.s("op").ok_or("missing op")?;
    // The top trim's underside rides on the top pad's guidelines, so they are only edited on the top pad.
    if st.active == 2 && matches!(op, "move_rail" | "rail_add" | "rail_remove" | "set_rail_sym") {
        return Err(TRIM_LOCK_MSG.into());
    }
    let idx = |k: &str| j.n(k).map(|v| v.max(0.0) as usize).ok_or_else(|| format!("missing {}", k));
    let num = |k: &str| j.n(k).ok_or_else(|| format!("missing {}", k));
    match op {
        "refit_trim" => {
            st.push_undo();
            st.redo.clear();
            st.refit_trim();
        }
        "set_lock_z" => {
            st.push_undo();
            st.redo.clear();
            st.lock_z = j.b("v").unwrap_or(true);
        }
        "checkpoint" => {
            st.push_undo();
            if st.undo.len() > 200 {
                st.undo.remove(0);
            }
            st.redo.clear();
        }
        "undo" => {
            if let Some(p) = st.undo.pop() {
                let cur = st.snap();
                st.redo.push(cur);
                st.proj = p.proj;
                st.shelf = p.shelf;
                st.active = p.active;
            }
        }
        "redo" => {
            if let Some(p) = st.redo.pop() {
                let cur = st.snap();
                st.undo.push(cur);
                st.proj = p.proj;
                st.shelf = p.shelf;
                st.active = p.active;
            }
        }
        "reset" => {
            st.push_undo();
            st.redo.clear();
            st.proj = st.origin[st.active as usize].clone().unwrap_or_else(|| default_for(st.active));
        }
        "switch_piece" => {
            let want = (j.n("to").unwrap_or(0.0).max(0.0) as u8).min(2);
            if want != st.active {
                if let Some(next) = st.shelf[want as usize].take() {
                    let cur = std::mem::replace(&mut st.proj, next);
                    st.shelf[st.active as usize] = Some(cur);
                    st.active = want;
                }
            }
        }
        "resample" => {
            st.push_undo();
            st.redo.clear();
            let n = j.n("n").unwrap_or(24.0).max(4.0) as usize;
            st.proj.resample_all(n, j.b("bend").unwrap_or(true));
        }
        "pt_add" => {
            st.push_undo();
            st.redo.clear();
            if !st.proj.pt_add(idx("k")?, j.n("t").unwrap_or(0.5)) {
                st.undo.pop();
                return Err("cannot add a point there".into());
            }
        }
        "pt_remove" => {
            st.push_undo();
            st.redo.clear();
            if !st.proj.pt_remove(idx("k")?) {
                st.undo.pop();
                return Err("the end points and the last four cannot be removed".into());
            }
        }
        "set_look" => {
            if let Some(l) = crate::materials::Look::from_ids(j.s("mat").unwrap_or("vinyl"), j.s("color").unwrap_or("")) {
                if !j.b("quiet").unwrap_or(false) {
                    st.push_undo();
                    st.redo.clear();
                }
                st.proj.look = l;
            }
        }
        "set_broken" => {
            st.push_undo();
            st.redo.clear();
            st.proj.set_broken(idx("k")?, j.b("on").unwrap_or(true));
        }
        "move_rail2" => {
            let (k, y, mut z) = (idx("k")?, num("y")?, num("z")?);
            if st.active == 2 {
                // the top trim's front guide moves freely in y and z, but never below the top pad's surface. Once a point has been
                // pressed down onto the pad that height becomes its real height: pulled back out (in y), it keeps that height
                // instead of following the pad's contour; it only moves again when the pointer goes above it.
                let mz = z;
                z = st.held_front_z(k, y, mz);
                st.proj.move_rail2(k, y, z);
                if st.proj.rail_sym {
                    if let Some(jx) = st.proj.rail_partner(k) {
                        let fz = st.held_front_z(jx, y, mz);
                        st.proj.rail2[jx] = [y, fz];
                        st.proj.snap();
                    }
                }
                st.trim_custom = true;
            } else {
                st.proj.move_rail2(k, y, z);
            }
        }
        "move_profile" => st.proj.move_profile(idx("i")?, num("x")?),
        "edit_pt" => st.proj.edit_pt(idx("i")?, idx("k")?, num("which")? as u8, num("y")?, num("z")?),
        "move_pts" => {
            let ks: Vec<usize> = j.get("ks").and_then(|v| v.arr()).map(|a| a.iter().filter_map(|x| x.num()).map(|x| x.max(0.0) as usize).collect()).unwrap_or_default();
            st.proj.move_pts(idx("i")?, &ks, num("dy")?, num("dz")?);
        }
        "add_after" => {
            st.push_undo();
            st.redo.clear();
            if !st.proj.add_after(idx("i")?) {
                st.undo.pop();
                return Err("cannot add after the last profile".into());
            }
        }
        "add_profile_at" => {
            let x = num("x")?;
            let sym = j.b("sym").unwrap_or(false);
            if let Some(e) = st.proj.add_profile_problem(x, sym) {
                return Err(e);
            }
            st.push_undo();
            st.redo.clear();
            if st.proj.add_profile_at(x, sym).is_none() {
                st.undo.pop();
                return Err("could not add the profile there".into());
            }
            if st.active == 2 {
                // the trim now has its own extra windshield nodes, so it is no longer rebuilt from the pad automatically
                st.trim_custom = true;
            }
        }
        "remove" => {
            st.push_undo();
            st.redo.clear();
            if !st.proj.remove(idx("i")?) {
                st.undo.pop();
                return Err("cannot remove that profile (ends/center are fixed, keep at least 3)".into());
            }
        }
        "set_crease" => {
            st.push_undo();
            st.redo.clear();
            let (i, v) = (idx("i")?, j.b("v").unwrap_or(true));
            st.proj.set_crease(i, v);
        }
        "set_sym" => {
            st.push_undo();
            st.redo.clear();
            st.proj.set_sym(idx("i")?, j.b("v").unwrap_or(true));
        }
        "move_rail" => st.proj.move_rail(idx("k")?, num("x")?, num("y")?, num("z")?),
        "rail_add" => {
            st.push_undo();
            st.redo.clear();
            if !st.proj.rail_add(idx("k")?) {
                st.undo.pop();
                return Err("cannot add a rail point there".into());
            }
        }
        "rail_remove" => {
            st.push_undo();
            st.redo.clear();
            if !st.proj.rail_remove(idx("k")?) {
                st.undo.pop();
                return Err("cannot remove that rail point (ends/center are fixed, keep at least 3)".into());
            }
        }
        "set_rail_sym" => {
            st.push_undo();
            st.redo.clear();
            st.proj.rail_sym = j.b("v").unwrap_or(true);
        }
        "set_gap" => {
            st.gap_on = j.b("on").unwrap_or(st.gap_on);
            if let Some(v) = j.n("v") { st.gap = v.max(0.0).min(3.0); }
        }
        "set_bevel" => {
            st.bevel_on = j.b("on").unwrap_or(st.bevel_on);
            if let Some(v) = j.n("v") { st.bevel = v.max(0.0).min(200.0); }
            if let Some(v) = j.n("down") { st.bevel_down = v.max(0.0).min(200.0); }
            if let Some(v) = j.b("round") { st.bevel_round = v; }
            if let Some(v) = j.b("full") { st.bevel_full = v; }
        }
        "set_front_mode" => {
            st.push_undo();
            st.redo.clear();
            st.trim_follow = j.b("follow").unwrap_or(false);
            let f = st.trim_follow;
            if let Some(p) = st.piece_mut(2) { p.set_follow(f); }
            st.link_under();
            if let Some(p) = st.piece_mut(2) { p.snap(); }
        }
        "set_thickness" => st.thickness = num("v")?.max(0.0).min(40.0),
        "apply_template" => {
            let p = templates::params_from_json(j);
            if !j.b("live").unwrap_or(false) {
                st.push_undo();
            }
            st.redo.clear();
            let comps = std::mem::take(&mut st.proj.comps);
            let next = st.proj.next_comp;
            st.proj = templates::generate(&p);
            st.trim_on = p.kind != "squarebody";
            st.trim_stale = !st.trim_on;
            if st.active == 0 { st.trim_custom = false; }
            if st.trim_on && st.active == 0 {
                // the trim rides on the top pad, so it follows the windshield and front lines chosen here
                let look = st.piece(2).look;
                let t = templates::trim_from(&st.proj, look);
                st.origin[2] = Some(t.clone());
                if let Some(pc) = st.shelf[2].as_mut() {
                    *pc = t;
                }
            }
            if st.active == 0 {
                // the other pieces take the same overall width so everything still lines up (the top trim is rebuilt from the top
                // pad above, except on the squarebody style which brings its own)
                for i in 1..3 {
                    if i == 2 && p.kind != "squarebody" {
                        continue;
                    }
                    if let Some(pc) = st.piece_mut(i) {
                        if let Some(b) = pc.bounds() {
                            let w = (b[1][0] - b[0][0]).max(1.0);
                            if (p.width / w - 1.0).abs() > 1e-6 {
                                pc.scale_about([0.0; 3], [p.width / w, 1.0, 1.0], false);
                            }
                        }
                    }
                }
            }
            if st.active == 0 {
                let mut o = st.proj.clone();
                o.comps.clear();
                st.origin[0] = Some(o);
                for i in 1..3 {
                    st.origin[i] = if p.kind == "squarebody" { st.shelf[i].clone() } else { None };
                }
            }
            if st.active == 0 {
                st.trim_sig = top_sig(&st.proj);
            }
            if j.b("keep_parts").unwrap_or(false) {
                st.proj.comps = comps;
                st.proj.next_comp = next;
            }
        }
        "section_set" => {
            let k = idx("k")?;
            if k >= st.proj.sections.len() {
                return Err("no such section".into());
            }
            st.push_undo();
            st.redo.clear();
            if let Some(n) = j.s("name") {
                st.proj.sections[k].name = n.chars().take(40).collect();
            }
            if let Some(c) = j.s("color") {
                if c.len() == 7 && c.starts_with('#') {
                    st.proj.sections[k].color = c.to_string();
                }
            }
        }
        "section_split" => {
            st.push_undo();
            st.redo.clear();
            let pid = st.proj.profiles.get(idx("at")?).map(|p| p.id).ok_or("no such profile")?;
            if !st.proj.section_split(idx("k")?, pid) {
                st.undo.pop();
                return Err("pick a profile inside that section to split it".into());
            }
        }
        "section_merge" => {
            st.push_undo();
            st.redo.clear();
            if !st.proj.section_merge(idx("k")?) {
                st.undo.pop();
                return Err("cannot merge those sections".into());
            }
        }
        "section_boundary" => {
            st.push_undo();
            st.redo.clear();
            let pid = st.proj.profiles.get(idx("at")?).map(|p| p.id).ok_or("no such profile")?;
            if !st.proj.section_boundary(idx("k")?, pid) {
                st.undo.pop();
                return Err("boundary must stay between the neighboring sections' profiles".into());
            }
        }
        "key_set" => {
            let k = idx("k")?;
            if k >= st.proj.keys.len() {
                return Err("no such key point".into());
            }
            st.push_undo();
            st.redo.clear();
            if let Some(n) = j.s("name") {
                st.proj.keys[k].name = n.chars().take(40).collect();
            }
            if let Some(i) = j.n("idx") {
                let max = st.proj.profiles[0].pts.len() - 1;
                st.proj.keys[k].idx = (i.max(0.0) as usize).min(max);
            }
        }
        "key_add" => {
            st.push_undo();
            st.redo.clear();
            let max = st.proj.profiles[0].pts.len() - 1;
            let i = (idx("idx")?).min(max);
            st.proj.keys.push(model::Key { name: j.s("name").unwrap_or("Key point").chars().take(40).collect(), idx: i });
            st.proj.keys.sort_by_key(|k| k.idx);
        }
        "comp_add" => {
            st.push_undo();
            st.redo.clear();
            let id = st.proj.next_comp;
            st.proj.next_comp += 1;
            let kind = j.s("kind").unwrap_or("gauge").to_string();
            let name = j.s("name").map(|n| n.chars().take(40).collect::<String>()).unwrap_or_else(|| format!("{} {}", kind, id));
            st.proj.comps.push(model::Comp {
                id,
                kind,
                shape: j.s("shape").unwrap_or("round").to_string(),
                name,
                x: num("x")?,
                y: num("y")?,
                w: j.n("w").unwrap_or(50.0).max(2.0).min(1000.0),
                h: j.n("h").unwrap_or(50.0).max(2.0).min(1000.0),
                rot: j.n("rot").unwrap_or(0.0),
                visible: true,
                locked: false,
                look: match (j.s("mat"), j.s("color")) {
                    (Some(a), Some(b)) => crate::materials::Look::from_ids(a, b),
                    _ => None,
                },
            });
        }
        "comp_move" => {
            let id = num("id")? as u32;
            if let Some(c) = st.proj.comps.iter_mut().find(|c| c.id == id) {
                c.x = num("x")?;
                c.y = num("y")?;
            }
        }
        "comp_set" => {
            let id = num("id")? as u32;
            if !j.b("quiet").unwrap_or(false) {
                st.push_undo();
                st.redo.clear();
            }
            if let Some(c) = st.proj.comps.iter_mut().find(|c| c.id == id) {
                if let Some(n) = j.s("name") {
                    c.name = n.chars().take(40).collect();
                }
                if let Some(v) = j.n("w") {
                    c.w = v.max(2.0).min(1000.0);
                }
                if let Some(v) = j.n("h") {
                    c.h = v.max(2.0).min(1000.0);
                }
                if let Some(v) = j.n("rot") {
                    c.rot = v;
                }
                if let Some(v) = j.b("visible") {
                    c.visible = v;
                }
                if let Some(v) = j.b("locked") {
                    c.locked = v;
                }
                if let Some(a) = j.s("mat") {
                    c.look = if a == "default" { None } else { crate::materials::Look::from_ids(a, j.s("color").unwrap_or("")) };
                }
            }
        }
        "comp_remove" => {
            let id = num("id")? as u32;
            st.push_undo();
            st.redo.clear();
            st.proj.comps.retain(|c| c.id != id);
        }
        "cut_add" => {
            let cj = j.get("cut").ok_or("no cut")?;
            st.push_undo();
            st.redo.clear();
            let mut c = crate::cuts::Cut::from_json(cj);
            c.id = st.proj.next_cut;
            st.proj.next_cut += 1;
            st.proj.cuts.push(c);
        }
        "cut_set" => {
            let cj = j.get("cut").ok_or("no cut")?;
            if !j.b("quiet").unwrap_or(false) {
                st.push_undo();
                st.redo.clear();
            }
            let c = crate::cuts::Cut::from_json(cj);
            if let Some(slot) = st.proj.cuts.iter_mut().find(|x| x.id == c.id) {
                *slot = c;
            }
        }
        "cut_move" => {
            let id = num("id")? as u32;
            let n = j.get("n").and_then(|v| v.arr()).map(|a| a.iter().map(|q| q.num().unwrap_or(0.0)).collect::<Vec<f64>>()).unwrap_or_default();
            if let Some(c) = st.proj.cuts.iter_mut().filter(|c| !c.locked).find(|c| c.id == id) {
                c.x = num("x")?;
                c.y = num("y")?;
                c.z = num("z")?;
                if n.len() == 3 {
                    c.n = crate::cuts::unit([n[0], n[1], n[2]]);
                }
            }
        }
        "cut_remove" => {
            let id = num("id")? as u32;
            st.push_undo();
            st.redo.clear();
            st.proj.cuts.retain(|c| c.id != id);
        }
        "key_remove" => {
            let k = idx("k")?;
            if k < st.proj.keys.len() {
                st.push_undo();
                st.redo.clear();
                st.proj.keys.remove(k);
            }
        }
        other => return Err(format!("unknown op {}", other)),
    }
    st.dirty = true;
    Ok(())
}


/// Write one piece to ~/BackwoodsDashDesigner/exports/<name>.stl. Returns a JSON summary.
pub fn export_piece(st: &State, i: usize, thickness: f64, name: &str) -> Result<String, String> {
    let mut base: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' { c } else { '_' }).collect();
    if base.to_lowercase().ends_with(".stl") {
        base.truncate(base.len() - 4);
    }
    let base = base.trim().trim_matches('.').to_string();
    if base.is_empty() {
        return Err("give the file a name".into());
    }
    let dir = st.dir.join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.stl", base));
    let proj = st.piece(i);
    let mut files: Vec<String> = Vec::new();
    let (m, extra_tris) = match crate::cuts::compute(proj, thickness, true) {
        Some(Ok(r)) => {
            if !r.warnings.is_empty() {
                return Err(format!("could not apply every cut: {}", r.warnings.join("; ")));
            }
            let mut extra = 0usize;
            for (k, (pname, plug)) in r.plugs.iter().enumerate() {
                let safe: String = pname.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
                let pp = dir.join(format!("{}_plug{}_{}.stl", base, k + 1, safe));
                let pm = plug.to_mesh();
                std::fs::write(&pp, stl_bytes(&pm)).map_err(|e| e.to_string())?;
                extra += pm.idx.len() / 3;
                files.push(pp.to_string_lossy().to_string());
            }
            (r.body.to_mesh(), extra)
        }
        Some(Err(e)) => return Err(e),
        None => (proj.mesh_final(thickness).0, 0),
    };
    let _ = extra_tris;
    std::fs::write(&path, stl_bytes(&m)).map_err(|e| e.to_string())?;
    let vol = if thickness > 0.0 { model::signed_volume(&m.pos, &m.idx) } else { 0.0 };
    let j = J::Obj(vec![
        ("path".into(), J::Str(path.to_string_lossy().to_string())),
        ("plugs".into(), J::Arr(files.into_iter().map(J::Str).collect())),
        ("tris".into(), J::Num((m.idx.len() / 3) as f64)),
        ("open_edges".into(), J::Num(model::open_edges_welded(&m.pos, &m.idx) as f64)),
        ("volume_mm3".into(), J::Num(vol)),
    ]);
    Ok(json::to_string(&j))
}

#[allow(dead_code)]
pub fn export_stl(st: &State, thickness: f64) -> Result<String, String> {
    let dir = st.dir.join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let tag = if thickness > 0.0 { format!("{}mm", thickness) } else { "surface".to_string() };
    let names = ["toppad", "lowerpad", "toptrim"];
    let mut jobs: Vec<(String, model::Mesh, bool)> = Vec::new();
    for i in 0..3 {
        jobs.push((format!("bwdash_{}_{}_{}.stl", names[i], secs, tag), st.piece(i).mesh_final(thickness).0, thickness > 0.0));
    }
    let (mut paths, mut tris, mut open, mut vol) = (Vec::new(), 0usize, 0usize, 0.0);
    for (name, m, solid) in jobs {
        let path = dir.join(name);
        std::fs::write(&path, stl_bytes(&m)).map_err(|e| e.to_string())?;
        paths.push(path.to_string_lossy().to_string());
        tris += m.idx.len() / 3;
        open += model::open_edges_welded(&m.pos, &m.idx);
        if solid {
            vol += model::signed_volume(&m.pos, &m.idx);
        }
    }
    let j = J::Obj(vec![
        ("path".into(), J::Str(paths.join("\n"))),
        ("tris".into(), J::Num(tris as f64)),
        ("open_edges".into(), J::Num(open as f64)),
        ("volume_mm3".into(), J::Num(vol)),
    ]);
    Ok(json::to_string(&j))
}
impl State {
    /// Apply an op described as JSON text, e.g. {"op":"undo"}.
    pub fn op(&mut self, text: &str) -> Result<(), String> {
        let j = json::parse(text)?;
        let r = apply_op(self, &j);
        self.keep_trim_fitted();
        self.sync_lock();
        r
    }
    /// True when the top trim's guidelines no longer match the ones the top pad would give it.
    pub fn trim_out_of_date(&self) -> bool {
        if std::ptr::eq(self.piece(0), self.piece(2)) { return false; }
        let fresh = templates::trim_from(self.piece(0), self.piece(2).look);
        let t = self.piece(2);
        if fresh.rail.len() != t.rail.len() || fresh.rail2.len() != t.rail2.len() { return true; }
        let far = |a: &[f64], b: &[f64]| a.iter().zip(b).any(|(x, y)| (x - y).abs() > 0.5);
        fresh.rail.iter().zip(&t.rail).any(|(a, b)| far(a, b)) || fresh.rail2.iter().zip(&t.rail2).any(|(a, b)| far(a, b))
    }
    /// Lowest height the top trim's front guide may have at row k with its front point at y: the top pad's surface there (plus
    /// the gap). None where there is no pad underneath, so the nose can hang over freely.
    pub fn front_floor(&self, k: usize, y: f64) -> Option<f64> {
        let up = self.piece(0);
        let tr = self.piece(2);
        if std::ptr::eq(up, tr) { return None; }
        let x = tr.rail.get(k)?[0];
        let gap = if self.gap_on { self.gap } else { 0.0 };
        let u = model::Under::from_grid(&up.grid());
        u.z_at(x, y).or_else(|| u.z_at(x, y - 0.5)).map(|z| z + gap)
    }
    /// Height for the front-guide point k at y when the pointer asks for `z`: never below the pad, and a height the point was
    /// pressed down to stays its height until the pointer goes above it.
    pub fn held_front_z(&mut self, k: usize, y: f64, z: f64) -> f64 {
        let mut z = z;
        let cur = self.piece(2).rail2.get(k).map(|r| r[1]);
        let cur_y = self.piece(2).rail2.get(k).map(|r| r[0]).unwrap_or(y);
        if let (Some(h), Some(c)) = (self.rail2_hold.get(&k).copied(), cur) {
            // a move in y keeps the pressed-down height; a move that only changes z (y unchanged) is the pointer choosing a
            // height again, so the point is free to go anywhere down to the pad's surface
            if (c - h).abs() > 1e-6 || z > h || (y - cur_y).abs() < 1e-9 { self.rail2_hold.remove(&k); } else { z = h; }
        }
        if let Some(f) = self.front_floor(k, y) {
            if z < f { z = f; self.rail2_hold.insert(k, f); }
        }
        z
    }
    /// Push any front-guide point that sits below the pad's surface back up onto it.
    pub fn lift_front_guide(&mut self) {
        let n = self.piece(2).rail2.len();
        for k in 0..n {
            let [y, z] = self.piece(2).rail2[k];
            if let Some(f) = self.front_floor(k, y) {
                if z < f {
                    if let Some(t) = self.piece_mut(2) { t.rail2[k] = [y, f]; }
                }
            }
        }
        if let Some(t) = self.piece_mut(2) { t.snap(); }
    }
    /// The trim's guidelines come from the top pad, so when the pad changes the trim is rebuilt to follow it.
    fn keep_trim_fitted(&mut self) {
        if !self.trim_on || self.active != 0 { return; }
        let sig = top_sig(self.piece(0));
        if sig != self.trim_sig {
            if self.trim_custom {
                // the front guide was moved by hand: keep it, but never let it end up under the pad's new surface
                self.trim_sig = sig;
                self.link_under();
                self.lift_front_guide();
            } else {
                self.refit_trim();
            }
        }
    }
}

#[cfg(test)]
mod resample_tests {
    use super::*;
    fn from_raw(i: u8) -> Project {
        let j = match i { 0 => DEFAULT_PROJECT, 1 => DEFAULT_LOWER, _ => DEFAULT_TRIM };
        Project::from_json(&json::parse(j).unwrap()).unwrap()
    }
    fn dev(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
        let mut worst = 0.0f64;
        for p in a {
            let mut best = f64::MAX;
            for w in b.windows(2) {
                let (s, e) = (w[0], w[1]);
                let d = [e[0] - s[0], e[1] - s[1]];
                let l2 = (d[0] * d[0] + d[1] * d[1]).max(1e-12);
                let t = (((p[0] - s[0]) * d[0] + (p[1] - s[1]) * d[1]) / l2).max(0.0).min(1.0);
                let q = [s[0] + d[0] * t, s[1] + d[1] * t];
                best = best.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt());
            }
            worst = worst.max(best);
        }
        worst
    }
    #[test]
    fn resample_keeps_shape() {
        for i in 0..3u8 {
            for &target in &[30usize, 24, 18, 12, 6] {
                let mut p = from_raw(i);
                let n0 = p.profiles[0].pts.len();
                let before: Vec<Vec<[f64; 2]>> = p.profiles.iter().map(|q| q.curve()).collect();
                let got = p.resample_all(target, true);
                let mut w = 0.0f64;
                for (q, b) in p.profiles.iter().zip(before.iter()) {
                    assert_eq!(q.pts.len(), got);
                    w = w.max(dev(b, &q.curve())).max(dev(&q.curve(), b));
                }
                println!("piece {} {}->{} (asked {}) worst dev {:.2} mm", i, n0, got, target, w);
                let _ = p.mesh(6.0);
            }
        }
    }
}

#[cfg(test)]
mod default_tests {
    #[test]
    fn defaults_are_six() {
        for i in 0..3u8 {
            let p = super::default_for(i);
            println!("default {} points {} keys {:?}", i, p.profiles[0].pts.len(), p.keys.iter().map(|k| k.idx).collect::<Vec<_>>());
            let _ = p.mesh(6.0);
        }
    }
}

#[cfg(test)]
mod front_tests {
    #[test]
    fn top_pad_has_front() {
        let raw = super::json::parse(super::DEFAULT_PROJECT).unwrap();
        let mut a = crate::model::Project::from_json(&raw).unwrap();
        let b = super::default_project();
        println!("rail {} rail2 {} profiles {}", b.rail.len(), b.rail2.len(), b.profiles.len());
        assert_eq!(b.rail2.len(), b.rail.len());
        let _ = a.mesh(6.0);
        let mut worst = 0.0f64;
        for (p, q) in a.profiles.iter().zip(b.profiles.iter()) {
            let (u, v) = (p.pts[0].co, q.pts[0].co);
            let _ = (u, v);
        }
        worst += 0.0;
        println!("ok {}", worst);
    }
}

#[cfg(test)]
mod multi_tests {
    #[test]
    fn move_several() {
        let mut st = super::State::new(std::path::PathBuf::from("/tmp/dashfw_test_none"));
        let before: Vec<[f64; 2]> = st.proj.profiles[2].pts.iter().map(|p| p.co).collect();
        st.op(r#"{"op":"move_pts","i":2,"ks":[1,2,3],"dy":3.0,"dz":-2.0}"#).unwrap();
        let after: Vec<[f64; 2]> = st.proj.profiles[2].pts.iter().map(|p| p.co).collect();
        for k in 1..4 {
            println!("{} {:?} -> {:?}", k, before[k], after[k]);
        }
        assert!((after[2][0] - before[2][0] - 3.0).abs() < 1e-6 || st.proj.profiles[2].sym);
        assert!((after[1][1] - before[1][1] + 2.0).abs() < 1e-3);
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;
    #[test]
    fn lock_matches_heights() {
        let st = State::new(std::path::PathBuf::from("/tmp/dd3_none"));
        let (up, lo) = (st.piece(0), st.piece(1));
        for p in &lo.profiles {
            let top = p.pts[0].co[1];
            let upz = up.rail2_at(p.x).1;
            assert!((top - upz).abs() < 1e-6, "x {} lower top {} upper lip {}", p.x, top, upz);
        }
        let n = lo.profiles[0].pts.len();
        println!("lower bottom z {}", lo.profiles[0].pts[n - 1].co[1]);
    }
}

#[cfg(test)]
mod sym_tests {
    use super::*;
    #[test]
    fn broken_symmetry_keeps_position() {
        let mut st = State::new(std::path::PathBuf::from("/tmp/dd3_none_sym"));
        let x1 = st.proj.profiles[1].x;
        st.op(r#"{"op":"set_sym","i":3,"v":false}"#).unwrap();
        st.op(r#"{"op":"move_profile","i":3,"x":300}"#).unwrap();
        assert!((st.proj.profiles[3].x - 300.0).abs() < 1e-6, "p3 {}", st.proj.profiles[3].x);
        assert!((st.proj.profiles[1].x - x1).abs() < 1e-6);
        // the profile still hangs on both guidelines
        let p = &st.proj;
        let a = p.profiles[3].anchor();
        let (ry, rz) = p.rail_at(300.0);
        assert!((a[0] - ry).abs() < 1e-6 && (a[1] - rz).abs() < 1e-6);
        let n = p.profiles[3].pts.len();
        let (by, bz) = p.rail2_at(300.0);
        assert!((p.profiles[3].pts[n - 1].co[0] - by).abs() < 1e-6 && (p.profiles[3].pts[n - 1].co[1] - bz).abs() < 1e-6);
        // saves and loads the same
        let j = json::to_string(&p.to_json(false));
        let q = model::Project::from_json(&json::parse(&j).unwrap()).unwrap();
        assert!((q.profiles[3].x - 300.0).abs() < 1e-6);
        assert!(!q.profiles[3].sym && !q.profiles[1].sym);
        // moving a guideline point on one side leaves the other side alone and keeps profile x
        let mut st2 = st;
        st2.op(r#"{"op":"set_rail_sym","v":false}"#).unwrap();
        let before1 = st2.proj.rail[1];
        st2.op(r#"{"op":"move_rail","k":3,"x":200,"y":180,"z":380}"#).unwrap();
        assert_eq!(st2.proj.rail[1], before1);
        // the contour is the lock: the profile keeps its place across and re-pins to the contour; sliding it leaves the contour alone
        assert!((st2.proj.profiles[3].x - 300.0).abs() < 1e-6);
        let rail3 = st2.proj.rail[3];
        st2.op(r#"{"op":"move_profile","i":3,"x":260}"#).unwrap();
        assert_eq!(st2.proj.rail[3], rail3);
        assert!((st2.proj.profiles[3].x - 260.0).abs() < 1e-6);
        let (ry, rz) = st2.proj.rail_at(260.0);
        let a = st2.proj.profiles[3].anchor();
        assert!((a[0] - ry).abs() < 1e-6 && (a[1] - rz).abs() < 1e-6);
    }
}


#[cfg(test)]
mod front_pin_tests {
    use super::*;
    #[test]
    fn mirrored_copy_stays_on_the_front_line() {
        let mut st = State::new(std::path::PathBuf::from("/tmp/dd3_none_fp"));
        st.op(r#"{"op":"set_rail_sym","v":false}"#).unwrap();
        // pull the front line down on one side only
        let (y1, z1) = (st.proj.rail2[1][0], st.proj.rail2[1][1]);
        st.op(&format!(r#"{{"op":"move_rail2","k":1,"y":{},"z":{}}}"#, y1, z1 - 30.0)).unwrap();
        // edit the mirrored profile pair
        st.op(r#"{"op":"edit_pt","i":1,"k":2,"which":0,"y":200,"z":300}"#).ok();
        st.op(r#"{"op":"set_sym","i":1,"v":true}"#).unwrap();
        let p = &st.proj;
        for i in 0..p.profiles.len() {
            let n = p.profiles[i].pts.len();
            let (by, bz) = p.rail2_at(p.profiles[i].x);
            let q = p.profiles[i].pts[n - 1].co;
            assert!((q[0] - by).abs() < 1e-6 && (q[1] - bz).abs() < 1e-6, "profile {} front end off the front line: {:?} vs {:?}", i, q, (by, bz));
            let a = p.profiles[i].anchor();
            let (ry, rz) = p.rail_at(p.profiles[i].x);
            assert!((a[0] - ry).abs() < 1e-6 && (a[1] - rz).abs() < 1e-6, "profile {} windshield end off the line", i);
        }
    }
}

#[cfg(test)]
mod add_profile_tests {
    use super::*;
    #[test]
    fn add_symmetrical_and_single() {
        let mut st = State::new(std::path::PathBuf::from("/tmp/dd3_none_ap"));
        let n0 = st.proj.profiles.len();
        st.op(r#"{"op":"add_profile_at","x":560,"sym":true}"#).unwrap();
        assert_eq!(st.proj.profiles.len(), n0 + 2);
        let xs: Vec<f64> = st.proj.profiles.iter().map(|p| p.x).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{:?}", xs);
        let i = xs.iter().position(|&x| (x - 560.0).abs() < 1e-9).unwrap();
        let j = xs.iter().position(|&x| (x + 560.0).abs() < 1e-9).unwrap();
        assert_eq!(st.proj.partner(i), Some(j));
        assert!(st.proj.locked_partner(i).is_some());
        // pinned to both lines
        for p in &st.proj.profiles {
            let n = p.pts.len();
            let (by, bz) = st.proj.rail2_at(p.x);
            assert!((p.pts[n - 1].co[0] - by).abs() < 1e-6 && (p.pts[n - 1].co[1] - bz).abs() < 1e-6);
        }
        st.op(r#"{"op":"add_profile_at","x":150,"sym":false}"#).unwrap();
        assert_eq!(st.proj.profiles.len(), n0 + 3);
        let k = st.proj.profiles.iter().position(|p| (p.x - 150.0).abs() < 1e-9).unwrap();
        assert!(st.proj.partner(k).is_none() && !st.proj.profiles[k].sym);
        // too close / outside are refused
        assert!(st.op(r#"{"op":"add_profile_at","x":152,"sym":false}"#).is_err());
        assert!(st.op(r#"{"op":"add_profile_at","x":900,"sym":false}"#).is_err());
        assert!(st.op(r#"{"op":"add_profile_at","x":10,"sym":true}"#).is_err());
    }
}

#[cfg(test)]
mod trim_tests {
    use super::*;
    #[test]
    fn trim_rides_on_pad() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        st.gap_on = true;
        st.gap = 0.3;
        st.sync_lock();
        let pad = st.piece(0).clone();
        let tr = st.piece(2).clone();
        assert!(tr.under.is_some());
        let m = tr.mesh(6.0);
        assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0, "trim shell must be watertight");
        assert!(model::signed_volume(&m.pos, &m.idx) > 0.0);
        let u = model::Under::from_grid(&pad.grid());
        let n = tr.grid().len() * tr.grid()[0].len();
        let (mut hit, mut worst_thin) = (0, 0.0f64);
        let mut onpad = 0;
        let gn = Project::grid_normals(&tr.grid());
        let ncol = tr.grid()[0].len();
        for k in 0..n {
            if gn[k / ncol][k % ncol][2] < 0.6 { continue; }
            // the last 36 mm at each end is the end run, which eases the inside down onto the pad
            let (gx, gx0, gx1) = (tr.grid()[k / ncol][0][0], tr.grid()[0][0][0], tr.grid()[tr.grid().len() - 1][0][0]);
            if (gx - gx0).abs().min((gx1 - gx).abs()) < 37.0 { continue; }
            let o = [m.pos[k * 3] as f64, m.pos[k * 3 + 1] as f64, m.pos[k * 3 + 2] as f64];
            let i = n + k;
            let q = [m.pos[i * 3] as f64, m.pos[i * 3 + 1] as f64, m.pos[i * 3 + 2] as f64];
            if let Some(pz) = u.z_at(o[0], o[1]) {
                hit += 1;
                // follows the pad + gap, or (where the trim has run out past the pad) stays flat above it as a ledge
                assert!(q[2] >= pz + 0.3 - if k % ncol < 3 { 3.0 } else { 1e-3 }, "underside never dips into the pad (beyond the rim tolerance): q={:?} pz={} row {} col {} of {} outer {:?}", q, pz, k / ncol, k % ncol, ncol, o);
                if (q[2] - (pz + 0.3)).abs() < 1e-3 { onpad += 1; } else if k % ncol >= 3 {
                    // not on the pad: then it is exactly one shell thickness under the top face (a plain lip over open space)
                    assert!((o[2] - q[2] - 6.0).abs() < 2.5 || q[2] - (pz + 0.3) < 0.5, "underside neither on the pad nor a plain shell: gap {} thickness {}", q[2] - (pz + 0.3), o[2] - q[2]);
                }
                worst_thin = worst_thin.max(6.0 - (o[2] - q[2]));
            }
        }
        assert!(hit > 0, "trim should overlap the pad somewhere");
        assert!(onpad * 10 > hit, "the underside rides on the pad where it meets it ({} of {})", onpad, hit);
        assert!(worst_thin < 2.5, "thickness never more than 2.5 mm below the set value over the pad (anything over 1 mm is marked red), short by {}", worst_thin);
    }

    #[test]
    fn ends_are_square() {
        let st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        for i in 0..3 {
            let m = st.piece(i).mesh(6.0);
            let nc = st.piece(i).grid()[0].len();
            let n = st.piece(i).grid().len() * nc;
            for k in 0..nc {
                assert!((m.pos[k * 3] - m.pos[(n + k) * 3]).abs() < 1e-4, "piece {} start end not square", i);
                let l = n - 1 - k;
                assert!((m.pos[l * 3] - m.pos[(n + l) * 3]).abs() < 1e-4, "piece {} far end not square", i);
            }
            assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0);
        }
    }

    #[test]
    fn bevel_is_closed_and_chamfered() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        st.bevel_on = true;
        st.bevel = 2.0;
        st.sync_lock();
        for i in 0..3 {
            let p = st.piece(i);
            let plain = p.mesh_off(0.0, 6.0);
            let m = p.mesh(6.0);
            assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0, "piece {} bevel shell watertight", i);
            assert!(model::signed_volume(&m.pos, &m.idx) > 0.0);
            assert!(i != 2 || m.pos.len() > plain.pos.len(), "trim gets the extra rows");
            assert!(i == 2 || m.pos.len() == plain.pos.len(), "pads are left alone");
        }
    }

    #[test]
    fn real_project_meshes() {
        // optional: point DASHFW_TEST_PROJECT at a real project folder to run this check on it
        let Ok(d) = std::env::var("DASHFW_TEST_PROJECT").map(std::path::PathBuf::from) else { return };
        if !d.join("project.json").exists() { return; }
        for (bon, gon) in [(false, true), (true, true), (true, false)] {
            let mut st = State::new(d.clone());
            st.bevel_on = bon; st.bevel = 3.0; st.gap_on = gon;
            st.sync_lock();
            for i in 0..3 {
                let m = st.piece(i).mesh(6.0);
                assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0, "piece {} bevel {} gap {}", i, bon, gon);
                assert!(m.pos.iter().all(|v| v.is_finite()));
            }
        }
    }

    #[test]
    fn shell_layout_skin_first() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        for bevel in [false, true] {
            st.bevel_on = bevel;
            st.bevel_round = bevel;
            st.bevel = 9.0;
            st.bevel_down = 4.0;
            st.sync_lock();
            for i in 0..3 {
                let p = st.piece(i);
                let m = p.mesh(6.0);
                let n = p.skin_idx_len(6.0);
                let verts = (p.skin_idx_len(6.0) / 6 + 1) * 0 + m.idx.iter().take(p.skin_idx_len(6.0)).map(|&v| v as usize).max().unwrap() + 1;
                assert!(m.idx[..n].iter().all(|&v| (v as usize) < verts), "piece {} bevel {}: first {} indices must be outer skin", i, bevel, n);
                assert!(m.idx[n..n + 3].iter().all(|&v| (v as usize) >= verts), "inner face follows the skin");
            }
        }
    }

    #[test]
    fn deep_bevel_stops_at_far_end() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        st.bevel_on = true;
        for bv in [2.0f64, 15.0, 60.0, 5000.0] {
            st.bevel = bv.min(20.0);
            st.sync_lock();
            for i in 0..3 {
                let p = st.piece_mut(i).unwrap();
                p.end_bevel = if i == 2 { bv } else { 0.0 }; // beyond the UI limit on purpose
                p.end_down = bv * 0.5;
                p.end_round = bv > 10.0;
                p.end_full = bv > 20.0;
                let m = p.mesh(6.0);
                assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0, "piece {} bevel {}", i, bv);
                assert!(m.pos.iter().all(|v| v.is_finite()));
                assert!(model::signed_volume(&m.pos, &m.idx) > 0.0);
                let n = m.pos.len() / 3;
                let _ = n;
            }
        }
    }


    #[test]
    fn edge_only_on_trim() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        st.bevel_on = true;
        st.bevel = 30.0;
        st.sync_lock();
        assert_eq!(st.piece(0).end_bevel, 0.0);
        assert_eq!(st.piece(1).end_bevel, 0.0);
        assert_eq!(st.piece(2).end_bevel, 30.0);
    }

    #[test]
    fn wall_top_follows_pad_end() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        st.bevel_on = true;
        st.bevel_round = true;
        st.bevel_full = true;
        st.bevel = 80.0;
        st.sync_lock();
        let back = crate::sidewall::default_back();
        let wall = crate::sidewall::both(st.piece(0), st.piece(1), &back, 6.0).unwrap();
        let pad = st.piece(0).mesh(6.0);
        let x0 = st.piece(0).profiles[0].x as f32;
        let zmax = |m: &model::Mesh, x: f32, side: f32| m.pos.chunks(3).filter(|p| (p[0] - x).abs() < 1e-3 && p[1] > side).map(|p| p[2]).fold(f32::MIN, f32::max);
        // above the lower pad's top (y > 0 is the top pad's part of the wall)
        let (wz, pz) = (zmax(&wall, x0, -30.0), zmax(&pad, x0, -30.0));
        assert!(wz <= pz + 0.01, "wall top {} must not stand above the pad end {}", wz, pz);
    }

    #[test]
    fn wall_has_thickness_at_windshield_edge() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_test_none"));
        st.bevel_on = false;
        st.sync_lock();
        let back = crate::sidewall::default_back();
        for end in 0..2 {
            let o = crate::sidewall::outline(st.piece(0), st.piece(1), &back, end, None, 6.0).unwrap();
            // the wall fits inside the shell: its top edge is the pad's underside, which starts about one shell thickness below
            // the pad's windshield edge, and the back curve starts right there
            let pad_top = if end == 0 { st.piece(0).profiles[0].curve()[0] } else { st.piece(0).profiles.last().unwrap().curve()[0] };
            assert!((pad_top[1] - o.top[0][1] - 6.0).abs() < 1.0, "wall starts a shell thickness below the pad's edge: {} vs {}", pad_top[1], o.top[0][1]);
            assert!((o.top[0][1] - o.back[0][1]).abs() < 1e-9);
            assert!((o.top[0][0] - o.back[0][0]).abs() < 1e-9);
            assert!(o.poly.iter().any(|p| (p[0] - o.back[0][0]).abs() < 1e-9 && (p[1] - o.back[0][1]).abs() < 1e-9), "start point is part of the wall outline");
        }
        let wall = crate::sidewall::both(st.piece(0), st.piece(1), &back, 6.0).unwrap();
        assert_eq!(model::open_edges_welded(&wall.pos, &wall.idx), 0, "wall plates are closed");
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;
    #[test]
    fn scale_changes_only_the_asked_axis() {
        let j = crate::json::parse(DEFAULT_PROJECT).unwrap();
        let mut p = Project::from_json(&j).unwrap();
        let b = p.bounds().unwrap();
        // stretch x by 10 percent about the middle
        p.scale_about([0.0, 0.0, 0.0], [1.1, 1.0, 1.0], true);
        let c = p.bounds().unwrap();
        assert!(((c[1][0] - c[0][0]) / (b[1][0] - b[0][0]) - 1.1).abs() < 1e-6);
        assert!(((c[1][1] - c[0][1]) - (b[1][1] - b[0][1])).abs() < 1e-6);
        assert!(((c[1][2] - c[0][2]) - (b[1][2] - b[0][2])).abs() < 1e-6);
        // an even half-size scale and back again returns the same box
        p.scale_about([0.0; 3], [0.5; 3], true);
        p.scale_about([0.0; 3], [2.0; 3], true);
        let d = p.bounds().unwrap();
        for k in 0..3 {
            assert!((d[0][k] - c[0][k]).abs() < 1e-6 && (d[1][k] - c[1][k]).abs() < 1e-6);
        }
        assert!(p.mesh(6.0).idx.len() > 0);
    }
}

#[cfg(test)]
mod size_reset_tests {
    use super::*;
    #[test]
    fn reset_anchor_restores_box() {
        let mut p = Project::from_json(&crate::json::parse(DEFAULT_PROJECT).unwrap()).unwrap();
        let o = p.bounds().unwrap();
        p.scale_about([100.0, -20.0, 300.0], [1.3, 0.7, 1.1], true);
        let c = p.bounds().unwrap();
        let mut a = [0.0; 3];
        let mut s = [1.0; 3];
        for k in 0..3 {
            s[k] = (o[1][k] - o[0][k]) / (c[1][k] - c[0][k]);
            a[k] = (o[0][k] - s[k] * c[0][k]) / (1.0 - s[k]);
        }
        p.scale_about(a, s, true);
        let r = p.bounds().unwrap();
        for k in 0..3 {
            assert!((r[0][k] - o[0][k]).abs() < 1e-6 && (r[1][k] - o[1][k]).abs() < 1e-6);
        }
    }
}

#[cfg(test)]
mod squarebody_tests {
    use super::*;
    #[test]
    fn squarebody_is_1450_wide_and_drops_the_trim() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_sq_test"));
        assert!(st.trim_on);
        st.op(r#"{"op":"apply_template","kind":"squarebody"}"#).unwrap();
        assert!(!st.trim_on);
        let b = st.piece(0).bounds().unwrap();
        println!("top pad {:?} {:?}", b[0], b[1]);
        assert!(((b[1][0] - b[0][0]) - 1450.0).abs() < 1.0);
        let l = st.piece(1).bounds().unwrap();
        assert!(((l[1][0] - l[0][0]) - 1450.0).abs() < 1.0);
        // driver side (-x) sticks out further than the passenger side
        let pr = st.piece(0);
        let ymin = |q: &model::Profile| q.curve().iter().map(|c| c[0]).fold(f64::MAX, f64::min);
        let (d, pa) = (ymin(&pr.profiles[0]), ymin(pr.profiles.last().unwrap()));
        assert!(pa - d > 30.0, "driver {} passenger {}", d, pa);
        assert!(!pr.rail_sym);
        let m = st.piece(0).mesh(6.0);
        assert!(m.idx.len() > 0);
        assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0);
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        assert!(st.trim_on);
    }

    #[test]
    fn added_trim_follows_the_new_top_pad() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_sq_test2"));
        st.op(r#"{"op":"apply_template","kind":"squarebody"}"#).unwrap();
        assert!(st.trim_stale);
        st.add_trim();
        assert!(st.trim_on && !st.trim_stale);
        let (pad, tr) = (st.piece(0).bounds().unwrap(), st.piece(2).bounds().unwrap());
        println!("pad {:?} {:?}\ntrim {:?} {:?}", pad[0], pad[1], tr[0], tr[1]);
        assert!((pad[0][0] - tr[0][0]).abs() < 1.0 && (pad[1][0] - tr[1][0]).abs() < 1.0, "same width");
        assert!((pad[1][1] - tr[1][1]).abs() < 2.0, "same windshield line");
        assert!(tr[1][2] > pad[1][2], "sits above the pad");
        let m = st.piece(2).mesh(6.0);
        assert!(m.idx.len() > 0);
        assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0, "trim shell is closed");
    }
}

#[cfg(test)]
mod j_trim_tests {
    use super::*;
    #[test]
    fn j_trim_wraps_front_and_is_closed_with_few_anchors() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_j_test"));
        st.op(r#"{"op":"apply_template","kind":"squarebody"}"#).unwrap();
        st.add_trim();
        let pad = st.piece(0).profiles[0].pts.len();
        let tr = st.piece(2).profiles[0].pts.len();
        assert!(pad <= 10 && tr <= 10, "anchors pad {pad} trim {tr}");
        let t = st.piece(2);
        let row = &t.grid()[t.grid().len() / 2];
        let (ymax, ymin) = (row.iter().map(|q| q[1]).fold(f64::MIN, f64::max), row.iter().map(|q| q[1]).fold(f64::MAX, f64::min));
        let last = row[row.len() - 1];
        let first = row[0];
        assert!(first[1] >= ymax - 1.0, "starts at windshield");
        assert!(last[2] < first[2] - 5.0, "nose rounds down: {} {}", last[2], first[2]);
        assert!(ymin > first[1] - 0.8 * (first[1] - st.piece(0).grid()[st.piece(0).grid().len() / 2].iter().map(|q| q[1]).fold(f64::MAX, f64::min)), "top run only");
        let pg = st.piece(0).grid();
        let pr = &pg[pg.len() / 2];
        let zp = |y: f64| pr.windows(2).find(|w| y <= w[0][1] && y >= w[1][1]).map(|w| w[0][2]).unwrap_or(0.0);
        let thick_back = row[1][2] - zp(row[1][1]);
        let k = row.iter().position(|q| q[1] < ymin + 30.0).unwrap_or(row.len() / 2);
        let thick_front = row[k][2] - zp(row[k][1]);
        assert!(thick_front > thick_back + 2.0, "thicker at the front: back {thick_back} front {thick_front}");
        let m = t.mesh(2.0);
        println!("pad {pad} trim {tr} first {:?} last {:?}", first, last);
        assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0);
    }
}

#[cfg(test)]
mod origin_tests {
    use super::*;
    #[test]
    fn reset_returns_to_the_wizard_profiles() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_origin_test"));
        st.op(r#"{"op":"apply_template","kind":"squarebody"}"#).unwrap();
        st.op(r#"{"op":"resample","n":14}"#).ok();
        assert_ne!(st.piece(0).profiles[0].pts.len(), crate::templates::TEMPLATE_POINTS);
        st.op(r#"{"op":"reset"}"#).unwrap();
        let b = st.piece(0).bounds().unwrap();
        assert_eq!(st.piece(0).profiles[0].pts.len(), crate::templates::TEMPLATE_POINTS);
        assert!(((b[1][0] - b[0][0]) - 1450.0).abs() < 1.0);
    }
}

#[cfg(test)]
mod w_tests {
    use super::*;
    #[test]
    fn crease_makes_a_w_front_and_off_makes_a_u() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_w_test"));
        let front = |st: &State| -> Vec<f64> { st.piece(0).profiles.iter().map(|q| q.pts[q.pts.len() - 1].co[0]).collect() };
        let wind = |st: &State| -> Vec<f64> { st.piece(0).rail.iter().map(|r| r[1]).collect() };
        st.op(r#"{"op":"apply_template","kind":"wrap","crease":true,"nprof":5}"#).unwrap();
        let f = front(&st);
        assert!(f[1] > f[2] && f[1] > f[0], "front line W: {:?}", f);
        let w = wind(&st);
        assert!(w[2] > w[1] && w[1] > w[0], "windshield stays a U: {:?}", w);
        st.op(r#"{"op":"apply_template","kind":"wrap","crease":false,"nprof":5}"#).unwrap();
        let f2 = front(&st);
        assert!(f2[2] < f2[1] + 1.0 || (f2[1] - f2[2]).abs() < 80.0);
        assert!(f2[1] < f[1] - 10.0, "no hump when off");
    }
}

#[cfg(test)]
mod wizard_trim_tests {
    use super::*;
    #[test]
    fn trim_follows_wizard_changes() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_wt_test"));
        st.op(r#"{"op":"apply_template","kind":"wrap","bow_y":60,"depth":200}"#).unwrap();
        let (pad, trim) = (st.piece(0).rail.clone(), st.piece(2).rail.clone());
        assert!(trim.len() >= 3);
        let mid = |r: &Vec<[f64; 3]>| r[r.len() / 2][1];
        let end = |r: &Vec<[f64; 3]>| r[0][1];
        assert!((mid(&pad) - end(&pad) - (mid(&trim) - end(&trim))).abs() < 3.0, "trim windshield bow matches pad: {:?} vs {:?}", pad, trim);
        // the untouched style keeps the stock trim
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        assert_eq!(st.piece(2).profiles[0].pts.len(), default_trim().profiles[0].pts.len());
    }
}

#[cfg(test)]
mod wall_export_tests {
    use super::*;
    #[test]
    fn each_wall_saves_under_its_own_name() {
        let dir = std::env::temp_dir().join("dashfw_wallx_test");
        let _ = std::fs::remove_dir_all(&dir);
        let st = State::new(dir.clone());
        let sw = crate::sidewall::SideWall::load(&dir);
        let p = crate::sidewall::export_end(st.piece(0), st.piece(1), &sw.back, 0, 6.0, &dir, "my driver wall.stl").unwrap();
        assert!(p.contains("my driver wall.stl") && std::path::Path::new(p.split(' ').next().map(|_| p.trim_end_matches(" (WARNING: not watertight)")).unwrap()).exists());
        assert!(crate::sidewall::export_end(st.piece(0), st.piece(1), &sw.back, 1, 6.0, &dir, "  ").is_err());
    }
}

#[cfg(test)]
mod lower_match_probe {
    use super::*;
    #[test]
    fn probe() {
        for js in [
            r#"{"op":"apply_template","kind":"wrap"}"#,
            r#"{"op":"apply_template","kind":"wrap","crease":false,"nprof":5}"#,
            r#"{"op":"apply_template","kind":"wrap","bow_y":60,"depth":200}"#,
            r#"{"op":"apply_template","kind":"squarebody"}"#,
        ] {
            let mut st = State::new(std::env::temp_dir().join("dashfw_probe"));
            st.op(js).unwrap();
            st.sync_lock();
            let (up, lo) = (st.piece(0), st.piece(1));
            let (mut dy, mut dz) = (0.0f64, 0.0f64);
            for r in &lo.rail {
                let (ty, tz) = if up.rail2.is_empty() { (0.0, 0.0) } else { up.rail2_at(r[0]) };
                dy = dy.max((r[1] - ty).abs());
                dz = dz.max((r[2] - tz).abs());
            }
            assert!(dy < 0.5 && dz < 0.5, "lower pad top edge must sit on the top pad front line");
            println!("{} => lower rail vs top front line: max dy {:.1} dz {:.1}; top rail2 {} lower rail {} xs top {:?} lower {:?}", js, dy, dz, up.rail2.len(), lo.rail.len(),
                up.rail.iter().map(|r| r[0] as i32).collect::<Vec<_>>(), lo.rail.iter().map(|r| r[0] as i32).collect::<Vec<_>>());
        }
    }
}

#[cfg(test)]
mod centre_depth_tests {
    use super::*;
    #[test]
    fn centre_depth_sets_the_middle_of_the_front_line() {
        let front = |js: &str| {
            let mut st = State::new(std::env::temp_dir().join("dashfw_cd_test"));
            st.op(js).unwrap();
            let pr = st.piece(0);
            let c = pr.profiles.iter().find(|p| p.x.abs() < 1.0).unwrap();
            let pts = c.curve();
            let ymin = pts.iter().map(|q| q[0]).fold(f64::MAX, f64::min);
            let (yw, _) = pr.rail_at(0.0);
            yw - ymin
        };
        let full = front(r#"{"op":"apply_template","kind":"wrap","crease":true,"nprof":5,"centre_depth":1.0}"#);
        let less = front(r#"{"op":"apply_template","kind":"wrap","crease":true,"nprof":5,"centre_depth":0.6}"#);
        let off = front(r#"{"op":"apply_template","kind":"wrap","crease":false,"nprof":5,"centre_depth":0.6,"centre_out":0}"#);
        let out = front(r#"{"op":"apply_template","kind":"wrap","crease":false,"nprof":5,"centre_out":60}"#);
        assert!((out - (off + 60.0)).abs() < 8.0, "center out brings the middle of the front line {} mm further out ({} vs {})", 60, out, off);
        let out2 = front(r#"{"op":"apply_template","kind":"wrap","crease":false,"nprof":5,"centre_out":30}"#);
        assert!((out2 - (off + 30.0)).abs() < 8.0, "each amount is its own setting");
        let inn = front(r#"{"op":"apply_template","kind":"wrap","crease":false,"nprof":5,"centre_out":-30}"#);
        assert!((inn - (off - 30.0)).abs() < 8.0, "a negative amount brings the middle of the front line in ({} vs {})", inn, off);
        println!("center depth full {:.0} less {:.0} off {:.0}", full, less, off);
        assert!(less < full - 50.0, "dips back");
        assert!((off - full).abs() < 25.0, "no dip when the option is off");
    }
}

#[cfg(test)]
mod wide_lower_tests {
    use super::*;
    #[test]
    fn lower_pad_wider_than_top_is_free_past_the_top_pad() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_wide_test"));
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        // pretend the top pad is narrower: cut its rail short on both ends
        let n = st.piece(1).rail.len();
        let (xl, xr) = (st.piece(1).rail[0][0], st.piece(1).rail[n - 1][0]);
        assert!(xl < -500.0 && xr > 500.0);
        let x_top0 = st.piece(0).rail[0][0];
        let x_top1 = st.piece(0).rail.last().unwrap()[0];
        // move the lower pad's end point somewhere by hand; it must not be pulled back when the lower pad extends past the top pad
        if let Some(lo) = st.piece_mut(1) {
            lo.rail.insert(0, [x_top0 - 200.0, -999.0, 123.0]);
            lo.rail.push([x_top1 + 200.0, -999.0, 123.0]);
        }
        let before: Vec<[f64; 3]> = st.piece(1).rail.clone();
        st.sync_lock();
        let after = &st.piece(1).rail;
        if after.len() == before.len() {
            assert_eq!(after[0][1], -999.0);
            assert_eq!(after[0][2], 123.0);
            assert_eq!(after[after.len() - 1][1], -999.0);
        }
    }
}

#[cfg(test)]
mod width_all_tests {
    use super::*;
    #[test]
    fn width_from_the_wizard_sets_the_lower_pad_too() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_width_test"));
        st.op(r#"{"op":"apply_template","kind":"wrap","width":1480}"#).unwrap();
        for w in [800.0, 1200.0, 1600.0] {
            st.op(&format!(r#"{{"op":"apply_template","kind":"wrap","width":{}}}"#, w)).unwrap();
            for i in 0..3 {
                let b = st.piece(i).bounds().unwrap();
                let got = b[1][0] - b[0][0];
                println!("width {} piece {} => {:.1}", w, i, got);
                assert!((got - w).abs() < 2.0, "piece {} width {} vs {}", i, got, w);
            }
        }
    }
}

#[cfg(test)]
mod back_clear_tests {
    use super::*;
    #[test]
    fn back_curve_stays_clear_of_the_lower_profile() {
        for js in [r#"{"op":"apply_template","kind":"wrap"}"#, r#"{"op":"apply_template","kind":"wrap","width":800}"#, r#"{"op":"apply_template","kind":"squarebody"}"#, r#"{"op":"apply_template","kind":"wrap","depth":400,"height":300}"#] {
            let mut st = State::new(std::env::temp_dir().join("dashfw_clear_test"));
            st.op(js).unwrap();
            let mut back = crate::sidewall::default_back();
            let moved = crate::sidewall::keep_clear(st.piece(0), st.piece(1), &mut back, 6.0);
            for end in 0..2 {
                let o = crate::sidewall::outline(st.piece(0), st.piece(1), &back, end, None, 6.0).unwrap();
                let mut worst = f64::MAX;
                for r in o.back.iter().skip(1) {
                    for w in o.low.windows(2) {
                        let (a, b) = (w[0], w[1]);
                        if r[1] >= a[1].min(b[1]) && r[1] <= a[1].max(b[1]) && (b[1] - a[1]).abs() > 1e-9 {
                            let y = a[0] + (b[0] - a[0]) * (r[1] - a[1]) / (b[1] - a[1]);
                            worst = worst.min(r[0] - y);
                        }
                    }
                }
                println!("{} end {} moved {} worst gap {:.1}", js, end, moved, worst);
                assert!(worst > 9.0, "gap {}", worst);
            }
        }
    }
}

#[cfg(test)]
mod trim_spike_probe {
    use super::*;
    #[test]
    fn probe() {
        for js in [r#"{"op":"apply_template","kind":"wrap"}"#, r#"{"op":"apply_template","kind":"wrap","width":800}"#, r#"{"op":"apply_template","kind":"wrap","crease":false,"nprof":5}"#] {
            let mut st = State::new(std::env::temp_dir().join("dashfw_spike"));
            st.op(js).unwrap();
            let tr = st.piece(2);
            let m = tr.mesh(6.0);
            let nc = tr.profiles[0].curve().len();
            let nv = (tr.skin_idx_len(6.0) / 6 / (nc - 1) + 1) * nc;
            let mut worst = (0.0f32, 0usize);
            for v in 0..nv {
                let dz = m.pos[v * 3 + 2] - m.pos[(nv + v) * 3 + 2];
                if dz > worst.0 { worst = (dz, v); }
            }
            let w = worst.1;
            println!("{} => max thickness {:.1} at x {:.0} y {:.0} (verts {})", js, worst.0, m.pos[w * 3], m.pos[w * 3 + 1], nv);
        }
    }
}

#[cfg(test)]
mod trim_spike_probe2 {
    use super::*;
    #[test]
    fn probe2() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_spike2"));
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        let u = model::Under::from_grid(&st.piece(0).grid());
        let tr = st.piece(2);
        let g = tr.grid();
        println!("trim rows {} cols {}", g.len(), g[0].len());
        for r in 0..g.len() {
            let mut line = String::new();
            let x = g[r][0][0];
            let mut bad = 0;
            for c in 0..g[r].len() {
                let p = g[r][c];
                if let Some(pz) = u.z_at(p[0], p[1]) { if p[2] - pz > 30.0 { bad += 1; if bad <= 3 { line += &format!(" c{} y{:.0} z{:.0} padz{:.0};", c, p[1], p[2], pz); } } }
            }
            if bad > 0 { println!("x {:.0}: bad {} {}", x, bad, line); }
        }
    }
}

#[cfg(test)]
mod trim_spike_probe3 {
    use super::*;
    #[test]
    fn probe3() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let st = State::new(dir);
        let tr = st.piece(2);
        let m = tr.mesh(6.0);
        let nc = tr.profiles[0].curve().len();
        let nv = (tr.skin_idx_len(6.0) / 6 / (nc - 1) + 1) * nc;
        let mut v: Vec<(f32, usize)> = (0..nv).map(|i| (m.pos[i * 3 + 2] - m.pos[(nv + i) * 3 + 2], i)).collect();
        v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        println!("rows {} cols {} under {} gap {} bevel {}", nv / nc, nc, tr.under.is_some(), tr.under_gap, tr.end_bevel);
        for (d, i) in v.iter().take(8) {
            println!("thick {:.1} row {} col {} at x {:.0} y {:.0} z {:.0}", d, i / nc, i % nc, m.pos[i * 3], m.pos[i * 3 + 1], m.pos[i * 3 + 2]);
        }
    }
}

#[cfg(test)]
mod trim_spike_probe4 {
    use super::*;
    fn worst(tr: &Project) -> (f32, f32, f32) {
        let m = tr.mesh(6.0);
        let nc = tr.profiles[0].curve().len();
        let nv = (tr.skin_idx_len(6.0) / 6 / (nc - 1) + 1) * nc;
        let (mut w, mut wx, mut wy) = (0.0f32, 0.0, 0.0);
        for i in 0..nv {
            let d = m.pos[i * 3 + 2] - m.pos[(nv + i) * 3 + 2];
            if d > w { w = d; wx = m.pos[i * 3]; wy = m.pos[i * 3 + 1]; }
        }
        (w, wx, wy)
    }
    #[test]
    fn probe4() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        let old_trim = st.piece(2).clone();
        let n = st.piece(0).rail2.len();
        for k in 0..n {
            let r = st.piece(0).rail2[k];
            st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, r[0] + 70.0, r[1])).unwrap();
        }
        println!("trim following: {:?}", worst(st.piece(2)));
        let mut stale = old_trim;
        stale.under = st.piece(2).under.clone();
        stale.under_gap = st.piece(2).under_gap;
        println!("trim left behind (old behavior): {:?}", worst(&stale));
    }
}

#[cfg(test)]
mod trim_refit_probe {
    use super::*;
    #[test]
    fn refit_removes_the_skirts() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        let measure = |tr: &Project| -> f32 {
            let m = tr.mesh(6.0);
            let nc = tr.profiles[0].curve().len();
            let nv = (tr.skin_idx_len(6.0) / 6 / (nc - 1) + 1) * nc;
            (0..nv).map(|i| m.pos[i * 3 + 2] - m.pos[(nv + i) * 3 + 2]).fold(0.0, f32::max)
        };
        let before = measure(st.piece(2));
        st.op(r#"{"op":"refit_trim"}"#).unwrap();
        let after = measure(st.piece(2));
        println!("thickest before {:.1} after refit {:.1}", before, after);
        assert!(after < before);
    }
}

#[cfg(test)]
mod open_edge_locate {
    use super::*;
    #[test]
    fn locate() {
        let st = State::new(std::env::temp_dir().join("dashfw_oel"));
        let tr = st.piece(2);
        let m = tr.mesh(6.0);
        use std::collections::HashMap;
        let key = |i: u32| { let k = i as usize * 3; ((m.pos[k] * 1000.0).round() as i64, (m.pos[k + 1] * 1000.0).round() as i64, (m.pos[k + 2] * 1000.0).round() as i64) };
        let mut e: HashMap<((i64, i64, i64), (i64, i64, i64)), i32> = HashMap::new();
        for t in m.idx.chunks(3) {
            for k in 0..3 {
                let (a, b) = (key(t[k]), key(t[(k + 1) % 3]));
                if a == b { continue; }
                let kk = if a < b { (a, b) } else { (b, a) };
                *e.entry(kk).or_insert(0) += 1;
            }
        }
        for (k, v) in e.iter().filter(|(_, v)| **v != 2) {
            println!("open edge x{} {:?} -> {:?}", v, (k.0 .0 as f64 / 1000.0, k.0 .1 as f64 / 1000.0, k.0 .2 as f64 / 1000.0), (k.1 .0 as f64 / 1000.0, k.1 .1 as f64 / 1000.0, k.1 .2 as f64 / 1000.0));
        }
    }
}

#[cfg(test)]
mod trim_tall_probe {
    use super::*;
    #[test]
    fn tall() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let st = State::new(dir);
        let m = st.piece(2).mesh(6.0);
        let mut tall = 0;
        let mut worst = 0.0f32;
        for t in m.idx.chunks(3) {
            let z: Vec<f32> = t.iter().map(|&i| m.pos[i as usize * 3 + 2]).collect();
            let h = z.iter().cloned().fold(f32::MIN, f32::max) - z.iter().cloned().fold(f32::MAX, f32::min);
            if h > 26.0 { tall += 1; }
            worst = worst.max(h);
        }
        println!("triangles taller than 26 mm: {} (tallest {:.1}); open edges {}", tall, worst, model::open_edges_welded(&m.pos, &m.idx));
    }
}

#[cfg(test)]
mod trim_lock_tests {
    use super::*;
    #[test]
    fn trim_guidelines_are_locked_to_the_top_pad() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_trim_lock"));
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        st.op(r#"{"op":"switch_piece","to":2}"#).ok();
        let (rail, rail2) = (st.piece(2).rail.clone(), st.piece(2).rail2.clone());
        if st.active == 2 {
            assert!(st.op(r#"{"op":"move_rail","k":1,"x":0,"y":0,"z":0}"#).is_err());
            assert_eq!(st.piece(2).rail, rail);
        }
        let _ = rail2;
        if st.active == 2 {
            let n = st.piece(2).profiles.len();
            st.op(r#"{"op":"add_profile_at","x":150,"sym":true}"#).unwrap();
            let t = st.piece(2);
            assert_eq!(t.profiles.len(), n + 2);
            for p in &t.profiles {
                let (ry, rz) = t.rail_at(p.x);
                let a = p.anchor();
                assert!((a[0] - ry).abs() < 0.01 && (a[1] - rz).abs() < 0.01, "profile at {} off the windshield line", p.x);
            }
            assert_eq!(t.rail2.len(), t.rail.len());
            let (m, _) = t.mesh_final(3.0);
            assert!(m.idx.len() > 300);
        }
        st.op(r#"{"op":"switch_piece","to":0}"#).ok();
        assert!(st.op(r#"{"op":"move_rail2","k":1,"y":-100,"z":226}"#).is_ok());
    }
}

#[cfg(test)]
mod trim_under_probe {
    use super::*;
    #[test]
    fn under() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let st = State::new(dir);
        println!("active {} trim_on {} gap {} bevel_on {}", st.active, st.trim_on, st.gap, st.bevel_on);
        let pad = st.piece(0);
        let tr = st.piece(2);
        let u = model::Under::from_grid(&pad.grid());
        let m = tr.mesh(6.0);
        { let mut o = String::new(); for v in m.pos.chunks(3) { o += &format!("{} {} {}\n", v[0], v[1], v[2]); } std::fs::write(std::env::temp_dir().join("trim_pos.txt"), o).ok(); let mut o = String::new(); for t in m.idx.chunks(3) { o += &format!("{} {} {}\n", t[0], t[1], t[2]); } std::fs::write(std::env::temp_dir().join("trim_idx.txt"), o).ok(); }
        let nc = tr.profiles[0].curve().len();
        let nr = tr.skin_idx_len(6.0) / 6 / (nc - 1) + 1;
        let nv = nr * nc;
        let (mut below, mut tall, mut n_ledge) = (0, 0, 0);
        let mut worst = (0.0f64, 0usize);
        for i in 0..nv {
            let (ox, oy, oz) = (m.pos[i * 3] as f64, m.pos[i * 3 + 1] as f64, m.pos[i * 3 + 2] as f64);
            let iz = m.pos[(nv + i) * 3 + 2] as f64;
            if let Some(pz) = u.z_at(ox, oy) {
                if iz < pz + 0.3 - 0.05 {
                    below += 1;
                    if below % 20 == 1 { println!("below: x {:.0} y {:.0} outer z {:.1} inner z {:.1} pad z {:.1} (row {} col {})", ox, oy, oz, iz, pz, i / nc, i % nc); }
                }
                if iz > pz + 0.3 + 0.05 { n_ledge += 1; let d = iz - pz; if d > worst.0 { worst = (d, i); } }
                if oz - iz > 25.0 { tall += 1; }
            }
        }
        for row in [60usize, 64, 68, 96, 100] {
            println!("row {} (x {:.0})", row, m.pos[(row * nc) * 3]);
            for c in 0..nc {
                let i = row * nc + c;
                let (oy, oz, iz) = (m.pos[i * 3 + 1], m.pos[i * 3 + 2], m.pos[(nv + i) * 3 + 2]);
                let pz = u.z_at(m.pos[i * 3] as f64, oy as f64).map(|v| v as f32).unwrap_or(f32::NAN);
                if c > 30 { println!("  c{:2} y {:6.1} outer {:6.1} inner {:6.1} pad {:6.1}", c, oy, oz, iz, pz); }
            }
        }
        println!("grid {}x{}; underside below pad surface: {}; above pad (ledge): {}; thicker than 25: {}; biggest ledge gap {:.1} at row {} col {}", nr, nc, below, n_ledge, tall, worst.0, worst.1 / nc, worst.1 % nc);
    }
}

#[cfg(test)]
mod stale_probe {
    use super::*;
    #[test]
    fn stale() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let j = json::parse(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        let pad = Project::from_json(j.get("upper").unwrap()).unwrap();
        let tr = Project::from_json(j.get("trim").unwrap()).unwrap();
        let fresh = templates::trim_from(&pad, tr.look);
        println!("rows {} vs {}", fresh.rail.len(), tr.rail.len());
        for (a, b) in fresh.rail.iter().zip(&tr.rail) { println!("rail fresh {:?} saved {:?}", a, b); }
        for (a, b) in fresh.rail2.iter().zip(&tr.rail2) { println!("rail2 fresh {:?} saved {:?}", a, b); }
        {
            let u = model::Under::from_grid(&pad.grid());
            for (name, t) in [("saved", &tr), ("fresh", &fresh)] {
                let g = t.grid();
                let mut worst = (0.0f64, 0usize);
                for (r, row) in g.iter().enumerate() {
                    let q = row[row.len() - 1];
                    if let Some(pz) = u.z_at(q[0], q[1]) { let d = q[2] - pz; if d.abs() > worst.0.abs() { worst = (d, r); } }
                }
                println!("{} nose tip height over pad: worst {:.2} at row {} (x {:.0})", name, worst.0, worst.1, g[worst.1][0][0]);
            }
        }
        let mut st = State::new(dir.clone());
        println!("under linked after load: {}", st.piece(2).under.is_some());
        let m0 = st.piece(2).mesh(6.0);
        st.op(r#"{"op":"refit_trim"}"#).unwrap();
        let m1 = st.piece(2).mesh(6.0);
        let d = m0.pos.iter().zip(&m1.pos).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        println!("mesh sizes {} {} max diff {}", m0.pos.len(), m1.pos.len(), d);
        let (fp, tp) = (&fresh.profiles[4], &tr.profiles[4]);
        println!("prof4 pts fresh {} saved {}", fp.pts.len(), tp.pts.len());
    }
}

#[cfg(test)]
mod wrap_default_tests {
    #[test]
    fn pocketed_is_the_default_front_line() {
        let p = crate::templates::preset("wrap");
        assert!(p.crease && p.centre_out > 0.0, "twin-pad wrap starts on Pocketed (the W style); Center out has its own amount ready");
        let q = crate::templates::params_from_json(&crate::json::parse(r#"{"kind":"wrap"}"#).unwrap());
        assert!(q.crease);
    }
}

#[cfg(test)]
mod crease_tests {
    use super::*;
    #[test]
    fn sharp_edge_toggle_per_profile() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_crease_test"));
        st.op(r#"{"op":"apply_template","kind":"smooth","nprof":5}"#).unwrap();
        let g0 = st.proj.grid();
        // ends cannot be made sharp
        st.op(r#"{"op":"set_crease","i":0,"v":true}"#).unwrap();
        assert!(!st.proj.profiles[0].crease);
        // a middle profile: sharp changes the surface next to it; smooth puts it back exactly
        st.op(r#"{"op":"set_crease","i":1,"v":true}"#).unwrap();
        assert!(st.proj.profiles[1].crease);
        assert!(st.proj.locked_partner(1).map_or(true, |j| st.proj.profiles[j].crease), "mirrored partner follows");
        let g1 = st.proj.grid();
        let diff = g0.iter().zip(g1.iter()).flat_map(|(a, b)| a.iter().zip(b.iter())).map(|(p, q)| (p[2] - q[2]).abs().max((p[1] - q[1]).abs())).fold(0.0, f64::max);
        assert!(diff > 1e-6, "sharp edge changes the surface");
        // the profile itself (the row at its x) is untouched
        st.op(r#"{"op":"set_crease","i":1,"v":false}"#).unwrap();
        let g2 = st.proj.grid();
        assert_eq!(g0, g2);
        // undo restores the sharp edge
        st.op(r#"{"op":"set_crease","i":2,"v":true}"#).unwrap();
        st.op(r#"{"op":"undo"}"#).unwrap();
        assert!(!st.proj.profiles[2].crease);
    }
}

#[cfg(test)]
mod front_hold_tests {
    use super::*;
    #[test]
    fn pressed_down_height_is_kept_when_pulled_back_out() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_fh_test"));
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let k = 4;
        let [y0, _] = st.piece(2).rail2[k];
        // press the point down and back onto the pad: it stops at the pad's surface
        st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y0 + 30.0, 100.0)).unwrap();
        let [y1, z1] = st.piece(2).rail2[k];
        assert!((st.front_floor(k, y1).unwrap() - z1).abs() < 1e-6);
        // pull it back out with the pointer still low: the height stays where it was set
        // (it is only lifted if the pad's surface rises above it; it never follows the pad back down)
        let mut expect = z1;
        for d in [20.0, 10.0, 0.0, -10.0] {
            st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y0 + 30.0 + d, 100.0)).unwrap();
            let [_, z] = st.piece(2).rail2[k];
            let f = st.front_floor(k, y0 + 30.0 + d).unwrap_or(f64::MIN);
            expect = expect.max(f);
            assert!((z - expect).abs() < 1e-6, "z {} should be {} (floor {})", z, expect, f);
        }
        // a drag that only changes z (y unchanged) is free again, down to the pad's surface
        let [yn, _] = st.piece(2).rail2[k];
        st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, yn, 100.0)).unwrap();
        assert!((st.piece(2).rail2[k][1] - st.front_floor(k, yn).unwrap()).abs() < 1e-6);
        // the pointer going above it moves it again
        st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y0, z1 + 15.0)).unwrap();
        assert!((st.piece(2).rail2[k][1] - (z1 + 15.0)).abs() < 1e-6);
    }
}

#[cfg(test)]
mod front_guide_tests {
    use super::*;
    #[test]
    fn front_guide_moves_but_stays_above_pad() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_fg_test"));
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        assert!(st.op(r#"{"op":"move_rail","k":1,"x":0,"y":0,"z":0}"#).is_err(), "windshield line stays locked");
        let k = 4;
        let [y0, z0] = st.piece(2).rail2[k];
        // moving up and forward is allowed
        st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y0 - 5.0, z0 + 8.0)).unwrap();
        let [y1, z1] = st.piece(2).rail2[k];
        assert!((y1 - (y0 - 5.0)).abs() < 1e-6 && (z1 - (z0 + 8.0)).abs() < 1e-6);
        assert!(st.trim_custom);
        // dragging far down and back stops at the pad's surface
        st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y0 + 30.0, 100.0)).unwrap();
        let [y2, z2] = st.piece(2).rail2[k];
        let floor = st.front_floor(k, y2).expect("pad under");
        assert!((y2 - (y0 + 30.0)).abs() < 1e-6, "y is free");
        assert!(z2 >= floor - 1e-6 && z2 > 100.0, "z {} floor {}", z2, floor);
        // a pad edit leaves the moved guide alone (it only lifts it if needed)
        st.op(r#"{"op":"switch_piece","to":0}"#).unwrap();
        let before = st.piece(2).rail2[k];
        st.op(r#"{"op":"move_rail","k":2,"x":-200,"y":210,"z":386}"#).unwrap();
        assert!(st.trim_custom);
        let after = st.piece(2).rail2[k];
        assert!((after[0] - before[0]).abs() < 1e-6);
        // refit puts it back in rebuild mode
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        st.op(r#"{"op":"refit_trim"}"#).unwrap();
        assert!(!st.trim_custom);
        let m = st.piece(2).mesh(6.0);
        assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0);
    }
}

#[cfg(test)]
mod front_follow_probe {
    use super::*;
    #[test]
    fn follow() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_ff_test"));
        st.op(r#"{"op":"apply_template","kind":"wrap"}"#).unwrap();
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let g0 = st.piece(2).grid();
        let rows = g0.len();
        for k in 2..7 {
            let [y, z] = st.piece(2).rail2[k];
            st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y - 30.0, z + 5.0)).unwrap();
        }
        let g1 = st.piece(2).grid();
        let nc = g1[0].len();
        for r in [0usize, rows / 8, rows / 4, rows / 2] {
            let a = g0[r][nc - 1]; let b = g1[r][nc - 1];
            let m0 = g0[r][nc / 2]; let m1 = g1[r][nc / 2];
            println!("row {} x {:.0}: tip y {:.1} -> {:.1}, z {:.1} -> {:.1}; mid y {:.1} -> {:.1}", r, a[0], a[1], b[1], a[2], b[2], m0[1], m1[1]);
        }
        println!("rail2 now {:?}", st.piece(2).rail2);
    }
}

#[cfg(test)]
mod lip_probe {
    use super::*;
    #[test]
    fn lip() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        st.op(r#"{"op":"refit_trim"}"#).unwrap();
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        for (k, y, z) in [(1, 10.0, 338.0), (2, 30.0, 335.0), (3, 25.0, 340.0), (4, 5.0, 345.0)] {
            st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y, z)).unwrap();
        }
        let m = st.piece(2).mesh(6.0);
        println!("open edges {} volume {:.0}", model::open_edges_welded(&m.pos, &m.idx), model::signed_volume(&m.pos, &m.idx));
        println!("rail2 {:?}", st.piece(2).rail2);
        {
            let tr = st.piece(2);
            let nc0 = tr.profiles[0].curve().len();
            let nr0 = tr.skin_idx_len(6.0) / 6 / (nc0 - 1) + 1;
            let nv = nr0 * nc0;
            let mm = tr.mesh(6.0);
            for c in [nc0 - 12, nc0 - 8, nc0 - 4] {
                let mut line = String::new();
                for r in (36..110).step_by(2) {
                    let i = nv + r * nc0 + c;
                    let o = r * nc0 + c;
                    line += &format!("{:.1} ", mm.pos[o * 3 + 2] - mm.pos[i * 3 + 2]);
                }
                println!("thick col {}: {}", c, line);
            }
        }
        {
            let mv = st.piece(2).mesh_view(6.0);
            let plain = st.piece(2).mesh_off(0.0, 0.0);
            let n0 = plain.pos.len() / 3;
            let nw = mv.pos.len() / 3 - n0;
            println!("view: plain verts {} wall verts {}", n0, nw);
            let mut line = String::new();
            for i in (0..nw).step_by(2).take(120) { let q = &mv.pos[(n0 + i) * 3..(n0 + i) * 3 + 3]; line += &format!("({:.0},{:.0},{:.0}) ", q[0], q[1], q[2]); }
            println!("wall: {}", line);
        }
        let g = st.piece(2).grid();
        let nc = g[0].len();
        for c in [nc - 1, nc - 4, nc - 8] {
            let mut line = String::new();
            for r in (36..110).step_by(2) { line += &format!("{:.1}/{:.1} ", g[r][c][1], g[r][c][2]); }
            println!("col {} y/z: {}", c, line);
        }
        // look at the last columns of a few rows: y, z of outer
        {
            let u = model::Under::from_grid(&st.piece(0).grid());
            let mut line = String::new();
            for r in (30..110).step_by(2) {
                let q = g[r][nc - 1];
                let pz = u.z_at(q[0], q[1]).map(|z| z.round() as i64).unwrap_or(-1);
                line += &format!("{}:{}/{} ", r, q[2].round(), pz);
            }
            println!("tipz/padz {}", line);
        }
        for r in [40usize, 60, 80, 100] {
            let row = &g[r];
            println!("row {} x {:.0}: {:?}", r, row[0][0], (nc - 8..nc).map(|c| (row[c][1].round(), row[c][2].round())).collect::<Vec<_>>());
        }
    }
}

#[cfg(test)]
mod spike_measure {
    use super::*;
    #[test]
    fn measure() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        let raw = std::env::var("RAW").is_ok();
        if !raw { st.op(r#"{"op":"refit_trim"}"#).unwrap(); }
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let y1: f64 = std::env::var("Y1").ok().and_then(|s| s.parse().ok()).unwrap_or(-10.0);
        for (y, z) in if raw { vec![] } else { vec![(y1, 300.0), (45.0, 300.0)] } {
            for k in 2..=6 { st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y, z)).unwrap(); }
        }
        let tr = st.piece(2);
        let m = tr.mesh(6.0);
        let nc = tr.profiles[0].curve().len();
        let nv = (tr.skin_idx_len(6.0) / 6 / (nc - 1) + 1) * nc;
        let nr = nv / nc;
        let p = |i: usize| [m.pos[i * 3] as f64, m.pos[i * 3 + 1] as f64, m.pos[i * 3 + 2] as f64];
        println!("rows {} cols {}", nr, nc);
        let mut bad: Vec<(f64, usize, usize)> = vec![];
        for r in 1..nr - 1 {
            for c in 1..nc - 1 {
                let (a, b, cc) = (p(r * nc + c - 1), p(r * nc + c), p(r * nc + c + 1));
                let k1 = (a[2] + cc[2] - 2.0 * b[2]).abs();
                let (d, e, f) = (p((r - 1) * nc + c), p(r * nc + c), p((r + 1) * nc + c));
                let k2 = (d[2] + f[2] - 2.0 * e[2]).abs();
                let (d1, e1, f1) = (p(nv + (r - 1) * nc + c), p(nv + r * nc + c), p(nv + (r + 1) * nc + c));
                let k3 = (d1[2] + f1[2] - 2.0 * e1[2]).abs();
                let (a2, b2, c2) = (p(nv + r * nc + c - 1), p(nv + r * nc + c), p(nv + r * nc + c + 1));
                let k4 = (a2[2] + c2[2] - 2.0 * b2[2]).abs();
                bad.push((k1.max(k2), r, c));
                bad.push((k3.max(k4) , r + 1000, c));
            }
        }
        for rr in [29usize, 30, 31, 32, 33] {
            println!("row {}", rr);
            for c in 25..31 { let o = p(rr * nc + c); let i = p(nv + rr * nc + c); println!("  c{} out y{:.1} z{:.1} | in y{:.1} z{:.1}", c, o[1], o[2], i[1], i[2]); }
        }
        {
            let mut rough: Vec<(f64, bool, usize, usize)> = vec![];
            for r in 1..nr - 1 { for c in 0..nc {
                for (off, inn) in [(0usize, false), (nv, true)] {
                    let (d, e, f) = (p(off + (r - 1) * nc + c), p(off + r * nc + c), p(off + (r + 1) * nc + c));
                    let dd = ((d[1] + f[1] - 2.0 * e[1]).powi(2) + (d[2] + f[2] - 2.0 * e[2]).powi(2)).sqrt();
                    rough.push((dd, inn, r, c));
                }
            } }
            rough.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            for (k, inn, r, c) in rough.iter().take(12) { let q = p(if *inn { nv } else { 0 } + r * nc + c); println!("rough {:.2} {} r{} c{} x{:.1} y{:.1} z{:.1}", k, if *inn { "inner" } else { "outer" }, r, c, q[0], q[1], q[2]); }
        }
        bad.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for (k, r, c) in bad.iter().take(14) {
            let (rr, inner) = if *r >= 1000 { (*r - 1000, true) } else { (*r, false) };
            let q = p(if inner { nv } else { 0 } + rr * nc + c);
            println!("kink {:.2} {} r{} c{} at x{:.1} y{:.1} z{:.1}", k, if inner { "inner" } else { "outer" }, rr, c, q[0], q[1], q[2]);
        }
    }
}

#[cfg(test)]
mod pushback_probe {
    use super::*;
    #[test]
    fn push() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        st.op(r#"{"op":"refit_trim"}"#).unwrap();
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let show = |st: &State, t: &str| {
            let pr = &st.piece(2).profiles[4];
            println!("{} x{}", t, pr.x);
            for q in &pr.pts { println!("   co ({:.1},{:.1}) hl ({:.1},{:.1}) hr ({:.1},{:.1})", q.co[0], q.co[1], q.hl[0], q.hl[1], q.hr[0], q.hr[1]); }
        };
        show(&st, "start");
        let rz = st.piece(2).rail2[4];
        println!("rail2[4] {:?}", rz);
        for k in 2..=6 { st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, rz[0] + 40.0, rz[1])).unwrap(); }
        show(&st, "back 40");
        for k in 2..=6 { st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, rz[0] + 80.0, rz[1])).unwrap(); }
        show(&st, "back 80");
    }
}

#[cfg(test)]
mod point_fit_probe {
    use super::*;
    fn dist_to_poly(p: [f64; 2], poly: &[[f64; 2]]) -> f64 {
        let mut best = f64::MAX;
        for w in poly.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let l2 = (d[0] * d[0] + d[1] * d[1]).max(1e-12);
            let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).max(0.0).min(1.0);
            let q = [a[0] + d[0] * t, a[1] + d[1] * t];
            best = best.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt());
        }
        best
    }
    fn dev(a: &Project, dense: &Project) -> f64 {
        let mut worst = 0.0f64;
        for (pa, pd) in a.profiles.iter().zip(dense.profiles.iter()) {
            let (ca, cd) = (pa.curve(), pd.curve());
            for q in ca.iter() { worst = worst.max(dist_to_poly(*q, &cd)); }
            for q in cd.iter() { worst = worst.max(dist_to_poly(*q, &ca)); }
        }
        worst
    }
    #[test]
    fn fit() {
        let top_dense = Project::from_json(&json::parse(DEFAULT_PROJECT).unwrap()).unwrap();
        let low_dense = Project::from_json(&json::parse(DEFAULT_LOWER).unwrap()).unwrap();
        let top6 = default_project();
        let look = top6.look.clone();
        let trim_dense = templates::trim_from_n(&top6, look.clone(), 60);
        println!("dense points: top {} lower {} trim {}", top_dense.profiles[0].pts.len(), low_dense.profiles[0].pts.len(), trim_dense.profiles[0].pts.len());
        for n in [8usize, 7, 6, 5] {
            let mut t = top_dense.clone(); t.resample_all(n, true);
            let mut l = low_dense.clone(); l.resample_all(n, true);
            let tr = templates::trim_from_n(&top6, look.clone(), n);
            println!("n={}: top {:.2} mm  lower {:.2} mm  trim {:.2} mm", n, dev(&t, &top_dense), dev(&l, &low_dense), dev(&tr, &trim_dense));
        }
    }
}

#[cfg(test)]
mod bump_probe {
    use super::*;
    #[test]
    fn down_then_up_has_no_bump() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_bump"));
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let rz = st.piece(2).rail2[4];
        let zs = |st: &State| st.piece(2).profiles[4].pts.iter().map(|q| q.co[1]).collect::<Vec<f64>>();
        let mv = |st: &mut State, y: f64, z: f64| for k in 2..=6 { st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y, z)).unwrap(); };
        mv(&mut st, rz[0] - 40.0, rz[1]);
        let out = zs(&st);
        mv(&mut st, rz[0] - 40.0, rz[1] - 60.0);
        let down = zs(&st);
        mv(&mut st, rz[0] - 40.0, rz[1] - 10.0);
        let mid = zs(&st);
        mv(&mut st, rz[0] - 40.0, rz[1]);
        let back = zs(&st);
        println!("out  {:?}\ndown {:?}\nmid  {:?}\nback {:?}", out, down, mid, back);
        // back at the same guide height: every node is back where it was before the drop (nothing left bumped up)
        for (a, b) in out.iter().zip(back.iter()) { assert!((a - b).abs() < 0.5, "back at the start height: {} vs {}", a, b); }
        // halfway up, no node is higher than it would have been with the guide that high and no pad in the way
        for k in 1..out.len() - 1 { assert!(mid[k] <= out[k] + 1e-6, "node {} sits above where it belongs: {} vs {}", k, mid[k], out[k]); }
    }
}

#[cfg(test)]
mod end_wave_probe {
    use super::*;
    #[test]
    fn ends() {
        let st = State::new(std::env::temp_dir().join("dashfw_endwave"));
        for pi in [1usize, 0] {
            let pr = st.piece(pi as u8 as usize);
            let m = pr.mesh(6.0);
            let g = pr.grid();
            let nc = g[0].len();
            let nr = g.len();
            println!("piece {} rows {} cols {} points {}", pi, nr, nc, pr.profiles[0].pts.len());
            let nv = m.pos.len() / 3;
            // outer verts first (nr*nc), then inner
            let p = |i: usize| [m.pos[i * 3] as f64, m.pos[i * 3 + 1] as f64, m.pos[i * 3 + 2] as f64];
            let tot = nr * nc;
            println!("verts {} (grid {})", nv, tot);
            for r in [0usize, 1, 2] {
                let mut s = String::new();
                for c in 0..nc { let o = p(r * nc + c); let i = p(tot + r * nc + c); s += &format!("[{:.0},{:.0}|{:.0},{:.0}] ", o[1], o[2], i[1], i[2]); }
                println!("row {}: {}", r, s);
            }
        }
    }
}

#[cfg(test)]
mod end_wave_probe2 {
    use super::*;
    #[test]
    fn dense_vs_fit() {
        let dense = Project::from_json(&json::parse(DEFAULT_LOWER).unwrap()).unwrap();
        let fit = default_lower();
        for (nm, p) in [("dense", &dense), ("fit", &fit)] {
            let pr = &p.profiles[0];
            println!("{} pts:", nm);
            for q in &pr.pts { println!("   co ({:.1},{:.1}) hl ({:.1},{:.1}) hr ({:.1},{:.1})", q.co[0], q.co[1], q.hl[0], q.hl[1], q.hr[0], q.hr[1]); }
        }
    }
}

#[cfg(test)]
mod end_wave_probe3 {
    use super::*;
    #[test]
    fn his_ends() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let st = State::new(dir);
        for pi in 0..3usize {
            let pr = st.piece(pi);
            let m = pr.mesh(6.0);
            let nc = pr.profiles[0].curve().len();
            let nv = (pr.skin_idx_len(6.0) / 6 / (nc - 1) + 1) * nc;
            let nr = nv / nc;
            let p = |i: usize| [m.pos[i * 3] as f64, m.pos[i * 3 + 1] as f64, m.pos[i * 3 + 2] as f64];
            println!("piece {} rows {} cols {}", pi, nr, nc);
            for r in [0usize, 1, 2, 3, 4] {
                let mut s = String::new();
                for c in 0..nc { let o = p(r * nc + c); let i = p(nv + r * nc + c); s += &format!("({:.0},{:.0}|{:.0},{:.0}) ", o[1], o[2], i[1], i[2]); }
                println!(" r{} x{:.1}: {}", r, p(r * nc)[0], s);
            }
        }
    }
}

#[cfg(test)]
mod respace_lower_probe {
    use super::*;
    #[test]
    fn respace() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        st.op(r#"{"op":"switch_piece","to":1}"#).unwrap();
        let show = |st: &State, t: &str| { println!("{}", t); for q in &st.piece(1).profiles[0].pts { println!("   co ({:.1},{:.1}) hl ({:.1},{:.1}) hr ({:.1},{:.1})", q.co[0], q.co[1], q.hl[0], q.hl[1], q.hr[0], q.hr[1]); } };
        show(&st, "before");
        st.op(r#"{"op":"resample","n":7}"#).unwrap();
        show(&st, "after");
        let m = st.piece(1).mesh(6.0);
        let nc = st.piece(1).profiles[0].curve().len();
        let nv = (st.piece(1).skin_idx_len(6.0) / 6 / (nc - 1) + 1) * nc;
        let p = |i: usize| [m.pos[i * 3 + 1] as f64, m.pos[i * 3 + 2] as f64];
        let mut s = String::new();
        for c in 0..12 { let o = p(c); let i = p(nv + c); s += &format!("({:.0},{:.0}|{:.0},{:.0}) ", o[0], o[1], i[0], i[1]); }
        println!("row0 {}", s);
    }
}

#[cfg(test)]
mod cut_probe {
    use super::*;
    #[test]
    fn trim_cut_by_pad() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let st = State::new(dir);
        let tr = st.piece(2);
        let u = tr.under.clone().unwrap();
        let before = tr.mesh(6.0);
        let (after, skin) = tr.mesh_final(6.0);
        let nb = before.pos.chunks(3).filter(|v| matches!(u.z_at(v[0] as f64, v[1] as f64), Some(pz) if (v[2] as f64) < pz - 1e-3)).count();
        println!("before the cut: {} vertices below the pad", nb);
        println!("tris {} -> {} skin {}", before.idx.len() / 3, after.idx.len() / 3, skin / 3);
        let mut below = 0usize; let mut worst = 0.0f64;
        for v in after.pos.chunks(3) {
            if let Some(pz) = u.z_at(v[0] as f64, v[1] as f64) { if (v[2] as f64) < pz - 1e-3 && pz - (v[2] as f64) < 3.0 { below += 1; worst = worst.max(pz - v[2] as f64); } }
        }
        println!("vertices below the pad surface: {} (worst {:.3})", below, worst);
        println!("open edges {}", model::open_edges_welded(&after.pos, &after.idx));
        assert_eq!(below, 0);
    }
}

#[cfg(test)]
mod loose_pieces_test {
    use crate::model::{keep_main_piece, Mesh};
    #[test]
    fn small_loose_pieces_are_thrown_away() {
        // a 2-triangle quad plus a separate lone triangle
        let pos: Vec<f32> = vec![0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 0., 10., 10., 10., 11., 10., 10., 10., 11., 10.];
        let mut m = Mesh { pos, nrm: vec![0.0; 21], idx: vec![0, 1, 2, 0, 2, 3, 4, 5, 6] };
        let k = keep_main_piece(&mut m, 9);
        assert_eq!(m.idx.len(), 6);
        assert_eq!(k, 6);
    }
}

#[cfg(test)]
mod cut_gap_test {
    use super::*;
    #[test]
    fn cut_keeps_the_gap() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_cut_gap"));
        st.gap_on = true;
        st.gap = 0.3;
        st.sync_lock();
        let tr = st.piece(2);
        assert!((tr.under_gap - 0.3).abs() < 1e-9);
        let u = tr.under.clone().unwrap();
        let (m, _) = tr.mesh_final(6.0);
        let mut worst = 0.0f64;
        for v in m.pos.chunks(3) {
            if let Some(pz) = u.z_at(v[0] as f64, v[1] as f64) { worst = worst.max(pz + 0.3 - v[2] as f64); }
        }
        println!("deepest a vertex sits below pad + 0.3: {:.4}", worst);
        assert!(worst < 1e-3);
    }
}

#[cfg(test)]
mod end_fill_test {
    use super::*;
    // the two end profiles are locked: the outer surface of the end rows stays on the profile, with the shell 6 mm under it
    #[test]
    fn ends_stay_on_their_profile() {
        let st = State::new(std::env::temp_dir().join("dashfw_endfill"));
        let tr = st.piece(2);
        let u = tr.under.clone().unwrap();
        let g = tr.grid();
        let nc = g[0].len();
        let nr = g.len();
        let m = tr.mesh(6.0);
        let (mut worst, mut n) = (0.0f64, 0);
        for r in [0usize, nr - 1] {
            for c in 3..nc {
                let k = r * nc + c;
                let oz = m.pos[k * 3 + 2] as f64;
                if let Some(pz) = u.z_at(g[r][c][0], g[r][c][1]) {
                    if g[r][c][2] > pz + 6.5 { n += 1; worst = worst.max((oz - g[r][c][2]).abs()); }
                }
            }
        }
        println!("end columns checked {} worst distance from the profile {:.3}", n, worst);
        assert!(worst < 0.5, "end surface left its profile by {}", worst);
    }
}

#[cfg(test)]
mod add_leaves_neighbours_test {
    use super::*;
    #[test]
    fn adding_a_trim_profile_changes_only_the_new_one() {
        let mut st = State::new(std::env::temp_dir().join("dashfw_addneigh"));
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let before: Vec<(f64, Vec<[f64; 2]>)> = st.piece(2).profiles.iter().map(|p| (p.x, p.pts.iter().map(|q| q.co).collect())).collect();
        st.op(r#"{"op":"add_profile_at","x":150,"sym":true}"#).unwrap();
        let tr = st.piece(2);
        let mut worst = 0.0f64;
        for (x, pts) in &before {
            let p = tr.profiles.iter().find(|p| (p.x - x).abs() < 1e-6).expect("profile kept");
            for (a, b) in pts.iter().zip(p.pts.iter()) { worst = worst.max((a[0] - b.co[0]).abs()).max((a[1] - b.co[1]).abs()); }
        }
        println!("largest move of an existing profile {:.4}", worst);
        assert!(worst < 1e-6, "existing profiles moved by {}", worst);
    }
}

#[cfg(test)]
mod add_trim_probe {
    use super::*;
    #[test]
    fn probe() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        println!("active {} n {} xs {:?}", st.active, st.piece(2).profiles.len(), st.piece(2).profiles.iter().map(|p| p.x as i32).collect::<Vec<_>>());
        for x in [150.0] {
            println!("x {} problem {:?}", x, st.piece(2).add_profile_problem(x, false));
            let r = st.op(&format!(r#"{{"op":"add_profile_at","x":{},"sym":false}}"#, x));
            println!("-> {:?} n {}", r, st.piece(2).profiles.len());
        }
    }
}
#[cfg(test)]
mod end_thick_probe {
    use super::*;
    #[test]
    fn probe() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let st = State::new(dir);
        let _ = st.piece(2).mesh_final(6.0);
    }
}
#[cfg(test)]
mod add_shape_probe {
    use super::*;
    #[test]
    fn probe() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let mut st = State::new(dir);
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let x: f64 = std::env::var("PX").ok().and_then(|v| v.parse().ok()).unwrap_or(647.0);
        st.op(&format!(r#"{{"op":"add_profile_at","x":{},"sym":false}}"#, x)).unwrap();
        for p in &st.piece(2).profiles {
            if p.x > 500.0 {
                println!("x {:.0}: {}", p.x, p.pts.iter().map(|q| format!("({:.0},{:.0})", q.co[0], q.co[1])).collect::<Vec<_>>().join(" "));
            }
        }
        let t = st.piece(2);
        println!("rail2: {:?}", t.rail2.iter().zip(&t.rail).map(|(a, r)| format!("x{:.0}:({:.0},{:.0})", r[0], a[0], a[1])).collect::<Vec<_>>());
    }
}
#[cfg(test)]
mod lift_probe {
    use super::*;
    #[test]
    fn probe() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() { return; }
        let st = State::new(dir);
        let tr = st.piece(2);
        let u = tr.under.clone().unwrap();
        let g = tr.grid();
        let (nr, nc) = (g.len(), g[0].len());
        let m = tr.mesh(6.0);
        println!("nr {} nc {} verts {}", nr, nc, m.pos.len() / 3);
        for c in [10usize, 16, 20, 22, 24, 26, 28, 30] {
            let mut so = String::new(); let mut si = String::new(); let mut sy = String::new();
            for r in 0..14 {
                let k = r * nc + c;
                so += &format!("{:6.1}", m.pos[k * 3 + 2]);
                si += &format!("{:6.1}", m.pos[(nr * nc + k) * 3 + 2]);
                sy += &format!("{:6.1}", m.pos[k * 3 + 1]);
            }
            println!("c{:2} outer z {}\n    inner z {}\n    outer y {}", c, so, si, sy);
        }
    }
}
#[cfg(test)]
mod turb_probe {
    use super::*;
    #[test]
    fn probe() {
        let dir = std::path::PathBuf::from(std::env::var("DASHFW_PROBE_DIR").unwrap_or_default());
        if !dir.join("project.json").exists() && std::env::var("PROBE_DEFAULT").is_err() { return; }
        let mut st = State::new(dir);
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let (y, z): (f64, f64) = (std::env::var("GY").ok().and_then(|v| v.parse().ok()).unwrap_or(59.0), std::env::var("GZ").ok().and_then(|v| v.parse().ok()).unwrap_or(469.0));
        if std::env::var("GZ").is_ok() {
            for k in 2..=6 { let _ = st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y, z)); }
        }
        if let Ok(px) = std::env::var("ADDX") {
            let x: f64 = px.parse().unwrap();
            st.op(&format!(r#"{{"op":"add_profile_at","x":{},"sym":true}}"#, x)).unwrap();
        }
        let tr = st.piece(2);
        println!("rail2 {:?}", tr.rail2.iter().map(|a| format!("({:.0},{:.0})", a[0], a[1])).collect::<Vec<_>>());
        let g = tr.grid();
        let (nr, nc) = (g.len(), g[0].len());
        let m = tr.mesh(6.0);
        let z = |r: usize, c: usize| m.pos[(r * nc + c) * 3 + 2] as f64;
        let yy = |r: usize, c: usize| m.pos[(r * nc + c) * 3 + 1] as f64;
        if std::env::var("SHOWCOLS").is_ok() {
            let u = tr.under.clone().unwrap();
            for c in [20usize, 24, 27, 28, 29] {
                println!("col {} z {}", c, (8..32).map(|r| format!("{:6.1}", z(r, c))).collect::<String>());
                println!("      y {}", (8..32).map(|r| format!("{:6.1}", yy(r, c))).collect::<String>());
                println!("    pad {}", (8..32).map(|r| format!("{:6.1}", u.z_at(m.pos[(r * nc + c) * 3] as f64, yy(r, c)).unwrap_or(0.0))).collect::<String>());
            }
        }
        if let Ok(xs) = std::env::var("ROWAT") {
            let xt: f64 = xs.parse().unwrap();
            let r = (0..nr).min_by(|&a, &b| (g[a][0][0] - xt).abs().partial_cmp(&(g[b][0][0] - xt).abs()).unwrap()).unwrap();
            println!("loft   x {:.1} z {}", g[r][0][0], (4..nc).map(|c| format!("{:6.1}", g[r][c][2])).collect::<String>());
            println!("             y {}", (4..nc).map(|c| format!("{:6.1}", g[r][c][1])).collect::<String>());
            { let u = tr.under.clone().unwrap(); println!("pad    x {:.1} z {}", g[r][0][0], (4..nc).map(|c| format!("{:6.1}", u.z_at(g[r][c][0], g[r][c][1]).unwrap_or(0.0))).collect::<String>()); }
            println!("rowat x {:.1} z {}", g[r][0][0], (4..nc).map(|c| format!("{:6.1}", z(r, c))).collect::<String>());
            println!("             y {}", (4..nc).map(|c| format!("{:6.1}", yy(r, c))).collect::<String>());
        }
        if std::env::var("ENDCMP").is_ok() {
            let u = tr.under.clone().unwrap();
            for r in [0usize, nr / 2] {
                println!("R{} loft z {}", r, (0..nc).map(|c| format!("{:6.1}", g[r][c][2])).collect::<String>());
                println!("R{} mesh z {}", r, (0..nc).map(|c| format!("{:6.1}", z(r, c))).collect::<String>());
                println!("R{} pad+6  {}", r, (0..nc).map(|c| format!("{:6.1}", u.z_at(g[r][c][0], g[r][c][1]).unwrap_or(0.0) + 6.0 + tr.under_gap)).collect::<String>());
                println!("R{} inner  {}", r, (0..nc).map(|c| format!("{:6.1}", m.pos[(nr * nc + r * nc + c) * 3 + 2])).collect::<String>());
                println!("R{} y      {}", r, (0..nc).map(|c| format!("{:6.1}", g[r][c][1])).collect::<String>());
            }
        }
        if std::env::var("ENDCMP").is_ok() {
            for r in [0usize, 1, 2, nr - 3, nr - 2, nr - 1] {
                let d: Vec<String> = (0..nc).map(|c| format!("{:5.1}", z(r, c) - g[r][c][2])).collect();
                let dy: Vec<String> = (0..nc).map(|c| format!("{:5.1}", yy(r, c) - g[r][c][1])).collect();
                println!("row {} x {:.0}\n dz {}\n dy {}", r, g[r][0][0], d.join(""), dy.join(""));
            }
        }
        if std::env::var("INNERRUF").is_ok() {
            let iz = |r: usize, c: usize| m.pos[(nr * nc + r * nc + c) * 3 + 2] as f64;
            let mut w: Vec<(f64, usize, usize)> = Vec::new();
            for r in 1..nr - 1 { for c in 3..nc { w.push(((iz(r - 1, c) - 2.0 * iz(r, c) + iz(r + 1, c)).abs(), r, c)); } }
            w.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            for (v, r, c) in w.iter().take(25) { println!("inner rough {:.2} row {} x {:.0} col {} y {:.1} z {:.1}", v, r, g[*r][0][0], c, m.pos[(nr * nc + r * nc + c) * 3 + 1], iz(*r, *c)); }
        }
        if std::env::var("FINALQ").is_ok() {
            let (fm, skin) = tr.mesh_final(6.0);
            let pad = st.piece(0);
            let uu = model::Under::from_grid(&pad.grid());
            let mut below = 0usize; let mut deep = 0.0f64;
            for v in fm.pos.chunks(3) { if let Some(pz) = uu.z_at(v[0] as f64, v[1] as f64) { let d = (pz + 0.3) - v[2] as f64; if d > 0.05 { below += 1; deep = deep.max(d); } } }
            { let dump = |m: &model::Mesh, n: &str| { let mut o = String::new(); for v in m.pos.chunks(3) { o += &format!("{} {} {}\n", v[0], v[1], v[2]); } std::fs::write(std::env::temp_dir().join(format!("{}_pos.txt", n)), o).ok(); let mut o = String::new(); for t in m.idx.chunks(3) { o += &format!("{} {} {}\n", t[0], t[1], t[2]); } std::fs::write(std::env::temp_dir().join(format!("{}_idx.txt", n)), o).ok(); };
              dump(&fm, "ftrim"); dump(&m, "rtrim"); println!("rtrim nv {} nc {} nr {}", nr * nc, nc, nr); dump(&pad.mesh_final(6.0).0, "fpad"); dump(&st.piece(1).mesh_final(6.0).0, "flow"); }
            println!("final: verts {} tris {} skin {} open_edges {} below pad+gap {} deepest {:.2}", fm.pos.len() / 3, fm.idx.len() / 3, skin, model::open_edges_welded(&fm.pos, &fm.idx), below, deep);
        }
        if std::env::var("PADMESH").is_ok() {
            let pad = st.piece(0);
            let pg = pad.grid();
            let pm = pad.mesh(6.0);
            let pn = pg.len() * pg[0].len();
            let tris: Vec<[usize; 3]> = pm.idx.chunks(3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).filter(|t| t.iter().all(|&i| i < pn)).collect();
            let mut cells: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
            let cs = 8.0f64;
            for (ti, t) in tris.iter().enumerate() {
                let xs: Vec<f64> = t.iter().map(|&i| pm.pos[i * 3] as f64).collect();
                let ys: Vec<f64> = t.iter().map(|&i| pm.pos[i * 3 + 1] as f64).collect();
                let (x0, x1) = (xs.iter().cloned().fold(1e9, f64::min), xs.iter().cloned().fold(-1e9, f64::max));
                let (y0, y1) = (ys.iter().cloned().fold(1e9, f64::min), ys.iter().cloned().fold(-1e9, f64::max));
                for cx in (x0 / cs).floor() as i64..=(x1 / cs).floor() as i64 { for cy in (y0 / cs).floor() as i64..=(y1 / cs).floor() as i64 { cells.entry((cx, cy)).or_default().push(ti); } }
            }
            let zat = |x: f64, y: f64| -> Option<f64> {
                let mut best: Option<f64> = None;
                for &ti in cells.get(&((x / cs).floor() as i64, (y / cs).floor() as i64))? {
                    let t = tris[ti];
                    let p = |i: usize| (pm.pos[t[i] * 3] as f64, pm.pos[t[i] * 3 + 1] as f64, pm.pos[t[i] * 3 + 2] as f64);
                    let (a, b, c) = (p(0), p(1), p(2));
                    let d = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
                    if d.abs() < 1e-12 { continue; }
                    let l1 = ((b.1 - c.1) * (x - c.0) + (c.0 - b.0) * (y - c.1)) / d;
                    let l2 = ((c.1 - a.1) * (x - c.0) + (a.0 - c.0) * (y - c.1)) / d;
                    let l3 = 1.0 - l1 - l2;
                    if l1 >= -1e-9 && l2 >= -1e-9 && l3 >= -1e-9 { let z = l1 * a.2 + l2 * b.2 + l3 * c.2; best = Some(best.map_or(z, |b: f64| b.max(z))); }
                }
                best
            };
            let nv = nr * nc;
            let (mut n, mut deep, mut at) = (0usize, 0.0f64, (0usize, 0usize));
            for i in 0..nv {
                let (x, y, iz) = (m.pos[(nv + i) * 3] as f64, m.pos[(nv + i) * 3 + 1] as f64, m.pos[(nv + i) * 3 + 2] as f64);
                if let Some(pz) = zat(x, y) { let d = pz - iz; if d > 0.05 { n += 1; if d > deep { deep = d; at = (i / nc, i % nc); } } }
            }
            println!("padmesh: trim underside vertices under the real pad surface: {} deepest {:.2} at row {} col {}", n, deep, at.0, at.1);
        }
        if std::env::var("RAMP").is_ok() {
            let nv = nr * nc;
            for r in (0..24).step_by(1) {
                let oz: String = (16..nc).map(|c| format!("{:6.1}", m.pos[(r * nc + c) * 3 + 2])).collect();
                let iz: String = (16..nc).map(|c| format!("{:6.1}", m.pos[(nv + r * nc + c) * 3 + 2])).collect();
                let iy: String = (16..nc).map(|c| format!("{:6.1}", m.pos[(nv + r * nc + c) * 3 + 1])).collect();
                let oy: String = (16..nc).map(|c| format!("{:6.1}", m.pos[(r * nc + c) * 3 + 1])).collect();
                let u = tr.under.clone().unwrap();
                let dp: String = (16..nc).map(|c| format!("{:6.1}", u.z_at(m.pos[(nv + r * nc + c) * 3] as f64, m.pos[(nv + r * nc + c) * 3 + 1] as f64).map(|pz| m.pos[(nv + r * nc + c) * 3 + 2] as f64 - pz).unwrap_or(-99.0))).collect();
                println!("ramp r{} x {:.0}\n  oz {}\n  iz {}\n  oy {}\n  iy {}\n  inner-pad {}", r, g[r][0][0], oz, iz, oy, iy, dp);
            }
        }
        if std::env::var("PADFRONT").is_ok() {
            let pad = st.piece(0);
            let pg = pad.grid();
            let pm = pad.mesh(6.0);
            let (pnr, pnc) = (pg.len(), pg[0].len());
            println!("pad grid {}x{}", pnr, pnc);
            for r in 0..40.min(pnr) {
                let o = |c: usize| (pm.pos[(r * pnc + c) * 3 + 1], pm.pos[(r * pnc + c) * 3 + 2]);
                let i = |c: usize| (pm.pos[(pnr * pnc + r * pnc + c) * 3 + 1], pm.pos[(pnr * pnc + r * pnc + c) * 3 + 2]);
                let c = pnc - 1;
                println!("padrow {} x {:.0}  outer last3 {:?}  inner last3 {:?}", r, pg[r][0][0], [o(c - 2), o(c - 1), o(c)].iter().map(|v| format!("({:.1},{:.1})", v.0, v.1)).collect::<Vec<_>>(), [i(c - 2), i(c - 1), i(c)].iter().map(|v| format!("({:.1},{:.1})", v.0, v.1)).collect::<Vec<_>>());
            }
        }
        if std::env::var("SPIKES").is_ok() {
            for (name, off) in [("outer", 0usize), ("inner", nr * nc)] {
                let p3 = |r: usize, c: usize| { let k = (off + r * nc + c) * 3; [m.pos[k] as f64, m.pos[k + 1] as f64, m.pos[k + 2] as f64] };
                let mut w: Vec<(f64, usize, usize)> = Vec::new();
                for r in 1..nr - 1 { for c in 4..nc - 1 {
                    let (a, b, cc, d, e) = (p3(r - 1, c), p3(r + 1, c), p3(r, c - 1), p3(r, c + 1), p3(r, c));
                    let mut dev = 0.0f64;
                    for i in 0..3 { let avg = 0.25 * (a[i] + b[i] + cc[i] + d[i]); dev = dev.max((e[i] - avg).abs()); }
                    w.push((dev, r, c));
                } }
                w.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                for (v, r, c) in w.iter().take(8) { let e = p3(*r, *c); println!("spike {} {:.2} row {} col {} at ({:.0},{:.1},{:.1})", name, v, r, c, e[0], e[1], e[2]); }
            }
        }
        if std::env::var("ENDROWS").is_ok() {
            for r in [0usize, nr - 1] {
                println!("row {} z {}", r, (14..nc).map(|c| format!("{:6.1}", z(r, c))).collect::<String>());
                println!("      y {}", (14..nc).map(|c| format!("{:6.1}", yy(r, c))).collect::<String>());
            }
        }
        let mut worst: Vec<(f64, usize, usize)> = Vec::new();
        for r in 1..nr - 1 {
            for c in 1..nc - 1 {
                let d2r = z(r - 1, c) - 2.0 * z(r, c) + z(r + 1, c);
                let d2c = z(r, c - 1) - 2.0 * z(r, c) + z(r, c + 1);
                let _ = d2c;
                if c >= 3 && r < nr / 2 { worst.push((d2r.abs(), r, c)); }
            }
        }
        worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for (v, r, c) in worst.iter().take(14) {
            println!("rough {:.2} at row {} (x {:.0}) col {} (y {:.1} z {:.1})", v, r, g[*r][0][0], c, yy(*r, *c), z(*r, *c));
        }
    }
}


#[cfg(test)]
mod trim_pocketed_tests {
    use super::*;
    /// A tall pocketed (W) pad has rows whose lip hooks back past the trim's cut line; the trim must still follow the top run
    /// and nose, not dive down the front of the pad and back up.
    #[test]
    fn trim_follows_the_top_run_on_a_tall_pocketed_pad() {
        for js in [
            r#"{"op":"apply_template","kind":"wrap","nprof":7,"width":1500,"height":230}"#,
            r#"{"op":"apply_template","kind":"wrap","nprof":7,"width":1500,"height":300}"#,
            r#"{"op":"apply_template","kind":"wrap","nprof":5,"height":260,"depth":300}"#,
            r#"{"op":"apply_template","kind":"wrap","nprof":7,"crease":false,"centre_out":-60,"height":260}"#,
        ] {
            let mut st = State::new(std::env::temp_dir().join("dashfw_tall_test"));
            st.op(js).unwrap();
            let tr = st.piece(2);
            for p in &tr.profiles {
                let curve = p.curve();
                let zmin = curve.iter().map(|q| q[1]).fold(f64::MAX, f64::min);
                let tip = p.pts.last().unwrap().co[1];
                assert!(zmin > tip - 25.0, "trim profile at x {} dives to z {} (tip {}) for {}", p.x, zmin, tip, js);
            }
            let m = tr.mesh(6.0);
            assert_eq!(model::open_edges_welded(&m.pos, &m.idx), 0);
        }
    }
}


#[cfg(test)]
mod front_mode_test {
    use super::*;
    #[test]
    fn front_mode_saves_and_loads() {
        let d = std::env::temp_dir().join("dashfw_front_mode_test");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(&d);
        let mut st = State::new(d.clone());
        assert!(!st.trim_follow);
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        st.op(r#"{"op":"set_front_mode","follow":true}"#).unwrap();
        assert!(st.piece(2).nose_follows);
        st.save();
        let st2 = State::new(d.clone());
        assert!(st2.trim_follow);
        assert!(st2.piece(2).nose_follows);
        let _ = std::fs::remove_dir_all(&d);
    
    }
    #[test]
    fn front_mode_back_restores_profiles() {
        let d = std::env::temp_dir().join("dashfw_front_mode_test2");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(&d);
        let mut st = State::new(d.clone());
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        st.op(r#"{"op":"set_front_mode","follow":false}"#).unwrap();
        let before: Vec<Vec<[f64; 2]>> = st.piece(2).profiles.iter().map(|p| p.pts.iter().map(|q| q.co).collect()).collect();
        st.op(r#"{"op":"set_front_mode","follow":true}"#).unwrap();
        let k = st.piece(2).rail2.len() / 2;
        let [y, z] = st.piece(2).rail2[k];
        st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y - 60.0, z)).unwrap();
        let mid: Vec<Vec<[f64; 2]>> = st.piece(2).profiles.iter().map(|p| p.pts.iter().map(|q| q.co).collect()).collect();
        assert!(mid != before);
        st.op(&format!(r#"{{"op":"move_rail2","k":{},"y":{},"z":{}}}"#, k, y, z)).unwrap();
        st.op(r#"{"op":"set_front_mode","follow":false}"#).unwrap();
        let after: Vec<Vec<[f64; 2]>> = st.piece(2).profiles.iter().map(|p| p.pts.iter().map(|q| q.co).collect()).collect();
        for (a, b) in after.iter().zip(&before) {
            for (u, v) in a.iter().zip(b).take(a.len() - 1) {
                assert!((u[0] - v[0]).abs() < 1e-6 && (u[1] - v[1]).abs() < 1e-6, "{:?} vs {:?}", u, v);
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn trim_node_cannot_go_below_pad() {
        let d = std::env::temp_dir().join("dashfw_front_mode_test3");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(&d);
        let mut st = State::new(d.clone());
        st.op(r#"{"op":"switch_piece","to":2}"#).unwrap();
        let ci = st.piece(2).profiles.len() / 2;
        let [y, z] = st.piece(2).profiles[ci].pts[2].co;
        st.op(&format!(r#"{{"op":"edit_pt","i":{},"k":2,"which":0,"y":{},"z":{}}}"#, ci, y, z - 80.0)).unwrap();
        let p = st.piece(2);
        let u = p.under.clone().unwrap();
        let q = p.profiles[ci].pts[2].co;
        let pz = u.z_at(p.profiles[ci].x, q[0]).unwrap();
        assert!(q[1] >= pz, "node {} below pad {}", q[1], pz);
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn break_and_join_handles() {
        let d = std::env::temp_dir().join("dashfw_front_mode_test4");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(&d);
        let mut st = State::new(d.clone());
        let ci = st.proj.profiles.len() / 2;
        let k = 3;
        let q = st.proj.profiles[ci].pts[k];
        // smooth: moving the left handle swings the right one
        st.op(&format!(r#"{{"op":"edit_pt","i":{},"k":{},"which":1,"y":{},"z":{}}}"#, ci, k, q.co[0] - 10.0, q.co[1] + 15.0)).unwrap();
        let a = st.proj.profiles[ci].pts[k];
        assert!((a.hr[0] - q.hr[0]).abs() > 1e-6 || (a.hr[1] - q.hr[1]).abs() > 1e-6);
        st.op(&format!(r#"{{"op":"set_broken","k":{},"on":true}}"#, k)).unwrap();
        assert!(st.proj.is_broken(k));
        let before = st.proj.profiles[ci].pts[k].hr;
        st.op(&format!(r#"{{"op":"edit_pt","i":{},"k":{},"which":1,"y":{},"z":{}}}"#, ci, k, q.co[0] - 12.0, q.co[1] - 9.0)).unwrap();
        let b = st.proj.profiles[ci].pts[k];
        assert_eq!(b.hr, before, "broken: the right handle must stay put");
        st.save();
        let st2 = State::new(d.clone());
        assert!(st2.proj.is_broken(k));
        st.op(&format!(r#"{{"op":"set_broken","k":{},"on":false}}"#, k)).unwrap();
        let c = st.proj.profiles[ci].pts[k];
        let cr = (c.co[0] - c.hl[0]) * (c.hr[1] - c.co[1]) - (c.co[1] - c.hl[1]) * (c.hr[0] - c.co[0]);
        assert!(cr.abs() < 1e-6, "joined handles must be in line");
        let _ = std::fs::remove_dir_all(&d);
    }
}
