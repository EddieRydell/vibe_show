#![allow(
    clippy::needless_pass_by_value,
    clippy::unreachable
)]

use std::sync::Arc;

use tauri::State;

use crate::dsl;
use crate::error::AppError;
use crate::state::AppState;

// ── Unified command registry ─────────────────────────────────────

/// Execute any Command through the unified registry.
/// Async commands are awaited directly. Sync commands run on `spawn_blocking`
/// to keep the Tauri event loop responsive.
#[tauri::command]
pub async fn exec(
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    cmd: crate::registry::Command,
) -> Result<crate::registry::CommandResult, AppError> {
    let state_arc = (*state).clone();
    if cmd.is_async() {
        let output = cmd.dispatch_async(state_arc, Some(app_handle)).await?;
        Ok(output.result)
    } else {
        let output = tokio::task::spawn_blocking(move || {
            cmd.dispatch(&state_arc)
        })
        .await
        .map_err(|e| AppError::ApiError { message: e.to_string() })??;
        Ok(output.result)
    }
}

/// Get the full command registry with schemas.
#[tauri::command]
pub fn get_command_registry() -> serde_json::Value {
    crate::registry::catalog::to_json_schema()
}

// ── Types ────────────────────────────────────────────────────────

pub use crate::state::EffectDetail;

#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TickResult {
    pub frame: crate::engine::Frame,
    pub current_time: f64,
    pub playing: bool,
}

#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptCompileResult {
    pub success: bool,
    pub errors: Vec<ScriptError>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<ScriptParamInfo>>,
}

#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptParamInfo {
    pub name: String,
    pub param_type: crate::model::timeline::ParamType,
    pub default: Option<crate::model::timeline::ParamValue>,
}

#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptPreviewData {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ScriptError {
    pub message: String,
    pub offset: usize,
}

#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct EffectThumbnail {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub start_time: f64,
    pub end_time: f64,
}

// ── Helper functions (used by registry handlers) ─────────────────

/// Recompile all scripts from the global library
/// (e.g., after loading a show from disk).
/// Returns a list of scripts that failed to compile.
pub fn recompile_all_scripts(state: &AppState) -> Vec<String> {
    let sources: Vec<(String, String)> = {
        let libs = state.global_libraries.lock();
        libs.scripts.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    };

    let mut failures = Vec::new();
    let mut cache = state.script_cache.lock();
    cache.clear();

    for (name, source) in sources {
        match dsl::compile_source(&source) {
            Ok(compiled) => {
                cache.insert(name, std::sync::Arc::new(compiled));
            }
            Err(_) => {
                failures.push(name);
            }
        }
    }

    failures
}

/// Convert DSL `CompiledParam` entries to model-layer `ScriptParamInfo`.
pub fn extract_script_params(compiled: &dsl::compiler::CompiledScript) -> Vec<ScriptParamInfo> {
    compiled
        .params
        .iter()
        .map(|cp| {
            let (param_type, default) = dsl_param_to_model(cp, compiled);
            ScriptParamInfo {
                name: cp.name.clone(),
                param_type,
                default,
            }
        })
        .collect()
}

