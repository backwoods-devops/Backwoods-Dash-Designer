// Project files (.bwd): one file holds the whole dash. The data folder is the working copy; Open unpacks a file
// into it, Save packs it up again. Starting the app begins on a fresh default dash (the last session is kept in autosave/).
use crate::json::{self, J};
use std::path::{Path, PathBuf};

pub const EXT: &str = "bwd";
/// the old extension: still opened, never written
pub const OLD_EXT: &str = "dashfw";
/// Text files that make up a project. size_orig.txt and lighting.json travel with the file but do not count as edits.
/// The shapes library (shapes.json) and imported meshes belong to the app, not to one dash: they stay put when you start
/// or open a dash, and a file brings its own shapes along (added to the library if you do not have them yet).
const FILES: [&str; 6] = ["project.json", "shapes.json", "sidewall.json", "refs.json", "size_orig.txt", "lighting.json"];
const EDIT_FILES: [&str; 3] = ["project.json", "sidewall.json", "refs.json"];
/// Not removed by clear() because they are shared across dashes.
const KEEP: [&str; 2] = ["lighting.json", "shapes.json"];

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(d: &[u8]) -> String {
    let mut o = String::with_capacity(d.len() / 3 * 4 + 4);
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(B64[(n >> 18) as usize & 63] as char);
        o.push(B64[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    o
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for ch in s.bytes() {
        if ch == b'=' || ch == b'\n' || ch == b'\r' {
            continue;
        }
        let v = B64.iter().position(|&b| b == ch)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn mesh_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(dir.join("meshes"))
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| (e.file_name().to_string_lossy().to_string(), e.path())).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Everything in the working folder as one JSON text.
pub fn bundle(dir: &Path) -> String {
    let mut files = Vec::new();
    for f in FILES {
        if let Ok(t) = std::fs::read_to_string(dir.join(f)) {
            files.push((f.to_string(), J::Str(t)));
        }
    }
    let meshes: Vec<(String, J)> = mesh_files(dir).into_iter().filter_map(|(n, p)| std::fs::read(p).ok().map(|b| (n, J::Str(b64_encode(&b))))).collect();
    json::to_string(&J::Obj(vec![("dashfw".into(), J::Num(1.0)), ("files".into(), J::Obj(files)), ("meshes".into(), J::Obj(meshes))]))
}

/// Remove the working copy (the viewer lighting preference stays).
pub fn clear(dir: &Path) {
    for f in FILES {
        if !KEEP.contains(&f) {
            let _ = std::fs::remove_file(dir.join(f));
        }
    }
}

/// Add the shapes of a file to the library (skipping ones already there, whatever their id).
fn merge_shapes(dir: &Path, incoming: &str) {
    let strip = |j: &J| -> String {
        match j {
            J::Obj(v) => json::to_string(&J::Obj(v.iter().filter(|(k, _)| k != "id").cloned().collect())),
            o => json::to_string(o),
        }
    };
    let get = |t: &str| -> Vec<J> { json::parse(t).ok().and_then(|j| j.get("shapes").and_then(|a| a.arr().cloned())).unwrap_or_default() };
    let mut lib = get(&std::fs::read_to_string(dir.join("shapes.json")).unwrap_or_default());
    let have: Vec<String> = lib.iter().map(|s| strip(s)).collect();
    let mut next = lib.iter().filter_map(|s| s.n("id")).fold(0.0f64, f64::max) as u32 + 1;
    let mut added = false;
    for s in get(incoming) {
        if have.contains(&strip(&s)) {
            continue;
        }
        if let J::Obj(mut v) = s {
            v.retain(|(k, _)| k != "id");
            v.insert(0, ("id".into(), J::Num(next as f64)));
            next += 1;
            lib.push(J::Obj(v));
            added = true;
        }
    }
    if added {
        let _ = std::fs::write(dir.join("shapes.json"), json::to_string(&J::Obj(vec![("next".into(), J::Num(next as f64)), ("shapes".into(), J::Arr(lib))])));
    }
}

/// Replace the working copy with the contents of a project file.
pub fn unpack(text: &str, dir: &Path) -> Result<(), String> {
    let j = json::parse(text).map_err(|e| format!("Not a dash file ({e})"))?;
    if j.get("dashfw").is_none() {
        return Err("Not a dash file".into());
    }
    let Some(J::Obj(files)) = j.get("files") else { return Err("The file has no dash data".into()) };
    if !files.iter().any(|(k, _)| k == "project.json") {
        return Err("The file has no dash data".into());
    }
    clear(dir);
    for (k, v) in files {
        if let Some(t) = v.str() {
            if k == "shapes.json" {
                merge_shapes(dir, t);
            } else if FILES.contains(&k.as_str()) {
                std::fs::write(dir.join(k), t).map_err(|e| e.to_string())?;
            }
        }
    }
    if let Some(J::Obj(ms)) = j.get("meshes") {
        let md = dir.join("meshes");
        let _ = std::fs::create_dir_all(&md);
        for (k, v) in ms {
            if k.contains('/') || k.contains('\\') || k.starts_with('.') {
                continue;
            }
            if md.join(k).exists() {
                continue;
            }
            if let Some(b) = v.str().and_then(b64_decode) {
                std::fs::write(md.join(k), b).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

pub fn write_file(dir: &Path, path: &Path) -> Result<(), String> {
    // write beside, then rename, so a failed save never damages the old file
    let tmp = path.with_extension("bwd.tmp");
    std::fs::write(&tmp, bundle(dir)).map_err(|e| format!("Could not save: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Could not save: {e}"))
}

pub fn read_file(path: &Path, dir: &Path) -> Result<(), String> {
    let t = std::fs::read_to_string(path).map_err(|e| format!("Could not open: {e}"))?;
    unpack(&t, dir)
}

/// Cheap fingerprint of the editable parts of the working copy.
pub fn signature(dir: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in EDIT_FILES {
        std::fs::read(dir.join(f)).unwrap_or_default().hash(&mut h);
    }
    h.finish()
}

/// At launch: keep whatever the last session left in the working folder (autosave/, newest five), then start clean.
pub fn startup(dir: &Path) {
    if dir.join("project.json").exists() {
        let ad = dir.join("autosave");
        let _ = std::fs::create_dir_all(&ad);
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        if std::fs::write(ad.join(format!("last_session_{ts}.{EXT}")), bundle(dir)).is_ok() {
            let mut all: Vec<PathBuf> = std::fs::read_dir(&ad).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == EXT || x == OLD_EXT)).collect()).unwrap_or_default();
            all.sort();
            while all.len() > 5 {
                let _ = std::fs::remove_file(all.remove(0));
            }
        }
    }
    clear(dir);
    // an earlier version cleared the shapes library at launch: win it back from the newest autosave that has shapes
    if !dir.join("shapes.json").exists() {
        let mut all: Vec<PathBuf> = std::fs::read_dir(dir.join("autosave")).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
        all.sort();
        for p in all.iter().rev() {
            let Ok(t) = std::fs::read_to_string(p) else { continue };
            let Ok(j) = json::parse(&t) else { continue };
            let Some(sh) = j.get("files").and_then(|f| f.s("shapes.json")) else { continue };
            merge_shapes(dir, sh);
            if let Some(J::Obj(ms)) = j.get("meshes") {
                let md = dir.join("meshes");
                let _ = std::fs::create_dir_all(&md);
                for (k, v) in ms {
                    if !k.contains('/') && !md.join(k).exists() {
                        if let Some(b) = v.str().and_then(b64_decode) {
                            let _ = std::fs::write(md.join(k), b);
                        }
                    }
                }
            }
            break;
        }
    }
}

pub fn recent(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("recent.txt")).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(|s| s.to_string()).take(8).collect()
}

pub fn push_recent(dir: &Path, path: &Path) {
    let p = path.to_string_lossy().to_string();
    let mut v = recent(dir);
    v.retain(|x| *x != p);
    v.insert(0, p);
    v.truncate(8);
    let _ = std::fs::write(dir.join("recent.txt"), v.join("\n"));
}

pub fn with_ext(p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.extension().map_or(false, |e| e == EXT) { pb } else { PathBuf::from(format!("{p}.{EXT}")) }
}

/// Makes text safe inside a PowerShell single-quoted string. PowerShell treats the typographic quotes (U+2018, U+2019, U+201A, U+201B)
/// like a plain `'`, so each of them is doubled too; otherwise a file name containing one could end the string and run code.
pub fn ps_quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        o.push(c);
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            o.push(c);
        }
    }
    o
}

