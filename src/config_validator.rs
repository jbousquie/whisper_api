// Configuration validation module for Whisper API
//
// This module provides comprehensive validation for all configuration parameters
// and environment variables, ensuring early detection of configuration errors
// with clear, actionable error messages.
//
// The validation system is schema-driven, with a centralized parameter registry
// that defines validation rules, default values, and constraints for all configuration options.

use std::env;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::str::FromStr;
//use std::collections::HashMap;

use log::{error, info, warn};

/// Configuration parameter types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigType {
    String,
    Integer,
    UnsignedInteger,
    Boolean,
    Float,
    IpAddress,
    Port,
    SocketAddress,
    FilePath,
    DirectoryPath,
    Enum(&'static [&'static str]),
}

/// Validation severity levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidationLevel {
    Critical, // Must be valid for application to start
    Standard, // Important but application can start with defaults
    Warning,  // Optional, generates warnings only
}

/// Configuration parameter definition
#[derive(Debug, Clone)]
pub struct ConfigParam {
    pub name: &'static str,
    pub description: &'static str,
    pub param_type: ConfigType,
    pub default_value: Option<&'static str>,
    pub required: bool,
    pub validation_level: ValidationLevel,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub min_float_value: Option<f64>,
    pub max_float_value: Option<f64>,
}

