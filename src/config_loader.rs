// Configuration loader for Whisper API
//
// This module handles loading configuration from the TOML configuration file
// and environment variables with appropriate precedence.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use log::{debug, info, warn};
use toml::Value;

const CONFIG_FILE_PATH: &str = "whisper_api.conf";

/// Loads configuration from TOML file and environment variables
///
/// Configuration precedence (highest to lowest):
/// 1. Environment variables
/// 2. Configuration file values
/// 3. Default values (not handled here - application defaults)
///
/// # Returns
///
/// Returns true if the config file was successfully loaded, false otherwise
pub fn load_config() -> bool {
    let config_path = Path::new(CONFIG_FILE_PATH);

    // Check if configuration file exists
    if !config_path.exists() {
        debug!("Configuration file not found at: {}", CONFIG_FILE_PATH);
        return false;
    }

    // Read configuration file
    let config_content = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to read configuration file: {}", e);
            return false;
        }
    };

    // Parse TOML content
    let config_values: Value = match config_content.parse() {
        Ok(values) => values,
        Err(e) => {
            warn!("Failed to parse configuration file: {}", e);
            return false;
        }
    };

    // Convert TOML into flat key-value pairs
    let mut config_map = HashMap::new();

    // TOML is expected to be flat (not nested), simply extract key-value pairs
    if let Value::Table(table) = config_values {
        for (key, value) in table {
            match value {
                Value::String(s) => {
                    config_map.insert(key, s);
                }
                Value::Integer(i) => {
                    config_map.insert(key, i.to_string());
                }
                Value::Float(f) => {
                    config_map.insert(key, f.to_string());
                }
                Value::Boolean(b) => {
                    config_map.insert(key, b.to_string());
                }
                _ => {
                    // Skip other types (arrays, tables)
                    warn!("Skipping unsupported TOML value type for key: {}", key);
                }
            }
        }
    }

    // Set environment variables from config file if they don't already exist
    for (key, value) in config_map {
        // Only set if the environment variable doesn't already exist
        if env::var(&key).is_err() {
            debug!("Setting env var from config file: {} = {}", key, value);
            env::set_var(key, value);
        } else {
            debug!("Env var already exists, skipping: {}", key);
        }
    }

    info!("Configuration loaded from {}", CONFIG_FILE_PATH);
    true
}
