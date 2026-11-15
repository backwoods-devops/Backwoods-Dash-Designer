# Backwoods Dash Designer: project context for Claude Code

Owner: Chris (Backwoods Fab). The program is being published on GitHub under the Apache License 2.0 (see LICENSE, NOTICE, BRANDING.md).
The code, manual and screenshots are public. His artwork, factory scans, trace images, STLs and `.bwd` dash files stay private: never add them to the repo or upload them anywhere.

## What this is
A native desktop dash designer (Rust, eframe/egui 0.37, custom OpenGL 3.3 core renderer) for a 3D-printable
1973 Oldsmobile Cutlass "Super Rocket" dash. It replaces hand-patching a Blender mesh. Units: 1 unit = 1 mm.
Axes: +y toward the windshield, driver side is -x, +z up. Not a Blender add-on (he decided against that).

Run: `cargo run --release` from the repo folder (always --release; debug is too slow for the 3D view).
Data: `~/BackwoodsDashDesigner/project.json` (autosaved), exports in `~/BackwoodsDashDesigner/exports/` (STL, one per piece).
Env overrides: DASHFW_DIR (project dir), DASHFW_MSAA (0 on virtual displays).

## House rules (from Chris, follow them)
- Update THIS folder in place. Never create a new versioned directory (on his Mac: `~/Backwoods-Dash-Designer`, the git repository).
- When pictures with line art are supplied, trace the line art precisely rather than approximating.
- Reference scans/geometry are never edited, joined or cut. Original scan stays untouched.
- Software only: do not drive Blender (see Scope below).
- Don't claim something works on his Mac unless you ran it. Say what is untested.
- He decides when a section is "done" and refines by hand. Don't over-polish; get it close, then let him adjust.
- Plain language in replies; he is a CNC fabricator, comfortable with technical terms, not a Rust developer.

## Model (src/model.rs)
- A `Project` is one dash piece: ordered Bezier `Profile`s (36 points each, same index correspondence, (y,z) pairs) at x stations,
  plus a "rail": a PCHIP curve (x,y,z) through each profile's point 0 (the windshield/back anchor).
  Optional `rail2` (y,z per rail point): the last point of every profile rides on it (bottom guideline / front contour).
- Surface = cubic Hermite loft between profiles (K=6 samples per Bezier segment, M=32 rows per span).
  Solid shell = inward offset along grid normals (default 6 mm), watertight.
- Symmetry: profiles pair by stable ids (`pair`, `sym`); rail points mirror when `rail_sym`. Centre profile pairs with itself.
- Sections = named ranges over profile ids (colour-coded). Keys = named point indices shared by all profiles.
- `Comp` = placed part (gauge, vent, speaker, screen...) positioned by (x,y) on the surface; z and orientation come from the mesh.
  Optional `look` (material) per part.
- `Look` (src/materials.rs) = preset index + colour. 9 presets: vinyl, leather, suede, gloss, matte, brushed, chrome, wood, paint.

## Pieces (src/engine.rs)
Three pieces: 0 Top pad, 1 Lower pad, 2 Top trim. `State.proj` is always the piece being edited; the others sit in `State.shelf`.
`switch_piece` swaps them. Undo snapshots (`Snap`) hold proj + shelf + active, so undo works across pieces.
Defaults: `default_project.json` (top pad, 5 profiles), `default_lower.json` (from Dash_Lo slices), `default_trim.json` (from TOP_PANEL).
Generators: `gen_lower.py`, `gen_trim.py` read `data/*.json` and write `src/default_*.json` (run from the repo root).
Project file format: {"upper":..,"lower":..,"trim":..,"active":n}. A bare single-project file (old versions) loads as the top pad.

## Ops
Everything edits through `State::op(json_text)` -> `apply_op`: checkpoint, undo, redo, reset, switch_piece, set_look,
move_profile, edit_pt, add_after, remove, set_sym, move_rail, move_rail2, rail_add, rail_remove, set_rail_sym, set_thickness,
apply_template, section_set/split/merge/boundary, key_set/add/remove, comp_add/move/set/remove.
Drag edits call `checkpoint` once, then apply live ops (comp_move, edit_pt, move_profile, move_rail(2)) without pushing undo.
`dirty` flag + autosave 600 ms after the pointer is released.

## UI and rendering
- main.rs: App::ui layout and shortcuts. app.rs: App struct, refresh() (rebuilds meshes, materials, part outlines), shared widgets, `look_editor`.
- ui_panels.rs: toolbar, status bar, scene tree, parts library, inspectors. ui_view.rs: viewport input/overlay, bottom dock
  (profile shape editor, plan and height editors). ui_wiz.rs + templates.rs: "New dash..." wizard (top pad only).