/// Windows: the standard Open / Save dialog, through PowerShell (present on every Windows 10 and 11), so no extra library or
/// installer is needed. `filter` is a Windows filter string such as "Dash files (*.bwd)|*.bwd|All files|*.*".
#[cfg(windows)]
pub fn windows_dialog(save: bool, title: &str, filter: &str, name: &str) -> Option<std::process::Output> {
    use std::os::windows::process::CommandExt;
    let q = ps_quote;
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.{}FileDialog; $d.Title='{}'; $d.Filter='{}'; $d.FileName='{}'; if ($d.ShowDialog() -eq 'OK') {{ [Console]::Out.Write($d.FileName) }}",
        if save { "Save" } else { "Open" },
        q(title),
        q(filter),
        q(name)
    );
    // 0x08000000 = no console window flashing up
    std::process::Command::new("powershell").args(["-NoProfile", "-STA", "-Command", &script]).creation_flags(0x0800_0000).output().ok()
}
#[cfg(not(windows))]
pub fn windows_dialog(_save: bool, _title: &str, _filter: &str, _name: &str) -> Option<std::process::Output> {
    None
}

/// Ask the operating system for a file without blocking the editor. `save` asks for a new name.
pub fn pick(save: bool, start: Option<String>, name: String) -> std::sync::mpsc::Receiver<Option<String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let out = if cfg!(target_os = "macos") {
            let loc = start.as_deref().map(|d| format!(" default location (POSIX file \"{}\")", esc(d))).unwrap_or_default();
            let script = if save {
                format!("POSIX path of (choose file name with prompt \"Save dash as\" default name \"{}\"{})", esc(&name), loc)
            } else {
                format!("POSIX path of (choose file with prompt \"Open a dash\" of type {{\"{EXT}\", \"{OLD_EXT}\"}}{})", loc)
            };
            std::process::Command::new("osascript").args(["-e", &script]).output().ok()
        } else if cfg!(target_os = "windows") {
            windows_dialog(save, if save { "Save dash as" } else { "Open a dash" }, &format!("Dash files (*.{EXT};*.{OLD_EXT})|*.{EXT};*.{OLD_EXT}|All files|*.*"), &name)
        } else if save {
            std::process::Command::new("zenity").args(["--file-selection", "--save", "--confirm-overwrite", &format!("--filename={name}"), "--title=Save dash as"]).output().ok()
        } else {
            std::process::Command::new("zenity").args(["--file-selection", "--title=Open a dash", &format!("--file-filter=Dash files | *.{EXT} *.{OLD_EXT}")]).output().ok()
        };
        let path = out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
        let _ = tx.send(path);
    });
    rx
}

