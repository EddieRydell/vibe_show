use std::fmt;

use serde::Serialize;
use ts_rs::TS;

/// Structured error type for the application. Replaces stringly-typed errors
/// so the frontend can match on error codes and display appropriate UI.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "code", content = "detail")]
#[ts(export)]
pub enum AppError {
    NotFound { what: String },
    InvalidIndex { what: String, index: usize },
    ValidationError { message: String },
    IoError { message: String },
    NoProfile,
    NoSequence,
    NoSettings,
    ApiError { message: String },
    ImportError { message: String },
    SettingsSaveError { message: String },
    PythonNotReady,
    PythonError { message: String },
    AnalysisError { message: String },
    ModelNotInstalled { model: String },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound { what } => write!(f, "{what} not found"),
            AppError::InvalidIndex { what, index } => {
                write!(f, "Invalid {what} index: {index}")
            }
            AppError::ValidationError { message } => write!(f, "{message}"),
            AppError::IoError { message } => write!(f, "I/O error: {message}"),
            AppError::NoProfile => write!(f, "No profile loaded"),
            AppError::NoSequence => write!(f, "No sequence loaded"),
            AppError::NoSettings => write!(f, "Settings not initialized"),
            AppError::ApiError { message } => write!(f, "API error: {message}"),
            AppError::ImportError { message } => write!(f, "Import error: {message}"),
            AppError::SettingsSaveError { message } => {
                write!(f, "Failed to save settings: {message}")
            }
            AppError::PythonNotReady => write!(f, "Python environment not ready"),
            AppError::PythonError { message } => write!(f, "Python error: {message}"),
            AppError::AnalysisError { message } => write!(f, "Analysis error: {message}"),
            AppError::ModelNotInstalled { model } => {
                write!(f, "Required model not installed: {model}")
            }
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::IoError {
            message: e.to_string(),
        }
    }
}

impl From<crate::project::ProjectError> for AppError {
    fn from(e: crate::project::ProjectError) -> Self {
        match e {
            crate::project::ProjectError::Io(io_err) => AppError::IoError {
                message: io_err.to_string(),
            },
            crate::project::ProjectError::Json(json_err) => AppError::ValidationError {
                message: json_err.to_string(),
            },
            crate::project::ProjectError::InvalidProject(msg) => {
                AppError::ValidationError { message: msg }
            }
        }
    }
}

/// Allow converting AppError to String for backward compatibility and Tauri IPC.
impl From<AppError> for String {
    fn from(e: AppError) -> String {
        e.to_string()
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::ValidationError { message: s }
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::ValidationError {
            message: s.to_string(),
        }
    }
}
