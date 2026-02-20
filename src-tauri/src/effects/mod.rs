pub mod chase;
pub mod fade;
pub mod gradient;
pub mod rainbow;
pub mod solid;
pub mod strobe;
pub mod twinkle;

use crate::model::{BlendMode, Color, EffectKind, EffectParams, ParamSchema};

/// The core effect abstraction. An effect is a pure function from
/// (time, spatial position, parameters) → color.
///
/// This trait is intentionally minimal so it can be implemented by:
/// - Built-in Rust effects (now)
/// - A future DSL compiler
/// - WASM user scripts
/// - LLM-generated code
pub trait Effect: Send + Sync {
    /// Evaluate this effect for one pixel at one moment in time.
    ///
    /// - `t`: normalized time within the effect's duration (0.0 = start, 1.0 = end)
    /// - `pixel_index`: which pixel in the target group (0-based)
    /// - `pixel_count`: total pixels in the target group
    /// - `params`: user-configurable parameters for this effect instance
    fn evaluate(
        &self,
        t: f64,
        pixel_index: usize,
        pixel_count: usize,
        params: &EffectParams,
    ) -> Color;

    /// Human-readable name for this effect.
    fn name(&self) -> &'static str;

    /// Declares what parameters this effect accepts, their types, ranges, and defaults.
    fn param_schema(&self) -> Vec<ParamSchema>;
}

/// Resolve an EffectKind to its trait object implementation.
pub fn resolve_effect(kind: &EffectKind) -> Box<dyn Effect> {
    match kind {
        EffectKind::Solid => Box::new(solid::SolidEffect),
        EffectKind::Chase => Box::new(chase::ChaseEffect),
        EffectKind::Rainbow => Box::new(rainbow::RainbowEffect),
        EffectKind::Strobe => Box::new(strobe::StrobeEffect),
        EffectKind::Gradient => Box::new(gradient::GradientEffect),
        EffectKind::Twinkle => Box::new(twinkle::TwinkleEffect),
        EffectKind::Fade => Box::new(fade::FadeEffect),
    }
}

/// Enum dispatch for built-in effects. Zero-allocation, inlineable.
/// The `Effect` trait remains for future user-defined effects.
#[derive(Debug, Clone, Copy)]
pub enum BuiltinEffect {
    Solid,
    Chase,
    Rainbow,
    Strobe,
    Gradient,
    Twinkle,
    Fade,
}

impl BuiltinEffect {
    pub fn from_kind(kind: &EffectKind) -> Self {
        match kind {
            EffectKind::Solid => Self::Solid,
            EffectKind::Chase => Self::Chase,
            EffectKind::Rainbow => Self::Rainbow,
            EffectKind::Strobe => Self::Strobe,
            EffectKind::Gradient => Self::Gradient,
            EffectKind::Twinkle => Self::Twinkle,
            EffectKind::Fade => Self::Fade,
        }
    }

    /// Evaluate all pixels in a fixture in bulk.
    /// Extracts params once, then loops over pixels, blending in-place.
    #[inline]
    pub fn evaluate_pixels(
        &self,
        t: f64,
        dest: &mut [Color],
        global_offset: usize,
        total_pixels: usize,
        params: &EffectParams,
        blend_mode: BlendMode,
    ) {
        match self {
            Self::Solid => solid::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode),
            Self::Chase => chase::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode),
            Self::Rainbow => rainbow::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode),
            Self::Strobe => strobe::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode),
            Self::Gradient => gradient::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode),
            Self::Twinkle => twinkle::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode),
            Self::Fade => fade::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode),
        }
    }
}
