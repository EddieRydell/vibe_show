#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::dispatcher::EditCommand;
use crate::error::AppError;
use crate::model::{ColorGradient, Curve, ParamValue, Sequence};
use crate::registry::params::{
    LinkEffectToLibraryParams, NameParams, RenameParams, SetLibraryCurveParams,
    SetLibraryGradientParams,
};
use crate::registry::CommandOutput;
use crate::state::AppState;

/// Count how many effects in the sequence reference a library item by name.
fn count_refs(seq: &Sequence, name: &str, pred: fn(&ParamValue, &str) -> bool) -> usize {
    seq.tracks
        .iter()
        .flat_map(|t| &t.effects)
        .filter(|e| e.params.inner().values().any(|v| pred(v, name)))
        .count()
}

/// Execute an `EditCommand` via the dispatcher.
fn dispatch(state: &Arc<AppState>, cmd: &EditCommand) -> Result<(), AppError> {
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, cmd)?;
    Ok(())
}

pub fn list_library(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let seq = show.sequences.first().ok_or(AppError::NoSequence)?;
    let gradients: Vec<&String> = seq.gradient_library.keys().collect();
    let curves: Vec<&String> = seq.curve_library.keys().collect();
    let scripts: Vec<&String> = seq.scripts.keys().collect();
    let data = serde_json::json!({
        "gradients": gradients,
        "curves": curves,
        "scripts": scripts,
    });
    Ok(CommandOutput::data(
        serde_json::to_string(&data).unwrap_or_default(),
        data,
    ))
}

pub fn set_library_gradient(
    state: &Arc<AppState>,
    p: SetLibraryGradientParams,
) -> Result<CommandOutput, AppError> {
    let gradient = ColorGradient::new(p.stops).ok_or(AppError::ValidationError {
        message: "Gradient needs at least 1 stop".into(),
    })?;
    let seq_idx = state.active_sequence_index(&state.show.lock())?;
    dispatch(state, &EditCommand::SetGradient {
        sequence_index: seq_idx,
        name: p.name.clone(),
        gradient,
    })?;
    Ok(CommandOutput::unit(format!("Gradient \"{}\" saved.", p.name)))
}

pub fn set_library_curve(
    state: &Arc<AppState>,
    p: SetLibraryCurveParams,
) -> Result<CommandOutput, AppError> {
    let curve = Curve::new(p.points).ok_or(AppError::ValidationError {
        message: "Curve needs at least 2 points".into(),
    })?;
    let seq_idx = state.active_sequence_index(&state.show.lock())?;
    dispatch(state, &EditCommand::SetCurve {
        sequence_index: seq_idx,
        name: p.name.clone(),
        curve,
    })?;
    Ok(CommandOutput::unit(format!("Curve \"{}\" saved.", p.name)))
}

/// Check for dangling references before deleting a library item.
fn check_no_refs(
    state: &Arc<AppState>,
    name: &str,
    kind: &str,
    pred: fn(&ParamValue, &str) -> bool,
) -> Result<usize, AppError> {
    let show = state.show.lock();
    let seq_idx = state.active_sequence_index(&show)?;
    if let Some(seq) = show.sequences.get(seq_idx) {
        let ref_count = count_refs(seq, name, pred);
        if ref_count > 0 {
            return Err(AppError::ValidationError {
                message: format!(
                    "Cannot delete {kind} \"{name}\": it is referenced by {ref_count} effect(s). Remove the references first.",
                ),
            });
        }
    }
    state.active_sequence_index(&show)
}

pub fn delete_library_gradient(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let seq_idx = check_no_refs(state, &p.name, "gradient", |v, n| {
        matches!(v, ParamValue::GradientRef(r) if r == n)
    })?;
    dispatch(state, &EditCommand::DeleteGradient {
        sequence_index: seq_idx,
        name: p.name.clone(),
    })?;
    Ok(CommandOutput::unit(format!(
        "Gradient \"{}\" deleted.",
        p.name
    )))
}

pub fn delete_library_curve(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let seq_idx = check_no_refs(state, &p.name, "curve", |v, n| {
        matches!(v, ParamValue::CurveRef(r) if r == n)
    })?;
    dispatch(state, &EditCommand::DeleteCurve {
        sequence_index: seq_idx,
        name: p.name.clone(),
    })?;
    Ok(CommandOutput::unit(format!("Curve \"{}\" deleted.", p.name)))
}

pub fn list_library_gradients(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let gradients: Vec<(String, ColorGradient)> = show
        .sequences
        .first()
        .map(|seq| {
            seq.gradient_library
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    Ok(CommandOutput::json(
        format!("{} gradients.", gradients.len()),
        &gradients,
    ))
}

pub fn list_library_curves(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let curves: Vec<(String, Curve)> = show
        .sequences
        .first()
        .map(|seq| {
            seq.curve_library
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    Ok(CommandOutput::json(
        format!("{} curves.", curves.len()),
        &curves,
    ))
}

pub fn rename_library_gradient(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    let seq_idx = state.active_sequence_index(&state.show.lock())?;
    dispatch(state, &EditCommand::RenameGradient {
        sequence_index: seq_idx,
        old_name: p.old_name,
        new_name: p.new_name.clone(),
    })?;
    Ok(CommandOutput::unit(format!(
        "Gradient renamed to \"{}\".",
        p.new_name
    )))
}

pub fn rename_library_curve(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    let seq_idx = state.active_sequence_index(&state.show.lock())?;
    dispatch(state, &EditCommand::RenameCurve {
        sequence_index: seq_idx,
        old_name: p.old_name,
        new_name: p.new_name.clone(),
    })?;
    Ok(CommandOutput::unit(format!(
        "Curve renamed to \"{}\".",
        p.new_name
    )))
}

#[allow(clippy::cast_possible_truncation)]
pub fn link_effect_to_library(
    state: &Arc<AppState>,
    p: LinkEffectToLibraryParams,
) -> Result<CommandOutput, AppError> {
    let param_value = match p.ref_type.as_str() {
        "gradient" => ParamValue::GradientRef(p.library_name.clone()),
        "curve" => ParamValue::CurveRef(p.library_name.clone()),
        _ => {
            return Err(AppError::ValidationError {
                message: format!(
                    "Invalid ref_type: {}. Use 'gradient' or 'curve'.",
                    p.ref_type
                ),
            })
        }
    };
    let key = serde_json::from_value(serde_json::json!(p.key))
        .map_err(|e| AppError::ValidationError {
            message: format!("Invalid param key: {e}"),
        })?;
    let seq_idx = state.active_sequence_index(&state.show.lock())?;
    dispatch(state, &EditCommand::UpdateEffectParam {
        sequence_index: seq_idx,
        track_index: p.track_index,
        effect_index: p.effect_index,
        key,
        value: param_value,
    })?;
    Ok(CommandOutput::unit(format!(
        "Linked to {} \"{}\".",
        p.ref_type, p.library_name
    )))
}
