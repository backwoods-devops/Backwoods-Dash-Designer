// 3Dconnexion SpaceMouse support through the 3Dconnexion driver's own client interface (macOS).
// The driver (3DxWare) must be installed; the framework is loaded at run time, so the app still
// starts normally without it.
use crate::app::*;
use crate::render::Camera;
use eframe::egui;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU8, Ordering};
use std::sync::OnceLock;

/// 0 = not started / unsupported, 1 = driver not found, 2 = driver found, waiting for a device, 3 = device connected
pub static STATUS: AtomicU8 = AtomicU8::new(0);
static AXES: [AtomicI32; 6] = [AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0)];
static BUTTONS: AtomicU32 = AtomicU32::new(0);
static PRESSED: AtomicU32 = AtomicU32::new(0); // newly pressed since last frame
static ANY_MSG: AtomicBool = AtomicBool::new(false);
static SEEN_OTHER: AtomicU32 = AtomicU32::new(0);
static NONZERO_LOGS: AtomicU32 = AtomicU32::new(0);
static MSGS: AtomicU32 = AtomicU32::new(0);
static LAST_CMD: AtomicI32 = AtomicI32::new(-1);
static REGISTER_RESULT: AtomicI32 = AtomicI32::new(-1);
static CTX: OnceLock<egui::Context> = OnceLock::new();

