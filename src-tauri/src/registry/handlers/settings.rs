#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::Arc;

use crate::error::AppError;
use crate::registry::params::{InitializeDataDirParams, SetLlmConfigParams};
use crate::registry::{CommandOutput, CommandResult};
use crate::settings::{self, AppSettings, LlmConfigInfo, LlmProvider, LlmProviderConfig};
use crate::state::AppState;

pub fn get_settings(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let settings = state.settings.lock().clone();
    Ok(CommandOutput::new("Settings", CommandResult::GetSettings(settings)))
}

pub fn initialize_data_dir(
    state: &Arc<AppState>,
    p: InitializeDataDirParams,
) -> Result<CommandOutput, AppError> {
    let data_path = PathBuf::from(&p.data_dir);
    std::fs::create_dir_all(data_path.join("setups"))?;

    let new_settings = AppSettings::new(data_path);
    settings::save_settings(&state.app_config_dir, &new_settings)
        .map_err(|e| AppError::SettingsSaveError {
            message: e.to_string(),
        })?;

    *state.settings.lock() = Some(new_settings.clone());
    Ok(CommandOutput::new("Data directory initialized.", CommandResult::InitializeDataDir(new_settings)))
}

pub fn set_llm_config(
    state: &Arc<AppState>,
    p: SetLlmConfigParams,
) -> Result<CommandOutput, AppError> {
    let mut settings_guard = state.settings.lock();
    if let Some(ref mut s) = *settings_guard {
        s.llm = LlmProviderConfig {
            provider: p.provider,
            api_key: if p.api_key.is_empty() {
                None
            } else {
                Some(p.api_key.clone())
            },
            base_url: p.base_url,
            model: p.model,
        };
        settings::save_settings(&state.app_config_dir, s)
            .map_err(|e| AppError::SettingsSaveError {
                message: e.to_string(),
            })?;
        settings::save_api_key(&state.app_config_dir, &p.api_key)
            .map_err(|e| AppError::SettingsSaveError {
                message: e.to_string(),
            })?;
        // Kill agent sidecar when API key changes so it restarts with the new key
        if !p.api_key.is_empty() {
            state.agent_port.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
    Ok(CommandOutput::new("LLM config updated.", CommandResult::SetLlmConfig))
}

pub fn get_llm_config(state: &Arc<AppState>) -> Result<CommandOutput, AppError> {
    let info = state.settings.lock().as_ref().map_or(
        LlmConfigInfo {
            provider: LlmProvider::Anthropic,
            has_api_key: false,
            base_url: None,
            model: None,
        },
        |s| LlmConfigInfo::from_config(&s.llm),
    );
    Ok(CommandOutput::new("LLM config", CommandResult::GetLlmConfig(info)))
}
