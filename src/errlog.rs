//! Every error the app shows is also appended to `errors.log` in the data folder (~/BackwoodsDashDesigner), with the
//! state of the 3D parts at that moment, so a problem can be fixed from the file instead of a screenshot.
use std::io::Write;
use std::sync::OnceLock;

static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

pub fn init(dir: &std::path::Path) {
    let _ = PATH.set(dir.join("errors.log"));
    // keep the file from growing forever
    if let Some(p) = PATH.get() {
        if std::fs::metadata(p).map(|m| m.len() > 2_000_000).unwrap_or(false) {
            let _ = std::fs::rename(p, p.with_extension("old"));
        }
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write("PANIC", &format!("{}", info));
        prev(info);
    }));
    write("START", &format!("Backwoods Dash Designer {}", env!("CARGO_PKG_VERSION")));
}

fn stamp() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (s.div_euclid(86400), s.rem_euclid(86400));
    // civil date from days since 1970-01-01
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn write(kind: &str, msg: &str) {
    if let Some(p) = PATH.get() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "[{}] {}: {}", stamp(), kind, msg);
        }
    }
}