- render.rs: camera, CPU ray picking (Moller-Trumbore), GL pass. Fragment shader does the material look (fake studio environment,
  Fresnel, clear coat, procedural grain bump via screen-space derivatives). parts.rs: part catalogue and geometry.
- spacemouse.rs: 3Dconnexion SpaceMouse through the driver's client framework (macOS, loaded at run time). UNTESTED on real hardware.
- egui 0.37 API notes: `Panel::left/right/top/bottom(..).show(ui, ..)`, `eframe::App::ui`, `Frame::new()`, `CornerRadius::same(u8)`,
  `Margin::same(i8)`. The default font lacks arrows/triangles, so use plain text in labels.

## Scope: the software only, not Blender
- Work only on this app's code (this folder). Do NOT drive, script, open or edit Blender, and do not use the Blender MCP.
- Do not read, write or save anything outside this repo folder (no .blend files or other private documents of the owner).
- The shapes the app starts from were already traced from his Blender model and saved as plain data in `data/`
  (lo_outer.json from Dash_Lo, top_trim_slices.json from TOP_PANEL). Use those; if new reference data is needed, ask him for it.
- Terms he uses: "top cover/top trim" = TOP_PANEL, "upper dash pad" = Dash_Up_base, "lower dash" = Dash_Lo,
  "bottom trim plate" is probably the flat wood band (unconfirmed). The windshield contour is the anchor line everything hangs from.
- The default vinyl look is matched to his Blender Vinyl_Black (near-black, roughness 0.36, light clear coat, fine grain).

## Known gaps / next work (ask him which first)
1. Scan reference tools: load the factory scan as an underlay he positions himself, mark the windshield line (3D curve with a curl)
   and named anchor points by hand; the scan is noisy so no auto-tracing. Original scan never edited.
2. Possible 4th piece: TRIM_WOOD_BAND (confirm it is what he calls the bottom trim plate).
3. Top trim details: defroster vent openings, SuperRocket logo pocket (SR_ALU_01..10), speaker holes, 6 mm wall rules.
4. Parts cut into STL (holes, inset floors, vents), parts on lower pad/trim, front-surface placement.
5. Verify on his Mac: SpaceMouse (axis mapping and directions), drag-preview footprint, material look vs Blender.
6. Wizard: add steps for lower pad and trim.
7. Section/crease rendering and a leather stitching option are ideas only.

## Working notes
- Cargo needs network to crates.io; everything else is local. `cargo run --release` rebuilds only changed code.
- Test the UI by running it; screenshots help. Report what you saw, not what you expect.

## Added since the notes above (Nov 2026)
- Files: project extension is `.bwd` (`file.rs` EXT / OLD_EXT; `.dashfw` from earlier versions still opens). Dialogs: Mac osascript, Windows PowerShell, Linux zenity.
- Shapes (`shapes.rs`, `cuts.rs`, `ui_shapes.rs`): a `Shape` has `parts: Vec<Part>` (added pieces and holes, positions relative to the main outline's center).
  `Shape::region()` / `Cut::region()` give a `Region` (outer loop + holes + added loops); `region_prism` / `region_solid` / `prism_layers` extrude it
  (boolmesh booleans, thread_local solid cache). Editor drag modes `EdDrag::Part`, `EdDrag::PartSize`. Pen helpers (angle lock, guides, angles) are draw-mode only.
- Shape editor lock: `leave_click` / `leave_window` ask Save / Discard / Cancel when the Scene tab or top-row buttons are clicked; skipped if drawing has not started.
- SpaceMouse: `spacemouse.rs` drives the shape editor's 3D view (`sm_drive`) when open, otherwise the main camera.
- UI: dark theme forced on all OSes (`theme.rs`). Lighting is a top-row pill and a centered popup. Wizard has an orange NEW DASH SETUP title.
- Icon: `assets/` (SVG source, 256 px PNG embedded by `main.rs::app_icon`, `.icns` used by `make_app.sh`, `.ico` embedded on Windows by `build.rs` via winresource).
- Windows: `build_windows_on_mac.sh`, `build_windows.bat`, `.github/workflows/windows.yml`. Unverified on real hardware (file dialogs likeliest to need fixes).
- Docs: the manual is `src/help.rs`; its pictures are `src/help_img/*.png`, made by `docs/make_shots.sh` (runs the app with `DASHFW_DOCSHOT=<scene>`, see `src/docshot.rs`;
  needs Linux with xvfb + ImageMagick) then `docs/make_help_images.py` (crops). Re-run both after any UI change.
- Cloud-only scaffolding `probe_demo.rs` and `gen_demo.rs` must never be shipped; the Mac `main.rs` has no `mod` lines for them. Patch Mac `main.rs` by hand, never overwrite it.
- Files copied to the Mac lose their executable bit: `chmod +x make_app.sh build_windows_on_mac.sh` if a script says permission denied.

