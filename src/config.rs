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
}

/// Configuration for the Whisper API handlers
#[derive(Clone, Debug)]
pub struct HandlerConfig {
    /// Directory to store temporary files
    pub temp_dir: String,
    /// Path to HuggingFace token file
    pub hf_token_file: String,
}

impl Default for HandlerConfig {
    fn default() -> Self {
        Self {
            temp_dir: env::var("WHISPER_TMP_FILES")
                .unwrap_or_else(|_| String::from(defaults::TEMP_DIR)),
            hf_token_file: env::var("WHISPER_HF_TOKEN_FILE")
                .unwrap_or_else(|_| String::from(defaults::HF_TOKEN_FILE)),
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
    fn default() -> Self {
        Self {
            exporter_type: env::var("METRICS_EXPORTER").unwrap_or_else(|_| "none".to_string()),
            endpoint: env::var("METRICS_ENDPOINT").ok(),
            prefix: env::var("METRICS_PREFIX").ok(),
            sample_rate: env::var("METRICS_SAMPLE_RATE")
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
