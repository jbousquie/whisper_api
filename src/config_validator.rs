// Configuration validation module for Whisper API
//
// This module provides comprehensive validation for all configuration parameters
// and environment variables, ensuring early detection of configuration errors
// with clear, actionable error messages.

use std::env;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::str::FromStr;

use log::{debug, error, info, warn};

/// Configuration validation errors with detailed context
#[derive(Debug, Clone)]
pub struct ConfigValidationError {
    pub field: String,
    pub value: String,
    pub error_type: ConfigErrorType,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // TODO : some error types may be used in future validation extensions
pub enum ConfigErrorType {
    InvalidValue,
    InvalidFormat,
    InvalidRange,
    FileNotFound,
    DirectoryNotFound,
    PermissionDenied,
    NetworkError,
    Required,
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Configuration error in '{}' ({:?}): {} (value: '{}')",
            self.field, self.error_type, self.message, self.value
        )?;
        if let Some(suggestion) = &self.suggestion {
            write!(f, " - Suggestion: {}", suggestion)?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigValidationError {}

/// Result type for configuration validation
pub type ValidationResult<T> = Result<T, ConfigValidationError>;

/// Configuration validation results
#[derive(Debug)]
pub struct ValidationResults {
    pub errors: Vec<ConfigValidationError>,
    pub warnings: Vec<ConfigValidationError>,
    pub is_valid: bool,
}

impl ValidationResults {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            is_valid: true,
        }
    }

    pub fn add_error(&mut self, error: ConfigValidationError) {
        self.is_valid = false;
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: ConfigValidationError) {
        self.warnings.push(warning);
    }

    pub fn merge(&mut self, other: ValidationResults) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        if !other.is_valid {
            self.is_valid = false;
        }
    }

    pub fn print_summary(&self) {
        if !self.errors.is_empty() {
            error!(
                "Configuration validation found {} error(s):",
                self.errors.len()
            );
            for (i, err) in self.errors.iter().enumerate() {
                error!("  {}. {}", i + 1, err);
            }
        }

        if !self.warnings.is_empty() {
            warn!(
                "Configuration validation found {} warning(s):",
                self.warnings.len()
            );
            for (i, warn) in self.warnings.iter().enumerate() {
                warn!("  {}. {}", i + 1, warn);
            }
        }

        if self.is_valid && self.warnings.is_empty() {
            info!("Configuration validation passed successfully");
        } else if self.is_valid {
            info!(
                "Configuration validation passed with {} warning(s)",
                self.warnings.len()
            );
        }
    }
}

/// Helper functions for common validation patterns
pub mod validators {
    use super::*;

