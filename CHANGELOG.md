# Changelog

What changed in each version of Backwoods Dash Designer, newest at the bottom. (Version numbers here are feature milestones; `Cargo.toml` carries the build version.)

## v0.4 additions
- **Top pad / Lower pad** buttons in the toolbar switch which piece you edit. The other piece stays visible in grey. Undo/redo works across both.
- **Lower pad** starts from a built-in lower pad shape. It has a **top guideline** (blue) and a **bottom guideline** (teal); every profile starts on blue and ends on teal. Drag both in the Guidelines: plan / height editors. Profiles still slide along them.
- **Center line** (x = 0) is drawn dashed in 3D and in the plan/height editors. Parts snap to it; the part inspector has *Center on dash* and *Mirror copy*.
- **Top cover** (optional): Dash overview > "Add a top cover" with gap and thickness. Shown translucent, exported as its own STL.
- **Export STL** now writes top pad, lower pad and (if on) the cover.
- Parts always show an outline on the dash, and a library drag shows an orange footprint under the cursor.
- Project file is now `~/BackwoodsDashDesigner/project.json` with `upper`, `lower`, `active`, `cover` (old single-piece files still load as the top pad).

## v0.5: materials
- Dash overview > **Top pad material / Lower pad material / Cover material**: Vinyl, Leather, Suede, Gloss plastic, Matte plastic, Brushed metal, Chrome, Spalted maple, Gloss paint, each with color swatches and a free color picker.
- Placed parts have a **Material** section ("As drawn" keeps the catalogue look).
- The **Materials / Design colors** button above the 3D view switches between the real look and the editing colors. Default vinyl is near-black with a light clear coat and fine grain that shows up when you zoom in close.
- Materials are saved in `project.json` and are display only; STL export is unchanged.

## v0.6: SpaceMouse
- Uses the 3Dconnexion driver (3DxWare must be installed). Status button bottom right of the window: click it for sensitivity and a flip-direction switch.
- Tilt/twist orbit, push sideways and lift to pan, push forward/back to zoom. Left button = home view, right button = top view.

## v0.7: top trim is its own piece
- The "Add a top cover" option is gone. **Top trim** is a third piece next to **Top pad** and **Lower pad** (button in the toolbar). It has its own profiles, its own sections, a back edge on the windshield line (blue) and a front contour (teal), just like the lower pad has top and bottom guidelines.
- Its starting shape is a built-in top panel (top surface only; holes are filled, to be cut later).
- Export STL writes all three pieces. Each piece has its own material.

## v0.8: add and remove curve points
- Profile shape editor: **+ Point** adds a point after the selected one, **- Point** removes the selected one, or double-click the curve to add a point where you click. Changes apply to every profile in that piece so they stay matched. The two end points can't be removed; minimum 4 points.

## v0.9
- **Re-space all points**: button in the profile editor header and in the profile inspector. Pick a new point count; every profile of the piece is refit to it, spread evenly along the curve (optionally with extra points on tight bends). Sharp corners and key points are kept, shape stays within about 1.5 mm even at 12 points. Undo restores the old points.
- Individual + Point / - Point still leave every other point exactly where it is.
- Fresh pieces (and Reset) now start with 6 points per contour line (`DEFAULT_POINTS` in engine.rs).
- Top pad now has an editable Front contour guideline (seeded through the profile tips, also added to older saved projects).
- Profile editor: Shift/Cmd-click toggles points, Shift-drag a box selects several, dragging any selected point moves them all together (one undo step).
- Viewport **Wall** button: shows the exact printed shell (the same mesh Export STL writes) at the wall thickness from Printing. **Cut** adds a section plane; the orange face is the wall in cross-section, so you can read the thickness directly.
- Export STL now opens a Save STL popup: name each piece and save it on its own (exports/<name>.stl, replaces a file of the same name).

