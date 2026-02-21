pub mod chase;
pub mod fade;
pub mod gradient;
pub mod rainbow;
pub mod script;
pub mod solid;
pub mod strobe;
pub mod twinkle;
pub mod wipe;

use crate::model::show::Position2D;
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
/// Returns `None` for `Script` kinds (handled separately via the DSL VM).
pub fn resolve_effect(kind: &EffectKind) -> Option<Box<dyn Effect>> {
    match kind {
        EffectKind::Solid => Some(Box::new(solid::SolidEffect)),
        EffectKind::Chase => Some(Box::new(chase::ChaseEffect)),
        EffectKind::Rainbow => Some(Box::new(rainbow::RainbowEffect)),
        EffectKind::Strobe => Some(Box::new(strobe::StrobeEffect)),
        EffectKind::Gradient => Some(Box::new(gradient::GradientEffect)),
        EffectKind::Twinkle => Some(Box::new(twinkle::TwinkleEffect)),
        EffectKind::Fade => Some(Box::new(fade::FadeEffect)),
        EffectKind::Wipe => Some(Box::new(wipe::WipeEffect)),
        EffectKind::Script(_) => None,
    }
}

/// Returns true if the given effect kind requires spatial position data.
pub fn needs_positions(kind: &EffectKind) -> bool {
    matches!(kind, EffectKind::Wipe)
}

/// Evaluate all pixels in a fixture in bulk via enum dispatch on EffectKind.
/// Extracts params once, then loops over pixels, blending in-place.
/// Zero-allocation, inlineable. The `Effect` trait remains for future user-defined effects.
///
/// `positions` is only populated for spatial effects (e.g. Wipe). Non-spatial effects
/// ignore it entirely (zero overhead).
///
/// Returns `false` for `Script` kinds (caller must handle via DSL VM).
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn evaluate_pixels(
    kind: &EffectKind,
    t: f64,
    dest: &mut [Color],
    global_offset: usize,
    total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
    opacity: f64,
    positions: Option<&[Position2D]>,
) -> bool {
    match kind {
        EffectKind::Solid => { solid::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity); true }
        EffectKind::Chase => { chase::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity); true }
        EffectKind::Rainbow => { rainbow::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity); true }
        EffectKind::Strobe => { strobe::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity); true }
        EffectKind::Gradient => { gradient::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity); true }
        EffectKind::Twinkle => { twinkle::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity); true }
        EffectKind::Fade => { fade::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity); true }
        EffectKind::Wipe => { wipe::evaluate_pixels_batch(t, dest, global_offset, total_pixels, params, blend_mode, opacity, positions); true }
        EffectKind::Script(_) => false,
    }
}
