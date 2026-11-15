//! Pictures for the manual (made by docs/make_shots.sh and docs/make_help_images.py).

pub fn get(key: &str) -> Option<&'static [u8]> {
    Some(match key {
        "start" => include_bytes!("help_img/start.png"),
        "shape_combo" => include_bytes!("help_img/shape_combo.png"),
        "shape_3d" => include_bytes!("help_img/shape_3d.png"),
        "leave" => include_bytes!("help_img/leave.png"),
        "wiz_start" => include_bytes!("help_img/wiz_start.png"),
        "wiz_wind" => include_bytes!("help_img/wiz_wind.png"),
        "wiz_sections" => include_bytes!("help_img/wiz_sections.png"),
        "wiz_finish" => include_bytes!("help_img/wiz_finish.png"),
        "layout" => include_bytes!("help_img/layout.png"),
        "viewbar" => include_bytes!("help_img/viewbar.png"),
        "wallview" => include_bytes!("help_img/wallview.png"),
        "walls_ed" => include_bytes!("help_img/walls_ed.png"),
        "profile" => include_bytes!("help_img/profile.png"),
        "plan" => include_bytes!("help_img/plan.png"),
        "height" => include_bytes!("help_img/height.png"),
        "size" => include_bytes!("help_img/size.png"),
        "shapes_tab" => include_bytes!("help_img/shapes_tab.png"),
        "draw" => include_bytes!("help_img/draw.png"),
        "trace" => include_bytes!("help_img/trace.png"),
        "cut" => include_bytes!("help_img/cut.png"),
        "lighting" => include_bytes!("help_img/lighting.png"),
        "export" => include_bytes!("help_img/export.png"),
        _ => return None,
    })
}
