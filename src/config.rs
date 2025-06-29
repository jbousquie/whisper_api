// Whisper API configuration
//
// This module contains configuration structures and constants for the Whisper API.
// It centralizes all configuration parameters and provides defaults from environment variables.

use std::env;
use std::path::PathBuf;

/// Default values for configuration
pub mod defaults {
    // Temporary directory for file storage
    pub const TEMP_DIR: &str = "/home/llm/whisper_api/tmp";

    // Default language for transcription
    pub const LANGUAGE: &str = "fr";

    // Default WhisperX model
    pub const MODEL: &str = "large-v3";

    // Path to HuggingFace token file
    pub const HF_TOKEN_FILE: &str = "/home/llm/whisper_api/hf_token.txt";

    // Valid output formats
    pub const VALID_OUTPUT_FORMATS: [&str; 6] = ["srt", "vtt", "txt", "tsv", "json", "aud"];

    // Default device for PyTorch inference
    pub const DEVICE: &str = "cuda";

    // Default device index for inference
    pub const DEVICE_INDEX: &str = "0";

    // Default timeout in seconds for synchronous transcription requests
    pub const SYNC_REQUEST_TIMEOUT_SECONDS: u64 = 1800;

    // Default processing mode when 'sync' parameter is missing
    pub const DEFAULT_SYNC_MODE: bool = false;
}

/// Configuration for the Whisper API handlers
#[derive(Clone, Debug)]
pub struct HandlerConfig {
    /// Directory to store temporary files
    pub temp_dir: String,
    /// Path to HuggingFace token file
    pub hf_token_file: String,
    /// Timeout in seconds for synchronous transcription requests
    pub sync_request_timeout: u64,
    /// Default processing mode (true = synchronous, false = asynchronous)
    pub default_sync_mode: bool,
}

impl Default for HandlerConfig {
    fn default() -> Self {
        Self {
            temp_dir: env::var("WHISPER_TMP_FILES")
                .unwrap_or_else(|_| String::from(defaults::TEMP_DIR)),
            hf_token_file: env::var("WHISPER_HF_TOKEN_FILE")
                .unwrap_or_else(|_| String::from(defaults::HF_TOKEN_FILE)),
            sync_request_timeout: env::var("SYNC_REQUEST_TIMEOUT_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults::SYNC_REQUEST_TIMEOUT_SECONDS),
            default_sync_mode: env::var("DEFAULT_SYNC_MODE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults::DEFAULT_SYNC_MODE),
        }
    }
}

impl HandlerConfig {
    // This function was removed as it's not currently used
    // If needed in the future, uncomment and use it
    /*
    pub fn load_hf_token(&self) -> Option<String> {
        match std::fs::read_to_string(&self.hf_token_file) {
            Ok(token) => {
                let token = token.trim().to_string();
                if token.is_empty() {
                    None
                } else {
                    Some(token)
                }
            }
            Err(_) => None,
        }
    }
    */

    /// Validates if an output format is supported
    pub fn validate_output_format(format: &str) -> bool {
        defaults::VALID_OUTPUT_FORMATS.contains(&format)
    }

    /// Ensures the temporary directory exists
    pub fn ensure_temp_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.temp_dir)
    }
}

/// Configuration for metrics collection and export
#[derive(Clone, Debug)]
pub struct MetricsConfig {
    /// Type of metrics exporter ("prometheus", "statsd", "none")
    pub exporter_type: String,
    /// Endpoint for metrics exporter (if applicable)
    pub endpoint: Option<String>,
    /// Metrics prefix for all metrics (useful for StatsD)
    pub prefix: Option<String>,
    /// Sample rate for metrics (0.0 to 1.0, mainly for StatsD)
    pub sample_rate: Option<f64>,
}

impl Default for MetricsConfig {
    fn default() -> Self {        // Check if metrics are explicitly disabled via WHISPER_API_METRICS_ENABLED
        let metrics_enabled = env::var("WHISPER_API_METRICS_ENABLED")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(true); // Default to enabled
        
        // Support multiple environment variable names for maximum compatibility:
        // 1. WHISPER_API_METRICS_BACKEND (from config file)
        // 2. METRICS_BACKEND (standard)
        // 3. METRICS_EXPORTER (legacy)
        let exporter_type = if !metrics_enabled {
            "disabled".to_string()
        } else {
            env::var("WHISPER_API_METRICS_BACKEND")
                .or_else(|_| env::var("METRICS_BACKEND"))
                .or_else(|_| env::var("METRICS_EXPORTER"))
                .unwrap_or_else(|_| "none".to_string())
        };
          // Support both STATSD_ENDPOINT and STATSD_HOST/STATSD_PORT combination
        let endpoint = env::var("WHISPER_API_METRICS_ENDPOINT")
            .or_else(|_| env::var("METRICS_ENDPOINT"))
            .or_else(|_| env::var("STATSD_ENDPOINT"))
            .or_else(|_| env::var("WHISPER_API_STATSD_ENDPOINT"))
            .or_else(|_| {
                // Try to build endpoint from STATSD_HOST and STATSD_PORT
                let host = env::var("STATSD_HOST")
                    .or_else(|_| env::var("WHISPER_API_STATSD_HOST"))
                    .unwrap_or_else(|_| "127.0.0.1".to_string());
                let port = env::var("STATSD_PORT")
                    .or_else(|_| env::var("WHISPER_API_STATSD_PORT"))
                    .unwrap_or_else(|_| "8125".to_string());
                Ok::<String, env::VarError>(format!("{}:{}", host, port))
            })
            .ok();
        
        // Support multiple prefix variable names
        let prefix = env::var("WHISPER_API_METRICS_PREFIX")
            .or_else(|_| env::var("METRICS_PREFIX"))
            .or_else(|_| env::var("STATSD_PREFIX"))
            .ok();
            
        Self {
            exporter_type,
            endpoint,
            prefix,
            sample_rate: env::var("WHISPER_API_METRICS_SAMPLE_RATE")
                .or_else(|_| env::var("METRICS_SAMPLE_RATE"))
                .or_else(|_| env::var("STATSD_SAMPLE_RATE"))
                .ok()
                .and_then(|s| s.parse().ok()),
        }
    }
}

/// Represents the path to a job's files
#[derive(Debug, Clone)]
pub struct JobPaths {
    /// Unique folder for this job
    pub folder: PathBuf,
    /// Audio file path
    pub audio_file: PathBuf,
    /// Job ID (UUID)
    pub id: String,
}
