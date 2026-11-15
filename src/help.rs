// The built-in help manual: a searchable window with a topic list on the left and the text on the right.
// Press F1 or the Help button at any time (also on the start screen).
use crate::app::*;
use crate::theme::*;
use eframe::egui::{self, vec2, Align2, CornerRadius, RichText, Stroke};

/// One block of manual text.
pub enum B {
    /// sub-heading
    H(&'static str),
    /// paragraph
    P(&'static str),
    /// bullet list
    L(&'static [&'static str]),
    /// numbered steps
    S(&'static [&'static str]),
    /// tip box
    T(&'static str),
    /// picture (name of a file in src/help_img) with a caption
    I(&'static str, &'static str),
}
use B::*;

pub const TOPICS: &[(&str, &[B])] = &[
    ("Welcome", &[
        P("Backwoods Dash Designer builds 3D-printable car dashes. You shape a dash from a few simple lines (profiles and guidelines), add gauge holes, vents and trim, and export STL files ready to slice and print. Sizes are shown in millimeters unless you switch to inches (see Units)."),
        H("The short version"),
        S(&[
            "Start with Create new (the wizard) and pick a style, size, windshield line and front shape.",
            "Fine-tune the dash: click a profile in the 3D view and reshape it in the profile editor at the bottom.",
            "Add parts and shapes (gauges, vents, panels) from the Parts & shapes tab and drag them onto the dash. Draw your own shapes, even ones with holes in them.",
            "Check it in Materials and Wall views, then press Export STL(s) and save each piece.",
        ]),
        T("Press F1 at any time to open this manual. Use the search box to jump to a topic."),
        H("Directions"),
        P("The dash is built facing the windshield. +y points toward the windshield, the driver side is -x (left), and +z is up. The center line of the dash is x = 0."),
    ]),
    ("Screen layout", &[
        I("layout", "The main window: 1 top bar, 2 left panel, 3 the 3D view, 4 right panel, 5 bottom editor."),
        H("1  Top bar"),
        L(&[
            "Backwoods Dash Designer, then the File menu and the New dash button.",
            "Top pad, Lower pad, Top trim and Side walls: choose which piece you are editing.",
            "Undo and Redo (Ctrl/Cmd + Z, and Ctrl/Cmd + Shift + Z).",
            "On the right: the mm / in button, Lighting, Help, the wall thickness box, and Export STL(s).",
        ]),
        H("2  Left panel"),
        L(&[
            "Scene tab: the list of everything in the dash. The piece you are editing shows its sections, profiles, guidelines, parts and cut-outs. Click a row to select it; the eye icon shows or hides it.",
            "Parts & shapes tab: your library of 3D parts and shapes, and the tools to draw new ones.",
            "While the shape editor is open the editor has the whole window, so the Scene tab and the top-row buttons ask first: Save and continue, Discard, or Cancel (see Drawing your own shape).",
        ]),
        H("3  The 3D view"),
        P("The big view shows the dash. The pill bar at the top of the view switches the look (Materials or Design colors), the camera (3D, Driver, Top, Side), the Wall view, and the Cuts view. The Lighting button is in the top row, next to Help, and opens a window in the middle of the screen."),
        I("viewbar", "The view bar at the top of the 3D view."),
        H("4  Right panel"),
        P("Settings for whatever is selected: a section, a profile, a guideline, a part, a cut-out, the side walls, or the material of the piece. It also holds the printing options."),
        H("5  Bottom panel"),
        P("The editors for the selected piece: Profile shape, Guidelines: plan, Guidelines: height, Side walls, and Part (when a part is selected). The status bar underneath shows messages and counts."),
        T("Press [ to hide or show the left panel, ] for the right panel, and Tab to hide or show both, to give the 3D view more room."),
    ]),
    ("Units: millimeters and inches", &[
        P("The program starts in metric (millimeters). To work in inches, use the mm / in switch at the bottom-left of the start screen, or the mm / in button in the top bar at any time. The choice is remembered on this computer."),
        L(&[
            "Everything that shows a size follows the choice: the size read-outs, the number boxes, the wizard sliders, the rulers and grid in the shape editor, the measure tool, and the shape and cut-out sizes.",
            "You can type a size in either unit in any number box: a plain number uses the current unit, and adding mm or in (for example 2in or 30mm) uses that unit instead.",
            "Nothing is changed in your dash by switching. Files and exported STL files are always exact; STL files are written in millimeters, which is what slicers expect, so set your slicer to millimeters.",
        ]),
        T("A few labels that show picture-style axes (such as the numbers inside the guideline editors) can still read in millimeters; every box you type into follows your choice."),
    ]),
    ("Starting a dash", &[
        P("When the program opens it shows the start screen. Nothing of any dash is loaded until you choose."),
        I("start", "The start screen."),
        L(&[
            "Open last: goes straight back to the dash you saved most recently (or, if you never saved, the one you were working on last time).",
            "Open existing: browse for a saved .bwd file.",
            "Create new: starts the step-by-step wizard.",
        ]),
        P("The Recent list underneath shows your last few files. The same options are always in the File menu, and the New dash button in the top bar starts the wizard again at any time."),
        T("If you have unsaved changes, the program asks Save changes? before it opens or creates something else, or quits."),
    ]),
    ("The new dash wizard", &[
        P("The wizard builds a complete dash shell in six steps. The picture on the left updates as you change each setting, and you can turn it with the mouse: drag to turn, scroll to zoom, Shift-drag to slide, double-click to reset."),
        I("wiz_start", "Step 1: pick a starting style."),
        H("The steps"),
        S(&[
            "Start: pick a starting style: Twin-pad wrap, Smooth bow, Straight bench, Squarebody truck or Minimal.",
            "Size: overall width, depth, height and how many profiles the top pad has (3, 5 or 7).",
            "Windshield line: how far the line bows out at the center, how far it rises, the depth of the ends compared to the center, and the front-line dip with its Center depth.",
            "Front profile: the front arc radius, the slope of the top run, and the lip underneath.",
            "Sections: name and color each span between profiles. Click a colored section in the picture to jump to its name box.",
            "Finish: the whole dash is shown (top pad, lower pad and top trim). Press Finish to start working on it.",
        ]),
        I("wiz_wind", "Step 3: the windshield line, with the center dip."),
        I("wiz_sections", "Step 5: click a colored section in the picture to jump to its name."),
        I("wiz_finish", "Step 6: the whole dash, ready to finish."),
        H("Good to know"),
        L(&[
            "The lower pad and top trim follow the top pad's windshield line and front line, so the pieces always meet.",
            "Squarebody truck builds a stepped dash where the driver side sticks out further, and has no top trim.",
            "Cancel closes the wizard and puts everything back as it was. Undo reverses wizard changes one at a time after you finish.",
            "In the right panel, Reset the top pad to its original profiles returns a piece to the shape the wizard gave it, not to the factory default.",
        ]),
    ]),
    ("Moving around the 3D view", &[
        L(&[
            "Orbit: drag with the left mouse button.",
            "Pan: Shift + drag (or drag with another mouse button).",
            "Zoom: scroll wheel or trackpad scroll.",
            "Select: click a line, point, part or cut-out. Click it again, or click empty space, to deselect.",
            "Right-click a piece of the dash for the sizing menu, or a cut-out for its menu.",
        ]),
        H("View gizmo (top right of the 3D view)"),
        P("The colored X, Y and Z balls in the top right corner of the 3D view are always there, like Blender's. The solid balls point toward the front of each axis and the faded ones toward the back; the one facing you sits in the middle."),
        L(&[
            "Drag anywhere on the gizmo to turn the view around the dash (the dash itself does not move).",
            "Click a ball to look along that axis: Z from above, Y from the rear or the front, X from the side. Click the faded ball to look from the opposite side.",
            "To resize the dash, use the corner brackets (right-click a piece > Size > Drag bars); the gizmo only changes the view.",
        ]),
        H("View buttons"),
        L(&[
            "Materials / Design colors: the real material look, or the editing colors that show sections and lines.",
            "3D: the home view. Driver: looking toward the windshield from the seat. Top and Side: straight-on views.",
            "Wall: shows the real printed wall, the exact shell that gets exported, at the wall thickness set in Printing. Tick Cut and use the slider to slice the dash and see the wall in section.",
            "Cuts: shows the dash with the placed shapes cut, raised or inlaid.",
            "Lighting (top row, next to Help): opens the lighting window (see Materials and lighting).",
        ]),
        I("wallview", "Wall view with the thickness shown beside the button, and the Cut tick for slicing."),
        H("SpaceMouse"),
        P("If you have a 3Dconnexion SpaceMouse and its driver (3DxWare) installed, it moves the 3D view: tilt and twist to orbit, push sideways and lift to pan, push forward and back to zoom. The left button is the home view and the right button the top view. Click the SpaceMouse button in the bottom-right of the top bar to see its status and set the sensitivity. It also turns the solid in the shape editor's 3D view (see Combining shapes: pieces and holes)."),
    ]),
    ("Pieces of the dash", &[
        P("A dash is made of three pieces plus two end walls. Each is exported as its own STL so you can print them separately."),
        L(&[
            "Top pad: the main upper surface, the piece that carries most of the shape.",
            "Lower pad: the panel below the top pad. While Lock heights is ticked, its top edge is held on the top pad's front line, in height and in front-to-back position, so the two always meet cleanly.",
            "Top trim: a thin cover that rides on the top pad, thinner at the windshield and thicker toward the front, ending in a rounded nose. It follows the top pad's shape automatically. Use the Top trim piece tick in the right panel to turn it on or off; off hides it and leaves it out of the export. A trim that was left off while you changed the top pad is rebuilt to fit when you add it back.",
            "Side walls: the closing walls at the two ends of the dash. You edit one back curve in the bottom panel (Side walls) and both walls use it. They share the top pad's material so texture and color match.",
        ]),
        I("walls_ed", "Side walls: the back curve is edited in the bottom panel and both walls use it."),
        T("Click Top pad, Lower pad, Top trim or Side walls in the top bar to switch piece. Each piece has its own profiles, guidelines, parts and material."),
    ]),
    ("Shaping profiles and guidelines", &[
        P("A dash surface is built from profiles: cross-sections taken across the dash at different positions left to right. The surface is smoothly stretched between them. Two guidelines control how the profiles sit: the windshield line (the back edge every profile hangs from) and the front line (the front edge)."),
        I("profile", "The profile shape editor: one cross-section, with its points and handles."),
        H("Profile shape editor"),
        L(&[
            "Drag a point to move it. Drag its handles to change the curve.",
            "Double-click the curve to add a point to every profile. Use Remove point to take the selected one out of every profile.",
            "Shift-click or Shift-drag a box to select several points and move them together.",
            "Mirror lock: a profile can be locked to its partner on the other side of the dash, so changing one changes the other. The center profile has no partner.",
            "Re-space all points evenly sets a new point count on every profile while keeping the shape.",
        ]),
        I("plan", "Guidelines: plan, seen from above."),
        I("height", "Guidelines: height, seen from the side."),
        H("Guidelines: plan and height"),
        L(&[
            "Plan is the view from above; Height is the view from the side.",
            "Drag the circles to slide profiles along the dash. Drag the squares to bend the line.",
            "+ Add profile lets you add a profile mirrored on both sides, or on one side only, and choose where it goes.",
            "Keep both halves mirrored keeps the guidelines symmetric about the center line.",
        ]),
        H("Sections and key points"),
        L(&[
            "A section is the span between two profiles. Give it a name and color; you can split it, merge it with its neighbor, or move the line between sections to a different profile.",
            "Key points mark places you want to find again, such as the middle of a gauge pod. Select a point and press + Add key point at selected point; use selected moves a key point to the current point.",
        ]),
        H("Start over"),
        P("Reset the (piece) to its original profiles returns the piece to the shape the wizard gave it. Undo brings your changes back."),
    ]),
    ("Resizing the dash", &[
        P("Right-click the dash in the 3D view for the sizing menu, or turn on corner handles."),
        I("size", "Corner handles around the dash."),
        L(&[
            "Whole dash: all three pieces, side walls and parts change together so everything still fits.",
            "This piece only: only the selected piece changes.",
            "Corner handles: shows brackets around the dash. Grab a bracket and pull along it to stretch in that direction; hold Shift to scale all three together.",
            "Type a size (mm): enter exact lengths for width, depth or height.",
            "Scale everything evenly: enter a percentage and press Scale. Reset % clears the box.",
            "Resize parts and cut-outs too: on an even scale gauges, vents and shapes grow or shrink with the dash. When one side is stretched, they only move.",
            "Reset to original size puts it back as it was before any resizing.",
        ]),
    ]),
    ("Parts & shapes", &[
        P("The Parts & shapes tab is your library. It holds basic shapes (circles, rectangles and so on), shapes you have drawn or imported, and imported 3D parts."),
        I("shapes_tab", "The Parts & shapes tab: basic shapes on top, your own library below."),
        H("Placing a shape on the dash"),
        S(&[
            "Drag a shape from the library onto the dash. A preview outline follows the pointer.",
            "Release it where you want it. It becomes a cut-out you can move, size and turn.",
            "Choose what it does: Cut through, Recess, Raise or Inlay (see Cut-outs).",
        ]),
        H("Importing"),
        L(&[
            "Import… accepts STL (a 3D part to cut out of or add to the dash) and DXF, AI or EPS (a flat outline from closed paths).",
            "Imported items are stored in the library on this computer and are never sent anywhere.",
        ]),
        H("My library"),
        P("Shapes you draw and save appear in My library as thumbnails: drag one onto the dash (or double-click it) to place it. The editor only makes shapes; it never places them. Right-click one to rename or remove it. The library is kept between sessions and is not part of any single dash file."),
        T("Click a basic shape to open a copy in the editor, change it, and press Save to my library. To put it on the dash, drag it from My library."),
    ]),
    ("Drawing your own shape", &[
        P("Press Draw a closed loop in the Parts & shapes tab to draw an outline from scratch."),
        I("draw", "Drawing a loop on the canvas: vertices placed so far, with the view tools on the right."),
        S(&[
            "Pick a canvas size from the presets, or type your own width and height.",
            "Click to place vertices. Click the first (green) vertex, or press Enter, to close the loop. Esc cancels.",
            "Edit it: drag vertices, convert corners to smooth curve points (and back), break handles for a sharp corner, and stretch the shape (the lock keeps the width : height ratio).",
            "Optionally extrude it into a 3D solid and preview it in the editor.",
            "Press Done. The program asks for a name; give it one and the shape is saved to My library. Discard throws it away.",
        ]),
        H("Pen helpers (like a drawing program's pen tool)"),
        P("While you draw a loop, a Pen helpers card sits under the drawing. It makes tidy corners easy."),
        L(&[
            "Angle lock: every new line snaps to a multiple of the step angle (15 degrees by default; set the step from 1 to 90). Hold Shift while clicking to flip the lock for one line.",
            "Guides: pink dashed lines appear when the pointer lines up (across or up and down) with an earlier point, and it snaps there so shapes match up exactly.",
            "Angles: the angle at each corner and the direction of the line you are drawing are shown on the canvas; they turn green at 45, 90, 135 and 180 degrees.",
            "The helpers work while drawing a new loop. Points you drag later move freely.",
        ]),
        H("Leaving the editor"),
        I("leave", "The question that appears when you click outside the editor."),
        P("The shape editor takes over the main window, so the dash view stays hidden until you are done. If you click the Scene tab or a button in the top row, the program asks: Save and continue (keeps the shape, asking for a name if it needs one), Discard, or Cancel to go back to the editor. If you have not started drawing yet there is nothing to lose, so it just closes."),
        H("View tools in the editor"),
        L(&[
            "Scroll to zoom and drag the empty canvas to pan. Zoom to fit refits the drawing to the window.",
            "Measure: click two points on the canvas to read the distance (in your chosen unit).",
            "A grid and rulers help you judge size, and they follow your chosen unit.",
        ]),
        H("Tracing over a picture"),
        I("trace", "A picture behind the canvas, with corner handles to move and resize it."),
        L(&[
            "Load image… puts a photo or drawing behind the canvas. It stays on this computer. You can also drop an image file onto the window.",
            "Show and Opacity control how strongly it appears.",
            "Move / resize: drag the image to move it; drag a corner square to resize it. Center and Size boxes take exact values in your chosen unit.",
            "Keep proportions (on by default) keeps the picture's shape when resizing. Turn it off to stretch width and height separately.",
            "Mark two points sets the real size: click two points whose real distance you know, enter it, and press Set size. Level across and Level up straighten the picture using those two points.",
            "Rotate and Flip turn or mirror the picture. Replace… swaps the image; Remove takes it away.",
        ]),
    ]),
    ("Combining shapes: pieces and holes", &[
        P("A shape does not have to be one outline. In the shape editor you can add ready-made pieces to a drawing, or cut holes in it, to make a compound shape such as a bezel with a round opening and a mounting tab. Draw or pick the main shape first, then use the Combine shapes card in the right panel."),
        I("shape_combo", "A bezel with a round hole (red outline) and an added tab (green). The selected piece shows its own handles; the Combine shapes card lists the pieces further down the right panel."),
        S(&[
            "Press Add a piece... to join a circle, rectangle, polygon, star or cross to the shape, or Cut a hole... to remove one from it.",
            "Click a piece in the list to select it. Type its position and size in the boxes, or drag it on the drawing: drag the piece to move it, the square handles to resize it, and the diamond handle to turn it.",
            "Pieces are measured from the center of the main outline. When you resize or scale the main shape, the pieces are scaled with it so the design keeps its proportions.",
            "Remove deletes the selected piece. Holes show in red on the drawing, added pieces in the normal color.",
        ]),
        H("Size, circles and holes"),
        L(&[
            "A circle piece keeps its round shape while you resize it (Keep it round). Turn that off to make an oval.",
            "Holes are cut straight through the whole shape; the wall of a hole is straight, with no lean.",
            "The whole thing cuts, extrudes, saves to My library and exports as one shape.",
        ]),
        H("Looking at it in 3D"),
        P("Once the shape has a depth (Extrude section in the right panel), a Rotate in 3D button sits in the top right corner of the drawing (it is also under the Extrude settings as View in 3D). It turns the solid around so you can check the holes and added pieces from any side. Press Back to editing to return."),
        I("shape_3d", "Rotate in 3D: the solid with its hole and tab. Drag to tumble it."),
        L(&[
            "Drag to tumble the solid, Shift-drag or middle-drag to slide it, scroll to zoom.",
            "SpaceMouse: tilt and twist to tumble, push sideways and lift to slide, push forward and back to zoom. The left button is the front view and the right button the top view.",
        ]),
    ]),
    ("Cut-outs: cut, recess, raise, inlay", &[
        P("Every shape you place on the dash is a cut-out with one of four actions."),
        L(&[
            "Cut through: opens the shape straight through the wall (vents, speaker holes).",
            "Recess: sinks the shape into the surface as a pocket that stops part way through, like a sunk panel.",
            "Raise: pulls the shape out of the surface as a raised pad.",
            "Inlay: cuts the shape out and keeps it as a separate plug of the same shape that fills the opening.",
        ]),
        I("cut", "A cut-out placed on the dash: its settings are in the right panel."),
        H("Settings for a cut-out"),
        L(&[
            "Position, size and rotation: drag it in the 3D view or in the Part editor at the bottom (move from above, size with the side squares, scale with the corner, turn with the round handle; hold Shift for 15 degree steps).",
            "Sit on surface and Sink flush place a 3D part against the dash surface under its center.",
            "Mirror makes the matching copy on the other side of the center line. Center on dash moves it to x = 0.",
            "Depth (Height for a raise): how deep the recess or how tall the raise is. Keep a wall around the cut makes it a true pocket with a wall of its own. Floor follows the surface (Top follows the surface for a raise) keeps the depth the same on a curve. Keep the wall thickness pushes the inside of the wall with the surface so the dash stays as thick as elsewhere. Wall bevel slopes the walls like a countersink.",
            "Direction: Straight down works best on the flat top; Along the surface follows the slope under the shape.",
            "Lock: stops it being dragged, turned or edited by accident.",
            "Edit the outline… opens the shape editor on that cut-out's outline.",
            "Combine shapes (in the shape editor): add pieces to the outline or cut holes in it; the whole thing cuts, extrudes and exports as one shape (see Combining shapes: pieces and holes). A hole in a Raise or Inlay shape leaves an island of the original surface.",
            "Delete removes it (or press Delete / Backspace while it is selected).",
        ]),
        T("Use the Cuts button in the 3D view to see the dash with all cut-outs applied. Hidden cut-outs still export; hide only affects the view."),
    ]),
    ("Materials and lighting", &[
        H("Material"),
        P("Select a piece and choose its material in the right panel: Vinyl, Leather, Suede, Gloss plastic, Matte plastic, Brushed metal, Chrome, Oak or Gloss paint, each with color swatches. This changes how it looks in the 3D view; it does not change the printed shape."),
        I("lighting", "The Lighting window: press Lighting in the top row; Close or Esc shuts it."),
        H("Lighting window"),
        P("Press the Lighting button in the top row (next to Help). It opens in the middle of the screen, like the other pop-ups, and closes with its Close button or the Esc key."),
        L(&[
            "Exposure: overall brightness. Lower it if things look washed out.",
            "Main light, Fill light, Ambient: how strong each light is.",
            "Reflections: how strongly the room shows in shiny surfaces. Texture: strength of grain and pebble.",
            "Light turn and Light height: direction and height of the main light.",
            "Keep the lights fixed in the scene: off means the lights follow the camera; on means they stay put as you orbit.",
            "Reset lighting restores the defaults. Your lighting is remembered between sessions.",
        ]),
    ]),
    ("Printing and exporting", &[
        H("Wall thickness"),
        P("The wall box in the top bar sets the shell thickness used when exporting. 0 means a surface only with no thickness. Switch on Wall in the 3D view to see exactly the shell that will be exported."),
        H("Printing options (right panel)"),
        L(&[
            "Gap between parts (tick): keeps a small air gap where one part sits on another so printed parts fit together. The top trim follows the top pad with this gap underneath.",
            "Edge on the trim ends (tick): chamfer or round the top edge of the flat end faces of the top trim, from the windshield line to the front line. You can set how far it cuts down the end face and how far it reaches in across the top, and tick all the way to sweep it down the whole end face. The pads are left square.",
        ]),
        I("export", "The Export STL(s) window: settings at the top, then one name box and Save button per piece and side wall."),
        H("Export STL(s)"),
        S(&[
            "Press Export STL(s) in the top bar. A window opens in the middle of the screen with your wall thickness at the top, then the pieces and the two side walls, each with a name box and a Save button.",
            "Name each piece, then press its Save. Saving over a name replaces that file.",
            "Press Show exports folder to open the folder holding the files.",
        ]),
        P("Shapes you draw can be exported on their own from the shape editor (shape_<name>.stl in the same exports folder)."),
        T("Check the Wall view before exporting. If something looks wrong there, it will print wrong."),
    ]),
    ("Saving and opening files", &[
        P("A dash is one .bwd file that holds everything: the three pieces, side walls, parts, cut-outs, trace images placements and settings."),
        L(&[
            "File > Save (Ctrl/Cmd + S) saves. File > Save as… (Ctrl/Cmd + Shift + S) saves under a new name. File > Open… (Ctrl/Cmd + O) opens a file. Open recent lists your last files.",
            "The window title shows the file name, with a dot when there are unsaved changes.",
            "New dash… starts the wizard; if you have unsaved changes the program asks first.",
            "Open last session brings back the dash you were working on when the program last closed, even if you never saved it.",
        ]),
        H("Autosave and backups"),
        P("Every launch starts on a fresh dash, and the previous one is backed up in the autosave folder (the newest five are kept) so nothing is lost if you forget to save."),
        H("What is shared between dashes"),
        P("Your shape library and lighting are yours, not the dash's: they are kept on this computer and available in every dash. Opening a file adds any shapes it carries to your library."),
    ]),
    ("Keyboard and mouse reference", &[
        H("Keyboard"),
        L(&[
            "F1: open or close this manual.",
            "Ctrl/Cmd + Z: undo.  Ctrl/Cmd + Shift + Z (or Ctrl/Cmd + Y): redo.",
            "Ctrl/Cmd + S: save.  Ctrl/Cmd + Shift + S: save as.  Ctrl/Cmd + O: open.",
            "Delete or Backspace: remove the selected part or cut-out (not while typing in a box).",
            "[ and ]: hide or show the left and right panels.  Tab: hide or show both.",
            "Esc: cancel the current tool, such as marking points, drawing, or choosing where to add a profile; it also closes the Lighting window and the Save-or-discard question.",
            "Shift (while drawing a loop): flip the angle lock for the next line.",
            "Enter: close the loop while drawing a shape (three or more points).",
        ]),
        H("Mouse in the 3D view"),
        L(&[
            "Drag: orbit.  Shift + drag: pan.  Scroll: zoom.",
            "Click: select.  Click again or click empty space: deselect.",
            "Right-click: sizing menu on the dash, or menu on a cut-out.",
            "Drag a part from the library onto the dash to place it.",
            "In the shape editor's 3D view: drag to tumble, Shift-drag to slide, scroll to zoom.",
        ]),
        H("Mouse in the editors"),
        L(&[
            "Drag a point or handle to move it. Double-click a curve to add a point.",
            "Shift-click or Shift-drag a box to select several.",
            "Scroll to zoom, drag empty space to pan (shape editor).",
        ]),
    ]),
    ("About and license", &[
        P("Backwoods Dash Designer is made by Chris & Stacy Weston of Backwoods Fabrications, a small independent Mid-Michigan workshop. The program is open source under the Apache License 2.0: you may use it, change it and share it, including commercially, as long as the copyright notice and the NOTICE file that credits Backwoods Fabrications are kept."),
        L(&[
            "The Backwoods Fabrications name, logo and raccoon mascot, and the app icon, are not part of that license: they are copyright Backwoods Fabrications. If you publish your own modified version, please use your own name and logo.",
            "The program uses open-source Rust libraries; their licenses are listed in THIRD_PARTY.md with the source code.",
            "Car maker and model names that appear in example dashes belong to their owners. This program is not affiliated with any vehicle manufacturer.",
        ]),
    ]),
    ("Look and feel", &[
        P("The program always uses its dark look, on Mac, Windows and Linux, whatever your computer's light or dark setting. That keeps the colors, the contrast and the controls the same on every computer, and makes the shading in the 3D view easier to judge."),
        L(&[
            "Orange marks the thing you are working on: the selected tab, the main button in a window, the selected piece.",
            "Windows and pop-ups open in the middle of the screen. The wizard shows a NEW DASH SETUP title at the top right so you always know where you are.",
        ]),
    ]),
    ("Where your files live", &[
        P("Everything stays on your computer. The program keeps its working files in a folder called BackwoodsDashDesigner in your home folder."),
        L(&[
            "exports: the STL files you save.",
            "autosave: backups of earlier sessions (newest five).",
            "shapes.json: your shape library.",
            "lighting.json: your lighting settings.",
            "errors.log: a record of any error the program showed, with details. Send this file to support if something goes wrong.",
        ]),
        P("The dash you are working on is a scratch copy in this folder until you save it as a .bwd file. Save regularly, and keep your .bwd files somewhere you back up."),
    ]),
    ("Troubleshooting and tips", &[
        H("Something looks wrong"),
        L(&[
            "Spikes or odd bands in a picture: switch to Wall view and orbit around; if the shell looks wrong there, check for a profile whose points have crossed over or a guideline bent too hard.",
            "Lower pad does not meet the top pad: Lock heights keeps its top edge on the top pad's front line. If it still looks off, check that Lock heights is ticked (it is in the side wall and lower pad settings).",
            "A cut-out breaks through the surface: tick Keep the wall thickness, or reduce the depth.",
            "Nothing happens when you press Done in the shape editor: it asks for a name first; type one and confirm.",
            "A saved dash will not open: the file may have been moved or edited by hand. Try Open last session, or an earlier autosave.",
        ]),
        H("Habits that help"),
        L(&[
            "Run the wizard first and fine-tune afterwards: the wizard keeps all three pieces in step.",
            "Save before big changes. Undo covers edits, but a saved file is your safe point.",
            "Use Design colors while shaping and Materials when checking the look.",
            "Check the Wall view and slice through with Cut before exporting.",
            "Print a short test piece first to check fit and the gap between parts.",
        ]),
        T("This program is built for dashes but works for any shaped panel: set the size in the wizard, then shape and cut as usual."),
    ]),
];

fn matches(q: &str, title: &str, blocks: &[B]) -> bool {
    if q.is_empty() || title.to_lowercase().contains(q) {
        return true;
    }
    blocks.iter().any(|b| match b {
        H(s) | P(s) | T(s) => s.to_lowercase().contains(q),
        I(_, c) => c.to_lowercase().contains(q),
        L(v) | S(v) => v.iter().any(|s| s.to_lowercase().contains(q)),
    })
}

/// A manual picture as a texture (decoded once).
fn help_tex(ctx: &egui::Context, key: &str) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("help_img", key));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let bytes = crate::help_img::get(key)?;
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let t = ctx.load_texture(format!("help_{}", key), ci, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    Some(t)
}

