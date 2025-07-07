//! Error types for the metrics system
//!
//! This module defines comprehensive error handling for all metrics operations,
//! ensuring no unsafe operations that could panic in production.

use std::fmt;
use thiserror::Error;

/// Comprehensive error types for metrics operations
#[derive(Error, Debug, Clone)]
pub enum MetricsError {
    /// Invalid metric name (e.g., empty, invalid characters, wrong format)
    #[error("Invalid metric name '{name}': {reason}")]
    InvalidName { name: String, reason: String },

    /// Invalid label name or value
    #[error("Invalid label '{label}': {reason}")]
    InvalidLabel { label: String, reason: String },

    /// Metric registration failed (e.g., duplicate registration, internal error)
    #[error("Failed to register metric '{name}': {reason}")]
    RegistrationFailed { name: String, reason: String },

    /// Metric export failed
    #[error("Failed to export metrics: {reason}")]
    ExportFailed { reason: String },

    /// Value validation error (e.g., precision loss, out of range)
    #[error("Invalid value '{value}': {reason}")]
    InvalidValue { value: String, reason: String },

    /// Network or I/O error for exporters like StatsD
    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    /// Configuration error
    #[error("Configuration error: {reason}")]
    ConfigurationError { reason: String },

    /// Resource limit exceeded (e.g., too many metrics, memory limit)
    #[error("Resource limit exceeded: {reason}")]
    ResourceLimitExceeded { reason: String },
    /// Internal system error (should be rare)
    #[error("Internal error: {reason}")]
    #[allow(dead_code)] // Reserved for future use in panic handling
    InternalError { reason: String },
}