/// Extract the default `ParamValue` from the DSL default expression.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn eval_param_default(
    cp: &dsl::compiler::CompiledParam,
    compiled: &dsl::compiler::CompiledScript,
) -> Option<crate::model::timeline::ParamValue> {
    use crate::dsl::ast::{ExprKind, ParamType as Dsl};
    use crate::model::timeline::ParamValue;

    match (&cp.ty, &cp.default.kind) {
        // Float: literal number
        (Dsl::Float(_), ExprKind::FloatLit(v)) => Some(ParamValue::Float(*v)),
        (Dsl::Float(_), ExprKind::IntLit(v)) => Some(ParamValue::Float(f64::from(*v))),
        // Int: literal number
        (Dsl::Int(_), ExprKind::IntLit(v)) => Some(ParamValue::Int(*v)),
        (Dsl::Int(_), ExprKind::FloatLit(v)) => Some(ParamValue::Int(*v as i32)),
        // Bool: literal
        (Dsl::Bool, ExprKind::BoolLit(v)) => Some(ParamValue::Bool(*v)),
        // Color: hex literal
        (Dsl::Color, ExprKind::ColorLit { r, g, b }) => {
            Some(ParamValue::Color(crate::model::color::Color {
                r: *r,
                g: *g,
                b: *b,
                a: 255,
            }))
        }
        // Gradient: gradient literal with color stops
        (Dsl::Gradient, ExprKind::GradientLit(stops)) => {
            let count = stops.len();
            let model_stops: Vec<crate::model::color_gradient::ColorStop> = stops
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let position = s.position.unwrap_or_else(|| {
                        if count <= 1 {
                            0.0
                        } else {
                            i as f64 / (count - 1) as f64
                        }
                    });
                    crate::model::color_gradient::ColorStop {
                        position,
                        color: crate::model::color::Color {
                            r: s.color.0,
                            g: s.color.1,
                            b: s.color.2,
                            a: 255,
                        },
                    }
                })
                .collect();
            crate::model::color_gradient::ColorGradient::new(model_stops)
                .map(ParamValue::ColorGradient)
        }
        // Curve: curve literal with control points
        (Dsl::Curve, ExprKind::CurveLit(points)) => {
            let model_points: Vec<crate::model::curve::CurvePoint> = points
                .iter()
                .map(|(x, y)| crate::model::curve::CurvePoint { x: *x, y: *y })
                .collect();
            crate::model::curve::Curve::new(model_points).map(ParamValue::Curve)
        }
        // Enum: identifier (variant name)
        (Dsl::Named(type_name), ExprKind::Ident(variant)) => {
            // Verify it's an enum
            if compiled.enums.iter().any(|e| e.name == *type_name) {
                Some(ParamValue::EnumVariant(variant.clone()))
            } else {
                None
            }
        }
        // Flags: flag combination
        (Dsl::Named(_), ExprKind::FlagCombine(flags)) => {
            Some(ParamValue::FlagSet(flags.clone()))
        }
        _ => None,
    }
}

/// Map a DSL `CompiledParam` to model-layer `ParamType` + optional default `ParamValue`.
#[allow(clippy::cast_possible_truncation)]
pub fn dsl_param_to_model(
    cp: &dsl::compiler::CompiledParam,
    compiled: &dsl::compiler::CompiledScript,
) -> (
    crate::model::timeline::ParamType,
    Option<crate::model::timeline::ParamValue>,
) {
    use crate::dsl::ast::ParamType as Dsl;
    use crate::model::timeline::{ParamType as Model, ParamValue};

    let default = eval_param_default(cp, compiled);

    match &cp.ty {
        Dsl::Float(range) => {
            let (min, max) = range.unwrap_or((0.0, 1.0));
            (
                Model::Float {
                    min,
                    max,
                    step: 0.01,
                },
                default.or(Some(ParamValue::Float(min))),
            )
        }
        Dsl::Int(range) => {
            let (min, max) = range.unwrap_or((0, 100));
            (Model::Int { min, max }, default.or(Some(ParamValue::Int(min))))
        }
        Dsl::Bool => (Model::Bool, default.or(Some(ParamValue::Bool(false)))),
        Dsl::Color => (
            Model::Color,
            default.or(Some(ParamValue::Color(crate::model::color::Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }))),
        ),
        Dsl::Gradient => (
            Model::ColorGradient {
                min_stops: 2,
                max_stops: 8,
            },
            default,
        ),
        Dsl::Curve => (Model::Curve, default),
        Dsl::Path => (Model::Path, None),
        Dsl::Named(type_name) => {
            // Check enums first, then flags
            for e in &compiled.enums {
                if e.name == *type_name {
                    return (
                        Model::Enum {
                            options: e.variants.clone(),
                        },
                        default.or_else(|| {
                            e.variants.first().map(|v| ParamValue::EnumVariant(v.clone()))
                        }),
                    );
                }
            }
            for f in &compiled.flags {
                if f.name == *type_name {
                    return (
                        Model::Flags {
                            options: f.flags.clone(),
                        },
                        default.or(Some(ParamValue::FlagSet(vec![]))),
                    );
                }
            }
            // Fallback
            (Model::Bool, None)
        }
    }
}