    /// Validate boolean values from string
    pub fn validate_boolean(field: &str, value: &str) -> ValidationResult<bool> {
        match value.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::InvalidValue,
                message: "Invalid boolean value".to_string(),
                suggestion: Some("Use 'true' or 'false'".to_string()),
            }),
        }
    }

    /// Validate integer values with optional range
    pub fn validate_integer(
        field: &str,
        value: &str,
        min: Option<i64>,
        max: Option<i64>,
    ) -> ValidationResult<i64> {
        let parsed = value.parse::<i64>().map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid integer format".to_string(),
            suggestion: Some("Use a valid integer number".to_string()),
        })?;

        if let Some(min) = min {
            if parsed < min {
                return Err(ConfigValidationError {
                    field: field.to_string(),
                    value: value.to_string(),
                    error_type: ConfigErrorType::InvalidRange,
                    message: format!("Value {} is below minimum {}", parsed, min),
                    suggestion: Some(format!("Use a value >= {}", min)),
                });
            }
        }

        if let Some(max) = max {
            if parsed > max {
                return Err(ConfigValidationError {
                    field: field.to_string(),
                    value: value.to_string(),
                    error_type: ConfigErrorType::InvalidRange,
                    message: format!("Value {} is above maximum {}", parsed, max),
                    suggestion: Some(format!("Use a value <= {}", max)),
                });
            }
        }

        Ok(parsed)
    }

    /// Validate unsigned integer values with optional range
    pub fn validate_usize(
        field: &str,
        value: &str,
        min: Option<usize>,
        max: Option<usize>,
    ) -> ValidationResult<usize> {
        let parsed = value.parse::<usize>().map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid unsigned integer format".to_string(),
            suggestion: Some("Use a valid positive integer number".to_string()),
        })?;

        if let Some(min) = min {
            if parsed < min {
                return Err(ConfigValidationError {
                    field: field.to_string(),
                    value: value.to_string(),
                    error_type: ConfigErrorType::InvalidRange,
                    message: format!("Value {} is below minimum {}", parsed, min),
                    suggestion: Some(format!("Use a value >= {}", min)),
                });
            }
        }

        if let Some(max) = max {
            if parsed > max {
                return Err(ConfigValidationError {
                    field: field.to_string(),
                    value: value.to_string(),
                    error_type: ConfigErrorType::InvalidRange,
                    message: format!("Value {} is above maximum {}", parsed, max),
                    suggestion: Some(format!("Use a value <= {}", max)),
                });
            }
        }

        Ok(parsed)
    }

    /// Validate enumerated values
    pub fn validate_enum(
        field: &str,
        value: &str,
        valid_values: &[&str],
        case_sensitive: bool,
    ) -> ValidationResult<String> {
        let check_value = if case_sensitive {
            value
        } else {
            &value.to_lowercase()
        };
        let valid_list: Vec<String> = if case_sensitive {
            valid_values.iter().map(|s| s.to_string()).collect()
        } else {
            valid_values.iter().map(|s| s.to_lowercase()).collect()
        };

        if valid_list.contains(&check_value.to_string()) {
            Ok(value.to_string())
        } else {
            Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::InvalidValue,
                message: format!("Invalid value, must be one of: {}", valid_values.join(", ")),
                suggestion: Some(format!("Use one of: {}", valid_values.join(", "))),
            })
        }
    }

    /// Validate IP address
    pub fn validate_ip_address(field: &str, value: &str) -> ValidationResult<IpAddr> {
        IpAddr::from_str(value).map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid IP address format".to_string(),
            suggestion: Some(
                "Use a valid IPv4 or IPv6 address (e.g., 127.0.0.1 or ::1)".to_string(),
            ),
        })
    }

    /// Validate port number
    pub fn validate_port(field: &str, value: &str) -> ValidationResult<u16> {
        let port = value.parse::<u16>().map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid port number format".to_string(),
            suggestion: Some("Use a number between 1 and 65535".to_string()),
        })?;

        if port == 0 {
            return Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::InvalidRange,
                message: "Port number cannot be 0".to_string(),
                suggestion: Some("Use a port between 1 and 65535".to_string()),
            });
        }

        Ok(port)
    }

    /// Validate socket address (host:port)
    pub fn validate_socket_address(field: &str, value: &str) -> ValidationResult<SocketAddr> {
        SocketAddr::from_str(value).map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid socket address format".to_string(),
            suggestion: Some("Use format 'host:port' (e.g., 127.0.0.1:8080)".to_string()),
        })
    }

    /// Validate file path exists
    pub fn validate_file_exists(field: &str, value: &str) -> ValidationResult<String> {
        let path = Path::new(value);
        if !path.exists() {
            return Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::FileNotFound,
                message: "File does not exist".to_string(),
                suggestion: Some("Ensure the file exists and the path is correct".to_string()),
            });
        }

        if !path.is_file() {
            return Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::InvalidValue,
                message: "Path exists but is not a file".to_string(),
                suggestion: Some("Ensure the path points to a file, not a directory".to_string()),
            });
        }

        Ok(value.to_string())
    }

    /// Validate directory path exists
    pub fn validate_directory_exists(field: &str, value: &str) -> ValidationResult<String> {
        let path = Path::new(value);
        if !path.exists() {
            return Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::DirectoryNotFound,
                message: "Directory does not exist".to_string(),
                suggestion: Some("Ensure the directory exists or create it".to_string()),
            });
        }

        if !path.is_dir() {
            return Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::InvalidValue,
                message: "Path exists but is not a directory".to_string(),
                suggestion: Some("Ensure the path points to a directory, not a file".to_string()),
            });
        }

        Ok(value.to_string())
    }

    /// Validate file size in bytes
    pub fn validate_file_size(field: &str, value: &str) -> ValidationResult<usize> {
        let size = value.parse::<usize>().map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid file size format".to_string(),
            suggestion: Some("Use a valid number of bytes (e.g., 536870912 for 512MB)".to_string()),
        })?;

        // Warn if size is very large (> 1GB)
        if size > 1_073_741_824 {
            warn!(
                "Large file size configured for {}: {} bytes ({}MB)",
                field,
                size,
                size / 1_048_576
            );
        }

        // Error if size is unreasonably large (> 10GB)
        if size > 10_737_418_240 {
            return Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::InvalidRange,
                message: "File size is unreasonably large (>10GB)".to_string(),
                suggestion: Some(
                    "Consider using a smaller limit to prevent resource exhaustion".to_string(),
                ),
            });
        }

        Ok(size)
    }
}

