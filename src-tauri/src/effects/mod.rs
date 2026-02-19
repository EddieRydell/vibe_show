pub mod chase;
pub mod gradient;
pub mod rainbow;
pub mod solid;
pub mod strobe;
pub mod twinkle;

use crate::model::{Color, EffectKind, EffectParams, ParamSchema};

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
    }
}
