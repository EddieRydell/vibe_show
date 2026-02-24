/// Vixen effect type identifiers (from `_typeId` / `TypeId` fields).
pub(super) mod vixen_effect {
    pub const PULSE: &str = "Pulse";
    pub const SET_LEVEL: &str = "SetLevel";
    pub const CHASE: &str = "Chase";
    pub const SPIN: &str = "Spin";
    pub const WIPE: &str = "Wipe";
    pub const ALTERNATING: &str = "Alternating";
    pub const SHOCKWAVE: &str = "Shockwave";
    pub const GARLANDS: &str = "Garlands";
    pub const PIN_WHEEL: &str = "PinWheel";
    pub const BUTTERFLY: &str = "Butterfly";
    pub const DISSOLVE: &str = "Dissolve";
    pub const COLOR_WASH: &str = "ColorWash";
    pub const TWINKLE: &str = "Twinkle";
    pub const STROBE: &str = "Strobe";
    pub const RAINBOW: &str = "Rainbow";
}

/// Vixen color handling mode identifiers.
#[allow(dead_code)] // STATIC_COLOR matches the default arm but is defined for completeness
pub(super) mod vixen_color_handling {
    pub const GRADIENT_THROUGH_WHOLE_EFFECT: &str = "GradientThroughWholeEffect";
    pub const GRADIENT_ACROSS_ITEMS: &str = "GradientAcrossItems";
    pub const COLOR_ACROSS_ITEMS: &str = "ColorAcrossItems";
    pub const GRADIENT_FOR_EACH_PULSE: &str = "GradientForEachPulse";
    pub const GRADIENT_OVER_EACH_PULSE: &str = "GradientOverEachPulse";
    pub const GRADIENT_PER_PULSE: &str = "GradientPerPulse";
    pub const STATIC_COLOR: &str = "StaticColor";
}

/// Vixen wipe/movement direction identifiers.
#[allow(dead_code)] // Some directions only match via the default arm
pub(super) mod vixen_direction {
    pub const RIGHT: &str = "Right";
    pub const LEFT: &str = "Left";
    pub const REVERSE: &str = "Reverse";
    pub const UP: &str = "Up";
    pub const DOWN: &str = "Down";
    pub const HORIZONTAL: &str = "Horizontal";
    pub const VERTICAL: &str = "Vertical";
    pub const DIAGONAL_UP: &str = "DiagonalUp";
    pub const DIAGONAL_DOWN: &str = "DiagonalDown";
    pub const BURST: &str = "Burst";
    pub const BURST_IN: &str = "BurstIn";
    pub const BURST_OUT: &str = "BurstOut";
    pub const OUT: &str = "Out";
    pub const CIRCLE: &str = "Circle";
    pub const CIRCLE_IN: &str = "CircleIn";
    pub const CIRCLE_OUT: &str = "CircleOut";
    pub const DIAMOND: &str = "Diamond";
    pub const DIAMOND_IN: &str = "DiamondIn";
    pub const DIAMOND_OUT: &str = "DiamondOut";
}
