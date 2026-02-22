#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use crate::dispatcher::EditCommand;
use crate::error::AppError;
use crate::model::{ColorGradient, Curve, ParamValue};
use crate::registry::params::{
    LinkEffectToLibraryParams, NameParams, RenameParams, SetLibraryCurveParams,
    SetLibraryGradientParams,
};
use crate::registry::CommandOutput;
use crate::state::AppState;

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
    let cmd = EditCommand::SetGradient {
        sequence_index: 0,
        name: p.name.clone(),
        gradient,
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!("Gradient \"{}\" saved.", p.name)))
}

pub fn set_library_curve(
    state: &Arc<AppState>,
    p: SetLibraryCurveParams,
) -> Result<CommandOutput, AppError> {
    let curve = Curve::new(p.points).ok_or(AppError::ValidationError {
        message: "Curve needs at least 2 points".into(),
    })?;
    let cmd = EditCommand::SetCurve {
        sequence_index: 0,
        name: p.name.clone(),
        curve,
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!("Curve \"{}\" saved.", p.name)))
}

pub fn delete_library_gradient(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let cmd = EditCommand::DeleteGradient {
        sequence_index: 0,
        name: p.name.clone(),
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!(
        "Gradient \"{}\" deleted.",
        p.name
    )))
}

pub fn delete_library_curve(
    state: &Arc<AppState>,
    p: NameParams,
) -> Result<CommandOutput, AppError> {
    let cmd = EditCommand::DeleteCurve {
        sequence_index: 0,
        name: p.name.clone(),
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd)?;
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
    let cmd = EditCommand::RenameGradient {
        sequence_index: 0,
        old_name: p.old_name,
        new_name: p.new_name.clone(),
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!(
        "Gradient renamed to \"{}\".",
        p.new_name
    )))
}

pub fn rename_library_curve(
    state: &Arc<AppState>,
    p: RenameParams,
) -> Result<CommandOutput, AppError> {
    let cmd = EditCommand::RenameCurve {
        sequence_index: 0,
        old_name: p.old_name,
        new_name: p.new_name.clone(),
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd)?;
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
    let cmd = EditCommand::UpdateEffectParam {
        sequence_index: 0,
        track_index: p.track_index,
        effect_index: p.effect_index,
        key,
        value: param_value,
    };
    let mut dispatcher = state.dispatcher.lock();
    let mut show = state.show.lock();
    dispatcher.execute(&mut show, &cmd)?;
    Ok(CommandOutput::unit(format!(
        "Linked to {} \"{}\".",
        p.ref_type, p.library_name
    )))
}
