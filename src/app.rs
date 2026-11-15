// Backwoods Dash Designer native app: layout, state and shared helpers. Panels live in ui_*.rs.
use crate::engine::{self, State};
use crate::json;
use crate::parts;
use crate::render::*;
use crate::templates;
use crate::theme::*;
use crate::units::MmExt;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Dash,
    Profile,
    Section,
    Part,
    Rail,
    Cut,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LeftTab {
    Scene,
    Shapes,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DockTab {
    Profile,
    Plan,
    Height,
    Walls,
    Part,
}

pub struct Layers {
    pub skin: bool,
    pub sections: bool,
    pub lines: bool,
    pub rail: bool,
    pub parts: bool,
}

#[derive(Clone, Copy)]
pub struct View2 {
    pub s: f32,
    pub ox: f32,
    pub oy: f32,
    pub fitted: bool,
}

impl View2 {
    pub fn new() -> View2 {
        View2 { s: 1.0, ox: 0.0, oy: 0.0, fitted: false }
    }
    pub fn fit(&mut self, rect: Rect, b: [f32; 4], pad: f32) {
        let sx = (rect.width() - 2.0 * pad) / (b[1] - b[0]).max(1e-3);
        let sy = (rect.height() - 2.0 * pad) / (b[3] - b[2]).max(1e-3);
        self.s = sx.min(sy).max(0.01);
        self.ox = rect.width() / 2.0 - (b[0] + b[1]) / 2.0 * self.s;
        self.oy = rect.height() / 2.0 + (b[2] + b[3]) / 2.0 * self.s;
        self.fitted = true;
    }
    pub fn to_screen(&self, rect: Rect, x: f64, y: f64) -> Pos2 {
        pos2(rect.left() + self.ox + x as f32 * self.s, rect.top() + self.oy - y as f32 * self.s)
    }
    pub fn from_screen(&self, rect: Rect, p: Pos2) -> (f64, f64) {
        (((p.x - rect.left() - self.ox) / self.s) as f64, ((rect.top() + self.oy - p.y) / self.s) as f64)
    }
}

pub enum VpDrag {
    None,
    Orbit,
    Pan,
    Part { id: u32, ox: f64, oy: f64 },
    Cut { id: u32, ox: f64, oy: f64 },
    Gizmo { id: u32, axis: usize, rotate: bool, p0: [f32; 2], u: [f32; 2], k: f32, v0: f64, cs: [f32; 2], a0: f32, toward: bool, e0: [f64; 3] },
    Grip { i: usize },
    Size,
}

pub enum DockDrag {
    None,
    Pan,
    Pt { k: usize, which: u8 },
    Multi,
    Box { a: Pos2 },
    Rail { k: usize },
    Rail2 { k: usize },
    Grip { i: usize, off: f64 },
    /// several guide points / profile circles moved together: (kind 0 windshield point, 1 front point, 2 profile, index, original values)
    Group { sx: f64, sy: f64, items: Vec<(u8, usize, [f64; 3])> },
    GBox { a: Pos2 },
    Part { mode: u8, id: u32, ox: f64, oy: f64, w0: f64, h0: f64 },
}

pub struct Wiz {
    pub step: usize,
    pub p: templates::Params,
    /// wizard front line: the Center in / out choice is on (even while its amount passes through 0)
    pub inout: bool,
    pub applied: bool,
    pub undo_start: usize,
    /// the preview picture: turn, tilt, zoom and slide
    pub view: [f32; 5],
    /// Sections step: the section whose name box should take the keyboard focus (set by clicking it in the picture)
    pub focus_sec: Option<usize>,
}

pub struct App {
    pub st: State,
    pub rev: u64,
    pub mesh_rev: u64,
    pub parts_key: (u64, Option<u32>),
    pub mesh: Option<CpuMesh>,
    pub scene: Shared,
    pub glres: SharedGl,
    pub cam: Camera,
    pub sel_prof: usize,
    pub sel_pt: usize,
    /// Extra selected points on the selected profile (empty = just sel_pt).
    pub multi: Vec<usize>,
    pub wall_view: bool,
    pub wall_cut: bool,
    pub wall_x: f32,
    pub wall_key: (u64, bool, u64),
    pub other_key: (u64, u64, u64),
    pub other_pick: Vec<(usize, CpuMesh)>,
    pub side_pick: Option<CpuMesh>,
    pub multi_last: Option<(f64, f64)>,
    /// guideline editors: extra selected windshield points / front points / profile circles (kind, index)
    pub gsel: Vec<(u8, usize)>,
    pub sel_rail: Option<usize>,
    pub sel_sec: Option<usize>,
    pub sel_part: Option<u32>,
    pub focus: Focus,
    pub layers: Layers,
    pub left_tab: LeftTab,
    pub dock_tab: DockTab,
    pub dock_open: bool,
    pub side: View2,
    pub plan: View2,
    pub elev: View2,
    pub wiz: Option<Wiz>,
    pub drag_item: Option<usize>,
    pub vp_drag: VpDrag,
    pub gizmo_rot: bool,
    pub light: crate::lighting::Lighting,
    pub light_open: bool,
    pub sw: crate::sidewall::SideWall,
    pub sw_ui: crate::sidewall::WallUi,
    pub sw_key: u64,
    pub cut_logged: String,
    pub cut_menu: Option<(u32, egui::Pos2)>,
    /// Sizing: drag bars shown, whole dash or one piece, parts resize too, the right-click menu, the drag in progress.
    pub size_mode: bool,
    pub size_all: bool,
    pub size_parts: bool,
    pub size_menu: Option<egui::Pos2>,
    pub size_drag: Option<crate::ui_size::SizeDrag>,
    pub size_axis: Option<usize>,
    /// Size of each piece before any resize (kept in size_orig.txt so Reset to original works after a restart).
    pub size_orig: Option<[crate::ui_size::Box3; 3]>,
    pub size_edit: Option<(crate::ui_size::SizeBase, crate::ui_size::Box3)>,
    pub scale_pct: f64,
    pub dock_drag: DockDrag,
    pub vp_rect: Rect,
    pub toast: Option<(String, Instant, bool)>,
    pub last_save: Instant,
    pub export_info: Option<String>,
    pub shapes: Vec<crate::shapes::Shape>,
    pub refs: [Option<crate::refimg::RefImage>; 3],
    pub ref_open: bool,
    pub ref_rx: Option<std::sync::mpsc::Receiver<Option<String>>>,
    pub ref_msg: String,
    pub shape_next: u32,
    pub stl_pick: Option<std::sync::mpsc::Receiver<Option<String>>>,
    pub shape_ed: Option<crate::ui_shapes::ShapeEd>,
    pub exp_open: bool,
    /// "Add a profile" dialog: position across, and whether to add its mirror too.
    pub addp: Option<(f64, bool)>,
    /// Placing a profile by clicking on the windshield line or the front line (true = with its mirror).
    pub addp_pick: Option<bool>,
    pub exp_names: [String; 3],
    pub exp_msg: [String; 3],
    pub wall_names: [String; 2],
    pub wall_msg: [String; 2],
    pub skin_ver: u64,
    pub parts_ver: u64,
    pub preview_ver: u64,
    pub shot: Option<String>,
    pub frame_no: u32,
    pub center_line: Vec<V3>,
    pub snap_center: bool,
    pub show_center: bool,
    pub sel_rail2: Option<usize>,
    pub part_outlines: Vec<(u32, Vec<V3>)>,
    pub preview_outline: Vec<V3>,
    pub preview_name: String,
    pub view_mat: bool,
    pub sm_sens: f32,
    pub sm_invert: bool,
    pub sm_open: bool,
    /// Asking whether to save or discard the open shape before going on (a top-row button or the Scene tab was clicked).
    pub leave_ask: bool,
    pub leave_scene_req: bool,
    pub hide_left: bool,
    pub hide_right: bool,
    pub left_gen: u32,
    pub right_gen: u32,
    pub rs_open: bool,
    pub help_open: bool,
    pub help_topic: usize,
    pub help_q: String,
    pub rs_n: usize,
    pub rs_bend: bool,
    // ---- shapes placed on the piece
    pub sel_cut: Option<u32>,
    /// Layers panel: which pieces are hidden in the views, which are unfolded, and where a dragged part would land.
    pub piece_vis: [bool; 3],
    pub lay_open: [bool; 3],
    pub drop_target: Option<(usize, f64, f64)>,
    pub drag_shape: Option<crate::shapes::Shape>,
    pub cut_foot: Vec<(u32, Vec<V3>)>,
    pub cut_obj_key: (u64, u32),
    pub cut_obj_tm: Vec<(u32, crate::cuts::TriMesh)>,
    pub cut_prev: Vec<V3>,
    pub cut_key: (u64, u64),
    pub cut_tx: std::sync::mpsc::Sender<((u64, u64), Result<crate::cuts::CutResult, String>)>,
    pub cut_rx: std::sync::mpsc::Receiver<((u64, u64), Result<crate::cuts::CutResult, String>)>,
    pub cut_inflight: bool,
    pub cut_pending: bool,
    pub cut_ok: bool,
    pub cut_msg: String,
    pub show_cuts: bool,
    pub egui_ctx: egui::Context,
    pub file: crate::ui_file::FileState,
    pub shape_rename: Option<(u32, String)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, dir: std::path::PathBuf) -> App {
        apply(&cc.egui_ctx);
        crate::spacemouse::init(&cc.egui_ctx);
        let light = crate::lighting::Lighting::load(&dir);
        let swall = crate::sidewall::SideWall::load(&dir);
        let st = State::new(dir);
        let scene: Shared = Arc::new(Mutex::new(SceneData { show_skin: true, show_parts: true, ..Default::default() }));
        let mid = st.proj.profiles.len() / 2;
        let (shapes, shape_next) = crate::shapes::load(&st.dir);
        let dir_for_refs = st.dir.clone();
        let (cut_tx, cut_rx) = std::sync::mpsc::channel();
        App {
            sel_cut: None,
            piece_vis: [false, false, false],
            lay_open: [true; 3],
            drop_target: None,
            drag_shape: None,
            cut_foot: Vec::new(),
            cut_obj_key: (u64::MAX, 0),
            cut_obj_tm: Vec::new(),
            cut_prev: Vec::new(),
            cut_key: (u64::MAX, 0),
            cut_tx,
            cut_rx,
            cut_inflight: false,
            cut_pending: false,
            cut_ok: false,
            cut_msg: String::new(),
            show_cuts: true,
            egui_ctx: cc.egui_ctx.clone(),
            file: crate::ui_file::FileState::new(&dir_for_refs),
            shape_rename: None,
            st,
            rev: 1,
            mesh_rev: 0,
            parts_key: (0, None),
            mesh: None,
            scene,
            glres: Arc::new(Mutex::new(None)),
            cam: std::env::var("DASHFW_CAM").ok().and_then(|v| { let a: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if a.len() == 6 { Some(Camera { yaw: a[0], pitch: a[1], dist: a[2], target: [a[3], a[4], a[5]] }) } else { None } }).unwrap_or_else(Camera::home),
            sel_prof: mid,
            sel_pt: 1,
            multi: Vec::new(),
            wall_view: std::env::var("DASHFW_WALLTEST").is_ok(),
            wall_cut: std::env::var("DASHFW_WALLTEST").is_ok(),
            wall_x: std::env::var("DASHFW_WALLTEST").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            wall_key: (u64::MAX, false, 0),
            other_key: (u64::MAX, 0, 0),
            other_pick: Vec::new(),
            side_pick: None,
            multi_last: None,
            gsel: Vec::new(),
            sel_rail: None,
            sel_sec: None,
            sel_part: None,
            focus: Focus::Dash,
            layers: Layers { skin: true, sections: true, lines: true, rail: true, parts: true },
            left_tab: LeftTab::Scene,
            dock_tab: if std::env::var("DASHFW_SW").is_ok() { DockTab::Walls } else { DockTab::Profile },
            dock_open: true,
            side: View2::new(),
            plan: View2::new(),
            elev: View2::new(),
            wiz: None,
            drag_item: None,
            vp_drag: VpDrag::None,
            gizmo_rot: false,
            light,
            light_open: false,
            sw: swall,
            sw_ui: { let mut w = crate::sidewall::WallUi::new(); w.open = std::env::var("DASHFW_SW").is_ok(); w },
            sw_key: u64::MAX,
            cut_logged: String::new(),
            cut_menu: None,
            size_mode: false,
            size_all: true,
            size_parts: true,
            size_menu: None,
            size_drag: None,
            size_axis: None,
            size_orig: None,
            size_edit: None,
            scale_pct: 100.0,
            dock_drag: DockDrag::None,
            vp_rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
            toast: None,
            last_save: Instant::now(),
            export_info: None,
            shapes,
            shape_next,
            stl_pick: None,
            refs: crate::refimg::load_all(&dir_for_refs),
            ref_open: false,
            ref_rx: None,
            ref_msg: String::new(),
            shape_ed: std::env::var("DASHFW_EXTRUDETEST").ok().map(|_| {
                let mut sh = crate::shapes::Shape::new("Test slot", crate::shapes::Kind::Rect, 100.0, 30.0);
                sh.radius = 12.0;
                sh.depth = 12.0;
                sh.draft = 8.0;
                sh.bevel = 3.0;
                sh.bevel_round = true;
                crate::ui_shapes::ShapeEd::new(sh, None)
            }),
            exp_open: false,
            addp: None,
            addp_pick: None,
            exp_names: ["top_dash_pad".to_string(), "lower_dash_pad".to_string(), "top_trim".to_string()],
            exp_msg: [String::new(), String::new(), String::new()],
            wall_names: ["side_wall_driver".to_string(), "side_wall_passenger".to_string()],
            wall_msg: [String::new(), String::new()],
            skin_ver: 0,
            parts_ver: 0,
            preview_ver: 0,
            shot: std::env::var("DASHFW_SHOT").ok(),
            frame_no: 0,
            center_line: Vec::new(),
            snap_center: true,
            show_center: true,
            sel_rail2: None,
            part_outlines: Vec::new(),
            preview_outline: Vec::new(),
            preview_name: String::new(),
            view_mat: true,
            sm_sens: 1.0,
            sm_invert: false,
            sm_open: false,
            leave_ask: false,
            leave_scene_req: false,
            hide_left: false,
            hide_right: false,
            left_gen: 0,
            right_gen: 0,
            rs_open: false,
            help_open: false,
            help_topic: 0,
            help_q: String::new(),
            rs_n: 24,
            rs_bend: true,
        }
    }

    // ---------------------------------------------------------------- engine access
    pub fn op(&mut self, text: String) {
        if text.contains("pt_add") || text.contains("pt_remove") || text.contains("resample") || text.contains("undo") || text.contains("redo") || text.contains("reset") || text.contains("switch_piece") {
            self.multi.clear();
        }
        match self.st.op(&text) {
            Ok(()) => {
                if text.contains("apply_template") {
                    self.piece_vis[2] = self.st.trim_on;
                }
            }
            Err(e) => self.toast(&e, e != crate::engine::TRIM_LOCK_MSG),
        }
        self.rev += 1;
        self.clamp_sel();
    }
    pub fn op_s(&mut self, text: &str) {
        self.op(text.to_string());
    }
    pub fn toast(&mut self, msg: &str, err: bool) {
        if err {
            crate::errlog::write("ERROR", msg);
        }
        self.toast = Some((msg.to_string(), Instant::now(), err));
    }
    pub fn clamp_sel(&mut self) {
        let p = &self.st.proj;
        self.sel_prof = self.sel_prof.min(p.profiles.len() - 1);
        if let Some(r) = self.sel_rail2 {
            if r >= p.rail2.len() {
                self.sel_rail2 = None;
            }
        }
        self.sel_pt = self.sel_pt.min(p.profiles[self.sel_prof].pts.len() - 1);
        if let Some(r) = self.sel_rail {
            if r >= p.rail.len() {
                self.sel_rail = None;
            }
        }
        if let Some(s) = self.sel_sec {
            if s >= p.sections.len() {
                self.sel_sec = None;
            }
        }
        if let Some(id) = self.sel_cut {
            if !p.cuts.iter().any(|c| c.id == id) {
                self.sel_cut = None;
                if self.focus == Focus::Cut {
                    self.focus = Focus::Dash;
                }
            }
        }
        if let Some(id) = self.sel_part {
            if !p.comps.iter().any(|c| c.id == id) {
                self.sel_part = None;
                if self.focus == Focus::Part {
                    self.focus = Focus::Dash;
                }
            }
        }
    }
    /// Pull x onto the center line when it is close.
    pub fn snap_x(&self, x: f64) -> f64 {
        if self.snap_center && x.abs() < 7.0 { 0.0 } else { x }
    }
    pub fn switch_piece(&mut self, to: u8) {
        if self.st.active == to {
            return;
        }
        self.op(format!(r#"{{"op":"switch_piece","to":{}}}"#, to));
        self.sel_prof = self.st.proj.profiles.len() / 2;
        self.sel_pt = 1;
        self.sel_rail = None;
        self.sel_rail2 = None;
        self.sel_sec = None;
        self.sel_part = None;
        self.focus = Focus::Dash;
        self.side.fitted = false;
        self.plan.fitted = false;
        self.elev.fitted = false;
        self.parts_key = (0, None);
        self.cam.target[2] = [330.0, 195.0, 360.0][to as usize];
        self.cam.dist = [1750.0, 1400.0, 1750.0][to as usize];
        self.clamp_sel();
    }
    pub fn rail_label(&self) -> &'static str {
        if self.st.active == 1 { "Top guideline" } else { "Windshield line" }
    }
    pub fn select_profile(&mut self, i: usize) {
        if self.sel_prof != i {
            self.multi.clear();
        }
        self.sel_prof = i;
        self.sel_part = None;
        self.sel_sec = None;
        self.focus = Focus::Profile;
        self.clamp_sel();
    }
    pub fn select_part(&mut self, id: u32) {
        self.sel_cut = None;
        self.sel_part = Some(id);
        self.focus = Focus::Part;
        if let Some(x) = self.st.proj.comps.iter().find(|c| c.id == id).map(|c| c.x) {
            self.flip_dock_to_x(x);
        }
        // the bottom panel becomes this part's editor
        self.dock_tab = DockTab::Part;
        self.plan.fitted = false;
    }
    /// Show the profile editor in the bottom panel on the profile nearest to x.
    pub fn flip_dock_to_x(&mut self, x: f64) {
        let p = &self.st.proj;
        let mut best = (f64::MAX, self.sel_prof);
        for (i, pr) in p.profiles.iter().enumerate() {
            let d = (pr.x - x).abs();
            if d < best.0 {
                best = (d, i);
            }
        }
        self.sel_prof = best.1;
        self.multi.clear();
        self.clamp_sel();
        self.dock_open = true;
        self.dock_tab = DockTab::Profile;
    }
    /// Pick a profile from the main view or the scene tree: the bottom panel flips to its profile.
    pub fn select_profile_view(&mut self, i: usize) {
        self.select_profile(i);
        self.dock_open = true;
        self.dock_tab = DockTab::Profile;
    }
    pub fn select_section(&mut self, k: usize) {
        self.sel_sec = Some(k);
        self.sel_part = None;
        self.focus = Focus::Section;
    }
    pub fn add_part(&mut self, it: usize, x: f64, y: f64) {
        let c = &parts::CATALOG[it];
        self.op(format!(
            r#"{{"op":"comp_add","kind":"{}","shape":"{}","name":"{}","x":{},"y":{},"w":{},"h":{},"rot":0}}"#,
            c.kind, c.shape, c.label, x, y, c.w, c.h
        ));
        if let Some(c) = self.st.proj.comps.last() {
            let id = c.id;
            self.select_part(id);
        }
    }

    // ---------------------------------------------------------------- scene upload
    fn vtx_of(cm: &CpuMesh) -> Vec<Vtx> {
        (0..cm.pos.len() / 3)
            .map(|i| Vtx { p: [cm.pos[i * 3], cm.pos[i * 3 + 1], cm.pos[i * 3 + 2]], n: [cm.nrm[i * 3], cm.nrm[i * 3 + 1], cm.nrm[i * 3 + 2]], c: [1.0, 1.0, 1.0] })
            .collect()
    }

    pub fn refresh(&mut self) {
        let rebuilt = self.mesh_rev != self.rev;
        if rebuilt {
            let m = self.st.proj.mesh_view(self.st.thickness);
            let cm = CpuMesh { pos: m.pos, nrm: m.nrm, idx: m.idx };
            let nv = cm.pos.len() / 3;
            let mut skin = Vec::with_capacity(nv);
            for i in 0..nv {
                skin.push(Vtx {
                    p: [cm.pos[i * 3], cm.pos[i * 3 + 1], cm.pos[i * 3 + 2]],
                    n: [cm.nrm[i * 3], cm.nrm[i * 3 + 1], cm.nrm[i * 3 + 2]],
                    c: [1.0, 1.0, 1.0],
                });
            }
            let nc = self.st.proj.profiles[0].curve().len();
            let span_tris = crate::model::M * (nc - 1) * 2;
            self.skin_ver += 1;
            let mut sc = self.scene.lock().unwrap();
            sc.skin = skin;
            sc.skin_idx = cm.idx.clone();
            sc.skin_ver = self.skin_ver;
            sc.span_tris = span_tris;
            drop(sc);
            // center line: the loft row closest to x = 0
            let nc = self.st.proj.profiles[0].curve().len();
            let nr = crate::model::M * (self.st.proj.profiles.len() - 1) + 1;
            let mut best = (f32::MAX, 0usize);
            for r in 0..nr {
                let d = cm.pos[(r * nc) * 3].abs();
                if d < best.0 {
                    best = (d, r);
                }
            }
            self.center_line = (0..nc).map(|c| { let i = (best.1 * nc + c) * 3; [0.0, cm.pos[i + 1], cm.pos[i + 2]] }).collect();
            self.mesh = Some(cm);
            self.mesh_rev = self.rev;
        }
        let okey = (self.rev, self.st.thickness.to_bits(), self.st.active as u64);
        if okey != self.other_key {
            self.other_key = okey;
            // the two pieces on the shelf
            let others: Vec<usize> = (0..3).filter(|&k| k != self.st.active as usize).collect();
            let mut built: Vec<(Vec<Vtx>, Vec<u32>)> = Vec::new();
            self.other_pick.clear();
            for &k in &others {
                let m = self.st.piece(k).mesh(self.st.thickness);
                let c = CpuMesh { pos: m.pos, nrm: m.nrm, idx: m.idx };
                built.push((Self::vtx_of(&c), c.idx.clone()));
                self.other_pick.push((k, c));
            }
            self.skin_ver += 1;
            let mut sc = self.scene.lock().unwrap();
            let mut it = built.into_iter();
            let (v1, i1) = it.next().unwrap_or_default();
            let (v2, i2) = it.next().unwrap_or_default();
            sc.other = v1;
            sc.other_idx = i1;
            sc.other_ver = self.skin_ver;
            sc.cover = v2;
            sc.cover_idx = i2;
            sc.cover_ver = self.skin_ver;
            drop(sc);
        }
        let wkey = (self.rev, self.wall_view, self.st.thickness.to_bits());
        if wkey != self.wall_key {
            self.wall_key = wkey;
            let solid = self.st.thickness > 0.0;
            let (verts, idx, from) = if solid {
                let (m, from) = self.st.proj.mesh_final(self.st.thickness);
                let thin = self.st.proj.thin_mask(&m, self.st.thickness);
                let c = CpuMesh { pos: m.pos, nrm: m.nrm, idx: m.idx };
                let mut vs = Self::vtx_of(&c);
                // red where the shell is thinner than the set thickness by more than the allowed amount
                for (v, t) in vs.iter_mut().zip(thin.iter()) { if *t { v.c = [1.0, 0.15, 0.15]; } }
                (vs, c.idx, from)
            } else if let Some(c) = &self.mesh {
                (Self::vtx_of(c), c.idx.clone(), 0)
            } else {
                (Vec::new(), Vec::new(), 0)
            };
            self.skin_ver += 1;
            let mut sc = self.scene.lock().unwrap();
            sc.skin = verts;
            sc.skin_idx = idx;
            sc.skin_ver = self.skin_ver;
            sc.wall_from = from;
        }
        // the back curve may never cross the lower pad's end profile (10 mm clear); checked when the dash changes or a drag ends,
        // not while a point is being dragged (the dragged point is held clear on its own)
        let fk = self.rev.wrapping_mul(7919) ^ self.st.thickness.to_bits();
        if fk != self.sw_ui.fit_key {
            self.sw_ui.fit_key = fk;
            crate::sidewall::keep_clear(self.st.piece(0), self.st.piece(1), &mut self.sw.back, self.st.thickness);
        }
        let swk = self.rev.wrapping_mul(31) ^ self.sw.stamp() ^ self.st.thickness.to_bits() ^ 0x5157;
        if swk != self.sw_key {
            self.sw_key = swk;
            let built = if self.sw.on { crate::sidewall::both(self.st.piece(0), self.st.piece(1), &self.sw.back, self.st.thickness) } else { None };
            if std::env::var("DASHFW_SW").is_ok() {
                crate::errlog::write("WALL", &format!("built: {}", built.as_ref().map(|m| format!("{} verts {} tris", m.pos.len() / 3, m.idx.len() / 3)).unwrap_or("none".into())));
            }
            self.side_pick = None;
            let (v, i) = match built {
                Some(m) => {
                    let c = CpuMesh { pos: m.pos, nrm: m.nrm, idx: m.idx };
                    let out = (Self::vtx_of(&c), c.idx.clone());
                    self.side_pick = Some(c);
                    out
                }
                None => (Vec::new(), Vec::new()),
            };
            self.skin_ver += 1;
            let mut sc = self.scene.lock().unwrap();
            sc.side = v;
            sc.side_idx = i;
            sc.side_ver = self.skin_ver;
        }
        let key = (self.rev, self.sel_part);
        if self.parts_key != key {
            let mut out = Vec::new();
            let mut ranges: Vec<(usize, usize, MatU)> = Vec::new();
            if let Some(m) = &self.mesh {
                for c in self.st.proj.comps.iter().filter(|c| c.visible && self.piece_vis[self.st.active as usize]) {
                    let st = out.len();
                    parts::build(m, c, Some(c.id) == self.sel_part, false, &mut out);
                    let mat = match c.look {
                        Some(l) => {
                            for v in out[st..].iter_mut() {
                                v.c = [1.0, 1.0, 1.0];
                            }
                            mat_of(&l)
                        }
                        None => MatU { col: [1.0, 1.0, 1.0], a: [0.45, 0.0, 0.1, 0.0], b: [1.0, 0.0, 0.0, 0.0] },
                    };
                    ranges.push((st, out.len() - st, mat));
                }
            }
            self.part_outlines = match &self.mesh {
                Some(m) => self.st.proj.comps.iter().filter(|c| c.visible).map(|c| (c.id, parts::outline(m, c))).collect(),
                None => Vec::new(),
            };
            self.parts_ver += 1;
            let mut sc = self.scene.lock().unwrap();
            sc.parts = out;
            sc.part_ranges = ranges;
            sc.parts_ver = self.parts_ver;
            self.parts_key = key;
        }
        // per-span colors follow the layers and selection
        let ss = self.st.proj.span_sections();
        let mut cols = Vec::new();
        let mut mix = Vec::new();
        for s in ss {
            let mut c = [0.82f32, 0.84, 0.88];
            let mut k = 0.0f32;
            if self.view_mat {
                if s >= 0 && self.focus == Focus::Section && Some(s as usize) == self.sel_sec {
                    let h = parts::hex(&self.st.proj.sections[s as usize].color);
                    c = h;
                    k = 0.35;
                }
                cols.push(c);
                mix.push(k);
                continue;
            }
            mix.push(if self.layers.sections && s >= 0 { 1.0 } else { 0.0 });
            if self.layers.sections && s >= 0 {
                let h = parts::hex(&self.st.proj.sections[s as usize].color);
                c = [h[0] * 0.55 + 0.38, h[1] * 0.55 + 0.38, h[2] * 0.55 + 0.38];
                if Some(s as usize) == self.sel_sec {
                    c = [(c[0] * 1.2 + 0.08).min(1.1), (c[1] * 1.2 + 0.08).min(1.1), (c[2] * 1.2 + 0.08).min(1.1)];
                }
            }
            cols.push(c);
        }
        let flat = MatU { col: [1.0, 1.0, 1.0], a: [0.7, 0.0, 0.0, 0.0], b: [1.0, 0.0, 0.0, 0.0] };
        let grey = MatU { col: [0.82, 0.84, 0.88], a: [0.7, 0.0, 0.0, 0.0], b: [1.0, 0.0, 0.0, 0.0] };
        let mut sc = self.scene.lock().unwrap();
        let walls_on = self.walls_sel();
        sc.skin_mat = if self.view_mat { mat_of(&self.st.proj.look) } else if walls_on { MatU { col: [0.5, 0.55, 0.63], a: [0.7, 0.0, 0.0, 0.0], b: [1.0, 0.0, 0.0, 0.0] } } else { grey };
        let _ = flat;
        let oth: Vec<usize> = (0..3).filter(|&k| k != self.st.active as usize).collect();
        let shelf_mat = |k: usize| -> MatU {
            if self.view_mat { mat_of(&self.st.piece(k).look) } else { MatU { col: [0.5, 0.55, 0.63], a: [0.7, 0.0, 0.0, 0.0], b: [1.0, 0.0, 0.0, 0.0] } }
        };
        sc.other_mat = shelf_mat(oth[0]);
        sc.cover_mat = shelf_mat(oth[1]);
        sc.cover_alpha = 1.0;
        sc.side_mat = if self.view_mat { mat_of(&if self.sw.follow { self.st.piece(0).look } else { self.sw.look }) } else { let k = if walls_on { 1.0 } else { 0.82 }; MatU { col: [0.62 * k, 0.78 * k, 0.72 * k], a: [0.7, 0.0, 0.0, 0.0], b: [1.0, 0.0, 0.0, 0.0] } };
        sc.show_side = self.layers.skin && self.sw.on;
        sc.span_mix = mix;
        sc.span_colors = cols;
        sc.show_skin = self.layers.skin && self.piece_vis[self.st.active as usize];
        sc.clip_on = self.wall_view && self.wall_cut;
        sc.clip_x = self.wall_x;
        sc.clip_dir = if self.cam.eye()[0] > self.wall_x { 1.0 } else { -1.0 };
        let l = self.light;
        sc.light_a = [l.key, l.ambient, l.fill, l.reflect];
        sc.light_b = [l.exposure, l.texture, if l.fixed { 1.0 } else { 0.0 }, 0.0];
        sc.key_dir = l.key_dir();
        sc.show_parts = self.layers.parts;
        sc.show_result = self.cut_ok && self.show_cuts && (self.view_mat || self.wall_view);
        sc.show_objs = self.show_cuts;
        sc.result_cap = self.wall_view && self.wall_cut;
        sc.show_other = self.layers.skin && self.piece_vis[oth[0]];
        sc.show_cover = self.layers.skin && self.piece_vis[oth[1]];
        drop(sc);
        self.update_cuts();
        let show = self.cut_ok && self.show_cuts && (self.view_mat || self.wall_view);
        {
            let mut sc = self.scene.lock().unwrap();
            sc.show_result = show;
            sc.show_objs = self.show_cuts;
        }
    }

    pub fn set_preview(&mut self, comp: Option<&crate::model::Comp>) {
        let mut out = Vec::new();
        self.preview_outline.clear();
        if let (Some(c), Some(m)) = (comp, &self.mesh) {
            parts::build(m, c, false, true, &mut out);
            self.preview_outline = parts::outline(m, c);
            self.preview_name = c.name.clone();
        }
        let mut sc = self.scene.lock().unwrap();
        if out.is_empty() && sc.preview.is_empty() {
            return;
        }
        self.preview_ver += 1;
        sc.preview = out;
        sc.preview_ver = self.preview_ver;
    }

    pub fn save_if_needed(&mut self, ctx: &egui::Context) {
        let down = ctx.input(|i| i.pointer.any_down());
        if self.st.dirty && !down && self.last_save.elapsed().as_millis() > 600 {
            self.st.save();
            self.st.dirty = false;
            self.last_save = Instant::now();
        }
    }

    /// Opens the naming dialog (nothing is written until a piece's Save is pressed).
    pub fn export(&mut self) {
        self.exp_open = true;
    }

    pub fn export_one(&mut self, i: usize) {
        let name = self.exp_names[i].clone();
        self.exp_msg[i] = match engine::export_piece(&self.st, i, self.st.thickness, &name) {
            Ok(s) => match json::parse(&s) {
                Ok(j) => {
                    let tris = j.n("tris").unwrap_or(0.0);
                    let open = j.n("open_edges").unwrap_or(0.0);
                    let vol = j.n("volume_mm3").unwrap_or(0.0);
                    let mut info = format!("Saved {} triangles", tris as u64);
                    info.push_str(if open == 0.0 { " - watertight" } else { " - open edges (surface only)" });
                    if vol > 0.0 {
                        info.push_str(&format!(" - {:.0} cm3", vol / 1000.0));
                    }
                    let plugs = j.get("plugs").and_then(|v| v.arr()).map(|a| a.len()).unwrap_or(0);
                    if plugs > 0 {
                        info.push_str(&format!(" + {} inlay plug file{}", plugs, if plugs == 1 { "" } else { "s" }));
                    }
                    self.export_info = Some(format!("{}\n{}", j.s("path").unwrap_or(""), info));
                    info
                }
                Err(e) => format!("Saved, but could not read the summary: {}", e),
            },
            Err(e) => format!("Not saved: {}", e),
        };
    }

    pub fn export_window(&mut self, ctx: &egui::Context) {
        if !self.exp_open {
            return;
        }
        let mut open = true;
        let dir = self.st.dir.join("exports");
        egui::Window::new("Export STL(s)").anchor(Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).resizable(false).collapsible(false).fixed_size([470.0, 0.0]).show(ctx, |ui| {
            ui.label(egui::RichText::new("Name each piece (and each side wall) and save it on its own. Files go in the exports folder; saving over a name replaces that file.").size(12.0).color(MUTED));
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label("Wall thickness");
                    ui.add(egui::DragValue::new(&mut self.st.thickness).range(0.0..=40.0).speed(0.1).mm());
                    ui.label(egui::RichText::new(if self.st.thickness > 0.0 { "hollow shell" } else { "surface only" }).size(12.0).color(MUTED));
                });
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    self.gap_ui(ui);
                });
            });
            ui.add_space(6.0);
            for i in 0..3 {
                ui.separator();
                if i == 2 && !self.st.trim_on {
                    ui.label(egui::RichText::new("Top trim: not added to this dash").size(12.0).color(MUTED));
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(engine::PIECE_NAMES[i]).strong());
                    if i == self.st.active as usize {
                        ui.label(egui::RichText::new("(editing)").size(12.0).color(MUTED));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add(egui::TextEdit::singleline(&mut self.exp_names[i]).desired_width(260.0)).changed() {
                        self.exp_msg[i].clear();
                    }
                    ui.label(".stl");
                    if primary_button(ui, "Save").clicked() {
                        self.export_one(i);
                    }
                });
                let exists = !self.exp_names[i].trim().is_empty() && dir.join(format!("{}.stl", self.exp_names[i].trim().trim_end_matches(".stl"))).exists();
                if !self.exp_msg[i].is_empty() {
                    ui.label(egui::RichText::new(self.exp_msg[i].clone()).size(12.0).color(MUTED));
                } else if exists {
                    ui.label(egui::RichText::new("A file with this name exists and will be replaced").size(12.0).color(Color32::from_rgb(255, 217, 168)));
                }
            }
            for end in 0..2usize {
                ui.separator();
                ui.label(egui::RichText::new(if end == 0 { "Side wall: driver end" } else { "Side wall: passenger end" }).strong());
                ui.horizontal(|ui| {
                    if ui.add(egui::TextEdit::singleline(&mut self.wall_names[end]).desired_width(260.0)).changed() {
                        self.wall_msg[end].clear();
                    }
                    ui.label(".stl");
                    if primary_button(ui, "Save").clicked() {
                        let (up, lo) = (self.st.piece(0).clone(), self.st.piece(1).clone());
                        self.wall_msg[end] = match crate::sidewall::export_end(&up, &lo, &self.sw.back, end, self.st.thickness, &self.st.dir, &self.wall_names[end]) {
                            Ok(p) => format!("Saved {}", p),
                            Err(e) => format!("Could not save: {}", e),
                        };
                    }
                });
                if !self.wall_msg[end].is_empty() {
                    ui.label(egui::RichText::new(self.wall_msg[end].clone()).size(12.0).color(MUTED));
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Show exports folder").clicked() {
                    self.open_exports();
                }
                if ui.button("Close").clicked() {
                    self.exp_open = false;
                }
            });
        });
        if !open {
            self.exp_open = false;
        }
    }

    pub fn open_add_profile(&mut self) {
        // start halfway between the selected profile and the next one (or the one before the last)
        let p = &self.st.proj;
        let i = self.sel_prof.min(p.profiles.len() - 1);
        let (a, b) = if i + 1 < p.profiles.len() { (p.profiles[i].x, p.profiles[i + 1].x) } else { (p.profiles[i - 1].x, p.profiles[i].x) };
        let sym = p.partner(i).is_some() || p.profiles[i].x.abs() > 1.0;
        self.addp = Some((((a + b) / 2.0 * 10.0).round() / 10.0, sym));
    }

    pub fn addp_window(&mut self, ctx: &egui::Context) {
        let Some((mut x, mut sym)) = self.addp else { return };
        let mut open = true;
        let mut add = false;
        let mut pick = false;
        let mut cancel = false;
        let problem = self.st.proj.add_profile_problem(x, sym);
        egui::Window::new("Add a profile").anchor(Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).resizable(false).default_width(380.0).show(ctx, |ui| {
            ui.label(egui::RichText::new("It follows the windshield line along the top and the front line along the bottom. Its shape is blended from the profiles on either side.").size(12.0).color(MUTED));
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Mirrored?").strong());
            ui.radio_value(&mut sym, true, "Symmetrical: add one here and a matching one on the other side");
            ui.radio_value(&mut sym, false, "Just this side: add a single profile");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Where (across the dash)").strong());
                ui.add(egui::DragValue::new(&mut x).speed(1.0).mm());
            });
            ui.label(egui::RichText::new(if sym { format!("Adds profiles at x = {} and x = {}. Negative is the driver side.", crate::units::num(x, 1), crate::units::num(-x, 1)) } else { format!("Adds one profile at x = {:.1}. Negative is the driver side.", x) }).size(12.0).color(MUTED));
            if let Some(p) = &problem {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(p).size(12.0).color(DANGER));
            }
            ui.add_space(8.0);
            if ui.add_sized([ui.available_width(), 32.0], egui::Button::new(egui::RichText::new("Click on a guide to place it").strong().color(Color32::BLACK)).fill(ACCENT)).on_hover_text("Then click the windshield line or the front line, in the 3D view or in the Guidelines editor").clicked() {
                pick = true;
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Or type the position above and use Add.").size(12.0).color(MUTED));
            ui.horizontal(|ui| {
                if ui.add_enabled(problem.is_none(), egui::Button::new("Add at the typed position")).clicked() {
                    add = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        self.addp = Some((x, sym));
        if add {
            self.op(format!(r#"{{"op":"add_profile_at","x":{},"sym":{}}}"#, x, sym));
            if let Some(i) = self.st.proj.profiles.iter().position(|p| (p.x - x).abs() < 1e-6) {
                self.select_profile(i);
            }
            self.addp = None;
        } else if pick {
            // show the guidelines so there is something to click on
            self.addp_pick = Some(sym);
            self.addp = None;
            self.dock_open = true;
            if self.dock_tab != DockTab::Height {
                self.dock_tab = DockTab::Plan;
            }
            self.plan.fitted = false;
        } else if cancel || !open {
            self.addp = None;
        }
    }

    /// Put a new profile at x (and its mirror) after a click on a guide; stays in placing mode when it cannot go there.
    pub fn place_profile(&mut self, x: f64, sym: bool) {
        if let Some(e) = self.st.proj.add_profile_problem(x, sym) {
            self.toast(&e, true);
            return;
        }
        self.op(format!(r#"{{"op":"add_profile_at","x":{},"sym":{}}}"#, x, sym));
        match self.st.proj.profiles.iter().position(|p| (p.x - x).abs() < 1e-6) {
            Some(i) => {
                self.select_profile(i);
                self.addp_pick = None;
                self.toast("Profile added.", false);
            }
            // not added: the reason was toasted by op(); stay in placing mode so it can be tried again
            None => {}
        }
    }

    pub fn addp_pick_banner(&mut self, ctx: &egui::Context) {
        let Some(sym) = self.addp_pick else { return };
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.addp_pick = None;
            return;
        }
        let mut stop = false;
        egui::Area::new(egui::Id::new("addp_banner")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, [0.0, 70.0]).show(ctx, |ui| {
            egui::Frame::new().fill(ACCENT_DIM).stroke(Stroke::new(1.5, ACCENT)).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(if sym { "Click the windshield line or the front line: a mirrored pair goes there" } else { "Click the windshield line or the front line: one profile goes there" }).strong());
                    if ui.button("Cancel (Esc)").clicked() {
                        stop = true;
                    }
                });
            });
        });
        if stop {
            self.addp_pick = None;
        }
    }

    pub fn open_exports(&self) {
        let d = self.st.dir.join("exports");
        let _ = std::fs::create_dir_all(&d);
        let cmd = if cfg!(target_os = "macos") { "open" } else if cfg!(target_os = "windows") { "explorer" } else { "xdg-open" };
        let _ = std::process::Command::new(cmd).arg(d).spawn();
    }
}

