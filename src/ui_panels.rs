// Toolbar, scene tree, parts library, inspector and status bar.
use crate::app::*;
use crate::cuts::CutOp;
use crate::engine;
use crate::ui_cuts::op_color;
use crate::theme::*;
use crate::units::MmExt;
use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, Sense, Stroke, StrokeKind};

impl App {
    pub fn ui_top(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top").frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(14, 9)).stroke(Stroke::new(1.0, LINE))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Backwoods").size(18.0).strong().color(ACCENT));
                ui.label(egui::RichText::new("Dash Designer").size(12.0).color(MUTED));
                ui.add_space(10.0);
                let lock = self.shape_ed.is_some();
                if lock {
                    if ui.button("File").on_hover_text("The shape editor is open: save or discard it first").clicked() {
                        self.leave_click(false);
                    }
                } else {
                    self.file_menu(ui);
                }
                ui.add_space(10.0);
                if primary_button(ui, "New dash…").on_hover_text("Walk through the pieces of a dash step by step").clicked() {
                    if self.leave_click(false) {
                        // the shape editor has work in it: the prompt is showing
                    } else {
                        self.file_request(crate::ui_file::FileAct::NewDash);
                    }
                }
                ui.add_space(8.0);
                let walls = self.walls_sel();
                let names = [("Top pad", "Edit the top dash pad"), ("Lower pad", "Edit the lower dash pad"), ("Top trim", "Edit the top trim piece that sits over the top pad")];
                for (k, (nm, tip)) in names.iter().enumerate() {
                    if pill_button(ui, nm, self.st.active == k as u8 && !walls).on_hover_text(*tip).clicked() {
                        if self.leave_click(false) {
                            // the shape editor has work in it: the prompt is showing
                        } else {
                            self.switch_piece(k as u8);
                            self.dock_tab = DockTab::Profile;
                            self.focus = Focus::Dash;
                        }
                    }
                }
                if pill_button(ui, "Side walls", walls).on_hover_text("The closing walls at the two ends of the dash: back curve in the bottom panel, material on the right").clicked() {
                    if self.leave_click(false) {
                        // the shape editor has work in it: the prompt is showing
                    } else {
                        self.select_walls();
                    }
                }
                ui.add_space(8.0);
                let can_u = !self.st.undo.is_empty();
                let can_r = !self.st.redo.is_empty();
                if ui.add_enabled(can_u, egui::Button::new("Undo")).on_hover_text("Ctrl/Cmd + Z").clicked() {
                    if self.leave_click(false) {
                        // the shape editor has work in it: the prompt is showing
                    } else {
                        self.op_s(r#"{"op":"undo"}"#);
                    }
                }
                if ui.add_enabled(can_r, egui::Button::new("Redo")).on_hover_text("Ctrl/Cmd + Shift + Z").clicked() {
                    if self.leave_click(false) {
                        // the shape editor has work in it: the prompt is showing
                    } else {
                        self.op_s(r#"{"op":"redo"}"#);
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, "Export STL(s)").on_hover_text("Write a printable 3D file").clicked() {
                        if self.leave_click(false) {
                            // the shape editor has work in it: the prompt is showing
                        } else {
                            self.export();
                            self.focus = Focus::Dash;
                        }
                    }
                    ui.add_space(6.0);
                    ui.add(egui::DragValue::new(&mut self.st.thickness).range(0.0..=40.0).speed(0.1).prefix("wall ").mm())
                        .on_hover_text("Shell thickness used when exporting (0 = surface only)");
                    ui.add_space(6.0);
                    let inch = crate::units::inch();
                    if pill_button(ui, if inch { "in" } else { "mm" }, false).on_hover_text("Show sizes in millimeters or inches. Click to switch.").clicked() {
                        crate::units::set(!inch, &self.st.dir);
                    }
                    ui.add_space(6.0);
                    if pill_button(ui, "Lighting", self.light_open).on_hover_text("Adjust the lights: brightness, fill, reflections, texture strength").clicked() {
                        self.light_open = !self.light_open;
                    }
                    ui.add_space(6.0);
                    if pill_button(ui, "Help", self.help_open).on_hover_text("Open the manual (F1)").clicked() {
                        self.help_open = !self.help_open;
                    }
                });
            });
        });
    }

    pub fn ui_status(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(14, 5)).stroke(Stroke::new(1.0, LINE))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let p = &self.st.proj;
                let base = format!("{}  ·  {} profiles  ·  {} sections  ·  {} parts", self.st.piece_name(), p.profiles.len(), p.sections.len(), p.comps.len());
                if let Some((m, t, err)) = &self.toast {
                    if t.elapsed().as_secs() < 5 {
                        ui.label(egui::RichText::new(m).color(if *err { DANGER } else { TEAL }));
                    } else {
                        ui.label(egui::RichText::new(base).color(MUTED).size(12.0));
                    }
                } else {
                    ui.label(egui::RichText::new(base).color(MUTED).size(12.0));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(egui::RichText::new("Drag: orbit   Space+drag, Shift+drag or middle-drag: pan   Scroll: zoom   Click a line to select it").color(MUTED).size(12.0));
                    ui.add_space(10.0);
                    if ui.small_button(format!("SpaceMouse: {}", crate::spacemouse::status_text())).clicked() {
                        self.sm_open = !self.sm_open;
                    }
                });
            });
        });
    }

    // ---------------------------------------------------------------- left: scene + library
    /// A small pull-out tab on the edge of the viewport. Returns true when clicked.
    fn edge_tab(ctx: &egui::Context, id: &str, pos: egui::Pos2, glyph: &str, tip: &str) -> bool {
        if ctx.data(|d| d.get_temp::<bool>(egui::Id::new("modal_open"))).unwrap_or(false) {
            return false;
        }
        egui::Area::new(egui::Id::new(id))
            .fixed_pos(pos)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 62.0), Sense::click());
                let hot = resp.hovered();
                ui.painter().rect_filled(rect, CornerRadius::same(6), if hot { CARD_HI } else { PANEL });
                ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, if hot { ACCENT } else { LINE }), StrokeKind::Inside);
                let (c, d) = (rect.center(), if glyph.contains('\u{25C0}') { -1.0f32 } else { 1.0 });
                let tri = vec![pos2(c.x + 4.0 * d, c.y), pos2(c.x - 4.0 * d, c.y - 7.0), pos2(c.x - 4.0 * d, c.y + 7.0)];
                ui.painter().add(egui::Shape::convex_polygon(tri, if hot { ACCENT } else { TEXT }, Stroke::NONE));
                resp.on_hover_text(tip).clicked()
            })
            .inner
    }

    /// Same pull-out tab, lying along the top edge of the bottom editor. `down` = points down (fold away).
    pub fn edge_tab_h(ctx: &egui::Context, id: &str, pos: egui::Pos2, down: bool, tip: &str) -> bool {
        if ctx.data(|d| d.get_temp::<bool>(egui::Id::new("modal_open"))).unwrap_or(false) {
            return false;
        }
        egui::Area::new(egui::Id::new(id))
            .fixed_pos(pos)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(62.0, 20.0), Sense::click());
                let hot = resp.hovered();
                ui.painter().rect_filled(rect, CornerRadius::same(6), if hot { CARD_HI } else { PANEL });
                ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, if hot { ACCENT } else { LINE }), StrokeKind::Inside);
                let (c, d) = (rect.center(), if down { 1.0f32 } else { -1.0 });
                let tri = vec![pos2(c.x, c.y + 4.0 * d), pos2(c.x - 7.0, c.y - 4.0 * d), pos2(c.x + 7.0, c.y - 4.0 * d)];
                ui.painter().add(egui::Shape::convex_polygon(tri, if hot { ACCENT } else { TEXT }, Stroke::NONE));
                resp.on_hover_text(tip).clicked()
            })
            .inner
    }

    pub fn ui_left(&mut self, ui: &mut egui::Ui) {
        if self.hide_left {
            let r = ui.available_rect_before_wrap();
            if Self::edge_tab(ui.ctx(), "left_tab", pos2(r.left(), r.top() + 8.0), "\u{25B6}", "Open the left menu") {
                self.hide_left = false;
            }
            return;
        }
        let lid = egui::Id::new(("left", self.left_gen));
        let r = egui::Panel::left(lid).default_size(270.0).size_range(60.0..=380.0).frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(12)).stroke(Stroke::new(1.0, LINE))).show(ui, |ui| {
            let editing = self.shape_ed.is_some();
            if editing {
                self.left_tab = LeftTab::Shapes;
            }
            ui.horizontal(|ui| {
                if pill_button(ui, "Scene", self.left_tab == LeftTab::Scene).on_hover_text(if editing { "The shape editor is open: you will be asked to save or discard it first" } else { "The pieces and cut-outs in the dash" }).clicked() {
                    if !self.leave_click(true) {
                        self.left_tab = LeftTab::Scene;
                    }
                }
                if pill_button(ui, "Parts & shapes", self.left_tab == LeftTab::Shapes).on_hover_text("Your imported 3D parts, flat shapes and drawn outlines").clicked() {
                    self.left_tab = LeftTab::Shapes;
                }
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.left_tab {
                LeftTab::Scene => self.scene_tree(ui),
                LeftTab::Shapes => self.shapes_tab(ui),
            });
        });
        if r.response.rect.width() < 140.0 {
            self.hide_left = true;
            self.left_gen += 1;
        } else if Self::edge_tab(ui.ctx(), "left_tab", pos2(r.response.rect.right(), r.response.rect.top() + 8.0), "\u{25C0}", "Fold the left menu away") {
            self.hide_left = true;
            self.left_gen += 1;
        }
    }

    fn group_header(&mut self, ui: &mut egui::Ui, title: &str, layer: Option<u8>) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title.to_uppercase()).size(11.0).color(MUTED).strong());
            if let Some(l) = layer {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let cur = match l {
                        0 => self.layers.skin,
                        1 => self.layers.sections,
                        2 => self.layers.lines,
                        3 => self.layers.rail,
                        _ => self.layers.parts,
                    };
                    if eye(ui, cur).on_hover_text("Show / hide").clicked() {
                        match l {
                            0 => self.layers.skin = !cur,
                            1 => self.layers.sections = !cur,
                            2 => self.layers.lines = !cur,
                            3 => self.layers.rail = !cur,
                            _ => self.layers.parts = !cur,
                        }
                    }
                });
            }
        });
    }

    fn scene_tree(&mut self, ui: &mut egui::Ui) {
        // Dash
        let top = ui.cursor().min;
        let r = tree_row(ui, self.focus == Focus::Dash, None, "Dash", "overview", 0.0);
        if r.clicked() {
            self.focus = Focus::Dash;
            self.sel_part = None;
            self.sel_sec = None;
        }
        let er = egui::Rect::from_min_size(pos2(r.rect.right() - 28.0, top.y + 3.0), vec2(24.0, 22.0));
        let sk = self.layers.skin;
        if ui.put(er, |ui: &mut egui::Ui| eye(ui, sk)).on_hover_text("Show / hide the dash surface").clicked() {
            self.layers.skin = !sk;
        }
        // Sections
        self.group_header(ui, "Sections  (colors on the dash)", Some(1));
        let n = self.st.proj.sections.len();
        for k in 0..n {
            let (name, color, a, b) = {
                let p = &self.st.proj;
                let s = &p.sections[k];
                (s.name.clone(), hex(&s.color), p.pid_index(s.start).unwrap_or(0), p.pid_index(s.end).unwrap_or(0))
            };
            let sel = self.focus == Focus::Section && self.sel_sec == Some(k);
            if tree_row(ui, sel, Some(color), &name, &format!("P{}–P{}", a, b), 0.0).clicked() {
                self.select_section(k);
            }
        }
        // Profiles
        self.group_header(ui, "Profiles  (cross-sections)", Some(2));
        for i in 0..self.st.proj.profiles.len() {
            let (x, fixed, paired, linked) = {
                let p = &self.st.proj.profiles[i];
                (p.x, p.fixed, self.st.proj.partner(i).is_some(), self.st.proj.locked_partner(i).is_some())
            };
            let sel = self.focus == Focus::Profile && self.sel_prof == i;
            let cap = if fixed { "fixed" } else if linked { "mirrored" } else if paired { "own shape" } else { "" };
            if tree_row(ui, sel, None, &format!("P{}   x = {}", i, crate::units::num(x, 0)), cap, 0.0).clicked() {
                self.select_profile_view(i);
            }
        }
        // Rail
        let (hdr, rname) = match self.st.active {
            0 => ("Guidelines", "Windshield line"),
            1 => ("Guidelines", "Top guideline"),
            _ => ("Contour lines", "Windshield line (back)"),
        };
        self.group_header(ui, hdr, Some(3));
        let sel = self.focus == Focus::Rail;
        if tree_row(ui, sel && self.sel_rail2.is_none(), Some(BLUE), rname, &format!("{} points", self.st.proj.rail.len()), 0.0).clicked() {
            self.focus = Focus::Rail;
            self.sel_rail2 = None;
            self.sel_part = None;
        }
        if !self.st.proj.rail2.is_empty() {
            if tree_row(ui, sel && self.sel_rail2.is_some(), Some(TEAL), if self.st.active == 1 { "Bottom guideline" } else { "Front line" }, &format!("{} points", self.st.proj.rail2.len()), 0.0).clicked() {
                self.focus = Focus::Rail;
                self.sel_rail = None;
                self.sel_rail2 = Some(0);
                self.sel_part = None;
            }
        }
        if tree_row(ui, false, Some(Color32::from_rgb(255, 224, 120)), "Center line", if self.show_center { "shown" } else { "hidden" }, 0.0).clicked() {
            self.show_center = !self.show_center;
        }
    }


    // ---------------------------------------------------------------- layers: pieces, parts and shapes
    pub fn layers_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("PIECES & PARTS").size(11.0).color(MUTED).strong());
        ui.label(egui::RichText::new("Click to select, click again to deselect. Right-click a row for more.").size(11.0).color(MUTED));
        let mut acts: Vec<LayerAct> = Vec::new();
        egui::ScrollArea::vertical().id_salt("layers_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let walls = self.walls_sel();
            for k in 0..3 {
                let name = engine::PIECE_NAMES[k];
                let is_act = self.st.active as usize == k && !walls;
                let (comps, cuts): (Vec<(u32, String, bool, bool)>, Vec<(u32, String, CutOp, bool, bool, bool)>) = {
                    let p = self.st.piece(k);
                    (
                        p.comps.iter().map(|c| (c.id, c.name.clone(), c.visible, c.locked)).collect(),
                        p.cuts.iter().map(|c| (c.id, c.name.clone(), c.op, c.visible, c.locked, c.export)).collect(),
                    )
                };
                let n = comps.len() + cuts.len();
                let open = self.lay_open[k];
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
                let p = ui.painter();
                if is_act {
                    p.rect_filled(rect, CornerRadius::same(6), ACCENT_DIM);
                    p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
                } else if resp.hovered() {
                    p.rect_filled(rect, CornerRadius::same(6), CARD);
                }
                let c = pos2(rect.left() + 12.0, rect.center().y);
                let tri = if open { vec![c + vec2(-4.5, -2.5), c + vec2(4.5, -2.5), c + vec2(0.0, 3.5)] } else { vec![c + vec2(-2.5, -4.5), c + vec2(-2.5, 4.5), c + vec2(3.5, 0.0)] };
                p.add(egui::Shape::convex_polygon(tri, MUTED, Stroke::NONE));
                let vis = self.piece_vis[k];
                p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, name, FontId::proportional(13.5), if vis { TEXT } else { MUTED });
                p.text(pos2(rect.right() - 34.0, rect.center().y), Align2::RIGHT_CENTER, format!("{}", n), FontId::proportional(11.5), MUTED);
                let er = Rect::from_min_size(pos2(rect.right() - 28.0, rect.top() + 2.0), vec2(24.0, 22.0));
                if k == 2 && !self.st.trim_on {
                    let ar = Rect::from_min_size(pos2(rect.right() - 90.0, rect.top() + 2.0), vec2(84.0, 22.0));
                    let r = ui.put(ar, egui::Button::new(egui::RichText::new("Add trim").size(12.0))).on_hover_text("Add the top trim piece to this dash");
                    if r.clicked() {
                        acts.push(LayerAct::TrimOn(true));
                    }
                    continue;
                }
                let eresp = ui.put(er, |ui: &mut egui::Ui| eye(ui, vis)).on_hover_text(if vis { "Hide this piece in the views. It still exports." } else { "Show this piece in the views" });
                resp.clone().on_hover_text("Click to edit this piece. Click the arrow to fold or unfold its list.");
                if eresp.clicked() {
                    acts.push(LayerAct::PieceEye(k));
                } else if resp.clicked() {
                    let on_arrow = resp.interact_pointer_pos().map_or(false, |q| q.x < rect.left() + 24.0);
                    acts.push(if on_arrow { LayerAct::Fold(k) } else { LayerAct::SelectPiece(k) });
                }
                if open {
                    for (id, name, vis, locked) in &comps {
                        let sel = k == self.st.active as usize && self.sel_part == Some(*id) && !walls;
                        let (hit, r) = layer_row(ui, name, Color32::from_rgb(150, 160, 175), sel, *vis, *locked, None);
                        let id = *id;
                        match hit {
                            LayerHit::Select => acts.push(LayerAct::SelectPart(k, id)),
                            LayerHit::Eye => acts.push(LayerAct::Toggle(k, LayerItem::Part(id), LayerField::Vis)),
                            LayerHit::Lock => acts.push(LayerAct::Toggle(k, LayerItem::Part(id), LayerField::Lock)),
                            _ => {}
                        }
                        r.context_menu(|ui| {
                            if ui.button(if *vis { "Hide" } else { "Show" }).clicked() {
                                acts.push(LayerAct::Toggle(k, LayerItem::Part(id), LayerField::Vis));
                                ui.close();
                            }
                            if ui.button(if *locked { "Unlock" } else { "Lock in place" }).clicked() {
                                acts.push(LayerAct::Toggle(k, LayerItem::Part(id), LayerField::Lock));
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                acts.push(LayerAct::Delete(k, LayerItem::Part(id)));
                                ui.close();
                            }
                        });
                    }
                    for (id, name, op, vis, locked, exp) in &cuts {
                        let sel = k == self.st.active as usize && self.sel_cut == Some(*id) && self.focus == Focus::Cut && !walls;
                        let (hit, r) = layer_row(ui, name, op_color(*op), sel, *vis, *locked, Some(*exp));
                        let id = *id;
                        match hit {
                            LayerHit::Select => acts.push(LayerAct::SelectCut(k, id)),
                            LayerHit::Eye => acts.push(LayerAct::Toggle(k, LayerItem::Cut(id), LayerField::Vis)),
                            LayerHit::Lock => acts.push(LayerAct::Toggle(k, LayerItem::Cut(id), LayerField::Lock)),
                            LayerHit::Export => acts.push(LayerAct::Toggle(k, LayerItem::Cut(id), LayerField::Export)),
                            _ => {}
                        }
                        r.context_menu(|ui| {
                            if ui.button(if *vis { "Hide" } else { "Show" }).clicked() {
                                acts.push(LayerAct::Toggle(k, LayerItem::Cut(id), LayerField::Vis));
                                ui.close();
                            }
                            if ui.button(if *locked { "Unlock" } else { "Lock in place" }).clicked() {
                                acts.push(LayerAct::Toggle(k, LayerItem::Cut(id), LayerField::Lock));
                                ui.close();
                            }
                            if ui.button(if *exp { "Leave out of the export" } else { "Include in the export" }).clicked() {
                                acts.push(LayerAct::Toggle(k, LayerItem::Cut(id), LayerField::Export));
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                acts.push(LayerAct::Delete(k, LayerItem::Cut(id)));
                                ui.close();
                            }
                        });
                    }
                    if n == 0 {
                        ui.label(egui::RichText::new("   nothing placed on it yet").size(11.5).color(MUTED));
                    }
                }
            }
            // the closing side walls
            let walls = self.walls_sel();
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
            let p = ui.painter();
            if walls {
                p.rect_filled(rect, CornerRadius::same(6), ACCENT_DIM);
                p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            } else if resp.hovered() {
                p.rect_filled(rect, CornerRadius::same(6), CARD);
            }
            p.rect_filled(Rect::from_center_size(pos2(rect.left() + 12.0, rect.center().y), vec2(10.0, 10.0)), CornerRadius::same(2), Color32::from_rgb(120, 170, 150));
            p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, "Side walls", FontId::proportional(13.5), if self.sw.on { TEXT } else { MUTED });
            let er = Rect::from_min_size(pos2(rect.right() - 28.0, rect.top() + 2.0), vec2(24.0, 22.0));
            let on = self.sw.on;
            let eresp = ui.put(er, |ui: &mut egui::Ui| eye(ui, on)).on_hover_text(if on { "Hide the side walls in the views" } else { "Show the side walls in the views" });
            resp.clone().on_hover_text("The closing walls at the two ends of the dash. Click to edit their back curve and material; click again to deselect.");
            if eresp.clicked() {
                acts.push(LayerAct::WallsEye);
            } else if resp.clicked() {
                acts.push(if walls { LayerAct::Deselect } else { LayerAct::SelectWalls });
            }
            // empty space below the list: click it to deselect everything
            let h = ui.available_height().max(36.0);
            let (er2, eresp2) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
            ui.painter().text(er2.center_top() + vec2(0.0, 14.0), Align2::CENTER_CENTER, "click here to deselect", FontId::proportional(11.0), Color32::from_rgb(70, 76, 86));
            eresp2.clone().on_hover_text("Click empty space to deselect");
            if eresp2.clicked() {
                acts.push(LayerAct::Deselect);
            }
        });
        for a in acts {
            self.layer_apply(a);
        }
    }

    fn layer_apply(&mut self, a: LayerAct) {
        match a {
            LayerAct::Fold(k) => self.lay_open[k] = !self.lay_open[k],
            LayerAct::TrimOn(on) => {
                if on {
                    self.st.add_trim();
                }
                self.st.trim_on = on;
                self.piece_vis[2] = on;
                self.st.dirty = true;
                self.rev += 1;
            }
            LayerAct::PieceEye(k) => {
                self.piece_vis[k] = !self.piece_vis[k];
                self.rev += 1;
            }
            LayerAct::WallsEye => {
                self.sw.on = !self.sw.on;
                self.rev += 1;
            }
            LayerAct::SelectWalls => self.select_walls(),
            LayerAct::Deselect => {
                self.sel_part = None;
                self.sel_cut = None;
                self.focus = Focus::Dash;
                if self.walls_sel() {
                    self.dock_tab = DockTab::Profile;
                }
            }
            LayerAct::SelectPiece(k) => {
                let keep = self.cam;
                self.switch_piece(k as u8);
                self.cam = keep;
                self.dock_tab = DockTab::Profile;
                self.focus = Focus::Dash;
            }
            LayerAct::SelectPart(k, id) if k == self.st.active as usize && self.sel_part == Some(id) && self.focus == Focus::Part => self.layer_apply(LayerAct::Deselect),
            LayerAct::SelectCut(k, id) if k == self.st.active as usize && self.sel_cut == Some(id) && self.focus == Focus::Cut => self.layer_apply(LayerAct::Deselect),
            LayerAct::SelectPart(k, id) => {
                if k != self.st.active as usize {
                    let keep = self.cam;
                    self.switch_piece(k as u8);
                    self.cam = keep;
                }
                self.select_part(id);
            }
            LayerAct::SelectCut(k, id) => {
                if k != self.st.active as usize {
                    let keep = self.cam;
                    self.switch_piece(k as u8);
                    self.cam = keep;
                }
                self.select_cut(id);
            }
            LayerAct::Toggle(k, item, field) => {
                if k == self.st.active as usize {
                    match item {
                        LayerItem::Part(id) => {
                            if let Some(c) = self.st.proj.comps.iter().find(|c| c.id == id).cloned() {
                                let (key, v) = if matches!(field, LayerField::Vis) { ("visible", !c.visible) } else { ("locked", !c.locked) };
                                self.op(format!(r#"{{"op":"comp_set","id":{},"{}":{}}}"#, id, key, v));
                            }
                        }
                        LayerItem::Cut(id) => {
                            if let Some(mut c) = self.st.proj.cuts.iter().find(|c| c.id == id).cloned() {
                                match field {
                                    LayerField::Vis => c.visible = !c.visible,
                                    LayerField::Lock => c.locked = !c.locked,
                                    LayerField::Export => c.export = !c.export,
                                }
                                self.cut_put(&c, false);
                            }
                        }
                    }
                } else {
                    // a piece on the shelf: change it in place
                    self.st.push_undo();
                    self.st.redo.clear();
                    if let Some(p) = self.st.piece_mut(k) {
                        match item {
                            LayerItem::Part(id) => {
                                if let Some(c) = p.comps.iter_mut().find(|c| c.id == id) {
                                    if matches!(field, LayerField::Vis) { c.visible = !c.visible } else { c.locked = !c.locked }
                                }
                            }
                            LayerItem::Cut(id) => {
                                if let Some(c) = p.cuts.iter_mut().find(|c| c.id == id) {
                                    match field {
                                        LayerField::Vis => c.visible = !c.visible,
                                        LayerField::Lock => c.locked = !c.locked,
                                        LayerField::Export => c.export = !c.export,
                                    }
                                }
                            }
                        }
                    }
                    self.st.dirty = true;
                    self.rev += 1;
                }
            }
            LayerAct::Delete(k, item) => {
                if k == self.st.active as usize {
                    match item {
                        LayerItem::Part(id) => self.op(format!(r#"{{"op":"comp_remove","id":{}}}"#, id)),
                        LayerItem::Cut(id) => self.op(format!(r#"{{"op":"cut_remove","id":{}}}"#, id)),
                    }
                    self.focus = Focus::Dash;
                    self.sel_part = None;
                    self.sel_cut = None;
                } else {
                    self.st.push_undo();
                    self.st.redo.clear();
                    if let Some(p) = self.st.piece_mut(k) {
                        match item {
                            LayerItem::Part(id) => p.comps.retain(|c| c.id != id),
                            LayerItem::Cut(id) => p.cuts.retain(|c| c.id != id),
                        }
                    }
                    self.st.dirty = true;
                    self.rev += 1;
                }
            }
        }
    }

    fn _unused_marker(&self) {}

    /// Top trim only: what moving the front guideline does to the trim's profiles.
    pub fn front_mode_ui(&mut self, ui: &mut egui::Ui, compact: bool) {
        if self.st.active != 2 {
            return;
        }
        let mut follow = self.st.trim_follow;
        let mut changed = false;
        const A: &str = "Sides are filled solid down to the Top pad.";
        const B: &str = "Only the windshield end stays locked. Sides keep the shell thickness (no fill).";
        if compact {
            // the bottom editor bar: one short row, so the editor below keeps its room; the details are in the hover text
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Front guide moves:").size(13.0).strong().color(ACCENT));
                    changed |= ui.radio_value(&mut follow, false, "Profile end only").on_hover_text(A).clicked();
                    changed |= ui.radio_value(&mut follow, true, "Whole profile").on_hover_text(B).clicked();
                });
            });
        } else {
            ui.group(|ui| {
                ui.set_width(330.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Front guideline moves").size(13.0).strong().color(ACCENT));
                    ui.add_space(2.0);
                    changed |= ui.radio_value(&mut follow, false, "End of each profile only").clicked();
                    ui.label(egui::RichText::new(format!("    {}", A)).size(12.0).color(TEAL));
                    ui.add_space(2.0);
                    changed |= ui.radio_value(&mut follow, true, "Whole profile").clicked();
                    ui.label(egui::RichText::new(format!("    {}", B)).size(12.0).color(TEAL));
                });
            });
        }
        if changed {
            self.op(format!(r#"{{"op":"set_front_mode","follow":{}}}"#, follow));
        }
    }

    fn place_at_view_centre(&mut self, i: usize) {
        let r = self.vp_rect;
        let (o, d) = self.cam.ray(r.width() * 0.5, r.height() * 0.5, r.width(), r.height());
        let hit = self.mesh.as_ref().and_then(|m| m.raycast(o, d));
        match hit {
            Some(h) => self.add_part(i, h.p[0] as f64, h.p[1] as f64),
            None => self.add_part(i, 0.0, 100.0),
        }
    }

    // ---------------------------------------------------------------- right: inspector
    pub fn ui_right(&mut self, ui: &mut egui::Ui) {
        if self.hide_right {
            let r = ui.available_rect_before_wrap();
            if Self::edge_tab(ui.ctx(), "right_tab", pos2(r.right() - 20.0, r.top() + 8.0), "\u{25C0}", "Open the right menu") {
                self.hide_right = false;
            }
            return;
        }
        let rid = egui::Id::new((if self.shape_ed.is_some() { "right_shape" } else { "right" }, self.right_gen));
        let rr = egui::Panel::right(rid).default_size(if self.shape_ed.is_some() { 340.0 } else { 300.0 }).size_range(60.0..=420.0).frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(0)).stroke(Stroke::new(1.0, LINE))).show(ui, |ui| {
            if self.shape_ed.is_some() {
                egui::Frame::new().inner_margin(egui::Margin::same(14)).show(ui, |ui| self.shape_editor_right(ui));
                return;
            }
            let walls = self.walls_sel();
            // two clearly separate sections: the details above, a full-width Layers band below
            let lp = egui::Panel::bottom(egui::Id::new(("layers", self.right_gen))).resizable(true).default_size(230.0).size_range(90.0..=460.0).show_separator_line(false).frame(egui::Frame::new().fill(Color32::from_rgb(14, 16, 19)).inner_margin(egui::Margin { left: 14, right: 14, top: 12, bottom: 8 })).show(ui, |ui| {
                self.layers_panel(ui);
            });
            let r = lp.response.rect;
            ui.painter().rect_filled(egui::Rect::from_min_max(r.left_top(), pos2(r.right(), r.top() + 3.0)), CornerRadius::ZERO, Color32::from_rgb(96, 104, 118));
            egui::Frame::new().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.focus {
                    _ if walls => self.inspect_walls(ui),
                    Focus::Dash => self.inspect_dash(ui),
                    Focus::Profile => self.inspect_profile(ui),
                    Focus::Section => self.inspect_section(ui),
                    Focus::Part => self.inspect_part(ui),
                    Focus::Rail => self.inspect_rail(ui),
                    Focus::Cut => self.inspect_cut(ui),
                });
            });
        });
        if rr.response.rect.width() < 160.0 {
            self.hide_right = true;
            self.right_gen += 1;
        } else if Self::edge_tab(ui.ctx(), "right_tab", pos2(rr.response.rect.left() - 20.0, rr.response.rect.top() + 8.0), "\u{25B6}", "Fold the right menu away") {
            self.hide_right = true;
            self.right_gen += 1;
        }
    }

    fn title(&self, ui: &mut egui::Ui, t: &str, sub: &str) {
        ui.label(egui::RichText::new(t).size(18.0).strong());
        if !sub.is_empty() {
            ui.label(egui::RichText::new(sub).size(12.5).color(MUTED));
        }
        ui.add_space(6.0);
    }

    pub fn walls_sel(&self) -> bool {
        self.dock_open && self.dock_tab == DockTab::Walls
    }
    pub fn select_walls(&mut self) {
        self.dock_open = true;
        self.dock_tab = DockTab::Walls;
        self.sel_part = None;
        self.sel_sec = None;
        self.sel_cut = None;
        self.multi.clear();
        self.focus = Focus::Dash;
    }
    fn inspect_walls(&mut self, ui: &mut egui::Ui) {
        self.title(ui, "Side walls", "The closing wall at each end of the dash. Edit the back curve in the bottom panel.");
        label_caps(ui, "Side walls material");
        ui.checkbox(&mut self.sw.follow, "Same material and color as the top pad")
            .on_hover_text("The walls are drawn exactly like the top dash pad, so the texture and color match");
        if !self.sw.follow {
            ui.label(egui::RichText::new("Pick what the walls are made of and their color. Shown on the dash and kept in the project.").size(12.0).color(MUTED));
            let cur = self.sw.look;
            if let Some((Some(l), _)) = look_editor(ui, Some(cur), false) {
                self.sw.look = l;
            }
        }
        label_caps(ui, "Printing");
        ui.horizontal(|ui| {
            ui.label("Wall thickness");
            ui.add(egui::DragValue::new(&mut self.st.thickness).range(0.0..=40.0).speed(0.1).mm());
        });
        ui.label(egui::RichText::new("The side walls are always part of the dash and use this thickness.").size(12.0).color(MUTED));
    }

    pub fn inspect_dash(&mut self, ui: &mut egui::Ui) {
        let tt = ["Top dash pad", "Lower dash pad", "Top trim piece"][self.st.active as usize];
        self.title(ui, tt, "Nothing selected. Click a profile line in the 3D view, a section or a part to edit it.");
        let (np, ns, nk, nc) = {
            let p = &self.st.proj;
            (p.profiles.len(), p.sections.len(), p.keys.len(), p.comps.len())
        };
        egui::Frame::new().fill(CARD).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(format!("{} profiles across", np));
            ui.label(format!("{} sections", ns));
            ui.label(format!("{} key points", nk));
            ui.label(format!("{} parts placed", nc));
        });
        label_caps(ui, &format!("{} material", self.st.piece_name()));
        ui.label(egui::RichText::new("Pick what it is made of and its color. Shown on the dash and kept in the project.").size(12.0).color(MUTED));
        let cur = self.st.proj.look;
        if let Some((l, quiet)) = look_editor(ui, Some(cur), false) {
            if let Some(l) = l {
                self.op(format!(r#"{{"op":"set_look","target":"piece","mat":"{}","color":"{}","quiet":{}}}"#, l.preset().id, l.hex(), quiet));
            }
        }
        label_caps(ui, "Center line");
        ui.checkbox(&mut self.show_center, "Show the center line (x = 0)");
        ui.checkbox(&mut self.snap_center, "Parts snap to the center line");
        label_caps(ui, "Printing");
        ui.horizontal(|ui| {
            ui.label("Wall thickness");
            ui.add(egui::DragValue::new(&mut self.st.thickness).range(0.0..=40.0).speed(0.1).mm());
        });
        self.gap_ui(ui);
        if primary_button(ui, "Export STL(s)").clicked() {
            self.export();
        }
        if let Some(info) = self.export_info.clone() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(info).size(12.0).color(MUTED));
            if ui.button("Show exports folder").clicked() {
                self.open_exports();
            }
        }
        label_caps(ui, "Key points");
        ui.label(egui::RichText::new("Named points that sit in the same place on every profile (windshield line, front arc, lip tip…).").size(12.0).color(MUTED));
        self.keys_editor(ui);
        label_caps(ui, "Start over");
        if ui.button(format!("Reset the {} to its original profiles", self.st.piece_name().to_lowercase())).clicked() {
            self.op_s(r#"{"op":"reset"}"#);
            self.toast("Reset. Undo brings your dash back.", false);
        }
    }

    fn keys_editor(&mut self, ui: &mut egui::Ui) {
        let n = self.st.proj.keys.len();
        for k in 0..n {
            let (mut name, idx) = (self.st.proj.keys[k].name.clone(), self.st.proj.keys[k].idx);
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
                if r.changed() {
                    self.op(format!(r#"{{"op":"key_set","k":{},"name":{}}}"#, k, json_str(&name)));
                }
                if ui.small_button(format!("pt {}", idx)).on_hover_text("Select this point on the profile").clicked() {
                    self.sel_pt = idx;
                    self.focus = Focus::Profile;
                }
                if ui.small_button("use selected").on_hover_text("Move this key point to the selected point").clicked() {
                    let sp = self.sel_pt;
                    self.op(format!(r#"{{"op":"key_set","k":{},"idx":{}}}"#, k, sp));
                }
                if ui.small_button("×").clicked() {
                    self.op(format!(r#"{{"op":"key_remove","k":{}}}"#, k));
                }
            });
        }
        if ui.button("+ Add key point at selected point").clicked() {
            let sp = self.sel_pt;
            self.op(format!(r#"{{"op":"key_add","idx":{},"name":"Key point"}}"#, sp));
        }
    }

    fn inspect_profile(&mut self, ui: &mut egui::Ui) {
        let i = self.sel_prof;
        let (x, fixed, sym, paired, n) = {
            let p = &self.st.proj;
            (p.profiles[i].x, p.profiles[i].fixed, p.profiles[i].sym, p.partner(i).is_some(), p.profiles.len())
        };
        self.title(ui, &format!("Profile P{}", i), if fixed { "End or center profile: its position is fixed." } else { "Drag its handle in the 3D view or the rail editor, or type a position. Its top end stays pinned to the windshield line, its bottom end to the front line." });
        ui.horizontal(|ui| {
            ui.label("Position across (x)");
            let mut xv = x;
            let r = ui.add_enabled(!fixed, egui::DragValue::new(&mut xv).speed(1.0).mm());
            if r.drag_started() || r.gained_focus() {
                self.op_s(r#"{"op":"checkpoint"}"#);
            }
            if r.changed() {
                self.op(format!(r#"{{"op":"move_profile","i":{},"x":{}}}"#, i, xv));
            }
        });
        if paired {
            let mut s = sym;
            if ui.checkbox(&mut s, "Keep mirrored with the other side").changed() {
                self.op(format!(r#"{{"op":"set_sym","i":{},"v":{}}}"#, i, s));
            }
        }
        if i > 0 && i + 1 < n {
            let mut sh = self.st.proj.profiles[i].crease;
            if ui.checkbox(&mut sh, "Sharp edge at this profile").on_hover_text("On: the surface meets this profile with a sharp edge. Off: it flows smoothly through it.").changed() {
                self.op(format!(r#"{{"op":"set_crease","i":{},"v":{}}}"#, i, sh));
            }
        }
        ui.horizontal(|ui| {
            if ui.button("+ Add profile...").on_hover_text("Choose where it goes and whether it is mirrored").clicked() {
                self.open_add_profile();
            }
            if ui.add_enabled(!fixed, egui::Button::new("Remove")).clicked() {
                self.op(format!(r#"{{"op":"remove","i":{}}}"#, i));
            }
        });
        label_caps(ui, "Points on the curve");
        let np = self.st.proj.profiles[0].pts.len();
        ui.label(egui::RichText::new(format!("{} points on every profile. Adding or removing one keeps every other point where it is. Re-spacing recalculates them all evenly.", np)).size(12.0).color(MUTED));
        ui.horizontal(|ui| {
            if ui.add_enabled(self.sel_pt + 1 < np, egui::Button::new("+ Add point after selected")).clicked() {
                self.op(format!(r#"{{"op":"pt_add","k":{},"t":0.5}}"#, self.sel_pt));
                self.sel_pt += 1;
                self.clamp_sel();
            }
            if ui.add_enabled(self.sel_pt > 0 && self.sel_pt + 1 < np && np > 4, egui::Button::new("Remove selected")).clicked() {
                let k = self.sel_pt;
                self.op(format!(r#"{{"op":"pt_remove","k":{}}}"#, k));
                self.sel_pt = k.saturating_sub(1);
                self.clamp_sel();
            }
        });
        ui.add_space(2.0);
        if ui.button("Re-space all points evenly...").on_hover_text("Keep the shape, set a new point count on every profile").clicked() {
            self.rs_n = np.min(24).max(4);
            self.rs_open = true;
        }
        label_caps(ui, "Selected point");
        ui.label(egui::RichText::new(format!("Point {} of {}.  Edit its position and handles in the editor below the 3D view.", self.sel_pt, self.st.proj.profiles[i].pts.len() - 1)).size(12.5).color(MUTED));
        if self.sel_pt == 0 {
            ui.label(egui::RichText::new("Point 0 is the windshield anchor: it rides on the windshield rail.").size(12.5).color(ACCENT));
        }
        label_caps(ui, "Key points");
        self.keys_editor(ui);
    }

    fn inspect_section(&mut self, ui: &mut egui::Ui) {
        let Some(k) = self.sel_sec else { return self.inspect_dash(ui) };
        let (name, color, a, b, n, can_merge) = {
            let p = &self.st.proj;
            let s = &p.sections[k];
            (s.name.clone(), hex(&s.color), p.pid_index(s.start).unwrap_or(0), p.pid_index(s.end).unwrap_or(0), p.sections.len(), k + 1 < p.sections.len() && p.sections[k + 1].start == s.end)
        };
        self.title(ui, "Section", &format!("Covers profiles P{} to P{}.", a, b));
        let mut nm = name;
        if ui.add(egui::TextEdit::singleline(&mut nm).desired_width(f32::INFINITY)).changed() {
            self.op(format!(r#"{{"op":"section_set","k":{},"name":{}}}"#, k, json_str(&nm)));
        }
        ui.horizontal(|ui| {
            ui.label("Color");
            let mut c = [color.r(), color.g(), color.b()];
            if ui.color_edit_button_srgb(&mut c).changed() {
                self.op(format!(r##"{{"op":"section_set","k":{},"color":"#{:02x}{:02x}{:02x}"}}"##, k, c[0], c[1], c[2]));
            }
        });
        label_caps(ui, "Reshape the sections");
        ui.label(egui::RichText::new("Select a profile first (click it in the scene tree or 3D view), then use these.").size(12.0).color(MUTED));
        let sp = self.sel_prof;
        let (start, end) = (a, b);
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(sp > start && sp < end, egui::Button::new(format!("Split at P{}", sp))).clicked() {
                self.op(format!(r#"{{"op":"section_split","k":{},"at":{}}}"#, k, sp));
            }
            if ui.add_enabled(can_merge, egui::Button::new("Merge with next")).clicked() {
                self.op(format!(r#"{{"op":"section_merge","k":{}}}"#, k));
            }
            if ui.add_enabled(k + 1 < n, egui::Button::new(format!("Boundary to P{}", sp))).on_hover_text("Move the line between this section and the next to the selected profile").clicked() {
                self.op(format!(r#"{{"op":"section_boundary","k":{},"at":{}}}"#, k, sp));
            }
        });
    }

    fn inspect_part(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.sel_part else { return self.inspect_dash(ui) };
        let Some(c) = self.st.proj.comps.iter().find(|c| c.id == id).cloned() else { return };
        self.title(ui, &c.name, &format!("{}  ·  drag it in the 3D view to move", c.kind));
        let mut nm = c.name.clone();
        if ui.add(egui::TextEdit::singleline(&mut nm).desired_width(f32::INFINITY)).changed() {
            self.op(format!(r#"{{"op":"comp_set","id":{},"name":{}}}"#, id, json_str(&nm)));
        }
        let round = c.shape == "round";
        let mut w = c.w;
        let mut h = c.h;
        let mut rot = c.rot;
        egui::Grid::new("pp").num_columns(2).show(ui, |ui| {
            ui.label(if round { "Diameter" } else { "Width" });
            let r = ui.add(egui::DragValue::new(&mut w).range(2.0..=1000.0).speed(0.5).mm());
            if r.drag_started() || r.gained_focus() {
                self.op_s(r#"{"op":"checkpoint"}"#);
            }
            if r.changed() {
                let hh = if round { w } else { c.h };
                self.st.op(&format!(r#"{{"op":"comp_set","id":{},"w":{},"h":{}}}"#, id, w, hh)).ok();
                self.st.undo.pop();
                self.rev += 1;
            }
            ui.end_row();
            if !round {
                ui.label("Height");
                let r = ui.add(egui::DragValue::new(&mut h).range(2.0..=1000.0).speed(0.5).mm());
                if r.drag_started() || r.gained_focus() {
                    self.op_s(r#"{"op":"checkpoint"}"#);
                }
                if r.changed() {
                    self.st.op(&format!(r#"{{"op":"comp_set","id":{},"h":{}}}"#, id, h)).ok();
                    self.st.undo.pop();
                    self.rev += 1;
                }
                ui.end_row();
            }
            ui.label("Angle");
            let r = ui.add(egui::DragValue::new(&mut rot).speed(1.0).suffix("°"));
            if r.drag_started() || r.gained_focus() {
                self.op_s(r#"{"op":"checkpoint"}"#);
            }
            if r.changed() {
                self.st.op(&format!(r#"{{"op":"comp_set","id":{},"rot":{}}}"#, id, rot)).ok();
                self.st.undo.pop();
                self.rev += 1;
            }
            ui.end_row();
        });
        label_caps(ui, "Material");
        if let Some((l, quiet)) = look_editor(ui, c.look, true) {
            match l {
                Some(l) => self.op(format!(r#"{{"op":"comp_set","id":{},"mat":"{}","color":"{}","quiet":{}}}"#, id, l.preset().id, l.hex(), quiet)),
                None => self.op(format!(r#"{{"op":"comp_set","id":{},"mat":"default"}}"#, id)),
            }
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Position  x {:.1}   y {:.1}", c.x, c.y)).size(12.5).color(MUTED));
        ui.horizontal(|ui| {
            if ui.button("Center on dash").on_hover_text("Move to x = 0").clicked() {
                self.op_s(r#"{"op":"checkpoint"}"#);
                self.op(format!(r#"{{"op":"comp_move","id":{},"x":0,"y":{}}}"#, id, c.y));
            }
            if ui.button("Mirror copy").on_hover_text("Duplicate on the other side of the center line").clicked() {
                self.op(format!(
                    r#"{{"op":"comp_add","kind":"{}","shape":"{}","name":{},"x":{},"y":{},"w":{},"h":{},"rot":{}}}"#,
                    c.kind, c.shape, json_str(&format!("{} mirror", c.name)), -c.x, c.y, c.w, c.h, -c.rot
                ));
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Duplicate").clicked() {
                self.op(format!(
                    r#"{{"op":"comp_add","kind":"{}","shape":"{}","name":{},"x":{},"y":{},"w":{},"h":{},"rot":{}}}"#,
                    c.kind, c.shape, json_str(&format!("{} copy", c.name)), c.x + c.w * 0.6 + 6.0, c.y, c.w, c.h, c.rot
                ));
                if let Some(n) = self.st.proj.comps.last() {
                    let nid = n.id;
                    self.select_part(nid);
                }
            }
            if ui.add(egui::Button::new(egui::RichText::new("Delete").color(DANGER))).clicked() {
                self.op(format!(r#"{{"op":"comp_remove","id":{}}}"#, id));
                self.focus = Focus::Dash;
            }
        });
    }

    fn inspect_rail(&mut self, ui: &mut egui::Ui) {
        if self.st.active >= 1 {
            let t = if self.st.active == 1 { "Blue = top guideline, teal = bottom guideline. Every profile starts on the blue line and ends on the teal line." } else { "Blue = the back edge on the windshield line, teal = the front contour. Every profile starts on the blue line and ends on the teal line." };
            self.title(ui, "Guidelines", &format!("{} Drag the squares in the Plan and Height editors.", t));
            if let Some(k) = self.sel_rail2 {
                if let Some(r) = self.st.proj.rail2.get(k) {
                    ui.label(egui::RichText::new(format!("Bottom point {} selected  (y {:.1}, z {:.1})", k, r[0], r[1])).size(12.5).color(TEAL));
                }
            }
        } else {
            self.title(ui, "Windshield line", "The line every profile hangs from. It curves out and up toward the middle, then back down. Drag its square points in the Plan and Height editors.");
        }
        // x / y / z of the selected guideline nodes (the average when several are selected)
        {
            let p = &self.st.proj;
            let mut items: Vec<(u8, usize)> = self.gsel.iter().copied().filter(|i| i.0 <= 1).collect();
            if items.is_empty() {
                if let Some(k) = self.sel_rail { items.push((0, k)); }
                if let Some(k) = self.sel_rail2 { items.push((1, k)); }
            }
            let pts: Vec<[f64; 3]> = items
                .iter()
                .filter_map(|&(kd, k)| match kd {
                    0 => p.rail.get(k).copied(),
                    _ => p.rail2.get(k).and_then(|r| p.rail.get(k).map(|w| [w[0], r[0], r[1]])),
                })
                .collect();
            if !pts.is_empty() {
                let n = pts.len() as f64;
                let av = |d: usize| pts.iter().map(|q| q[d]).sum::<f64>() / n;
                let txt = if pts.len() == 1 {
                    format!("Selected node:  x {:.2}   y {:.2}   z {:.2}", av(0), av(1), av(2))
                } else {
                    format!("{} nodes selected, average:  x {:.2}   y {:.2}   z {:.2}", pts.len(), av(0), av(1), av(2))
                };
                ui.add_space(4.0);
                ui.label(egui::RichText::new(txt).size(13.0).strong().color(TEAL));
            }
        }
        let locked = self.st.active == 2;
        if locked {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("The windshield line is locked to the Top pad. The top trim's front line can be moved in y and z (drag the teal squares), but never below the Top pad's surface.").size(12.5).strong().color(ACCENT));
            ui.add_space(4.0);
            self.front_mode_ui(ui, false);
            ui.add_space(4.0);
        }
        let mut sym = self.st.proj.rail_sym;
        if ui.add_enabled(!locked, egui::Checkbox::new(&mut sym, "Keep both sides mirrored")).changed() {
            self.op(format!(r#"{{"op":"set_rail_sym","v":{}}}"#, sym));
        }
        let n = self.st.proj.rail.len();
        match self.sel_rail {
            Some(k) => {
                let r = self.st.proj.rail[k];
                ui.label(egui::RichText::new(format!("Point {} selected  (x {:.1}, y {:.1}, z {:.1})", k, r[0], r[1], r[2])).size(12.5).color(ACCENT));
                ui.horizontal(|ui| {
                    if ui.add_enabled(k + 1 < n && !locked, egui::Button::new("+ Add point after")).clicked() {
                        self.op(format!(r#"{{"op":"rail_add","k":{}}}"#, k));
                    }
                    if ui.add_enabled(!locked, egui::Button::new("Remove point")).clicked() {
                        self.op(format!(r#"{{"op":"rail_remove","k":{}}}"#, k));
                        self.sel_rail = None;
                    }
                });
            }
            None => {
                ui.label(egui::RichText::new("Click a rail point (square) in the Plan or Height editor to select it.").size(12.5).color(MUTED));
            }
        }
    }
}

pub fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 32 => o.push(' '),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub fn draw_icon(p: &egui::Painter, kind: &str, shape: &str, r: egui::Rect) {
    let c = r.center();
    let rad = r.height() / 2.0;
    match kind {
        "gauge" => {
            p.circle_filled(c, rad, Color32::from_rgb(184, 188, 196));
            p.circle_filled(c, rad * 0.82, Color32::from_rgb(17, 18, 20));
            p.line_segment([c, pos2(c.x + rad * 0.35, c.y - rad * 0.55)], Stroke::new(2.0, Color32::WHITE));
        }
        "vent" => {
            let rr = if shape == "round" { egui::Rect::from_center_size(c, vec2(rad * 2.0, rad * 2.0)) } else { r };
            p.rect_filled(rr, CornerRadius::same(5), Color32::from_rgb(58, 62, 68));
            for i in 0..3 {
                let y = rr.top() + rr.height() * (0.25 + 0.25 * i as f32);
                p.line_segment([pos2(rr.left() + 6.0, y), pos2(rr.right() - 6.0, y)], Stroke::new(3.0, Color32::from_rgb(10, 10, 11)));
            }
        }
        "speaker" => {
            if shape == "round" {
                p.circle_filled(c, rad, Color32::from_rgb(29, 30, 33));
            } else {
                p.rect_filled(r, CornerRadius::same(5), Color32::from_rgb(29, 30, 33));
            }
            for iy in 0..3 {
                for ix in 0..4 {
                    p.circle_filled(pos2(c.x - 13.0 + ix as f32 * 8.6, c.y - 9.0 + iy as f32 * 9.0), 1.7, Color32::BLACK);
                }
            }
        }
        "screen" => {
            p.rect_filled(r, CornerRadius::same(5), Color32::from_rgb(42, 45, 51));
            p.rect_filled(r.shrink(4.0), CornerRadius::same(2), Color32::from_rgb(10, 24, 40));
        }
        "switch" => {
            p.circle_filled(c, rad * 0.7, Color32::from_rgb(141, 146, 153));
            p.circle_filled(c, rad * 0.42, Color32::from_rgb(201, 206, 214));
        }
        _ => {
            p.circle_filled(c, rad * 0.35, Color32::from_rgb(217, 76, 61));
        }
    }
}

pub enum LayerItem {
    Part(u32),
    Cut(u32),
}
pub enum LayerField {
    Vis,
    Lock,
    Export,
}
pub enum LayerAct {
    Fold(usize),
    PieceEye(usize),
    TrimOn(bool),
    WallsEye,
    SelectWalls,
    Deselect,
    SelectPiece(usize),
    SelectPart(usize, u32),
    SelectCut(usize, u32),
    Toggle(usize, LayerItem, LayerField),
    Delete(usize, LayerItem),
}
#[derive(PartialEq)]
pub enum LayerHit {
    None,
    Select,
    Eye,
    Lock,
    Export,
}

/// One row of the layers list: swatch, name, and the lock / export / eye switches at the right.
fn layer_row(ui: &mut egui::Ui, name: &str, dot: Color32, sel: bool, vis: bool, locked: bool, export: Option<bool>) -> (LayerHit, egui::Response) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let p = ui.painter();
    if sel {
        p.rect_filled(rect, CornerRadius::same(6), ACCENT_DIM);
        p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), CARD);
    }
    let cy = rect.center().y;
    p.rect_filled(Rect::from_center_size(pos2(rect.left() + 24.0, cy), vec2(9.0, 9.0)), CornerRadius::same(2), if vis { dot } else { Color32::from_rgb(70, 75, 84) });
    let mut right = rect.right() - 4.0;
    let mut hit = LayerHit::None;
    let ptr = resp.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.interact_pos()));
    // eye
    let er = Rect::from_min_size(pos2(right - 24.0, rect.top() + 1.0), vec2(24.0, 22.0));
    let e = ui.put(er, |ui: &mut egui::Ui| eye(ui, vis)).on_hover_text(if vis { "Hide in the views" } else { "Show in the views" });
    if e.clicked() {
        hit = LayerHit::Eye;
    }
    right -= 24.0;
    // padlock
    let lr = Rect::from_min_size(pos2(right - 22.0, rect.top() + 1.0), vec2(22.0, 22.0));
    let lresp = ui.interact(lr, ui.id().with(("lk", name, rect.min.y as i32)), Sense::click());
    {
        let c = lr.center();
        let col = if locked { ACCENT } else if lresp.hovered() { TEXT } else { Color32::from_rgb(88, 94, 104) };
        let p = ui.painter();
        p.rect_filled(Rect::from_center_size(c + vec2(0.0, 2.5), vec2(10.0, 7.0)), CornerRadius::same(1), col);
        let sh = if locked { c + vec2(0.0, -2.5) } else { c + vec2(3.0, -4.0) };
        p.circle_stroke(sh, 3.0, Stroke::new(1.5, col));
    }
    lresp.clone().on_hover_text(if locked { "Locked: it cannot be dragged. Click to unlock." } else { "Click to lock it in place" });
    if lresp.clicked() {
        hit = LayerHit::Lock;
    }
    right -= 22.0;
    if let Some(ex) = export {
        let xr = Rect::from_min_size(pos2(right - 30.0, rect.top() + 3.0), vec2(28.0, 18.0));
        let xresp = ui.interact(xr, ui.id().with(("ex", name, rect.min.y as i32)), Sense::click());
        let p = ui.painter();
        p.rect_filled(xr, CornerRadius::same(4), if ex { Color32::from_rgb(52, 86, 66) } else { Color32::from_rgb(34, 37, 43) });
        p.rect_stroke(xr, CornerRadius::same(4), Stroke::new(1.0, if ex { TEAL } else { LINE }), StrokeKind::Inside);
        p.text(xr.center(), Align2::CENTER_CENTER, "STL", FontId::proportional(10.0), if ex { Color32::WHITE } else { MUTED });
        xresp.clone().on_hover_text(if ex { "Goes into the exported STL. Click to leave it out." } else { "Left out of the export. Click to include it." });
        if xresp.clicked() {
            hit = LayerHit::Export;
        }
        right -= 32.0;
    }
    let _ = ptr;
    ui.painter().with_clip_rect(Rect::from_min_max(rect.min, pos2(right, rect.max.y))).text(pos2(rect.left() + 34.0, cy), Align2::LEFT_CENTER, name, FontId::proportional(13.0), if vis { TEXT } else { MUTED });
    if hit == LayerHit::None && resp.clicked() {
        hit = LayerHit::Select;
    }
    resp.clone().on_hover_text("Click to select, click again to deselect. Right-click for more.");
    (hit, resp)
}

impl App {
    /// Small gap kept between parts that sit on each other (the trim riding on the top pad).
    pub fn gap_ui(&mut self, ui: &mut egui::Ui) {
        let mut tr = self.st.trim_on;
        if ui.checkbox(&mut tr, "Top trim piece").on_hover_text("Turn the top trim on or off for this dash. Off hides it and leaves it out of the export.").changed() {
            if tr {
                self.st.add_trim();
            }
            self.st.trim_on = tr;
            self.piece_vis[2] = tr;
            self.st.dirty = true;
            self.rev += 1;
        }
        if self.st.trim_on && self.st.trim_custom {
            ui.label(egui::RichText::new("The trim's front line has been moved by hand, so it no longer rebuilds when the Top pad changes.").size(12.5).color(ACCENT));
        }
        if self.st.trim_on && ui.button("Refit trim to top pad").on_hover_text("Rebuild the top trim so it follows the top pad's current windshield line, front line and shape. This also puts a hand-moved front line back to the original, and the trim goes back to rebuilding itself when the pad changes. Its material and any parts on it are kept; hand edits to the trim's own shape are replaced.").clicked() {
            self.op(r#"{"op":"refit_trim"}"#.to_string());
        }
        let (mut on, mut v) = (self.st.gap_on, self.st.gap);
        ui.horizontal(|ui| {
            ui.checkbox(&mut on, "Gap between parts")
                .on_hover_text("Keeps a small air gap where one part sits on another, so printed parts fit together. The top trim follows the top pad with this gap underneath.");
            ui.add_enabled(on, egui::DragValue::new(&mut v).range(0.0..=3.0).speed(0.01).mm());
        });
        if on != self.st.gap_on || (v - self.st.gap).abs() > 1e-9 {
            self.op(format!(r#"{{"op":"set_gap","on":{},"v":{}}}"#, on, v));
        }
        let (mut bon, mut bv, mut bd, mut rd, bf) = (self.st.bevel_on, self.st.bevel, self.st.bevel_down, self.st.bevel_round, self.st.bevel_full);
        ui.checkbox(&mut bon, "Edge on the trim ends")
            .on_hover_text("Chamfer or round the top edge of the flat face at each end of the Top trim piece, from the windshield line to the front line. The pads are left square.");
        if bon {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut rd, false, "Chamfer");
                ui.selectable_value(&mut rd, true, "Fillet");
            });
            // as seen from the front at the driver's end: "In" is how far the cut runs in along the top, "Down" how far it runs down
            // the end face; the cut joins those two points with a straight line (Chamfer) or a quarter round (Fillet)
            ui.horizontal(|ui| {
                ui.label("In").on_hover_text("How far the cut starts in from the end, measured along the top surface. It stops before the two ends would meet.");
                ui.add(egui::DragValue::new(&mut bv).range(0.0..=200.0).speed(0.1).mm());
                ui.add_space(10.0);
                ui.label("Down").on_hover_text("How far the cut goes down the end face from the top. On a thin end it stops half a millimeter short of cutting through.");
                ui.add(egui::DragValue::new(&mut bd).range(0.0..=200.0).speed(0.1).mm());
            });
        }
        if bon != self.st.bevel_on || rd != self.st.bevel_round || (bv - self.st.bevel).abs() > 1e-9 || (bd - self.st.bevel_down).abs() > 1e-9 || bf != self.st.bevel_full {
            self.op(format!(r#"{{"op":"set_bevel","on":{},"v":{},"down":{},"round":{},"full":{}}}"#, bon, bv, bd, rd, bf));
        }
    }
}
