use crate::model::{BlendMode, Color};

/// Blend a foreground color onto a background color using the given blend mode.
pub fn blend(bg: Color, fg: Color, mode: BlendMode) -> Color {
    match mode {
        BlendMode::Override => fg,
        BlendMode::Add => bg + fg,
        BlendMode::Multiply => bg.multiply(fg),
        BlendMode::Max => bg.max(fg),
        BlendMode::Alpha => fg.over(bg),
    }
}