pub fn status_text() -> &'static str {
    let st = STATUS.load(Ordering::Relaxed);
    let st = if st == 2 && ANY_MSG.load(Ordering::Relaxed) { 3 } else { st };
    match st {
        1 => "3Dconnexion driver not found",
        2 => "driver found, no device yet",
        3 => "connected",
        _ => "not available on this system",
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::ffi::{c_char, c_void, CString};

    extern "C" {
        fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
        fn dlsym(h: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    #[repr(C, packed(2))]
    #[derive(Clone, Copy)]
    struct DeviceState {
        version: u16,
        client: u16,
        command: u16,
        param: i16,
        value: i32,
        time: u64,
        report: [u8; 8],
        buttons8: u16,
        axis: [i16; 6],
        address: u16,
        buttons: u32,
    }

    const MSG_DEVICE_STATE: u32 = 0x3364_5352; // '3dRX'
    const CMD_AXIS: u16 = 3;
    const CMD_BUTTONS: u16 = 2;

    type MsgFn = extern "C" fn(u32, u32, *mut c_void);
    type DevFn = extern "C" fn(u32);

    extern "C" fn on_message(_conn: u32, msg: u32, arg: *mut c_void) {
        if SEEN_OTHER.fetch_add(1, Ordering::Relaxed) < 8 {
            crate::errlog::write("SPACEMOUSE", &format!("driver message type {:#x} (arg null: {})", msg, arg.is_null()));
        }
        if msg != MSG_DEVICE_STATE || arg.is_null() {
            return;
        }
        let st: DeviceState = unsafe { std::ptr::read_unaligned(arg as *const DeviceState) };
        let cmd = st.command;
        LAST_CMD.store(cmd as i32, Ordering::Relaxed);
        if MSGS.fetch_add(1, Ordering::Relaxed) == 0 {
            let ax = st.axis;
            crate::errlog::write("SPACEMOUSE", &format!("first device message: command {} axes {:?}", cmd, ax));
        }
        if NONZERO_LOGS.load(Ordering::Relaxed) < 0 {
            let a = st.axis;
            let raw = unsafe { std::slice::from_raw_parts(arg as *const u8, std::mem::size_of::<DeviceState>()) };
            if a.iter().any(|v| *v != 0) || cmd != CMD_AXIS {
                NONZERO_LOGS.fetch_add(1, Ordering::Relaxed);
                let hex: Vec<String> = raw.iter().map(|b| format!("{:02x}", b)).collect();
                let (p, v) = (st.param, st.value);
                crate::errlog::write("SPACEMOUSE", &format!("msg command {} param {} value {} axes {:?} raw {}", cmd, p, v, a, hex.join("")));
            }
        }
        if cmd == CMD_AXIS {
            let ax = st.axis;
            for i in 0..6 {
                AXES[i].store(ax[i] as i32, Ordering::Relaxed);
            }
        } else if cmd == CMD_BUTTONS {
            let now = st.value as u32;
            let old = BUTTONS.swap(now, Ordering::Relaxed);
            PRESSED.fetch_or(now & !old, Ordering::Relaxed);
        }
        ANY_MSG.store(true, Ordering::Relaxed);
        if let Some(c) = CTX.get() {
            c.request_repaint();
        }
    }
    extern "C" fn on_added(_c: u32) {
        crate::errlog::write("SPACEMOUSE", "device added");
        STATUS.store(3, Ordering::Relaxed);
        if let Some(c) = CTX.get() {
            c.request_repaint();
        }
    }
    extern "C" fn on_removed(_c: u32) {
        crate::errlog::write("SPACEMOUSE", "device removed");
        STATUS.store(2, Ordering::Relaxed);
        for a in AXES.iter() {
            a.store(0, Ordering::Relaxed);
        }
        if let Some(c) = CTX.get() {
            c.request_repaint();
        }
    }

    pub fn reregister() {
        unsafe {
            let path = CString::new("/Library/Frameworks/3DconnexionClient.framework/3DconnexionClient").unwrap();
            let h = dlopen(path.as_ptr(), 1);
            if h.is_null() {
                return;
            }
            let un = CString::new("UnregisterConnexionClient").unwrap();
            let rg = CString::new("RegisterConnexionClient").unwrap();
            let (u, r) = (dlsym(h, un.as_ptr()), dlsym(h, rg.as_ptr()));
            if u.is_null() || r.is_null() {
                return;
            }
            let unregister: extern "C" fn(u16) = std::mem::transmute(u);
            let register: extern "C" fn(u32, *const u8, u16, u32) -> u16 = std::mem::transmute(r);
            let old = CLIENT.load(Ordering::Relaxed);
            if old > 0 {
                unregister(old as u16);
            }
            let name: [u8; 8] = [6, b'D', b'a', b's', b'h', b'F', b'W', 0];
            let id = register(0x2A2A_2A2A, name.as_ptr(), 1, 0x3FFF);
            CLIENT.store(id as i32, Ordering::Relaxed);
            crate::errlog::write("SPACEMOUSE", &format!("window focused: re-registered, client id {}", id));
        }
    }

    pub fn init() {
        unsafe {
            let path = CString::new("/Library/Frameworks/3DconnexionClient.framework/3DconnexionClient").unwrap();
            let h = dlopen(path.as_ptr(), 1);
            if h.is_null() {
                crate::errlog::write("SPACEMOUSE", "3DconnexionClient.framework could not be loaded (driver not installed?)");
                STATUS.store(1, Ordering::Relaxed);
                return;
            }
            let sym = |n: &str| {
                let c = CString::new(n).unwrap();
                dlsym(h, c.as_ptr())
            };
            let inst = sym("InstallConnexionHandlers");
            let reg = sym("RegisterConnexionClient");
            if inst.is_null() || reg.is_null() {
                STATUS.store(1, Ordering::Relaxed);
                return;
            }
            let install: extern "C" fn(MsgFn, DevFn, DevFn) -> i16 = std::mem::transmute(inst);
            let register: extern "C" fn(u32, *const u8, u16, u32) -> u16 = std::mem::transmute(reg);
            let err = install(on_message, on_added, on_removed);
            if err != 0 {
                crate::errlog::write("SPACEMOUSE", &format!("InstallConnexionHandlers failed with {}", err));
                STATUS.store(1, Ordering::Relaxed);
                return;
            }
            // Pascal string name, take over mode (1), all axes and buttons (0x3FFF)
            let name: [u8; 8] = [6, b'D', b'a', b's', b'h', b'F', b'W', 0];
            let id = register(0x2A2A_2A2A, name.as_ptr(), 1, 0x3FFF);
            REGISTER_RESULT.store(id as i32, Ordering::Relaxed);
            CLIENT.store(id as i32, Ordering::Relaxed);
            crate::errlog::write("SPACEMOUSE", &format!("registered with the driver, client id {}", id));
            STATUS.store(2, Ordering::Relaxed);
        }
    }
}

static CLIENT: AtomicI32 = AtomicI32::new(0);
static WAS_FOCUSED: AtomicBool = AtomicBool::new(false);

pub fn init(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
    #[cfg(target_os = "macos")]
    mac::init();
}

impl App {
    /// Drive the camera from the SpaceMouse. Called once per frame.
    pub fn spacemouse_frame(&mut self, ctx: &egui::Context) {
        let ax: Vec<f32> = AXES.iter().map(|a| a.load(Ordering::Relaxed) as f32).collect();
        let dead = 25.0;
        let f = |v: f32| if v.abs() < dead { 0.0 } else { (v - v.signum() * dead) / 350.0 };
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        let s = self.sm_sens;
        let inv = if self.sm_invert { -1.0 } else { 1.0 };
        let (tx, ty, tz, rx, rz) = (f(ax[0]), f(ax[1]), f(ax[2]), f(ax[3]), f(ax[5]));
        // Ignore the weaker axes when one dominates: pulling the puck back also nudges it
        // up a little, which made the view drift while zooming.
        let tm = tx.abs().max(ty.abs()).max(tz.abs());
        let keep = |v: f32, m: f32| if v.abs() >= 0.5 * m { v } else { 0.0 };
        let (tx, ty, tz) = (keep(tx, tm), keep(ty, tm), keep(tz, tm));
        let rm = rx.abs().max(rz.abs());
        let (rx, rz) = (keep(rx, rm), keep(rz, rm));
        let moving = tx != 0.0 || ty != 0.0 || tz != 0.0 || rx != 0.0 || rz != 0.0;
        // the shape editor's 3D view takes the puck while it is showing
        if let Some(ed) = self.shape_ed.as_mut().filter(|e| e.view3d) {
            if moving {
                let px = 700.0 * dt * s;
                ed.sm_drive((-rz * px * inv, rx * px * inv), (tx * px, tz * px), ty * 500.0 * dt * s * inv);
                ctx.request_repaint();
            }
            let pressed = PRESSED.swap(0, Ordering::Relaxed);
            if pressed & 1 != 0 {
                ed.sm_home(0);
            }
            if pressed & 2 != 0 {
                ed.sm_home(1);
            }
            return;
        }
        if moving {
            let px = 700.0 * dt * s;
            self.cam.orbit(-rz * px * inv, rx * px * inv);
            self.cam.pan(tx * px, tz * px);
            self.cam.zoom(ty * 500.0 * dt * s * inv);
            ctx.request_repaint();
        }
        let pressed = PRESSED.swap(0, Ordering::Relaxed);
        if pressed & 1 != 0 {
            self.cam = Camera::home();
            self.cam.target[2] = [330.0, 195.0, 360.0][self.st.active as usize];
        }
        if pressed & 2 != 0 {
            let zc = [300.0, 165.0, 340.0][self.st.active as usize];
            self.cam = Camera { yaw: 0.0, pitch: 1.5, dist: 2600.0, target: [0.0, 120.0, zc] };
        }
    }

    pub fn spacemouse_window(&mut self, ctx: &egui::Context) {
        if !self.sm_open {
            return;
        }
        let mut open = true;
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        egui::Window::new("SpaceMouse").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).resizable(false).default_width(300.0).show(ctx, |ui| {
            ui.label(format!("Status: {}", status_text()));
            let ax: Vec<i32> = AXES.iter().map(|a| a.load(Ordering::Relaxed)).collect();
            ui.label(egui::RichText::new(format!("Driver messages received: {}   client id: {}", MSGS.load(Ordering::Relaxed), REGISTER_RESULT.load(Ordering::Relaxed))).size(12.0));
            ui.label(egui::RichText::new(format!("Axes now: {:?}", ax)).size(12.0));
            ui.label(egui::RichText::new("Push or twist the puck: if the numbers stay at 0 the app is not receiving anything from the driver.").size(11.5).color(crate::theme::MUTED));
            if STATUS.load(Ordering::Relaxed) == 1 {
                ui.label("Install 3DxWare from 3dconnexion.com, restart it and reopen Backwoods Dash Designer.");
            }
            ui.add(egui::Slider::new(&mut self.sm_sens, 0.2..=4.0).text("Sensitivity"));
            ui.checkbox(&mut self.sm_invert, "Flip orbit and zoom direction");
            ui.label("Tilt and twist orbit, push sideways and lift to pan, push forward and back to zoom. Left button: home view. Right button: top view.");
        });
        if !open {
            self.sm_open = false;
        }
    }
}