#[cfg(test)]
mod tests {
    #[test]
    fn powershell_quotes_are_doubled() {
        assert_eq!(super::ps_quote("a'b"), "a''b");
        assert_eq!(super::ps_quote("x\u{2019}; calc; \u{2018}"), "x\u{2019}\u{2019}; calc; \u{2018}\u{2018}");
        assert_eq!(super::ps_quote("plain name.bwd"), "plain name.bwd");
    }

    use super::*;
    #[test]
    fn base64_roundtrip() {
        for n in 0..9 {
            let d: Vec<u8> = (0..n).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&d)).unwrap(), d);
        }
    }
    #[test]
    fn bundle_roundtrip() {
        let a = std::env::temp_dir().join("dashfw_file_a");
        let b = std::env::temp_dir().join("dashfw_file_b");
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
        std::fs::create_dir_all(a.join("meshes")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("project.json"), "{\"upper\":{\"a\":\"q\\\"x\"}}\n").unwrap();
        std::fs::write(a.join("meshes/m.stl"), [0u8, 255, 7, 8, 9, 250]).unwrap();
        let s = bundle(&a);
        unpack(&s, &b).unwrap();
        assert_eq!(std::fs::read(b.join("project.json")).unwrap(), std::fs::read(a.join("project.json")).unwrap());
        assert_eq!(std::fs::read(b.join("meshes/m.stl")).unwrap(), vec![0u8, 255, 7, 8, 9, 250]);
        assert_eq!(signature(&a), signature(&b));
        // shapes library: kept by clear(), merged (not duplicated) by unpack()
        std::fs::write(a.join("shapes.json"), "{\"next\":3,\"shapes\":[{\"id\":2,\"name\":\"Gauge\"}]}").unwrap();
        std::fs::write(b.join("shapes.json"), "{\"next\":2,\"shapes\":[{\"id\":1,\"name\":\"Vent\"}]}").unwrap();
        let s2 = bundle(&a);
        unpack(&s2, &b).unwrap();
        unpack(&s2, &b).unwrap();
        let lib = std::fs::read_to_string(b.join("shapes.json")).unwrap();
        assert_eq!(lib.matches("Gauge").count(), 1);
        assert!(lib.contains("Vent"));
        clear(&b);
        assert!(b.join("shapes.json").exists());
        assert!(unpack("{\"x\":1}", &b).is_err());
        clear(&b);
        assert!(!b.join("project.json").exists());
    }
}