## v0.10 - Shapes
- New **Shapes** tab (left, next to Parts library): 12 basic shapes (rectangle, square, rounded rect, slot, circle, ellipse, triangle, diamond, pentagon, hexagon, octagon, star, cross), **My shapes**, and **Draw a closed loop** (click vertices, click the first one to close).
- Click any shape to open a copy in the **Shape editor**: drag the 8 blue handles to stretch (Alt = from center), numeric width/height, **Aspect ratio locked/free** toggle, scale 50/90/110/200 %, corner radius, sides, star point depth, cross arm thickness, rotate, snap to grid. **Edit points** (or type Free loop) turns any shape into a loop whose vertices can be dragged, added (double-click an edge) and deleted.
- Saved shapes live in ~/BackwoodsDashDesigner/shapes.json. Using shapes to cut / imprint / pull out sections of a piece comes next.
- Free loops have a **Contour**: round all corners at once, or click a vertex and round just that corner (mm radius). Sharp resets.
- Free loops: right-click a vertex for Smooth curve (Bezier handles) / Sharp corner / Round corner / Add / Delete; right-click an edge to add a vertex; All smooth / All sharp. Drag the handles to shape the curve (Alt = one side only). The editor window no longer jumps when its panel changes height.

## v0.11 - Trace over an image
- **Load trace image...** (a pill in the dock header, right after the three editor tabs) opens a window for the profile editor, the windshield-line plan view and the height view (one image each).
- Choose an image (system file dialog, or drop a file on the window; png / jpg). 1: **Mark points A and B** by clicking on the image. 2: enter their real distance in mm and **Apply scale** (A stays put). 3: **Level A-B** (rotate so the line is horizontal / vertical), **A to origin**, turn, mirror, position, or drag the image around. Opacity and Show toggle it.
- Only the file's location and placement are stored (~/BackwoodsDashDesigner/refs.json); the image itself is never copied or uploaded. Fit includes the image. Drag the top edge of the dock up to get a bigger tracing area.

## v0.12 - Extrude
Shape editor: Extrude (3D) section - depth, wall lean (draft), top edge chamfer/round. The solid draws in the editor canvas (oblique view, base stays editable), panel scrolls, "Save STL" (exports/shape_<name>.stl). Saved with the shape.
The shape editor now fills the main viewport (canvas) with its properties in the right panel; Done closes it.

## v0.13 - Shape cuts (drag and drop onto the dash)
Open the **Shapes** tab and drag a saved shape onto the dash (or press **Place**). Then pick what it does in the right panel:
- **Cut through** - opens the shape straight through the wall (vents, speaker holes).
- **Recess** - sets the shape in by a depth you choose.
- **Raise** - pulls the shape out of the surface.
- **Inlay** - cuts the shape and leaves a separate plug filling it (exported as its own STL).
Depth, wall bevel (0 = straight), size, turn and 'follow the surface' are in the panel. Drag a placed cut to move it. The **Cuts** pill in the viewport shows or hides the result. Cuts are saved with the project and applied on Export STL.

v0.13b: Recess / Raise / Inlay have **Keep the wall thickness** (on by default for new cuts): the inside of the wall moves with the surface so the wall stays as thick however deep the recess. The shape editor's Place / Apply button is pinned so it is always visible.