impl MetricsError {
    /// Create an invalid name error
    pub fn invalid_name<N: Into<String>, R: Into<String>>(name: N, reason: R) -> Self {
        Self::InvalidName {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create an invalid label error
    pub fn invalid_label<L: Into<String>, R: Into<String>>(label: L, reason: R) -> Self {
        Self::InvalidLabel {
            label: label.into(),
            reason: reason.into(),
        }
    }

    /// Create a registration failed error
    pub fn registration_failed<N: Into<String>, R: Into<String>>(name: N, reason: R) -> Self {
        Self::RegistrationFailed {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create an export failed error
    pub fn export_failed<R: Into<String>>(reason: R) -> Self {
        Self::ExportFailed {
            reason: reason.into(),
        }
    }

    /// Create an invalid value error
    pub fn invalid_value<V: fmt::Display, R: Into<String>>(value: V, reason: R) -> Self {
        Self::InvalidValue {
            value: value.to_string(),
            reason: reason.into(),
        }
    }

    /// Create a network error
    pub fn network_error<R: Into<String>>(reason: R) -> Self {
        Self::NetworkError {
            reason: reason.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration_error<R: Into<String>>(reason: R) -> Self {
        Self::ConfigurationError {
            reason: reason.into(),
        }
    }

    /// Create a resource limit exceeded error
    pub fn resource_limit_exceeded<R: Into<String>>(reason: R) -> Self {
        Self::ResourceLimitExceeded {
            reason: reason.into(),
        }
    }
    /// Create an internal error
    //#[allow(dead_code)]
    // TODO : reserved for future use in panic handling
    pub fn _internal_error<R: Into<String>>(reason: R) -> Self {
        Self::InternalError {
            reason: reason.into(),
        }
    }
}

/// Validation functions for metrics names and labels
pub mod validation {
    use super::MetricsError;
    use std::collections::HashSet;

    /// Prometheus reserved label names that cannot be used
    const PROMETHEUS_RESERVED_LABELS: &[&str] = &["__name__", "__value__"];

    /// Maximum label value length to prevent memory issues
    const MAX_LABEL_VALUE_LENGTH: usize = 1024;

    /// Maximum number of labels per metric to prevent resource exhaustion
    const MAX_LABELS_PER_METRIC: usize = 32;

    /// Validate metric name according to Prometheus rules
    ///
    /// Prometheus metric names must:
    /// - Not be empty
    /// - Start with a letter or underscore
    /// - Contain only letters, digits, underscores, and colons
    /// - Not exceed reasonable length limits
    pub fn validate_metric_name(name: &str) -> Result<(), MetricsError> {
        if name.is_empty() {
            return Err(MetricsError::invalid_name(
                name,
                "Metric name cannot be empty",
            ));
        }

        if name.len() > 512 {
            return Err(MetricsError::invalid_name(
                name,
                "Metric name too long (max 512 characters)",
            ));
        }

        // Check first character
        let first_char = name.chars().next().unwrap(); // Safe because we checked empty above
        if !first_char.is_alphabetic() && first_char != '_' {
            return Err(MetricsError::invalid_name(
                name,
                "Metric name must start with a letter or underscore",
            ));
        }

        // Check all characters
        for (i, ch) in name.chars().enumerate() {
            if !ch.is_alphanumeric() && ch != '_' && ch != ':' {
                return Err(MetricsError::invalid_name(
                    name,
                    format!("Invalid character '{}' at position {}", ch, i),
                ));
            }
        }

        Ok(())
    }

    /// Validate label key according to Prometheus rules
    pub fn validate_label_key(key: &str) -> Result<(), MetricsError> {
        if key.is_empty() {
            return Err(MetricsError::invalid_label(
                key,
                "Label key cannot be empty",
            ));
        }

        if key.len() > 256 {
            return Err(MetricsError::invalid_label(
                key,
                "Label key too long (max 256 characters)",
            ));
        }

        // Check for reserved labels
        if PROMETHEUS_RESERVED_LABELS.contains(&key) {
            return Err(MetricsError::invalid_label(
                key,
                "Label key is reserved by Prometheus",
            ));
        }

        // Prometheus label names must start with letter or underscore
        let first_char = key.chars().next().unwrap(); // Safe because we checked empty above
        if !first_char.is_alphabetic() && first_char != '_' {
            return Err(MetricsError::invalid_label(
                key,
                "Label key must start with a letter or underscore",
            ));
        }

        // Check all characters
        for (i, ch) in key.chars().enumerate() {
            if !ch.is_alphanumeric() && ch != '_' {
                return Err(MetricsError::invalid_label(
                    key,
                    format!("Invalid character '{}' at position {}", ch, i),
                ));
            }
        }

        Ok(())
    }

    /// Validate label value
    pub fn validate_label_value(value: &str) -> Result<(), MetricsError> {
        if value.len() > MAX_LABEL_VALUE_LENGTH {
            return Err(MetricsError::invalid_label(
                value,
                format!(
                    "Label value too long (max {} characters)",
                    MAX_LABEL_VALUE_LENGTH
                ),
            ));
        }

        // Check for invalid control characters
        for (i, ch) in value.chars().enumerate() {
            if ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' {
                return Err(MetricsError::invalid_label(
                    value,
                    format!("Invalid control character at position {}", i),
                ));
            }
        }

        Ok(())
    }

    /// Validate all labels in a slice
    pub fn validate_labels(labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        if labels.len() > MAX_LABELS_PER_METRIC {
            return Err(MetricsError::invalid_label(
                "",
                format!("Too many labels (max {} allowed)", MAX_LABELS_PER_METRIC),
            ));
        }

        let mut seen_keys = HashSet::new();

        for (key, value) in labels {
            // Validate key
            validate_label_key(key)?;

            // Validate value
            validate_label_value(value)?;

            // Check for duplicate keys
            if !seen_keys.insert(key) {
                return Err(MetricsError::invalid_label(*key, "Duplicate label key"));
            }
        }

        Ok(())
    }

    /// Validate numeric value for potential precision loss
    pub fn validate_numeric_value(value: f64) -> Result<(), MetricsError> {
        if !value.is_finite() {
            return Err(MetricsError::invalid_value(
                value,
                "Value must be finite (not NaN or infinite)",
            ));
        }

        // Check for potential precision issues with very large values
        if value.abs() > (1u64 << 53) as f64 {
            return Err(MetricsError::invalid_value(
                value,
                "Value too large, may lose precision in f64",
            ));
        }

        Ok(())
    }

    /// Validate usize to f64 conversion for potential precision loss
    pub fn validate_usize_conversion(value: usize) -> Result<f64, MetricsError> {
        if value > (1u64 << 53) as usize {
            return Err(MetricsError::invalid_value(
                value,
                "Value too large for accurate f64 conversion",
            ));
        }

        Ok(value as f64)
    }
}

#[cfg(test)]
mod tests {

    use crate::metrics::error::validation::*;

    #[test]
    fn test_validate_metric_name() {
        // Valid names
        assert!(validate_metric_name("http_requests_total").is_ok());
        assert!(validate_metric_name("_internal_metric").is_ok());
        assert!(validate_metric_name("process:cpu_seconds_total").is_ok());

        // Invalid names
        assert!(validate_metric_name("").is_err());
        assert!(validate_metric_name("123invalid").is_err());
        assert!(validate_metric_name("invalid-name").is_err());
        assert!(validate_metric_name("invalid.name").is_err());
    }

    #[test]
    fn test_validate_label_key() {
        // Valid keys
        assert!(validate_label_key("method").is_ok());
        assert!(validate_label_key("_internal").is_ok());
        assert!(validate_label_key("endpoint_name").is_ok());

        // Invalid keys
        assert!(validate_label_key("").is_err());
        assert!(validate_label_key("__name__").is_err()); // Reserved
        assert!(validate_label_key("123invalid").is_err());
        assert!(validate_label_key("invalid-key").is_err());
    }

    #[test]
    fn test_validate_label_value() {
        // Valid values
        assert!(validate_label_value("POST").is_ok());
        assert!(validate_label_value("/api/v1/transcribe").is_ok());
        assert!(validate_label_value("").is_ok()); // Empty is allowed

        // Invalid values
        let long_value = "a".repeat(2000);
        assert!(validate_label_value(&long_value).is_err());
    }

    #[test]
    fn test_validate_labels() {
        // Valid labels
        assert!(validate_labels(&[("method", "POST"), ("endpoint", "/api")]).is_ok());

        // Duplicate keys
        assert!(validate_labels(&[("method", "POST"), ("method", "GET")]).is_err());

        // Too many labels
        let many_labels: Vec<(&str, &str)> = (0..50)
            .map(|i| {
                (
                    Box::leak(format!("key{}", i).into_boxed_str()) as &str,
                    "value",
                )
            })
            .collect();
        assert!(validate_labels(&many_labels).is_err());
    }

    #[test]
    fn test_validate_numeric_value() {
        // Valid values
        assert!(validate_numeric_value(42.0).is_ok());
        assert!(validate_numeric_value(0.0).is_ok());
        assert!(validate_numeric_value(-100.5).is_ok());

        // Invalid values
        assert!(validate_numeric_value(f64::NAN).is_err());
        assert!(validate_numeric_value(f64::INFINITY).is_err());
        assert!(validate_numeric_value(f64::NEG_INFINITY).is_err());
        assert!(validate_numeric_value(1e20).is_err()); // Too large
    }

    #[test]
    fn test_validate_usize_conversion() {
        // Valid conversions
        assert!(validate_usize_conversion(42).is_ok());
        assert!(validate_usize_conversion(1000000).is_ok());

        // Large value that would lose precision
        let large_value = (1u64 << 54) as usize;
        assert!(validate_usize_conversion(large_value).is_err());
    }
}
