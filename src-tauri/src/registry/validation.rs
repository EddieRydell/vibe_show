//! Shared validation helpers for registry command handlers.
//!
//! Centralizes common checks (time ranges, opacity, numeric bounds) so all
//! handlers produce consistent error messages.

use crate::error::AppError;

/// Validate a time range: both values must be finite, non-negative, and start < end.
pub fn validate_time_range(start: f64, end: f64) -> Result<(), AppError> {
    crate::model::TimeRange::new(start, end).ok_or_else(|| AppError::ValidationError {
        message: format!(
            "Invalid time range: start={start:.3}, end={end:.3}. Must be finite, start >= 0, start < end"
        ),
    })?;
    Ok(())
}

/// Validate that opacity is a finite number in [0.0, 1.0].
pub fn validate_opacity(opacity: f64) -> Result<(), AppError> {
    if !opacity.is_finite() {
        return Err(AppError::ValidationError {
            message: "Opacity must be finite".to_string(),
        });
    }
    if !(0.0..=1.0).contains(&opacity) {
        return Err(AppError::ValidationError {
            message: format!("Opacity ({opacity:.3}) must be between 0.0 and 1.0"),
        });
    }
    Ok(())
}

/// Validate that a value is finite and positive.
pub fn validate_positive_finite(value: f64, name: &str) -> Result<(), AppError> {
    if !value.is_finite() {
        return Err(AppError::ValidationError {
            message: format!("{name} must be finite"),
        });
    }
    if value <= 0.0 {
        return Err(AppError::ValidationError {
            message: format!("{name} must be positive"),
        });
    }
    Ok(())
}
