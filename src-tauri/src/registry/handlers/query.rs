#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::Value;

use crate::describe;
use crate::effects::{self, resolve_effect};
use crate::error::AppError;
use crate::model::EffectKind;
use crate::registry::params::GetEffectDetailParams;
use crate::registry::reference;
use crate::registry::CommandOutput;
use crate::state::{AppState, EffectDetail, EffectInfo};

pub fn get_show(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let summary = describe::summarize_show(&show);
    Ok(CommandOutput::data(summary, serde_json::to_value(&*show).unwrap_or_default()))
}

pub fn get_effect_catalog(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let mut catalog = Vec::new();

    // Built-in effects
    for kind in EffectKind::all_builtin() {
        if let Some(effect) = effects::resolve_effect(kind) {
            let schemas = effect.param_schema();
            let params: Vec<Value> = schemas
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "key": format!("{:?}", s.key),
                        "label": s.label,
                        "type": format!("{:?}", s.param_type),
                        "default": s.default,
                    })
                })
                .collect();
            catalog.push(serde_json::json!({
                "kind": format!("{kind:?}"),
                "name": effect.name(),
                "type": "builtin",
                "params": params,
            }));
        }
    }

    // Script effects
    if let Some(seq) = show.sequences.first() {
        for (script_name, source) in &seq.scripts {
            let mut entry = serde_json::json!({
                "kind": format!("Script(\"{script_name}\")"),
                "name": script_name,
                "type": "script",
            });
            if let Ok(compiled) = crate::dsl::compile_source(source) {
                let params: Vec<Value> = compiled
                    .params
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "key": format!("Custom(\"{}\")", p.name),
                            "label": p.name,
                            "type": format!("{:?}", p.ty),
                        })
                    })
                    .collect();
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("params".to_string(), Value::Array(params));
                }
            }
            catalog.push(entry);
        }
    }

    Ok(CommandOutput::data(
        serde_json::to_string_pretty(&catalog).unwrap_or_default(),
        Value::Array(catalog),
    ))
}

pub fn get_design_guide() -> Result<CommandOutput, AppError> {
    let guide = reference::design_guide();
    Ok(CommandOutput::data(
        guide.clone(),
        serde_json::json!(guide),
    ))
}

pub fn list_effects() -> Result<CommandOutput, AppError> {
    let effects: Vec<EffectInfo> = crate::state::all_effect_info();
    let mut lines = vec![format!("{} effect types available:", effects.len())];
    for e in &effects {
        let param_names: Vec<&str> = e.schema.iter().map(|s| s.label.as_str()).collect();
        lines.push(format!(
            "  - {:?}: {} (params: {})",
            e.kind,
            e.name,
            if param_names.is_empty() { "none".to_string() } else { param_names.join(", ") }
        ));
    }
    Ok(CommandOutput::data(lines.join("\n"), serde_json::to_value(&effects).unwrap_or_default()))
}

pub fn get_effect_detail(
    state: &Arc<AppState>,
    p: GetEffectDetailParams,
) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let sequence = show
        .sequences
        .get(p.sequence_index)
        .ok_or(AppError::InvalidIndex {
            what: "sequence".into(),
            index: p.sequence_index,
        })?;
    let track = sequence
        .tracks
        .get(p.track_index)
        .ok_or(AppError::InvalidIndex {
            what: "track".into(),
            index: p.track_index,
        })?;
    let effect_instance =
        track
            .effects
            .get(p.effect_index)
            .ok_or(AppError::InvalidIndex {
                what: "effect".into(),
                index: p.effect_index,
            })?;

    let schema = resolve_effect(&effect_instance.kind)
        .map_or_else(Vec::new, |e| e.param_schema());

    let detail = EffectDetail {
        kind: effect_instance.kind.clone(),
        schema,
        params: effect_instance.params.clone(),
        time_range: effect_instance.time_range,
        track_name: track.name.clone(),
        blend_mode: effect_instance.blend_mode,
        opacity: effect_instance.opacity,
    };

    let effect_desc = crate::describe::describe_effect(effect_instance);
    let message = format!(
        "Effect on track \"{}\" [{}:{}]: {}",
        track.name, p.track_index, p.effect_index, effect_desc
    );
    Ok(CommandOutput::data(message, serde_json::to_value(&detail).unwrap_or_default()))
}
