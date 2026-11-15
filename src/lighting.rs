//! Scene lighting the user can adjust (Lighting button in the viewport bar). Saved in the data folder.
use crate::json::{self, J};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Lighting {
    pub key: f32,      // main light strength
    pub fill: f32,     // second, softer light from the other side
    pub ambient: f32,  // overall base light
    pub reflect: f32,  // how strongly the room shows in shiny surfaces
    pub exposure: f32, // overall brightness
    pub texture: f32,  // grain / pebble strength
    pub yaw: f32,      // key light direction, degrees (0 = from the front)
    pub pitch: f32,    // key light height, degrees
    pub fixed: bool,   // lights stay put in the scene instead of following the camera
}

impl Default for Lighting {
    fn default() -> Lighting {
        Lighting { key: 1.0, fill: 1.0, ambient: 1.0, reflect: 1.0, exposure: 1.0, texture: 1.0, yaw: 24.0, pitch: 50.0, fixed: false }
    }
}

impl Lighting {
    /// Direction towards the key light (x right, y up the screen, z towards the viewer when not fixed).
    pub fn key_dir(&self) -> [f32; 3] {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        if self.fixed {
            // world: x across the car, y toward the windshield, z up
            [p.cos() * y.sin(), -p.cos() * y.cos(), p.sin()]
        } else {
            [p.cos() * y.sin(), p.sin(), p.cos() * y.cos()]
        }
    }
    pub fn to_json(&self) -> J {
        let n = |v: f32| J::Num(v as f64);
        J::Obj(vec![
            ("key".into(), n(self.key)),
            ("fill".into(), n(self.fill)),
            ("ambient".into(), n(self.ambient)),
            ("reflect".into(), n(self.reflect)),
            ("exposure".into(), n(self.exposure)),
            ("texture".into(), n(self.texture)),
            ("yaw".into(), n(self.yaw)),
            ("pitch".into(), n(self.pitch)),
            ("fixed".into(), J::Bool(self.fixed)),
        ])
    }
    pub fn from_json(j: &J) -> Lighting {
        let d = Lighting::default();
        let g = |k: &str, v: f32| j.n(k).map(|x| x as f32).unwrap_or(v);
        Lighting {
            key: g("key", d.key),
            fill: g("fill", d.fill),
            ambient: g("ambient", d.ambient),
            reflect: g("reflect", d.reflect),
            exposure: g("exposure", d.exposure),
            texture: g("texture", d.texture),
            yaw: g("yaw", d.yaw),
            pitch: g("pitch", d.pitch),
            fixed: j.b("fixed").unwrap_or(d.fixed),
        }
    }
    pub fn load(dir: &std::path::Path) -> Lighting {
        std::fs::read_to_string(dir.join("lighting.json")).ok().and_then(|s| json::parse(&s).ok()).map(|j| Lighting::from_json(&j)).unwrap_or_default()
    }
    pub fn save(&self, dir: &std::path::Path) {
        let _ = std::fs::write(dir.join("lighting.json"), json::to_string(&self.to_json()));
    }
}