v0.14: 3D parts. An extruded shape or an imported STL (Shapes tab > Import STL...) is a solid part with two operations: **Cut** (removes the part's volume from the dash; "Keep a wall around the cut" leaves a wall of the dash thickness around it, off gives a plain hole) and **Add**. Imported STLs are copied to ~/BackwoodsDashDesigner/meshes. Use Turn part (X 90 / Y 90 / Flip) if the file is not Z-up. Resize with Width / Length / Part height; "Sunk into dash" sets how far it goes in.

v0.14b: every piece (active and the two on the shelf) is drawn as a shell with its wall thickness in the main view.

v0.14c: Import... takes STL (3D part, 3D icon at any size) and DXF / AI / EPS (flat outline shapes; each closed path becomes a shape, biggest first).

v0.14d: 3D parts are drawn as the actual object (teal = add, red = cut) whenever the cut result is not on screen (Design colors view, or while it is still calculating).

v0.14e: STL import repairs doubled faces, mixed winding and shells that touch along an edge (the 'edge shared by more than two faces' error), and warns at import if the part is still not a clean solid.

v0.15: 3D parts are free objects: X / Y / Z position of the center, rotation about X, Y and Z in degrees (with -90 / +90 buttons), size, Add or Cut by where they sit against the dash, Sit on surface / Sink flush helpers. Every shape and part has a Symmetrical toggle (mirror image across x = 0). STL import also closes open holes.

v0.15b: the app is now called Backwoods Dash Designer (window title, header and docs).
v0.15c: right-click a 3D part for a menu (Move/Rotate handles, turn 90 per axis, Reset rotation, Add/Cut, Sit on surface, Sink flush, Center on dash, Symmetrical, Delete). A selected 3D part shows move arrows (X red, Y green, Z blue) to drag along each axis, or rotate rings to turn it about each axis.
v0.15d: a 3D part that does not overlap the pad now says so in the cut message ("cuts nothing") instead of silently doing nothing.
v0.15e: the "keep a wall around the cut" step no longer fails on detailed STLs (boolean inputs that have non-manifold edges are repaired first).
v0.15f: rotating or stacking several 3D parts (with walls) no longer fails with "edge shared by more than two faces"; the cut engine now chains its own results instead of rebuilding them.
v0.15g: every error is also written to errors.log in the data folder (~/BackwoodsDashDesigner/errors.log) with the position, rotation and size of each part; a newly dropped part starts with the move arrows.
v0.15h: if the geometry engine crashes on a shape mid-drag, the cut is skipped with a message (and logged) instead of leaving "Working out the cuts..." on screen forever.
v0.15i: Lighting button in the viewport bar (exposure, main/fill/ambient light, reflections, texture strength, light direction, fixed or camera-following lights; saved in lighting.json). Leather now has a visible pebble grain and spalted maple is less washed out.
v0.15j: cuts wait until you release the mouse (no more re-cutting on every drag step, which was triggering the engine crash); Lock switch on every placed part (side panel, right-click menu, shown in the parts list).
v0.15m: SpaceMouse axes and buttons are read correctly, so the puck now moves the view.
v0.15n: SpaceMouse keeps working after you switch away from the app and back.
v0.15y: side menus fold into a thin strip of vertical tabs (Blender style): drag a menu narrow, or use the Hide buttons / [ ] / Tab. Click a tab to open it again. My shapes are now thumbnails like the basic shapes; drag or double-click to place, right-click for Place, Edit, Copy, Turn, Delete.
v0.15z: side menus fold away with a small arrow tab at the top corner of the viewport (click it to fold; click the tab left behind to pull the menu back out). The bottom-bar buttons are gone.
v0.16: SpaceMouse left/right pan direction reversed.
v0.16a: Side walls. A wall at each end of the dash, bounded by the back curve and the front profiles of the top and lower pads, extruded inward by the wall thickness. "Side walls" pill in the viewport bar opens a side-view editor for the back curve (drag points and handles, add or remove points, reset to the traced curve) and an Export walls button (side_wall_driver.stl, side_wall_passenger.stl in the exports folder). Saved in sidewall.json.
v0.16b: the side wall back-curve editor is now a tab (Side walls) in the bottom panel next to Profile shape and the guideline editors, with its buttons in that header. The Side walls pill in the viewport bar now just shows or hides the walls (and opens the tab).
v0.16c: Lock heights (Side walls tab): the lower pad top height follows the top pad front lip height, z only, so the two ends meet and the side profile joins cleanly. On by default, saved in project.json as lock_z.
v0.16d: a Side walls button in the top bar (next to Top trim) opens the wall curve editor. Clicking a part, shape or profile in the main view or the scene list flips the bottom panel to the Profile shape editor on the profile nearest to it.
v0.16e: the top-bar Side walls button now toggles: second click hides the walls and returns the bottom panel to Profile shape.
v0.16f: clicking a piece of the dash in the 3D view (top pad, lower pad, top trim) switches to that piece and flips the bottom panel to its profile at the click spot. The camera stays where it is.
v0.16g: clicking a side wall in the 3D view opens the Side walls tab
v0.16h: Side walls is a main piece in the top row (always there, no on/off) with its own material and color in the right menu; piece buttons always leave the Side walls tab
v0.16i: selecting Side walls makes them the focus in the 3D view (other pieces dim, no profile lines)
v0.16j: selecting a placed part opens a Part editor in the bottom panel: top view with mouse handles to move, size (width, height, scale) and turn it
v0.17a: Layers panel at the bottom of the right menu (pieces, parts, shapes with hide, lock, STL export switch); drag a part onto another piece to move it there; Parts and Shape cuts lists moved out of the Scene tab
v0.17b: Layers panel is a rounded card with a gap above it (single clean separator)
v0.17c: Layers is a full-width band across the bottom of the right menu with a bar above it
v0.17d: Layers: click a selected row or empty space to deselect; hover hints on every control
v0.17e: top pad editor tabs renamed Guidelines: plan / height; second line named Front line
v0.17f: panel renamed Pieces & parts; Mirror checkbox in the bottom editor toolbar (profiles and guidelines)
v0.17g: the teal mirror guide and the 'mirrored' tag show only while a profile pair is actually linked
v0.17j: profiles slide freely along the dash; their ends stay pinned to the windshield and front contours
v0.17k: clicking the body of a piece selects the piece; only a click on a profile line or handle selects that profile
v0.17l: a mirrored profile copy is re-pinned to the windshield and front lines, so uneven guidelines no longer leave a gap at the front end
v0.17m: '+ Profile...' dialog (bottom editor toolbar and right menu): symmetrical or one side, and exactly where across the dash
v0.17n: Add a profile: 'Click on a guide to place it' - click the windshield line or the front line in the 3D view or the Guidelines editor, with a live preview
v0.17o: Top trim now rides on the top pad: its underside follows the pad's outer surface with a 0.3 mm gap (Gap between parts, toggle + size in Printing / export), and its top rises where needed to keep the full wall thickness. Edges stay straight; away from the pad it uses the normal shell.
v0.17p: The two ends of every piece (first/last profile) are now cut square, a flat 90 degree face, instead of following the surface angle.
v0.17q: Bevel end faces option (Printing: checkbox + depth mm) chamfers the top edge of each piece's end faces. Bottom editor header is now two wrapping rows (tabs + Fit/Hide, then tools) with no scrolling; the hint text moved onto the canvas bottom-left.
v0.17r: The Top trim's skin in the editor now has side walls that drop to the top pad's surface (plus the gap), so it no longer stops short of the pad.
v0.17s: Fixed white showing on the top trim: its shell now lists the outer skin triangles first (as the viewer expects) so the leather covers the whole outside; the editor skirt is drawn with the same material.
v0.17t: Trim edge walls now shade as real walls (own face normals) instead of borrowing the top surface's shading, which showed as a white strip; cut engine welds coincident vertices.
v0.17u: Bevel depth can go well past one row of the surface (up to 200 mm). It slopes down from the end face and stops where it would meet the other end (never more than 45 percent of the length) or the inside face of the wall.
v0.17v: End-face edge options: Chamfer or Fillet, with separate reach down the end face (A) and in along the top toward the center (B). Old Bevel setting becomes B; A defaults to the same value.
v0.17w: Editor folds away with an arrow tab on its top edge (like the side menus). Trace image reworked: pick or drop an image, Trace image on/off checkbox, Move / size mode (drag to move, drag a corner square to resize), see-through slider, Change / Remove. No calibration.
v0.17x: End-face edge: 'all the way' (on by default) sweeps the chamfer/fillet down the whole end face to the pad / underside, leaving no flat edge below the curve.
v0.17y: Guidelines plan/height editors: Shift-click or Shift-drag a box to select several windshield points, front points and profile circles; dragging one moves them all by the same amount (mirror partners are not double-moved).
v0.17z: Side walls follow the top pad's real end: the wall's top corner and back curve start where the pad's (rounded/chamfered) end face begins, and its top edge never stands above the pad surface. The Top trim is not involved.
v0.18a: The end-face edge (chamfer / fillet, down and in sizes) now applies only to the Top trim piece; the upper and lower pads keep square ends, and the side walls start at the pad's plain corner again.
v0.18b: Side walls: the back curve starts one wall-thickness (6 mm by default) below the top surface at the windshield edge, so the wall has thickness all the way instead of a spike. One start point, 6 mm down.
v0.18c: Side walls: removed the pad-surface height clamp unless the top pad's own end has an edge treatment (it is trim-only now), which caused small steps around the front lip corner.
v0.18d: Side walls: the wall's inner face edge now follows the top pad's and lower pad's real surface 6 mm in from the end (the loft changes shape slightly along x), so the wall matches the pad's side profile around the front.
v0.18e: Side walls use the top pad's material and color by default ('Same material and color as the top pad' checkbox on the Side walls panel); untick it for a separate look.
v0.18f: The side wall plates and the pad/trim edge bands are now shaded like the leather on top (screen only; printed files unchanged), so there is no bluish or pale band next to the end pieces.
v0.18g: Draw shape curve points can now have their tangent broken (like the Illustrator anchor point tool): each handle moves on its own so a point can turn a sharp corner. Use the Handles button in the panel, the right-click menu, or Alt-drag a handle; click again to join.
v0.18h: The wood material is now Oak, made from a reference photo: arched growth rings, dark pore dashes and pale streaks, with slow noise so the grain never repeats. Pick a wood tone from the swatches or the color box. (Projects that used the old wood material keep the same slot and now show oak.)
v0.19a: Sizing. A light size read-out (W / D / H in mm) sits in the top right of the main view. Right-click any piece of the dash for a Size menu: whole dash or just that piece, type a size for width, depth or height, scale everything evenly by a percent, and turn on Drag bars. Drag bars show a box around the dash with a grab bar on each side; drag one to stretch that way (Shift = scale all three together) with a label at the cursor showing the new size and the original. Ctrl+Z undoes a resize.
v0.19b: Sizing handles are now corner brackets (a quarter of each edge long) instead of a full box with square bars. Hover a bracket and it goes bold; pull it along its edge to stretch that way (Shift = all three axes).
v0.19c: Corner handles: a grab near a corner now takes the long edge (width / depth) instead of the short height stub, and hovering a bracket names the axis it will change.
v0.19d: The size read-out moved to the top left. With corner handles on, a Blender-style axis tool sits in the top right: drag the X, Y or Z ball up to grow that size, down to shrink it (the dim ball on the other end grows it from the other side; Shift = all three). The corner brackets still work too.
v0.19e: Corner handles work by direction: grab any corner bracket and pull; the way you pull picks the axis (along the width edge = width, along the depth edge = depth, along the height edge = height). Hovering a corner makes its three brackets bold.
v0.19f: The size read-out is centered above the viewport buttons (it was hidden behind the fold tab).
v0.19g: Right-click Size menu has Reset to original size (whole dash or the one piece, as chosen). The original sizes are remembered in size_orig.txt in your data folder the first time the menu opens, so it still works after a restart. Delete that file to take a new "original".
v0.19h: New dash style "Squarebody truck" (classic full-size truck dash): 1450 mm wide, deep nearly-flat top with a slight bow, plain front, blank shell with no top trim. The wizard now treats the top trim as optional: styles can start without it, the Pieces list shows an Add trim button for it, and a Top trim piece checkbox (next to the gap setting) turns it on or off. A trim that is off is hidden, left out of the size box and shows "not added" in Save STL.
v0.19i: Squarebody truck is no longer symmetric: the driver side is 38 mm deeper with a longer lip, and a short step just past the center (x = +70) brings it back to the shallower passenger side. It has 7 profiles and the left and right no longer mirror each other (Mirror is off), so you can shape each side on its own.
v0.19j: New dash cards show a small shaded 3D picture of each style (seen from the front and a little above) instead of the profile line.
v0.19k: The New dash style picker now covers most of the window with the style cards in a roomy grid and bigger pictures. The later steps (size, windshield line and so on) stay as the side panel so you can watch the dash change.
v0.19l: Every step of the New dash flow now fills most of the window. Steps after the first show a live 3D picture of the dash on the left ("Your dash so far") and the controls on the right.
v0.19m: The "Your dash so far" picture in the New dash steps can be moved: drag to turn it, scroll to zoom, Shift-drag to slide it, double-click to reset, so you can look at your custom build from any side.
v0.19n: The New dash picture keeps one fixed scale, so changing the width no longer makes the whole dash look like it shrinks or grows on every axis.
v0.19o: On the Sections step of New dash the 3D picture is colored by section, matching the color boxes in the list, so you can see which part of the dash each section is. Change a color or name and the picture follows.
v0.19p: The New dash flow is now six steps: the Parts step is gone, so it covers just the overall shell (style, size, windshield line, front profile, sections, finish). Add parts and details afterwards.
v0.19q: Adding the top trim to a dash that started without one (the Squarebody style) now builds a trim that follows the new top pad: same windshield line and front profile, sitting 5.5 mm above it and covering the top run, instead of the stock trim. Your trim material is kept. A trim you already have is never rebuilt.
v0.19r: The "New dash…" button now stays in the top bar whatever piece is selected. The added top trim stays where it was (over the top run of the pad) but is now thin at the windshield (3.5 mm), thicker toward the front (11 mm) and ends in a rounded nose. New-dash pads and the trim use only 8 anchor points per profile (was 36). The trim underside only rides on the pad over the flat run; the nose keeps an even thickness.
v0.19s: "Reset to original profiles" now returns each piece to what the New dash wizard made (including a trim added afterwards), not the built-in defaults. Dashes that never used the wizard still reset to the built-in defaults.
v0.20: Project files. A new File menu (Open, Open recent, Save, Save as, plus Cmd+O / S / Shift+S). The existing New dash… button is the "new" command: it asks to save first, starts a fresh untitled dash and opens the wizard. Dashes are saved as single files that hold the whole dash: pieces, shapes, side walls, imported meshes, sizing and lighting. The app now starts on a fresh default dash, shows the file name in the top bar and window title (a dot means unsaved changes) and asks to save before New dash, Open or Quit. Whatever you had open last time is kept automatically in BackwoodsDashDesigner/autosave (newest five) and File > Open last session reopens it. Reference photos stay as links to the image files on your disk.
v0.20a: In the New dash wizard the center crease now makes the FRONT line a W like the default dash: each side a hump forward, the center dipping back to a point. The windshield line stays one U. Turn it off for a full U front from end to end (the W needs 5 or more profiles).
v0.20b: Your shapes library (My shapes, imported shapes and meshes) is shared by every dash again: it is no longer wiped when the app starts or you start/open a dash. A .dashfw file carries the shapes it uses and adds any you do not already have. If your shapes were lost by v0.20, they are restored from the last-session autosave on the next launch.
v0.20c: Custom loop editor: no more "Place on dash" button (it closed the editor). A new loop starts with a blank name and needs one before it can be saved to My shapes; place it from the My shapes list afterwards.
v0.20d: Custom loop editor: Done now saves a new loop to My shapes (name required, the editor stays open with a message if it has none) instead of dropping it. A new Discard button closes without keeping anything.
v0.20e: Custom loop: a Name box now sits at the top of the editor panel for a new loop, and pressing Done without a name shows the reminder there and as a pop-up instead of silently doing nothing.
v0.20f: Pressing Done (or Save to my shapes) on a new custom loop with no name now pops up a "Name this shape" box; Save puts it in My shapes and closes the editor, Cancel returns to editing.
v0.21: Shape editor (draw a closed loop and edit) gets drawing tools. Rulers in mm along the top and left that follow zoom and pan, a live pointer read-out, a Measure tool (click two points for distance, dx, dy), the length of the segment you are drawing, and Zoom -, +, Fit buttons (scroll still zooms; drag the empty canvas to pan, also while drawing). A new loop first asks for a canvas size (presets or your own numbers) and shows that area outlined. Trace an image: Load image, Mark two points and enter their real distance to set the size, Level, Move image, opacity, rotate and flip. The image stays on your computer and is not saved with the shape.
v0.21a: Right-click a shape in My shapes and choose Rename… to give it a new name (type it in the pop-up; Enter or Rename saves, Esc cancels).
v0.21b: In the New dash wizard the top trim now follows the changes you make (windshield bow, depth, width, front shape): its windshield and front guide lines are rebuilt from the new top pad. A style you have not changed keeps the stock trim.
v0.21c: The Save STL window now lists the two side walls (driver end and passenger end) under the three pieces, each with its own name box and Save button. The Walls tab Export walls button still writes both with the default names.
v0.21d: The built-in parts library (the basic gauge, vent and switch pieces) is removed. The left tab that held your shapes is now called "Parts & shapes": it is where imported 3D parts (STL), imported outlines (DXF, AI, EPS) and your drawn or saved shapes live. Dashes that already used the old basic parts still open and keep them.
v0.21e: "My shapes" is now "My library" and the Save to my shapes button is "Save to my library", to match the Parts & shapes tab.
v0.22: The app no longer opens straight onto a dash. A start-up pop-up asks you to Create a new dash (opens the wizard) or Open an existing dash, and lists your recent files. The dash stays hidden until you choose.
v0.22a: The start-up pop-up now offers Open last (your newest saved file, or the last autosaved session; greyed out when there is nothing yet), Open existing dash… (file picker) and Create a new dash (wizard). With nothing saved before, only the last two are usable and the view behind the pop-up is empty.
v0.22b: Behind the start-up pop-up the app is now completely blank (no dash, profile editor or panels) until you choose, like a first run. Nothing built in is removed: the default dash is still there for Reset and the wizard.
v0.22c: Wizard Sections step: click a colored section in the picture and the matching name box on the right takes the keyboard focus with its text selected, ready to type over. The pointer turns into a hand over the sections.
v0.23: New start-up screen: a full-window welcome with the Backwoods Dash Designer mark and three large cards side by side with icons (Open last, Open existing, Create new), hover highlight, and a Recent list underneath.
v0.23a: Wizard last step now shows the whole dash (top pad, lower pad and top trim if on) and reads "Your dash shell is ready" with plain next steps.
v0.23b: Wizard 3D picture: patches are drawn as plain triangles (no outline), which removes the long spikes; all pieces now use the same grey as the earlier steps.
v0.23c: Wizard 3D picture is much finer (56 rows, 10 columns per span) and smooth between profiles (crease-preserving cubic), drawn as one mesh.
v0.23d: Lower pad top edge now locks onto the top pad front line in position as well as height, so it follows what the wizard sets (was off by 30-145 mm front to back).
v0.23e: Wizard windshield step: "Center depth" slider for the dip in the front line, and the checkbox is reworded (no longer says W).
v0.24: Tracing image: "Move / resize" shows corner handles (drag the image to move, drag a corner to resize), number boxes for center and width/height, and a "Keep proportions" tick (off = stretch width and height separately). Saved with the file.
v0.24a: Start-up screen shows the Backwoods Fabrications logo (built into the program) and a 3D picture of a dash in place of the profile icon.
v0.25: Help button (top bar, also on the start screen) and F1 open a searchable manual: 16 topics covering the whole program.
v0.25a: The manual now has 19 pictures (start screen, wizard, layout, editors, export and more). Fixed two messages overlapping at the top of the drawing canvas.
v0.26: Units: metric (millimeters) by default, with an mm / in switch on the start screen (bottom-left) and an mm / in button in the top bar. Every size read-out, number box, wizard slider, ruler, grid and measure follows it; typing 2in or 30mm works in any box. STL files are always written in millimeters. The choice is kept in units.txt. Help index is now left-aligned; spelling is "millimeters".
Sharp edge per profile: every profile except the two ends has a *Sharp edge* checkbox (in the profile inspector and the bottom editor toolbar). Off, the surface flows smoothly through the profile; on, it meets it with a sharp edge. A mirrored partner follows, and it works on all three pieces.
Top trim: the front corner is closed off with a straight line where the hook is too tight for the full thickness; where the shell ends up more than 1 mm thinner than its set thickness it is tinted red in the 3D view. A front-guide point pressed down onto the Top pad keeps that height when pulled back out in y.
Wizard step 3, front line across the middle: Straight, Center in / out (the middle of the front line comes further out, or further in with a negative amount, in one smooth curve; amount in mm) or Pocketed (the W style; amount is the center depth). Pocketed is the default for the twin-pad wrap. Spelling in the program and manual is now American (center, color).

## v0.27 - Tidy-up and file type
- The Save STL window is now **Export STL(s)** (button and window): settings grouped at the top, one row per piece and side wall, no wasted space, nothing clipped under the view bar.
- Dash files end in **.bwd** (was .dashfw). Old .dashfw files still open from File > Open, the recent list and Open last; saving writes .bwd. File menu text cleaned up (no odd characters; plain-word shortcut hints).

## v0.28 - Compound shapes (pieces and holes)
- Shape editor, Combine shapes card: **Add a piece...** joins a circle, rectangle, polygon, star or cross to the drawing; **Cut a hole...** removes one. Click a piece to type its position, size and turn, or drag it, its square size handles and its diamond turn handle on the canvas. Saved with the shape (`parts` in shapes.json and .bwd files).
- Compound shapes cut, extrude (layers, draft, bevel) and export as one solid using the boolean engine; a hole in a Raise / Inlay shape leaves an island. Holes are straight through.
- **Rotate in 3D** in the shape editor (after the shape has a depth) turns the solid around in the canvas; the button sits inside the canvas corner.

## v0.29 - Pen helpers and SpaceMouse in the shape editor
- Drawing a loop: **Pen helpers** card with Angle lock (step 1-90 degrees, Shift flips it for one line), pink alignment **Guides** that snap to other points, and corner **Angles** (green at 45 / 90 / 135 / 180). Draw mode only; edited points still move freely.
- The SpaceMouse drives the shape editor's 3D view (tilt/twist tumble, push/lift pan, forward/back zoom; buttons 1 and 2 = front and top views).

## v0.30 - Editor lock, Lighting, wizard title
- While the shape editor is open, clicking the Scene tab or any top-row button asks **Save and continue / Discard / Cancel** first. If drawing has not started there is nothing to lose and the editor just closes.
- **Lighting** is a top-row button (next to Help) and opens a centered window with Close, like the other pop-ups.
- The wizard shows an orange **NEW DASH SETUP** title at the top right.

## v0.31 - Same look everywhere, icon, Windows builds
- The program always uses the dark theme on Mac, Windows and Linux, so contrast and controls match on every OS (a Windows machine in light mode used to give pale buttons and dim labels).
- App icon: chrome BWF with a small dash and pine tree in the Backwoods orange glow. Used as the window and taskbar icon on every OS, the Mac .app icon (`make_app.sh`) and, with `build.rs`, the Windows .exe icon (needs mingw `windres`; otherwise the build only warns). Sources in `assets/`.
- Windows: `build_windows_on_mac.sh` cross-builds a test .exe from a Mac (target x86_64-pc-windows-gnu, mingw-w64); `build_windows.bat` builds on Windows; `.github/workflows/release.yml` builds it in CI. Unsigned: expect SmartScreen's unknown-publisher warning.
- Manual and screenshots brought up to date (F1).
- The shape editor no longer has a Place on dash button: it only makes shapes. Name it, press Save to my library, then drag the thumbnail from My library onto the dash (the Apply to the dash button for editing an already placed cut-out stays).
- Profile editor key-point labels (Windshield line, Front arc, Lip tip) now alternate sides, so Front arc sits to the left of its point and never overlaps Lip tip.
- Start-screen logo is now the Backwoods Fabrications logo with the raccoon mascot climbing over the "oo" (transparent background, 1400 px wide; the start screen spacing allows for its extra height).
- The X / Y / Z balls in the top right of the 3D view are now a Blender-style view gizmo that is always on: drag it to turn the view, click a ball to look along that axis (faded ball = from the opposite side). It no longer resizes the dash; resizing is by the corner brackets and the right-click Size menu.

## v0.32 (first public release)
- Released under the Apache License 2.0; the program is now called **bw-dash-designer**.
- Start screen: a "made by" box with PayPal tip, YouTube, website, Facebook and **Our story** buttons. The thank-you note opens at every launch and again from the Our story button. The start screen scrolls when the window is short.
- Links open in the default browser on Mac, Windows and Linux.
- New public README with screenshots, GitHub Actions for tests and for Windows, Mac and Linux downloads, and third-party license notes.
- Exported STL files are named `bwdash_...`. Cleaner build (debug switches removed, unused code warnings fixed).
- The working folder is now `~/BackwoodsDashDesigner` (an existing `~/DashFW` folder is renamed on first start, nothing is lost). Set `BW_DASH_DIR` to use another folder; the older `DASHFW_DIR` still works.
