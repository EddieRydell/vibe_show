pub mod vixen;

/// Backward-compatible alias: `crate::import::vixen_preview` still works.
pub use vixen::preview as vixen_preview;

use std::fmt;

use crate::model::color::Color;

// ── Shared Vixen format constants ──────────────────────────────────
// Centralized so that callers (api.rs, commands.rs, registry handlers)
// don't hard-code file/directory names.

/// Directory inside a Vixen 3 project containing system configuration.
pub const VIXEN_SYSTEM_DATA_DIR: &str = "SystemData";

/// Main Vixen system-config filename.
pub const VIXEN_SYSTEM_CONFIG_FILE: &str = "SystemConfig.xml";

/// Vixen sequence file extension.
pub const VIXEN_SEQUENCE_EXT: &str = "tim";

/// Vixen module-store filename (for preview data).
pub const VIXEN_MODULE_STORE_FILE: &str = "ModuleStore.xml";

// ── Error type (shared across all importers) ────────────────────────

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    Xml(quick_xml::Error),
    Parse(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Io(e) => write!(f, "I/O error: {e}"),
            ImportError::Xml(e) => write!(f, "XML error: {e}"),
            ImportError::Parse(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<quick_xml::Error> for ImportError {
    fn from(e: quick_xml::Error) -> Self {
        ImportError::Xml(e)
    }
}

// ── ISO 8601 duration parser (shared) ───────────────────────────────

/// Parse ISO 8601 duration strings like `PT1M53.606S`, `P0DT0H5M30.500S`, etc.
/// Returns duration in seconds.
#[must_use]
pub fn parse_iso_duration(s: &str) -> Option<f64> {
    let s = s.trim();
    if !s.starts_with('P') {
        return None;
    }

    let s = &s[1..]; // Strip 'P'
    let mut total_seconds = 0.0;
    let mut current_num = String::new();
    let mut in_time_part = false;

    for ch in s.chars() {
        match ch {
            'T' => {
                in_time_part = true;
                current_num.clear();
            }
            'D' => {
                let days: f64 = current_num.parse().ok()?;
                total_seconds += days * 86400.0;
                current_num.clear();
            }
            'H' if in_time_part => {
                let hours: f64 = current_num.parse().ok()?;
                total_seconds += hours * 3600.0;
                current_num.clear();
            }
            'M' if in_time_part => {
                let minutes: f64 = current_num.parse().ok()?;
                total_seconds += minutes * 60.0;
                current_num.clear();
            }
            'S' if in_time_part => {
                let secs: f64 = current_num.parse().ok()?;
                total_seconds += secs;
                current_num.clear();
            }
            _ => {
                current_num.push(ch);
            }
        }
    }

    Some(total_seconds)
}

// ── CIE XYZ → sRGB conversion (shared) ─────────────────────────────

/// Convert CIE XYZ (D65, 0-100 scale) to sRGB Color.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn xyz_to_srgb(x: f64, y: f64, z: f64) -> Color {
    // Apply sRGB gamma
    fn gamma(c: f64) -> f64 {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    }

    // Normalize from 0-100 to 0-1
    let x = x / 100.0;
    let y = y / 100.0;
    let z = z / 100.0;

    // XYZ to linear sRGB (D65 reference, sRGB primaries)
    let r_lin = x * 3.240_454_2 + y * -1.537_138_5 + z * -0.498_531_4;
    let g_lin = x * -0.969_266_0 + y * 1.876_010_8 + z * 0.041_556_0;
    let b_lin = x * 0.055_643_4 + y * -0.204_025_9 + z * 1.057_225_2;

    Color::rgb(
        (gamma(r_lin) * 255.0).round() as u8,
        (gamma(g_lin) * 255.0).round() as u8,
        (gamma(b_lin) * 255.0).round() as u8,
    )
}
