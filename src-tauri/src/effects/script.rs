use crate::dsl::compiler::CompiledScript;
use crate::dsl::vm::{self, VmContext};
use crate::model::color::Color;
use crate::model::color_gradient::ColorGradient;
use crate::model::curve::Curve;
use crate::model::timeline::{BlendMode, EffectParams, ParamKey, ParamValue};
use crate::model::show::Position2D;

use crate::dsl::ast;

/// Evaluate a compiled DSL script for a batch of pixels, blending into `dest`.
///
/// This mirrors the signature of native `evaluate_pixels_batch` functions.
/// `positions` is provided for spatial scripts (`@spatial true`).
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments, clippy::indexing_slicing)]
pub fn evaluate_pixels_batch(
    script: &CompiledScript,
    t: f64,
    dest: &mut [Color],
    global_offset: usize,
    total_pixels: usize,
    params: &EffectParams,
    blend_mode: BlendMode,
    opacity: f64,
    positions: Option<&[Position2D]>,
) {
    // Build runtime param arrays from EffectParams.
    // Each compiled param maps to a slot by index.
    let param_count = script.params.len();
    let mut param_values = vec![0.0f64; param_count];
    let mut gradients_owned: Vec<Option<ColorGradient>> = vec![None; param_count];
    let mut curves_owned: Vec<Option<Curve>> = vec![None; param_count];

    for (i, cp) in script.params.iter().enumerate() {
        let key = ParamKey::Custom(cp.name.clone());
        if let Some(val) = params.get(key) {
            match val {
                ParamValue::Float(f) => param_values[i] = *f,
                ParamValue::Int(n) => param_values[i] = f64::from(*n),
                ParamValue::Bool(b) => param_values[i] = if *b { 1.0 } else { 0.0 },
                ParamValue::ColorGradient(g) => gradients_owned[i] = Some(g.clone()),
                ParamValue::Curve(c) => curves_owned[i] = Some(c.clone()),
                ParamValue::EnumVariant(variant_name) => {
                    // Resolve variant name to index
                    if let ast::ParamType::Named(ref type_name) = cp.ty {
                        // For enum params, look up variant index
                        // The variant name maps to its position in the enum definition
                        // This info was captured in the compiled param
                        param_values[i] = resolve_enum_variant(script, type_name, variant_name);
                    }
                }
                ParamValue::FlagSet(flags) => {
                    if let ast::ParamType::Named(ref type_name) = cp.ty {
                        param_values[i] = resolve_flag_set(script, type_name, flags);
                    }
                }
                _ => {}
            }
        } else {
            // Use default from the script's param definition
            // (The defaults are already embedded in the AST; for now, 0.0 is fine)
        }
    }

    // Build reference arrays for the VM context
    let gradient_refs: Vec<Option<&ColorGradient>> = gradients_owned
        .iter()
        .map(|g| g.as_ref())
        .collect();
    let curve_refs: Vec<Option<&Curve>> = curves_owned
        .iter()
        .map(|c| c.as_ref())
        .collect();

    for (local_idx, pixel) in dest.iter_mut().enumerate() {
        let global_idx = global_offset + local_idx;
        let pos = if total_pixels > 1 {
            global_idx as f64 / (total_pixels - 1) as f64
        } else {
            0.0
        };

        let pos2d = positions
            .and_then(|p| p.get(local_idx))
            .map_or((pos, 0.0), |p| (f64::from(p.x), f64::from(p.y)));

        let ctx = VmContext {
            t,
            pixel: global_idx,
            pixels: total_pixels,
            pos,
            pos2d,
            param_values: &param_values,
            gradients: &gradient_refs,
            curves: &curve_refs,
        };

        let mut color = vm::execute(script, &ctx);
        if opacity < 1.0 {
            color = color.scale(opacity);
        }
        *pixel = pixel.blend(color, blend_mode);
    }
}

/// Resolve an enum variant name to its integer index.
/// Returns index 0 if the variant or type is not found (with an eprintln warning).
#[allow(clippy::cast_precision_loss)]
fn resolve_enum_variant(script: &CompiledScript, type_name: &str, variant_name: &str) -> f64 {
    for enum_def in &script.enums {
        if enum_def.name == type_name {
            if let Some(idx) = enum_def.variants.iter().position(|v| v == variant_name) {
                return idx as f64;
            }
            eprintln!(
                "[VibeLights DSL] Unknown variant '{variant_name}' for enum '{type_name}'; \
                 valid variants: {:?}. Falling back to index 0.",
                enum_def.variants
            );
            return 0.0;
        }
    }
    eprintln!(
        "[VibeLights DSL] Unknown enum type '{type_name}' in script '{}'. Falling back to 0.",
        script.name
    );
    0.0
}

/// Resolve a flag set to a bitmask value.
/// Unknown flags are skipped with a warning.
fn resolve_flag_set(script: &CompiledScript, type_name: &str, flags: &[String]) -> f64 {
    for flags_def in &script.flags {
        if flags_def.name == type_name {
            let mut mask: u32 = 0;
            for flag in flags {
                if let Some(idx) = flags_def.flags.iter().position(|f| f == flag) {
                    mask |= 1u32 << idx;
                } else {
                    eprintln!(
                        "[VibeLights DSL] Unknown flag '{flag}' for flags type '{type_name}'; \
                         valid flags: {:?}. Skipping.",
                        flags_def.flags
                    );
                }
            }
            return f64::from(mask);
        }
    }
    eprintln!(
        "[VibeLights DSL] Unknown flags type '{type_name}' in script '{}'. Falling back to 0.",
        script.name
    );
    0.0
}