/// Get environment variable value with fallback to default
pub fn get_env_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Get optional environment variable value
pub fn get_env_optional(key: &str) -> Option<String> {
    env::var(key).ok()
}

/// Comprehensive configuration validator for Whisper API
pub struct WhisperConfigValidator;

impl WhisperConfigValidator {
    /// Validate all configuration parameters and return comprehensive results
    pub fn validate_all() -> ValidationResults {
        let mut results = ValidationResults::new();

        info!("Starting comprehensive configuration validation...");

        // Server Configuration
        results.merge(Self::validate_server_config());

        // Metrics Configuration
        results.merge(Self::validate_metrics_config());

        // File Storage Configuration
        results.merge(Self::validate_file_storage_config());

        // WhisperX Configuration
        results.merge(Self::validate_whisperx_config());

        // Job Management Configuration
        results.merge(Self::validate_job_management_config());

        // Upload Configuration
        results.merge(Self::validate_upload_config());

        // Concurrency Configuration
        results.merge(Self::validate_concurrency_config());

        // Security Configuration
        results.merge(Self::validate_security_config());

        // API Configuration
        results.merge(Self::validate_api_config());

        results.print_summary();
        results
    }

    /// Validate server configuration parameters
    fn validate_server_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating server configuration...");

        // Validate WHISPER_API_HOST
        if let Some(host) = get_env_optional("WHISPER_API_HOST") {
            if let Err(err) = validators::validate_ip_address("WHISPER_API_HOST", &host) {
                results.add_error(err);
            }
        }

        // Validate WHISPER_API_PORT
        if let Some(port) = get_env_optional("WHISPER_API_PORT") {
            if let Err(err) = validators::validate_port("WHISPER_API_PORT", &port) {
                results.add_error(err);
            }
        }

        // Validate WHISPER_API_TIMEOUT
        if let Some(timeout) = get_env_optional("WHISPER_API_TIMEOUT") {
            if let Err(err) =
                validators::validate_integer("WHISPER_API_TIMEOUT", &timeout, Some(1), Some(3600))
            {
                results.add_error(err);
            }
        }

        // Validate WHISPER_API_KEEPALIVE
        if let Some(keepalive) = get_env_optional("WHISPER_API_KEEPALIVE") {
            if let Err(err) = validators::validate_integer(
                "WHISPER_API_KEEPALIVE",
                &keepalive,
                Some(1),
                Some(3600),
            ) {
                results.add_error(err);
            }
        }

        // Validate HTTP_WORKER_NUMBER
        if let Some(workers) = get_env_optional("HTTP_WORKER_NUMBER") {
            if let Err(err) =
                validators::validate_usize("HTTP_WORKER_NUMBER", &workers, Some(0), Some(64))
            {
                results.add_error(err);
            }
        }

