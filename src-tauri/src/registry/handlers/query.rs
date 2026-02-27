#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::describe;
use crate::effects::resolve_effect;
use crate::error::AppError;
use crate::registry::params::{GetEffectDetailParams, HelpParams};
use crate::registry::{catalog, reference};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{AppState, EffectDetail, EffectInfo};

pub fn get_show(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let summary = describe::summarize_show(&show);
    Ok(CommandOutput::new(summary, CommandResult::GetShow(Box::new(show.clone()))))
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

pub fn describe_show(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let mut text = describe::describe_show(&show);
    if let Some(seq) = show.sequences.first() {
        text.push('\n');
        text.push('\n');
        text.push_str(&describe::describe_sequence(seq));
    }
    Ok(CommandOutput::new(text.clone(), CommandResult::DescribeShow(text)))
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

pub fn help(_state: &Arc<AppState>, p: HelpParams) -> Result<CommandOutput, AppError> {
    let text = catalog::help_text(p.topic.as_deref());
    Ok(CommandOutput::new(text.clone(), CommandResult::Help(text)))
}
