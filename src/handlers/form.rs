// Form data processing for Whisper API
//
// This module handles the extraction and processing of form data for the Whisper API.
// It provides functions to parse multipart forms and extract fields like audio files and parameters.

use actix_multipart::Multipart;
use futures::{StreamExt, TryStreamExt};
use log::{error, info, warn};
use std::env;

use crate::config::{defaults, HandlerConfig, JobPaths};
use crate::error::HandlerError;
use crate::file_utils::{generate_unique_job_paths, read_text_file, save_file_data};
use crate::models::TranscriptionParams;

/// Default max file size (512MB)
const DEFAULT_MAX_FILE_SIZE: usize = 536870912;

/// Get max file size from environment or use default
fn get_max_file_size() -> usize {
    env::var("MAX_FILE_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_FILE_SIZE)
}

/// Extract and process multipart form data for transcription requests
///
/// # Arguments
///
/// * `form` - The multipart form from the HTTP request
/// * `config` - Handler configuration
///
/// # Returns
///
/// * `Result<(TranscriptionParams, JobPaths), HandlerError>` - Extracted parameters and job paths, or an error
pub async fn extract_form_data(
    mut form: Multipart,
    config: &HandlerConfig,
) -> Result<(TranscriptionParams, JobPaths), HandlerError> {
    // Initialize parameters with defaults
    let mut params = TranscriptionParams {
        language: String::from(defaults::LANGUAGE),
        model: String::from(defaults::MODEL),
        diarize: true, // Default: enable speaker diarization
        prompt: String::new(),
        hf_token: None, // Will be set from file if not in request
        response_format: None,
        audio_file: None,
        folder_path: None,
        sync: config.default_sync_mode, // Use configured default sync mode
        device: None,
        device_index: None,
    };

    let mut job_paths: Option<JobPaths> = None;

    // Ensure the temp directory exists
    config.ensure_temp_dir().map_err(|e| {
        error!("Failed to create main tmp directory: {}", e);
        HandlerError::FileError(e)
    })?;

    // Process form data
    while let Ok(Some(mut field)) = form.try_next().await {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition
            .and_then(|cd| cd.get_name().map(|name| name.to_string()))
            .unwrap_or_default();

        match field_name.as_str() {
            "language" | "model" | "prompt" | "hf_token" | "response_format" | "sync"
            | "device" | "device_index" => {
                // Read text parameter
                let mut value = String::new();
                while let Some(chunk) = field.next().await {
                    let chunk = chunk.map_err(|e| {
                        HandlerError::form_error(format!(
                            "Error reading field {}: {}",
                            field_name, e
                        ))
                    })?;
                    if let Ok(s) = std::str::from_utf8(&chunk) {
                        value.push_str(s);
                    }
                }

                let value = value.trim().to_string();
                if !value.is_empty() {
                    match field_name.as_str() {
                        "language" => params.language = value,
                        "model" => params.model = value,
                        "prompt" => params.prompt = value,
                        "hf_token" => params.hf_token = Some(value),
                        "device" => params.device = Some(value),
                        "device_index" => params.device_index = Some(value),
                        "response_format" => {
                            // Validate output format
                            if HandlerConfig::validate_output_format(&value) {
                                params.response_format = Some(value);
                            } else {
                                return Err(HandlerError::InvalidOutputFormat(value));
                            }
                        }
                        "sync" => {
                            // Parse boolean value (various formats)
                            params.sync = match value.trim().to_lowercase().as_str() {
                                "true" | "1" | "yes" | "on" => true,
                                _ => false, // Default to async mode for any other value
                            };
                        }
                        _ => {}
                    }
                }
            }
            "diarize" => {
                // Parse boolean parameter
                let mut value = String::new();
                while let Some(chunk) = field.next().await {
                    let chunk = chunk.map_err(|e| {
                        HandlerError::form_error(format!("Error reading diarize field: {}", e))
                    })?;
                    if let Ok(s) = std::str::from_utf8(&chunk) {
                        value.push_str(s);
                    }
                }

                // Parse boolean value (various formats)
                let value = value.trim().to_lowercase();
                params.diarize = match value.as_str() {
                    "false" | "0" | "no" | "off" => false,
                    _ => true, // Default to true for any other input
                };
            }
            "file" => {
                // Validate content type before processing
                let content_type = field.content_type().map(|ct| ct.to_string());
                
                // Check if this is an audio file based on content type
                let is_audio_file = content_type.as_ref().map_or(false, |ct| {
                    ct.starts_with("audio/") || 
                    ct.ends_with("/wav") || 
                    ct.ends_with("/mp3") || 
                    ct.ends_with("/ogg") ||
                    ct.ends_with("/flac")
                });
                
                if !is_audio_file {
                    warn!("Rejected non-audio file with content type: {:?}", content_type);
                    return Err(HandlerError::form_error(
                        format!("Invalid file type. Expected audio file, got: {:?}", content_type)
                    ));
                }

                // Generate a unique folder and filename for the audio file
                let paths = generate_unique_job_paths(&config.temp_dir, "whisper", "audio")
                    .map_err(|e| {
                        error!("Failed to create unique directory: {}", e);
                        HandlerError::FileError(e)
                    })?;

                // Store job paths for returning and possible cleanup
                job_paths = Some(paths.clone());
                params.folder_path = Some(paths.folder.clone());
                params.audio_file = Some(paths.audio_file.clone());

                // Process the file data
                let mut total_size = 0;
                let mut file_data = Vec::new();

                while let Some(chunk) = field.next().await {
                    let data = chunk.map_err(|e| {
                        let mut error = HandlerError::form_error(format!("Error processing file upload: {}", e));
                        error = error.with_cleanup(Some(&paths.folder));
                        error
                    })?;

                    // Check file size limits
                    let max_file_size = get_max_file_size();
                    total_size += data.len();
                    if total_size > max_file_size {
                        let mut error = HandlerError::FileTooLarge(total_size, max_file_size);
                        error = error.with_cleanup(Some(&paths.folder));
                        return Err(error);
                    }

                    file_data.extend_from_slice(&data);
                }

                // Validate we received some data
                if file_data.is_empty() {
                    let mut error = HandlerError::form_error("Uploaded file is empty");
                    error = error.with_cleanup(Some(&paths.folder));
                    return Err(error);
                }

                // Save the file data
                save_file_data(&file_data, &paths.audio_file)
                    .map_err(|e| {
                        let mut error = HandlerError::FileError(e);
                        error = error.with_cleanup(Some(&paths.folder));
                        error
                    })?;

                info!("Saved audio file: {} (content-type: {:?})", paths.audio_file.display(), content_type);
            }
            _ => {
                // Skip unknown fields
                while let Some(_) = field.next().await {}
            }
        }
    }

    // Validate the parameters before continuing
    if let Err(e) = params.validate() {
        let mut error = HandlerError::form_error(format!("Parameter validation failed: {}", e));
        if let Some(paths) = &job_paths {
            error = error.with_cleanup(Some(&paths.folder));
        }
        return Err(error);
    }

    // Ensure we have all required data
    let job_paths = job_paths.ok_or_else(|| HandlerError::NoAudioFile)?;

    // If no HF token was provided in the form, try to read it from the token file
    if params.hf_token.is_none() {
        // Try to read the token from the configured file
        let hf_token_path = std::path::Path::new(&config.hf_token_file);
        if hf_token_path.exists() {
            match read_text_file(hf_token_path) {
                Ok(token) => {
                    let token = token.trim();
                    if !token.is_empty() {
                        info!("Using HuggingFace token from file");
                        params.hf_token = Some(token.to_string());
                    } else {
                        // Empty file
                        warn!("HuggingFace token file is empty");
                        // If no token is available, disable diarization
                        if params.diarize {
                            warn!(
                                "Disabling diarization because no HuggingFace token is available"
                            );
                            params.diarize = false;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read HuggingFace token file: {}", e);
                    // If no token is available, disable diarization
                    if params.diarize {
                        warn!("Disabling diarization because no HuggingFace token is available");
                        params.diarize = false;
                    }
                }
            }
        } else {
            warn!(
                "HuggingFace token file not found at: {}",
                config.hf_token_file
            );
            // If no token is available, disable diarization
            if params.diarize {
                warn!("Disabling diarization because no HuggingFace token is available");
                params.diarize = false;
            }
        }
    }

    Ok((params, job_paths))
}
