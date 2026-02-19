use std::ops;

use serde::{Deserialize, Serialize};

/// RGBA color with 8-bit channels. Alpha is used for blending during composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create from HSV (hue 0-360, saturation 0-1, value 0-1)
    pub fn from_hsv(h: f64, s: f64, v: f64) -> Self {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r1, g1, b1) = match h as u16 {
            0..60 => (c, x, 0.0),
            60..120 => (x, c, 0.0),
            120..180 => (0.0, c, x),
            180..240 => (0.0, x, c),
            240..300 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Self::rgb(
            ((r1 + m) * 255.0) as u8,
            ((g1 + m) * 255.0) as u8,
            ((b1 + m) * 255.0) as u8,
        )
    }

    /// Linear interpolation between two colors. t is clamped to [0, 1].
    pub fn lerp(self, other: Self, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Self {
            r: (self.r as f64 * inv + other.r as f64 * t) as u8,
            g: (self.g as f64 * inv + other.g as f64 * t) as u8,
            b: (self.b as f64 * inv + other.b as f64 * t) as u8,
            a: (self.a as f64 * inv + other.a as f64 * t) as u8,
        }
    }

    /// Multiplicative blend (0-255 scale).
    pub fn multiply(self, other: Self) -> Self {
        Self {
            r: ((self.r as u16 * other.r as u16) / 255) as u8,
            g: ((self.g as u16 * other.g as u16) / 255) as u8,
            b: ((self.b as u16 * other.b as u16) / 255) as u8,
            a: 255,
        }
    }

    /// Per-channel maximum.
    pub fn max(self, other: Self) -> Self {
        Self {
            r: self.r.max(other.r),
            g: self.g.max(other.g),
            b: self.b.max(other.b),
            a: self.a.max(other.a),
        }
    }

    /// Scale brightness by a factor (0.0 - 1.0).
    pub fn scale(self, factor: f64) -> Self {
        let f = factor.clamp(0.0, 1.0);
        Self {
            r: (self.r as f64 * f) as u8,
            g: (self.g as f64 * f) as u8,
            b: (self.b as f64 * f) as u8,
            a: self.a,
        }
    }

    /// Alpha-composite `self` over `other` (self is foreground).
    pub fn over(self, other: Self) -> Self {
        let fa = self.a as f64 / 255.0;
        let ba = other.a as f64 / 255.0;
        let out_a = fa + ba * (1.0 - fa);
        if out_a == 0.0 {
            return Self::TRANSPARENT;
        }
        Self {
            r: ((self.r as f64 * fa + other.r as f64 * ba * (1.0 - fa)) / out_a) as u8,
            g: ((self.g as f64 * fa + other.g as f64 * ba * (1.0 - fa)) / out_a) as u8,
            b: ((self.b as f64 * fa + other.b as f64 * ba * (1.0 - fa)) / out_a) as u8,
            a: (out_a * 255.0) as u8,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Additive blend, clamped at 255 per channel.
impl ops::Add for Color {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            r: self.r.saturating_add(other.r),
            g: self.g.saturating_add(other.g),
            b: self.b.saturating_add(other.b),
            a: 255,
        }
    }
}