// ---------------------------------------------------------------- small shared widgets
/// A section heading with a clear rule above it (no rule when it is the first thing in a panel).
pub fn label_caps(ui: &mut egui::Ui, text: &str) {
    let first = ui.min_rect().height() < 2.0;
    if first {
        ui.add_space(2.0);
    } else {
        ui.add_space(10.0);
        rule(ui);
        ui.add_space(6.0);
    }
    ui.label(egui::RichText::new(text.to_uppercase()).size(11.0).color(MUTED).strong());
    ui.add_space(2.0);
}

/// A thin full-width divider line.
pub fn rule(ui: &mut egui::Ui) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, Color32::from_rgb(70, 76, 88)));
}

/// A bordered card for one group of related controls, with a heading.
pub fn card(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(Color32::from_rgb(30, 34, 40))
        .stroke(Stroke::new(1.0, Color32::from_rgb(70, 76, 88)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(title.to_uppercase()).size(11.0).color(ACCENT).strong());
            ui.add_space(4.0);
            add(ui);
        });
}

/// A lighter divider between sub-groups inside a card.
pub fn sub_rule(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    rule(ui);
    ui.add_space(6.0);
}

pub fn pill_button(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.0), TEXT);
    let size = vec2(galley.size().x + 22.0, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if on { ACCENT_DIM } else if resp.hovered() { CARD_HI } else { CARD };
    ui.painter().rect_filled(rect, CornerRadius::same(13), fill);
    ui.painter().rect_stroke(rect, CornerRadius::same(13), Stroke::new(1.0, if on { ACCENT } else { LINE }), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(13.0), if on { Color32::WHITE } else { TEXT });
    resp
}

pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(text).color(Color32::from_rgb(27, 19, 8)).strong()).fill(ACCENT).stroke(Stroke::NONE).min_size(vec2(0.0, 30.0)))
}

pub fn eye(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
    let c = rect.center();
    let col = if on { if resp.hovered() { Color32::WHITE } else { TEXT } } else { Color32::from_rgb(88, 94, 104) };
    let p = ui.painter();
    if on {
        let mut pts = Vec::new();
        for i in 0..=16 {
            let t = i as f32 / 16.0 * std::f32::consts::PI;
            pts.push(pos2(c.x - 8.0 + 16.0 * (1.0 - t.cos()) / 2.0, c.y - 5.0 * t.sin()));
        }
        p.add(egui::Shape::line(pts.clone(), Stroke::new(1.4, col)));
        let mut low: Vec<Pos2> = pts.iter().map(|q| pos2(q.x, 2.0 * c.y - q.y)).collect();
        low.reverse();
        p.add(egui::Shape::line(low, Stroke::new(1.4, col)));
        p.circle_filled(c, 2.6, col);
    } else {
        p.line_segment([pos2(c.x - 8.0, c.y), pos2(c.x + 8.0, c.y)], Stroke::new(1.4, col));
    }
    resp
}

/// Row used in the scene tree: highlight, optional color swatch, text, optional trailing caption.
pub fn tree_row(ui: &mut egui::Ui, selected: bool, swatch: Option<Color32>, text: &str, caption: &str, indent: f32) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(7), ACCENT_DIM);
        p.rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), CARD);
    }
    let mut x = rect.left() + 10.0 + indent;
    if let Some(c) = swatch {
        p.rect_filled(Rect::from_center_size(pos2(x + 6.0, rect.center().y), vec2(12.0, 12.0)), CornerRadius::same(3), c);
        x += 20.0;
    }
    p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, FontId::proportional(14.0), TEXT);
    if !caption.is_empty() {
        p.text(pos2(rect.right() - 34.0, rect.center().y), Align2::RIGHT_CENTER, caption, FontId::proportional(11.5), MUTED);
    }
    resp
}

