// Backwoods Dash Designer. Native desktop app (Rust + egui/OpenGL).
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
mod app;
mod engine;
mod docshot;
mod errlog;
mod help;
mod help_img;
mod file;
mod import;
mod lighting;
mod links;
mod sidewall;
mod json;
mod materials;
mod model;
mod parts;
mod cuts;
mod ui_cuts;
mod ui_file;
mod ui_gizmo;
mod refimg;
mod render;
mod shapes;
mod spacemouse;
mod templates;
mod theme;
mod ui_panels;
mod ui_ref;
mod ui_shapes;
mod ui_size;
mod ui_view;
mod ui_wiz;
mod units;

use app::*;
use eframe::egui;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_no += 1;
        self.docshot();
        // start-up: nothing of any dash is shown until the person chooses (create, open last, or open one)
        if self.file.welcome {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG)).show(ui, |ui| self.welcome_ui(ui));
            self.file_frame(&ctx);
            self.help_window(&ctx);
            return;
        }
        // keyboard shortcuts (not while typing)
        if !ctx.egui_wants_keyboard_input() {
            let (kl, kr, kt) = ctx.input(|i| (i.key_pressed(egui::Key::OpenBracket), i.key_pressed(egui::Key::CloseBracket), i.key_pressed(egui::Key::Tab)));
            if kl {
                self.hide_left = !self.hide_left;
            }
            if kr {
                self.hide_right = !self.hide_right;
            }
            if kt {
                let hide = !(self.hide_left && self.hide_right);
                self.hide_left = hide;
                self.hide_right = hide;
            }
            let (undo, redo, del) = ctx.input(|i| {
                let m = i.modifiers.command;
                (m && i.key_pressed(egui::Key::Z) && !i.modifiers.shift, (m && i.key_pressed(egui::Key::Z) && i.modifiers.shift) || (m && i.key_pressed(egui::Key::Y)), i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
            });
            if undo {
                self.op_s(r#"{"op":"undo"}"#);
            }
            if redo {
                self.op_s(r#"{"op":"redo"}"#);
            }
            if del && self.shape_ed.is_none() && self.focus == Focus::Cut {
                if let Some(id) = self.sel_cut.filter(|id| !self.st.proj.cuts.iter().any(|c| c.id == *id && c.locked)) {
                    self.op(format!(r#"{{"op":"cut_remove","id":{}}}"#, id));
                    self.sel_cut = None;
                    self.focus = Focus::Dash;
                }
            } else if del && self.shape_ed.is_none() {
                if let Some(id) = self.sel_part {
                    self.op(format!(r#"{{"op":"comp_remove","id":{}}}"#, id));
                }
            }
        }
        let modal = self.exp_open;
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("modal_open"), modal));
        self.spacemouse_frame(&ctx);
        self.ui_top(ui);
        self.ui_status(ui);
        self.ui_left(ui);
        self.ui_right(ui);
        if self.shape_ed.is_none() {
            self.ui_dock(ui);
        }
        self.refresh();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG)).show(ui, |ui| {
            if self.shape_ed.is_some() {
                self.shape_editor_center(ui);
            } else {
                self.ui_viewport(ui);
            }
        });
        self.ui_wizard(&ctx);
        self.help_window(&ctx);
        self.spacemouse_window(&ctx);
        self.resample_window(&ctx);
        self.leave_window(&ctx);
        self.lighting_window(&ctx);
        self.export_window(&ctx);
        self.addp_window(&ctx);
        self.addp_pick_banner(&ctx);
        self.ref_window(&ctx);
        self.save_if_needed(&ctx);
        self.file_frame(&ctx);
        if self.drag_item.is_some() || self.toast.is_some() {
            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result {
    // Working folder: BW_DASH_DIR (or the older DASHFW_DIR) if set, otherwise BackwoodsDashDesigner in the home folder.
    // A folder called DashFW from earlier versions is renamed to the new name the first time (nothing is copied or lost).
    let dir = std::env::var("BW_DASH_DIR").or_else(|_| std::env::var("DASHFW_DIR")).map(std::path::PathBuf::from).unwrap_or_else(|_| {
        let home = std::path::PathBuf::from(std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into()));
        let (new, old) = (home.join("BackwoodsDashDesigner"), home.join("DashFW"));
        if !new.exists() && old.is_dir() && std::fs::rename(&old, &new).is_err() {
            return old;
        }
        new
    });
    let _ = std::fs::create_dir_all(&dir);
    file::startup(&dir);
    units::load(&dir);
    errlog::init(&dir);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1480.0, 900.0]).with_min_inner_size([1000.0, 640.0]).with_title("Backwoods Dash Designer").with_icon(app_icon()),
        depth_buffer: 24,
        multisampling: std::env::var("DASHFW_MSAA").ok().and_then(|v| v.parse().ok()).unwrap_or(4),
        ..Default::default()
    };
    eframe::run_native("Backwoods Dash Designer", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, dir.clone())))))
}

fn app_icon() -> egui::IconData {
    match image::load_from_memory(include_bytes!("../assets/icon_256.png")) {
        Ok(img) => {
            let img = img.to_rgba8();
            let (width, height) = img.dimensions();
            egui::IconData { rgba: img.into_raw(), width, height }
        }
        Err(_) => egui::IconData::default(),
    }
}