/// Centralized configuration parameter registry
/// This ensures consistency across the entire project and makes adding new parameters straightforward
pub const CONFIG_PARAMS: &[ConfigParam] = &[
    // Server Configuration
    ConfigParam {
        name: "WHISPER_API_HOST",
        description: "Host IP address for the API server",
        param_type: ConfigType::IpAddress,
        default_value: Some("127.0.0.1"),
        required: false,
        validation_level: ValidationLevel::Critical,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_API_PORT",
        description: "Port for the API server",
        param_type: ConfigType::Port,
        default_value: Some("8181"),
        required: false,
        validation_level: ValidationLevel::Critical,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_API_TIMEOUT",
        description: "HTTP request timeout in seconds",
        param_type: ConfigType::Integer,
        default_value: Some("480"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(1),
        max_value: Some(3600),
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_API_KEEPALIVE",
        description: "Keep-alive timeout in seconds",
        param_type: ConfigType::Integer,
        default_value: Some("480"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(1),
        max_value: Some(3600),
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "HTTP_WORKER_NUMBER",
        description: "Number of HTTP worker processes (0 = use CPU cores)",
        param_type: ConfigType::UnsignedInteger,
        default_value: Some("0"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(0),
        max_value: Some(64),
        min_float_value: None,
        max_float_value: None,
    },
    // Metrics Configuration
    ConfigParam {
        name: "WHISPER_API_METRICS_ENABLED",
        description: "Enable metrics collection for the Whisper API",
        param_type: ConfigType::Boolean,
        default_value: Some("true"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_API_METRICS_BACKEND",
        description: "Metrics backend type",
        param_type: ConfigType::Enum(&["prometheus", "statsd", "none", "disabled"]),
        default_value: Some("none"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_API_METRICS_ENDPOINT",
        description: "Metrics endpoint for StatsD or other backends (host:port)",
        param_type: ConfigType::SocketAddress,
        default_value: Some("127.0.0.1:8125"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_API_METRICS_PREFIX",
        description: "Prefix for all exported metrics",
        param_type: ConfigType::String,
        default_value: Some("whisper_api"),
        required: false,
        validation_level: ValidationLevel::Warning,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_API_METRICS_SAMPLE_RATE",
        description: "Sample rate for metrics (0.0-1.0)",
        param_type: ConfigType::Float,
        default_value: Some("1.0"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: Some(0.0),
        max_float_value: Some(1.0),
    },
    // File Storage Configuration
    ConfigParam {
        name: "WHISPER_TMP_FILES",
        description: "Directory for temporary files",
        param_type: ConfigType::String, // DirectoryPath validation handled separately
        default_value: Some("/home/llm/whisper_api/tmp"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_HF_TOKEN_FILE",
        description: "Path to HuggingFace token file for diarization",
        param_type: ConfigType::String, // FilePath validation handled separately
        default_value: Some("/home/llm/whisper_api/hf_token.txt"),
        required: false,
        validation_level: ValidationLevel::Warning,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    // WhisperX Configuration
    ConfigParam {
        name: "WHISPER_CMD",
        description: "Path to the WhisperX command or wrapper script",
        param_type: ConfigType::FilePath,
        default_value: Some("/home/llm/whisper_api/whisperx.sh"),
        required: true,
        validation_level: ValidationLevel::Critical,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_MODELS_DIR",
        description: "Directory containing WhisperX models",
        param_type: ConfigType::DirectoryPath,
        default_value: Some("/home/llm/whisperx/models"),
        required: true,
        validation_level: ValidationLevel::Critical,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_OUTPUT_DIR",
        description: "Directory for WhisperX output files",
        param_type: ConfigType::String, // DirectoryPath validation handled separately
        default_value: Some("/home/llm/whisper_api/output"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_OUTPUT_FORMAT",
        description: "Default output format for transcription",
        param_type: ConfigType::Enum(&["srt", "vtt", "txt", "tsv", "json", "aud"]),
        default_value: Some("txt"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    // Job Management Configuration
    ConfigParam {
        name: "WHISPER_JOB_RETENTION_HOURS",
        description: "Number of hours to retain completed jobs",
        param_type: ConfigType::Integer,
        default_value: Some("48"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(1),
        max_value: Some(8760),
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "WHISPER_CLEANUP_INTERVAL_HOURS",
        description: "Interval in hours between job cleanup runs",
        param_type: ConfigType::Integer,
        default_value: Some("12"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(1),
        max_value: Some(168),
        min_float_value: None,
        max_float_value: None,
    },
    // Upload Configuration
    ConfigParam {
        name: "MAX_FILE_SIZE",
        description: "Maximum file size for uploads in bytes",
        param_type: ConfigType::UnsignedInteger,
        default_value: Some("536870912"), // 512MB
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(1024),        // 1KB minimum
        max_value: Some(10737418240), // 10GB maximum
        min_float_value: None,
        max_float_value: None,
    },
    // Concurrency Configuration
    ConfigParam {
        name: "ENABLE_CONCURRENCY",
        description: "Enable concurrent job processing",
        param_type: ConfigType::Boolean,
        default_value: Some("false"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "MAX_CONCURRENT_JOBS",
        description: "Maximum number of concurrent jobs when concurrency is enabled",
        param_type: ConfigType::UnsignedInteger,
        default_value: Some("6"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(1),
        max_value: Some(32),
        min_float_value: None,
        max_float_value: None,
    },
    // Security Configuration
    ConfigParam {
        name: "ENABLE_AUTHORIZATION",
        description: "Enable authentication requirement",
        param_type: ConfigType::Boolean,
        default_value: Some("true"),
        required: false,
        validation_level: ValidationLevel::Critical,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
    // API Configuration
    ConfigParam {
        name: "SYNC_REQUEST_TIMEOUT_SECONDS",
        description: "Default timeout in seconds for synchronous transcription requests",
        param_type: ConfigType::Integer,
        default_value: Some("1800"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: Some(0),
        max_value: Some(7200),
        min_float_value: None,
        max_float_value: None,
    },
    ConfigParam {
        name: "DEFAULT_SYNC_MODE",
        description: "Default processing mode when 'sync' parameter is missing",
        param_type: ConfigType::Boolean,
        default_value: Some("true"),
        required: false,
        validation_level: ValidationLevel::Standard,
        min_value: None,
        max_value: None,
        min_float_value: None,
        max_float_value: None,
    },
];

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
#[allow(dead_code)] // Some error types may be used in future validation extensions
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

    // TODO 
    pub fn _merge(&mut self, other: ValidationResults) {
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

/// Type-safe configuration struct holding validated values
#[derive(Debug, Clone)]
pub struct WhisperConfig {
    pub host: IpAddr,
    pub port: u16,
    pub timeout: i64,
    pub keepalive: i64,
    pub workers: usize,
    pub metrics_enabled: bool,
    pub metrics_backend: String,
    pub metrics_endpoint: Option<SocketAddr>,
    pub metrics_prefix: Option<String>,
    pub metrics_sample_rate: Option<f64>,
    pub tmp_dir: String,
    pub hf_token_file: Option<String>,
    pub whisper_cmd: String,
    pub models_dir: String,
    pub output_dir: String,
    pub output_format: String,
    pub job_retention_hours: i64,
    pub cleanup_interval_hours: i64,
    pub max_file_size: usize,
    pub enable_concurrency: bool,
    pub max_concurrent_jobs: usize,
    pub enable_authorization: bool,
    pub sync_timeout: i64,
    pub default_sync_mode: bool,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".parse().unwrap(),
            port: 8181,
            timeout: 480,
            keepalive: 480,
            workers: 0,
            metrics_enabled: true,
            metrics_backend: "none".to_string(),
            metrics_endpoint: None,
            metrics_prefix: Some("whisper_api".to_string()),
            metrics_sample_rate: Some(1.0),
            tmp_dir: "/home/llm/whisper_api/tmp".to_string(),
            hf_token_file: Some("/home/llm/whisper_api/hf_token.txt".to_string()),
            whisper_cmd: "/home/llm/whisper_api/whisperx.sh".to_string(),
            models_dir: "/home/llm/whisperx/models".to_string(),
            output_dir: "/home/llm/whisper_api/output".to_string(),
            output_format: "txt".to_string(),
            job_retention_hours: 48,
            cleanup_interval_hours: 12,
            max_file_size: 536870912,
            enable_concurrency: false,
            max_concurrent_jobs: 6,
            enable_authorization: true,
            sync_timeout: 1800,
            default_sync_mode: true,
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
        min: Option<i64>,
        max: Option<i64>,
    ) -> ValidationResult<usize> {
        let parsed = value.parse::<usize>().map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid unsigned integer format".to_string(),
            suggestion: Some("Use a valid positive integer number".to_string()),
        })?;

        if let Some(min) = min {
            if (parsed as i64) < min {
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
            if (parsed as i64) > max {
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

    /// Validate float values with optional range
    pub fn validate_float(
        field: &str,
        value: &str,
        min: Option<f64>,
        max: Option<f64>,
    ) -> ValidationResult<f64> {
        let parsed = value.parse::<f64>().map_err(|_| ConfigValidationError {
            field: field.to_string(),
            value: value.to_string(),
            error_type: ConfigErrorType::InvalidFormat,
            message: "Invalid float format".to_string(),
            suggestion: Some("Use a valid decimal number".to_string()),
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

    /// Validate file path exists and is readable
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

        // Check if file is readable
        if let Err(_) = std::fs::metadata(path) {
            return Err(ConfigValidationError {
                field: field.to_string(),
                value: value.to_string(),
                error_type: ConfigErrorType::PermissionDenied,
                message: "File is not readable".to_string(),
                suggestion: Some("Ensure the file has read permissions".to_string()),
            });
        }

        Ok(value.to_string())
    }

    /// Validate directory path exists and is writable
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

        // Check if directory is writable
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.permissions().readonly() {
                return Err(ConfigValidationError {
                    field: field.to_string(),
                    value: value.to_string(),
                    error_type: ConfigErrorType::PermissionDenied,
                    message: "Directory is not writable".to_string(),
                    suggestion: Some("Ensure the directory has write permissions".to_string()),
                });
            }
        }

        Ok(value.to_string())
    }
}

/// TODO : helper function to get environment variable or return default
pub fn _get_env_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Helper function to get optional environment variable
pub fn get_env_optional(key: &str) -> Option<String> {
    env::var(key).ok()
}

/// Comprehensive configuration validator for Whisper API
pub struct WhisperConfigValidator;

impl WhisperConfigValidator {
    /// Validate all configuration parameters and return type-safe configuration struct
    pub fn validate_and_load() -> Result<WhisperConfig, ValidationResults> {
        let mut results = ValidationResults::new();
        let mut config = WhisperConfig::default();

        info!("Starting comprehensive configuration validation...");

        // Validate all parameters using the schema
        for param in CONFIG_PARAMS {
            let value = get_env_optional(param.name)
                .or_else(|| param.default_value.map(String::from))
                .unwrap_or_default();

            // Check if required parameter is missing
            if param.required && value.is_empty() {
                let error = ConfigValidationError {
                    field: param.name.to_string(),
                    value: "".to_string(),
                    error_type: ConfigErrorType::Required,
                    message: "Required parameter is missing".to_string(),
                    suggestion: Some(format!("Set {} environment variable", param.name)),
                };
                match param.validation_level {
                    ValidationLevel::Critical | ValidationLevel::Standard => {
                        results.add_error(error)
                    }
                    ValidationLevel::Warning => results.add_warning(error),
                }
                continue;
            }

            // Skip validation if value is empty and not required
            if value.is_empty() {
                continue;
            }

            // Validate parameter and store in config struct
            match Self::validate_and_store_parameter(param, &value, &mut config) {
                Ok(_) => {}
                Err(error) => match param.validation_level {
                    ValidationLevel::Critical | ValidationLevel::Standard => {
                        results.add_error(error)
                    }
                    ValidationLevel::Warning => results.add_warning(error),
                },
            }
        }

        // Perform cross-parameter validation
        Self::validate_cross_dependencies(&mut results, &config);

        results.print_summary();

        if results.is_valid {
            Ok(config)
        } else {
            Err(results)
        }
    }

    /// Validate a single parameter and store its value in the config struct
    fn validate_and_store_parameter(
        param: &ConfigParam,
        value: &str,
        config: &mut WhisperConfig,
    ) -> ValidationResult<()> {
        match param.param_type {
            ConfigType::String => {
                // Store string value directly
                match param.name {
                    "WHISPER_TMP_FILES" => config.tmp_dir = value.to_string(),
                    "WHISPER_HF_TOKEN_FILE" => config.hf_token_file = Some(value.to_string()),
                    "WHISPER_OUTPUT_DIR" => config.output_dir = value.to_string(),
                    "WHISPER_API_METRICS_PREFIX" => config.metrics_prefix = Some(value.to_string()),
                    _ => {} // Other string params handled elsewhere
                }
            }
            ConfigType::Integer => {
                let parsed = validators::validate_integer(
                    param.name,
                    value,
                    param.min_value,
                    param.max_value,
                )?;
                match param.name {
                    "WHISPER_API_TIMEOUT" => config.timeout = parsed,
                    "WHISPER_API_KEEPALIVE" => config.keepalive = parsed,
                    "WHISPER_JOB_RETENTION_HOURS" => config.job_retention_hours = parsed,
                    "WHISPER_CLEANUP_INTERVAL_HOURS" => config.cleanup_interval_hours = parsed,
                    "SYNC_REQUEST_TIMEOUT_SECONDS" => config.sync_timeout = parsed,
                    _ => {}
                }
            }
            ConfigType::UnsignedInteger => {
                let parsed = validators::validate_usize(
                    param.name,
                    value,
                    param.min_value,
                    param.max_value,
                )?;
                match param.name {
                    "HTTP_WORKER_NUMBER" => config.workers = parsed,
                    "MAX_FILE_SIZE" => config.max_file_size = parsed,
                    "MAX_CONCURRENT_JOBS" => config.max_concurrent_jobs = parsed,
                    _ => {}
                }
            }
            ConfigType::Boolean => {
                let parsed = validators::validate_boolean(param.name, value)?;
                match param.name {
                    "WHISPER_API_METRICS_ENABLED" => config.metrics_enabled = parsed,
                    "ENABLE_CONCURRENCY" => config.enable_concurrency = parsed,
                    "ENABLE_AUTHORIZATION" => config.enable_authorization = parsed,
                    "DEFAULT_SYNC_MODE" => config.default_sync_mode = parsed,
                    _ => {}
                }
            }
            ConfigType::Float => {
                let parsed = validators::validate_float(
                    param.name,
                    value,
                    param.min_float_value,
                    param.max_float_value,
                )?;
                match param.name {
                    "WHISPER_API_METRICS_SAMPLE_RATE" => config.metrics_sample_rate = Some(parsed),
                    _ => {}
                }
            }
            ConfigType::IpAddress => {
                let parsed = validators::validate_ip_address(param.name, value)?;
                match param.name {
                    "WHISPER_API_HOST" => config.host = parsed,
                    _ => {}
                }
            }
            ConfigType::Port => {
                let parsed = validators::validate_port(param.name, value)?;
                match param.name {
                    "WHISPER_API_PORT" => config.port = parsed,
                    _ => {}
                }
            }
            ConfigType::SocketAddress => {
                let parsed = validators::validate_socket_address(param.name, value)?;
                match param.name {
                    "WHISPER_API_METRICS_ENDPOINT" => config.metrics_endpoint = Some(parsed),
                    _ => {}
                }
            }
            ConfigType::FilePath => {
                let parsed = validators::validate_file_exists(param.name, value)?;
                match param.name {
                    "WHISPER_CMD" => config.whisper_cmd = parsed,
                    _ => {}
                }
            }
            ConfigType::DirectoryPath => {
                let parsed = validators::validate_directory_exists(param.name, value)?;
                match param.name {
                    "WHISPER_MODELS_DIR" => config.models_dir = parsed,
                    _ => {}
                }
            }
            ConfigType::Enum(valid_values) => {
                let parsed = validators::validate_enum(param.name, value, valid_values, false)?;
                match param.name {
                    "WHISPER_API_METRICS_BACKEND" => config.metrics_backend = parsed,
                    "WHISPER_OUTPUT_FORMAT" => config.output_format = parsed,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Validate cross-parameter dependencies
    fn validate_cross_dependencies(results: &mut ValidationResults, config: &WhisperConfig) {
        // Check concurrency configuration consistency
        if !config.enable_concurrency && config.max_concurrent_jobs > 1 {
            results.add_warning(ConfigValidationError {
                field: "MAX_CONCURRENT_JOBS".to_string(),
                value: config.max_concurrent_jobs.to_string(),
                error_type: ConfigErrorType::InvalidValue,
                message: "MAX_CONCURRENT_JOBS > 1 but ENABLE_CONCURRENCY is false".to_string(),
                suggestion: Some(
                    "Either enable concurrency or set MAX_CONCURRENT_JOBS to 1".to_string(),
                ),
            });
        }

        // Check metrics configuration consistency
        if config.metrics_enabled
            && config.metrics_backend != "none"
            && config.metrics_backend != "disabled"
        {
            if config.metrics_backend == "statsd" && config.metrics_endpoint.is_none() {
                results.add_error(ConfigValidationError {
                    field: "WHISPER_API_METRICS_ENDPOINT".to_string(),
                    value: "".to_string(),
                    error_type: ConfigErrorType::Required,
                    message: "Metrics endpoint required when StatsD backend is enabled".to_string(),
                    suggestion: Some(
                        "Set WHISPER_API_METRICS_ENDPOINT for StatsD backend".to_string(),
                    ),
                });
            }
        }
    }

    /// Quick validation for critical parameters only (fail-fast)
    pub fn validate_critical() -> Result<(), ValidationResults> {
        let mut results = ValidationResults::new();

        info!("Running critical configuration validation...");

        for param in CONFIG_PARAMS {
            if param.validation_level == ValidationLevel::Critical {
                let value = get_env_optional(param.name)
                    .or_else(|| param.default_value.map(String::from))
                    .unwrap_or_default();

                if param.required && value.is_empty() {
                    results.add_error(ConfigValidationError {
                        field: param.name.to_string(),
                        value: "".to_string(),
                        error_type: ConfigErrorType::Required,
                        message: "Critical required parameter is missing".to_string(),
                        suggestion: Some(format!("Set {} environment variable", param.name)),
                    });
                    continue;
                }

                if !value.is_empty() {
                    let mut dummy_config = WhisperConfig::default();
                    if let Err(error) =
                        Self::validate_and_store_parameter(param, &value, &mut dummy_config)
                    {
                        results.add_error(error);
                    }
                }
            }
        }

        if results.is_valid {
            Ok(())
        } else {
            results.print_summary();
            Err(results)
        }
    }
}

/// Documentation and configuration generation utilities
impl WhisperConfigValidator {
    /// Generate a sample configuration file with all parameters and descriptions
    pub fn generate_sample_config() -> String {
        let mut output = String::new();
        output.push_str("# Whisper API Configuration File\n");
        output.push_str("# This file contains all available configuration parameters\n\n");

        let mut current_category = "";
        for param in CONFIG_PARAMS {
            // Group parameters by category (based on name prefix)
            let category = if param.name.starts_with("WHISPER_API_METRICS") {
                "Metrics Configuration"
            } else if param.name.starts_with("WHISPER_API") {
                "Server Configuration"
            } else if param.name.starts_with("WHISPER_") {
                "WhisperX Configuration"
            } else if param.name.starts_with("HTTP_") {
                "Server Configuration"
            } else if param.name.starts_with("ENABLE_") || param.name.starts_with("MAX_") {
                "Processing Configuration"
            } else if param.name.starts_with("SYNC_") || param.name.starts_with("DEFAULT_") {
                "API Configuration"
            } else {
                "General Configuration"
            };

            if category != current_category {
                output.push_str(&format!("\n# ======== {} ========\n", category));
                current_category = category;
            }

            output.push_str(&format!("# {}\n", param.description));
            if param.required {
                output.push_str("# REQUIRED\n");
            }
            output.push_str(&format!(
                "{} = {}\n\n",
                param.name,
                param.default_value.unwrap_or("\"\"")
            ));
        }
        output
    }

    /// Generate markdown documentation for all configuration parameters
    pub fn generate_config_documentation() -> String {
        let mut output = String::new();
        output.push_str("# Whisper API Configuration Reference\n\n");
        output.push_str("This document describes all available configuration parameters for the Whisper API.\n\n");

        output.push_str("| Parameter | Type | Required | Default | Description |\n");
        output.push_str("|-----------|------|----------|---------|-------------|\n");

        for param in CONFIG_PARAMS {
            output.push_str(&format!(
                "| `{}` | {:?} | {} | `{}` | {} |\n",
                param.name,
                param.param_type,
                if param.required { "Yes" } else { "No" },
                param.default_value.unwrap_or("none"),
                param.description
            ));
        }

        output
    }
}