pub fn mat_of(l: &crate::materials::Look) -> MatU {
    let p = l.preset();
    MatU { col: [l.col[0] as f32 / 255.0, l.col[1] as f32 / 255.0, l.col[2] as f32 / 255.0], a: [p.rough, p.metal, p.coat, p.kind], b: [p.cell, p.depth, 0.0, 0.0] }
}

/// Material picker: preset pills, color swatches and a free color button. Returns the new look when changed.
pub fn look_editor(ui: &mut egui::Ui, cur: Option<crate::materials::Look>, allow_default: bool) -> Option<(Option<crate::materials::Look>, bool)> {
    use crate::materials::{Look, PRESETS};
    let mut out: Option<(Option<Look>, bool)> = None;
    ui.horizontal_wrapped(|ui| {
        if allow_default && pill_button(ui, "As drawn", cur.is_none()).clicked() {
            out = Some((None, false));
        }
        for (i, p) in PRESETS.iter().enumerate() {
            let on = cur.map_or(false, |l| l.mat as usize == i);
            if pill_button(ui, p.label, on).clicked() {
                let col = match cur {
                    // keep the color when the new material can wear it (non-metal to non-metal)
                    Some(l) if PRESETS[l.mat as usize].metal == p.metal && p.metal == 0.0 => l.col,
                    _ => p.col,
                };
                out = Some((Some(Look { mat: i as u8, col }), false));
            }
        }
    });
    if let Some(l) = cur {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for c in l.preset().swatches.iter() {
                let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                ui.painter().rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(c[0], c[1], c[2]));
                ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(if l.col == *c { 2.0 } else { 1.0 }, if l.col == *c { ACCENT } else { LINE }), StrokeKind::Inside);
                if resp.clicked() {
                    out = Some((Some(Look { mat: l.mat, col: *c }), false));
                }
            }
            let mut rgb = l.col;
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                out = Some((Some(Look { mat: l.mat, col: rgb }), true));
            }
        });
    }
    out
}
