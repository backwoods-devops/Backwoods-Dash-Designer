// Display units: everything in the program is stored in millimeters; this only changes how lengths are shown and typed.
// Metric is the default. The choice is kept in units.txt in the data folder (it belongs to the person, not to a dash).
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};

static INCH: AtomicBool = AtomicBool::new(false);
pub const MM_PER_IN: f64 = 25.4;

pub fn inch() -> bool {
    INCH.load(Ordering::Relaxed)
}

pub fn load(dir: &std::path::Path) {
    let on = std::fs::read_to_string(dir.join("units.txt")).map(|s| s.trim() == "inch").unwrap_or(false);
    INCH.store(on, Ordering::Relaxed);
}

pub fn set(on: bool, dir: &std::path::Path) {
    INCH.store(on, Ordering::Relaxed);
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("units.txt"), if on { "inch" } else { "mm" });
}

/// "mm" or "in"
pub fn label() -> &'static str {
    if inch() { "in" } else { "mm" }
}

/// Spelled out, for sentences.
pub fn word() -> &'static str {
    if inch() { "inches" } else { "millimeters" }
}

/// A length given in mm, as a number in the current unit.
pub fn disp(mm: f64) -> f64 {
    if inch() { mm / MM_PER_IN } else { mm }
}

fn decimals(dec: usize) -> usize {
    if inch() { dec + 2 } else { dec }
}

/// Number only: 12.3 (mm) or 0.485 (in).
pub fn num(mm: f64, dec: usize) -> String {
    format!("{:.*}", decimals(dec), disp(mm))
}

/// With the unit: "12.3 mm" / "0.485 in".
pub fn len(mm: f64, dec: usize) -> String {
    format!("{} {}", num(mm, dec), label())
}

/// With a sign: "+12.3 mm".
pub fn signed(mm: f64, dec: usize) -> String {
    format!("{:+.*} {}", decimals(dec), disp(mm), label())
}

/// Area given in mm2.
pub fn area(mm2: f64) -> String {
    if inch() { format!("{:.2} in\u{00B2}", mm2 / (MM_PER_IN * MM_PER_IN)) } else { format!("{:.0} mm\u{00B2}", mm2) }
}

/// Replaces a leading "mm" in a unit caption ("mm up") with the current unit.
pub fn caption(s: &str) -> String {
    match s.strip_prefix("mm") {
        Some(rest) => format!("{}{}", label(), rest),
        None => s.to_string(),
    }
}

/// Reads what was typed in a box: a bare number is in the current unit; "mm", "in" or a double quote force one.
pub fn parse(text: &str) -> Option<f64> {
    let t = text.trim().replace(',', ".").to_lowercase();
    let (body, force) = if let Some(b) = t.strip_suffix("mm") {
        (b, Some(false))
    } else if let Some(b) = t.strip_suffix("in") {
        (b, Some(true))
    } else if let Some(b) = t.strip_suffix('"') {
        (b, Some(true))
    } else {
        (t.as_str(), None)
    };
    let v: f64 = body.trim().parse().ok()?;
    Some(if force.unwrap_or(inch()) { v * MM_PER_IN } else { v })
}

/// Turns a DragValue (whose number is in mm) into one that shows and reads the current unit.
pub trait MmExt: Sized {
    fn mm(self) -> Self;
}

impl MmExt for egui::DragValue<'_> {
    fn mm(self) -> Self {
        let on = inch();
        self.custom_formatter(move |n, r| {
            if on {
                format!("{} in", egui::emath::format_with_decimals_in_range(n / MM_PER_IN, 2..=3))
            } else {
                format!("{} mm", egui::emath::format_with_decimals_in_range(n, r))
            }
        })
        .custom_parser(parse)
    }
}

/// The same for a Slider.
impl MmExt for egui::Slider<'_> {
    fn mm(self) -> Self {
        let on = inch();
        self.custom_formatter(move |n, r| {
            if on {
                egui::emath::format_with_decimals_in_range(n / MM_PER_IN, 2..=3)
            } else {
                egui::emath::format_with_decimals_in_range(n, r)
            }
        })
        .custom_parser(parse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn convert_and_read() {
        set(false, &std::env::temp_dir().join("dashfw_units_test"));
        assert_eq!(len(25.4, 1), "25.4 mm");
        assert_eq!(parse("2in"), Some(50.8));
        assert_eq!(parse("12"), Some(12.0));
        set(true, &std::env::temp_dir().join("dashfw_units_test"));
        assert_eq!(len(25.4, 1), "1.000 in");
        assert_eq!(parse("2"), Some(50.8));
        assert_eq!(parse("30mm"), Some(30.0));
        assert_eq!(caption("mm up"), "in up");
        assert!(area(645.16).starts_with("1.00 in"));
        set(false, &std::env::temp_dir().join("dashfw_units_test"));
        assert_eq!(caption("mm up"), "mm up");
    }
}