        results
    }

    /// Validate metrics configuration parameters
    fn validate_metrics_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating metrics configuration...");

        // Validate WHISPER_API_METRICS_ENABLED
        if let Some(enabled) = get_env_optional("WHISPER_API_METRICS_ENABLED") {
            if let Err(err) = validators::validate_boolean("WHISPER_API_METRICS_ENABLED", &enabled)
            {
                results.add_error(err);
            }
        }

        // Validate WHISPER_API_METRICS_BACKEND
        if let Some(backend) = get_env_optional("WHISPER_API_METRICS_BACKEND") {
            let valid_backends = &["prometheus", "statsd", "none", "disabled"];
            if let Err(err) = validators::validate_enum(
                "WHISPER_API_METRICS_BACKEND",
                &backend,
                valid_backends,
                false,
            ) {
                results.add_error(err);
            }
        }

        // Validate WHISPER_API_METRICS_ENDPOINT
        if let Some(endpoint) = get_env_optional("WHISPER_API_METRICS_ENDPOINT") {
            if let Err(err) =
                validators::validate_socket_address("WHISPER_API_METRICS_ENDPOINT", &endpoint)
            {
                results.add_error(err);
            }
        }

        // Validate WHISPER_API_METRICS_PREFIX
        if let Some(prefix) = get_env_optional("WHISPER_API_METRICS_PREFIX") {
            if prefix.is_empty() {
                results.add_warning(ConfigValidationError {
                    field: "WHISPER_API_METRICS_PREFIX".to_string(),
                    value: prefix,
                    error_type: ConfigErrorType::InvalidValue,
                    message: "Empty metrics prefix may cause naming conflicts".to_string(),
                    suggestion: Some(
                        "Consider using a descriptive prefix like 'whisper_api'".to_string(),
                    ),
                });
            }
        }

        // Validate WHISPER_API_METRICS_SAMPLE_RATE
        if let Some(rate) = get_env_optional("WHISPER_API_METRICS_SAMPLE_RATE") {
            match rate.parse::<f64>() {
                Ok(parsed_rate) => {
                    if !(0.0..=1.0).contains(&parsed_rate) {
                        results.add_error(ConfigValidationError {
                            field: "WHISPER_API_METRICS_SAMPLE_RATE".to_string(),
                            value: rate,
                            error_type: ConfigErrorType::InvalidRange,
                            message: "Sample rate must be between 0.0 and 1.0".to_string(),
                            suggestion: Some(
                                "Use a value like 0.1 (10%) or 1.0 (100%)".to_string(),
                            ),
                        });
                    }
                }
                Err(_) => {
                    results.add_error(ConfigValidationError {
                        field: "WHISPER_API_METRICS_SAMPLE_RATE".to_string(),
                        value: rate,
                        error_type: ConfigErrorType::InvalidFormat,
                        message: "Invalid sample rate format".to_string(),
                        suggestion: Some("Use a decimal number between 0.0 and 1.0".to_string()),
                    });
                }
            }
        }

        results
    }

    /// Validate file storage configuration parameters
    fn validate_file_storage_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating file storage configuration...");

        // Validate WHISPER_TMP_FILES
        if let Some(tmp_dir) = get_env_optional("WHISPER_TMP_FILES") {
            // Don't require directory to exist yet, but warn if parent doesn't exist
            let path = Path::new(&tmp_dir);
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    results.add_warning(ConfigValidationError {
                        field: "WHISPER_TMP_FILES".to_string(),
                        value: tmp_dir.clone(),
                        error_type: ConfigErrorType::DirectoryNotFound,
                        message: "Parent directory does not exist".to_string(),
                        suggestion: Some(
                            "Ensure parent directories exist or will be created".to_string(),
                        ),
                    });
                }
            }
        } // Validate WHISPER_HF_TOKEN_FILE
        if let Some(token_file) = get_env_optional("WHISPER_HF_TOKEN_FILE") {
            // Only warn if file doesn't exist (it's optional for diarization)
            if let Err(mut err) =
                validators::validate_file_exists("WHISPER_HF_TOKEN_FILE", &token_file)
            {
                err.message =
                    "HuggingFace token file not found (diarization will be disabled)".to_string();
                err.suggestion =
                    Some("Create the file with your HF token or disable diarization".to_string());
                results.add_warning(err);
            }
        }

        results
    }

    /// Validate WhisperX configuration parameters
    fn validate_whisperx_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating WhisperX configuration..."); // Validate WHISPER_CMD
        if let Some(cmd) = get_env_optional("WHISPER_CMD") {
            if let Err(err) = validators::validate_file_exists("WHISPER_CMD", &cmd) {
                results.add_error(err);
            }
        }

        // Validate WHISPER_MODELS_DIR
        if let Some(models_dir) = get_env_optional("WHISPER_MODELS_DIR") {
            if let Err(err) =
                validators::validate_directory_exists("WHISPER_MODELS_DIR", &models_dir)
            {
                results.add_error(err);
            }
        }

        // Validate WHISPER_OUTPUT_DIR
        if let Some(output_dir) = get_env_optional("WHISPER_OUTPUT_DIR") {
            // Don't require directory to exist yet, but warn if parent doesn't exist
            let path = Path::new(&output_dir);
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    results.add_warning(ConfigValidationError {
                        field: "WHISPER_OUTPUT_DIR".to_string(),
                        value: output_dir.clone(),
                        error_type: ConfigErrorType::DirectoryNotFound,
                        message: "Parent directory does not exist".to_string(),
                        suggestion: Some(
                            "Ensure parent directories exist or will be created".to_string(),
                        ),
                    });
                }
            }
        }

        // Validate WHISPER_OUTPUT_FORMAT
        if let Some(format) = get_env_optional("WHISPER_OUTPUT_FORMAT") {
            let valid_formats = &["srt", "vtt", "txt", "tsv", "json", "aud"];
            if let Err(err) =
                validators::validate_enum("WHISPER_OUTPUT_FORMAT", &format, valid_formats, false)
            {
                results.add_error(err);
            }
        }

        results
    }

    /// Validate job management configuration parameters
    fn validate_job_management_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating job management configuration...");

        // Validate WHISPER_JOB_RETENTION_HOURS
        if let Some(retention) = get_env_optional("WHISPER_JOB_RETENTION_HOURS") {
            if let Err(err) = validators::validate_integer(
                "WHISPER_JOB_RETENTION_HOURS",
                &retention,
                Some(1),
                Some(8760),
            ) {
                results.add_error(err);
            }
        }

        // Validate WHISPER_CLEANUP_INTERVAL_HOURS
        if let Some(interval) = get_env_optional("WHISPER_CLEANUP_INTERVAL_HOURS") {
            if let Err(err) = validators::validate_integer(
                "WHISPER_CLEANUP_INTERVAL_HOURS",
                &interval,
                Some(1),
                Some(168),
            ) {
                results.add_error(err);
            }
        }

        results
    }

    /// Validate upload configuration parameters
    fn validate_upload_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating upload configuration...");

        // Validate MAX_FILE_SIZE
        if let Some(size) = get_env_optional("MAX_FILE_SIZE") {
            if let Err(err) = validators::validate_file_size("MAX_FILE_SIZE", &size) {
                results.add_error(err);
            }
        }

        results
    }

    /// Validate concurrency configuration parameters
    fn validate_concurrency_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating concurrency configuration...");

        // Validate ENABLE_CONCURRENCY
        if let Some(enabled) = get_env_optional("ENABLE_CONCURRENCY") {
            if let Err(err) = validators::validate_boolean("ENABLE_CONCURRENCY", &enabled) {
                results.add_error(err);
            }
        }

        // Validate MAX_CONCURRENT_JOBS
        if let Some(max_jobs) = get_env_optional("MAX_CONCURRENT_JOBS") {
            if let Err(err) =
                validators::validate_usize("MAX_CONCURRENT_JOBS", &max_jobs, Some(1), Some(32))
            {
                results.add_error(err);
            }
        }

        // Cross-validation: warn if concurrency is disabled but max jobs > 1
        let concurrency_enabled = get_env_optional("ENABLE_CONCURRENCY")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
        let max_jobs = get_env_optional("MAX_CONCURRENT_JOBS")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(6);

        if !concurrency_enabled && max_jobs > 1 {
            results.add_warning(ConfigValidationError {
                field: "MAX_CONCURRENT_JOBS".to_string(),
                value: max_jobs.to_string(),
                error_type: ConfigErrorType::InvalidValue,
                message: "MAX_CONCURRENT_JOBS > 1 but ENABLE_CONCURRENCY is false".to_string(),
                suggestion: Some(
                    "Either enable concurrency or set MAX_CONCURRENT_JOBS to 1".to_string(),
                ),
            });
        }

        results
    }

    /// Validate security configuration parameters
    fn validate_security_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating security configuration...");

        // Validate ENABLE_AUTHORIZATION
        if let Some(enabled) = get_env_optional("ENABLE_AUTHORIZATION") {
            if let Err(err) = validators::validate_boolean("ENABLE_AUTHORIZATION", &enabled) {
                results.add_error(err);
            }
        }

        results
    }

    /// Validate API configuration parameters
    fn validate_api_config() -> ValidationResults {
        let mut results = ValidationResults::new();
        debug!("Validating API configuration...");

        // Validate SYNC_REQUEST_TIMEOUT_SECONDS
        if let Some(timeout) = get_env_optional("SYNC_REQUEST_TIMEOUT_SECONDS") {
            if let Err(err) = validators::validate_integer(
                "SYNC_REQUEST_TIMEOUT_SECONDS",
                &timeout,
                Some(0),
                Some(7200),
            ) {
                results.add_error(err);
            }
        }

        // Validate DEFAULT_SYNC_MODE
        if let Some(sync_mode) = get_env_optional("DEFAULT_SYNC_MODE") {
            if let Err(err) = validators::validate_boolean("DEFAULT_SYNC_MODE", &sync_mode) {
                results.add_error(err);
            }
        }

        results
    }

    /// Quick validation for critical parameters only
    pub fn validate_critical() -> ValidationResults {
        let mut results = ValidationResults::new();

        info!("Running critical configuration validation...");

        // Only validate essential parameters that could cause startup failures
        let critical_checks = [
            (
                "WHISPER_API_HOST",
                Self::validate_host_critical as fn(&str, &str) -> ValidationResult<()>,
            ),
            (
                "WHISPER_API_PORT",
                Self::validate_port_critical as fn(&str, &str) -> ValidationResult<()>,
            ),
            (
                "WHISPER_API_METRICS_BACKEND",
                Self::validate_metrics_backend_critical as fn(&str, &str) -> ValidationResult<()>,
            ),
            (
                "WHISPER_CMD",
                Self::validate_whisper_cmd_critical as fn(&str, &str) -> ValidationResult<()>,
            ),
        ];

        for (param_name, validator) in critical_checks.iter() {
            if let Some(value) = get_env_optional(param_name) {
                if let Err(err) = validator(param_name, &value) {
                    results.add_error(err);
                }
            }
        }

        results
    }

    // Critical validation helpers
    fn validate_host_critical(field: &str, value: &str) -> ValidationResult<()> {
        validators::validate_ip_address(field, value)?;
        Ok(())
    }

    fn validate_port_critical(field: &str, value: &str) -> ValidationResult<()> {
        validators::validate_port(field, value)?;
        Ok(())
    }

    fn validate_metrics_backend_critical(field: &str, value: &str) -> ValidationResult<()> {
        let valid_backends = &["prometheus", "statsd", "none", "disabled"];
        validators::validate_enum(field, value, valid_backends, false)?;
        Ok(())
    }

    fn validate_whisper_cmd_critical(field: &str, value: &str) -> ValidationResult<()> {
        validators::validate_file_exists(field, value)?;
        Ok(())
    }
}
