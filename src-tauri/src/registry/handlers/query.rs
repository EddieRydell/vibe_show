#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use crate::describe;
use crate::effects::{self, resolve_effect};
use crate::error::AppError;
use crate::model::EffectKind;
use crate::registry::params::GetEffectDetailParams;
use crate::registry::reference;
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{AppState, EffectDetail, EffectInfo};

/// A single entry in the effect catalog.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct EffectCatalogEntry {
    pub kind: String,
    pub name: String,
    #[serde(rename = "type")]
    pub effect_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<EffectCatalogParam>>,
}

/// A parameter in an effect catalog entry.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct EffectCatalogParam {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<crate::model::ParamValue>,
}

pub fn get_show(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let summary = describe::summarize_show(&show);
    Ok(CommandOutput::new(summary, CommandResult::GetShow(Box::new(show.clone()))))
}

pub fn get_effect_catalog(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let _show = state.show.lock();
    let mut catalog: Vec<EffectCatalogEntry> = Vec::new();

    // Built-in effects
    for kind in EffectKind::all_builtin() {
        if let Some(effect) = effects::resolve_effect(kind) {
            let params: Vec<EffectCatalogParam> = effect
                .param_schema()
                .iter()
                .map(|s| EffectCatalogParam {
                    key: format!("{:?}", s.key),
                    label: s.label.clone(),
                    param_type: format!("{:?}", s.param_type),
                    default: Some(s.default.clone()),
                })
                .collect();
            catalog.push(EffectCatalogEntry {
                kind: format!("{kind:?}"),
                name: effect.name().to_string(),
                effect_type: "builtin".to_string(),
                params: Some(params),
            });
        }
    }

    // Script effects from global library
    {
        let libs = state.global_libraries.lock();
        for (script_name, source) in &libs.scripts {
            let params = crate::dsl::compile_source(source).ok().map(|compiled| {
                compiled
                    .params
                    .iter()
                    .map(|p| EffectCatalogParam {
                        key: format!("Custom(\"{}\")", p.name),
                        label: p.name.clone(),
                        param_type: format!("{:?}", p.ty),
                        default: None,
                    })
                    .collect()
            });
            catalog.push(EffectCatalogEntry {
                kind: format!("Script(\"{script_name}\")"),
                name: script_name.clone(),
                effect_type: "script".to_string(),
                params,
            });
        }
    }

    Ok(CommandOutput::new(
        serde_json::to_string_pretty(&catalog).unwrap_or_default(),
        CommandResult::GetEffectCatalog(catalog),
    ))
}

pub fn get_design_guide(_state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let guide = reference::design_guide();
    Ok(CommandOutput::new(guide.clone(), CommandResult::GetDesignGuide(guide)))
}

pub fn list_effects(_state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
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
    Ok(CommandOutput::new(lines.join("\n"), CommandResult::ListEffects(effects)))
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
    Ok(CommandOutput::new(message, CommandResult::GetEffectDetail(detail)))
}
