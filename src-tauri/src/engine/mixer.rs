use crate::model::{BlendMode, Color};

/// Blend a foreground color onto a background color using the given blend mode.
pub fn blend(bg: Color, fg: Color, mode: BlendMode) -> Color {
    bg.blend(fg, mode)
}