impl App {
    pub fn help_window(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.help_open = !self.help_open;
        }
        if !self.help_open {
            return;
        }
        let mut open = true;
        let q = self.help_q.trim().to_lowercase();
        let hits: Vec<usize> = (0..TOPICS.len()).filter(|i| matches(&q, TOPICS[*i].0, TOPICS[*i].1)).collect();
        if !hits.contains(&self.help_topic) {
            if let Some(f) = hits.first() {
                self.help_topic = *f;
            }
        }
        egui::Window::new("Help").open(&mut open).collapsible(false).default_size([940.0, 640.0]).min_size([620.0, 360.0]).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            let h = ui.available_height().max(320.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui(vec2(230.0, h), |ui| {
                    ui.vertical(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.help_q).hint_text("Search the manual").desired_width(220.0));
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().id_salt("help_topics").max_height(h - 40.0).show(ui, |ui| {
                            if hits.is_empty() {
                                ui.label(RichText::new("No topic mentions that.").color(MUTED));
                            }
                            for i in &hits {
                                let on = *i == self.help_topic;
                                let (rect, r) = ui.allocate_exact_size(vec2(220.0, 28.0), egui::Sense::click());
                                if on || r.hovered() {
                                    ui.painter().rect_filled(rect, CornerRadius::same(6), if on { ACCENT_DIM } else { CARD_HI });
                                }
                                ui.painter().text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, TOPICS[*i].0, egui::FontId::proportional(14.0), if on { ACCENT } else { TEXT });
                                if r.clicked() {
                                    self.help_topic = *i;
                                }
                            }
                        });
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical().id_salt(("help_body", self.help_topic)).auto_shrink([false, false]).show(ui, |ui| {
                        let (title, blocks) = TOPICS[self.help_topic.min(TOPICS.len() - 1)];
                        ui.add_space(2.0);
                        ui.label(RichText::new(title).size(24.0).color(TEXT));
                        ui.add_space(8.0);
                        for b in blocks {
                            match b {
                                H(s) => {
                                    ui.add_space(8.0);
                                    ui.label(RichText::new(*s).size(16.0).strong().color(ACCENT));
                                    ui.add_space(2.0);
                                }
                                P(s) => {
                                    ui.add(egui::Label::new(RichText::new(*s).size(14.0).color(TEXT)).wrap());
                                    ui.add_space(4.0);
                                }
                                L(v) => {
                                    for s in v.iter() {
                                        ui.horizontal_top(|ui| {
                                            ui.add_space(8.0);
                                            ui.label(RichText::new("\u{2022}").color(ACCENT));
                                            ui.add(egui::Label::new(RichText::new(*s).size(14.0).color(TEXT)).wrap());
                                        });
                                    }
                                    ui.add_space(4.0);
                                }
                                S(v) => {
                                    for (n, s) in v.iter().enumerate() {
                                        ui.horizontal_top(|ui| {
                                            ui.add_space(8.0);
                                            ui.label(RichText::new(format!("{}.", n + 1)).strong().color(ACCENT));
                                            ui.add(egui::Label::new(RichText::new(*s).size(14.0).color(TEXT)).wrap());
                                        });
                                    }
                                    ui.add_space(4.0);
                                }
                                I(key, cap) => {
                                    if let Some(tex) = help_tex(ctx, key) {
                                        let sz = tex.size_vec2();
                                        let w = ui.available_width().min(sz.x).max(120.0);
                                        ui.add_space(4.0);
                                        ui.add(egui::Image::new((tex.id(), vec2(w, w * sz.y / sz.x))).corner_radius(CornerRadius::same(8)));
                                        ui.label(RichText::new(*cap).size(12.0).italics().color(MUTED));
                                        ui.add_space(8.0);
                                    }
                                }
                                T(s) => {
                                    ui.add_space(4.0);
                                    egui::Frame::new().fill(ACCENT_DIM).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(12, 8)).stroke(Stroke::new(1.0, ACCENT)).show(ui, |ui| {
                                        ui.add(egui::Label::new(RichText::new(format!("Tip: {}", s)).size(13.5).color(TEXT)).wrap());
                                    });
                                    ui.add_space(6.0);
                                }
                            }
                        }
                        ui.add_space(14.0);
                        ui.label(RichText::new(format!("Backwoods Dash Designer {}  \u{00B7}  F1 opens this manual", env!("CARGO_PKG_VERSION"))).size(11.5).color(MUTED));
                    });
                });
            });
        });
        if !open {
            self.help_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_is_complete_and_searchable() {
        assert!(TOPICS.len() >= 14);
        for (t, b) in TOPICS {
            assert!(!t.is_empty() && !b.is_empty(), "{}", t);
        }
        let hits = |q: &str| TOPICS.iter().filter(|(t, b)| matches(q, t, b)).count();
        assert_eq!(hits(""), TOPICS.len());
        assert!(hits("spacemouse") >= 1 && hits("inlay") >= 1 && hits("zzzz-none") == 0);
    }
}
