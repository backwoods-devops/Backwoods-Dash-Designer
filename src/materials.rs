// Material presets: what a surface is made of (vinyl, leather, metal...) plus a color the user picks.
// The shader in render.rs turns these numbers into the look; the same numbers are stored in the project.

pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub col: [u8; 3],
    pub rough: f32,
    pub metal: f32,
    pub coat: f32,
    pub kind: f32,   // 0 none, 1 vinyl grain, 2 leather pebble, 3 suede, 4 brushed, 5 wood, 6 carbon twill
    pub cell: f32,   // grain size in mm
    pub depth: f32,  // grain strength 0..1
    pub swatches: &'static [[u8; 3]],
}

const DARKS: &[[u8; 3]] = &[[20, 20, 22], [58, 38, 28], [34, 24, 18], [120, 82, 52], [150, 30, 30], [28, 44, 92], [200, 196, 184], [90, 90, 94]];
const METALS: &[[u8; 3]] = &[[200, 204, 210], [226, 228, 232], [190, 160, 110], [120, 124, 130]];
const WOODS: &[[u8; 3]] = &[[214, 172, 122], [234, 206, 162], [176, 128, 84], [120, 82, 52], [84, 56, 36]];
const PAINTS: &[[u8; 3]] = &[[138, 26, 26], [20, 20, 22], [28, 44, 92], [236, 232, 220], [32, 82, 56], [200, 120, 30]];

pub const PRESETS: [Preset; 9] = [
    Preset { id: "vinyl", label: "Vinyl", col: [20, 20, 22], rough: 0.36, metal: 0.0, coat: 0.12, kind: 1.0, cell: 0.5, depth: 0.35, swatches: DARKS },
    Preset { id: "leather", label: "Leather", col: [58, 38, 28], rough: 0.5, metal: 0.0, coat: 0.05, kind: 2.0, cell: 1.7, depth: 0.9, swatches: DARKS },
    Preset { id: "suede", label: "Suede", col: [48, 40, 36], rough: 0.95, metal: 0.0, coat: 0.0, kind: 3.0, cell: 0.6, depth: 0.25, swatches: DARKS },
    Preset { id: "gloss", label: "Gloss plastic", col: [12, 12, 14], rough: 0.12, metal: 0.0, coat: 0.5, kind: 0.0, cell: 1.0, depth: 0.0, swatches: DARKS },
    Preset { id: "matte", label: "Matte plastic", col: [40, 40, 44], rough: 0.62, metal: 0.0, coat: 0.0, kind: 0.0, cell: 1.0, depth: 0.0, swatches: DARKS },
    Preset { id: "brushed", label: "Brushed metal", col: [200, 204, 210], rough: 0.34, metal: 1.0, coat: 0.0, kind: 4.0, cell: 0.35, depth: 0.5, swatches: METALS },
    Preset { id: "chrome", label: "Chrome", col: [214, 216, 220], rough: 0.07, metal: 1.0, coat: 0.0, kind: 0.0, cell: 1.0, depth: 0.0, swatches: METALS },
    Preset { id: "wood", label: "Oak", col: [214, 172, 122], rough: 0.55, metal: 0.0, coat: 0.12, kind: 5.0, cell: 14.0, depth: 0.0, swatches: WOODS },
    Preset { id: "paint", label: "Gloss paint", col: [138, 26, 26], rough: 0.18, metal: 0.0, coat: 1.0, kind: 0.0, cell: 1.0, depth: 0.0, swatches: PAINTS },
];

/// A material choice: preset index + color.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Look {
    pub mat: u8,
    pub col: [u8; 3],
}

impl Look {
    pub fn vinyl() -> Look {
        Look { mat: 0, col: PRESETS[0].col }
    }
    pub fn preset(&self) -> &'static Preset {
        &PRESETS[(self.mat as usize).min(PRESETS.len() - 1)]
    }
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.col[0], self.col[1], self.col[2])
    }
    pub fn from_ids(id: &str, hex: &str) -> Option<Look> {
        let m = PRESETS.iter().position(|p| p.id == id)? as u8;
        let h = hex.trim_start_matches('#');
        let col = if h.len() == 6 {
            [u8::from_str_radix(&h[0..2], 16).ok()?, u8::from_str_radix(&h[2..4], 16).ok()?, u8::from_str_radix(&h[4..6], 16).ok()?]
        } else {
            PRESETS[m as usize].col
        };
        Some(Look { mat: m, col })
    }
}
