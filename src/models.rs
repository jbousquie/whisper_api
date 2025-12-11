// Whisper API data models
//
// This module contains the data models used for the Whisper API.
// It includes request and response types used across the API.

use crate::queue_manager::JobStatus;
use serde::Serialize;
use std::path::PathBuf;

/// Response for transcription request
#[derive(Serialize)]
pub struct TranscriptionResponse {
    /// Job ID assigned to the transcription request
    pub job_id: String,
    /// URL to check the status of the transcription
    pub status_url: String,
}

/// Request parameters for transcription
#[derive(Debug, Default)]
pub struct TranscriptionParams {
    /// Language for transcription (e.g., "en", "fr")
    pub language: String,
    /// Model to use (e.g., "large-v3", "tiny")
    pub model: String,
    /// Enable speaker diarization
    pub diarize: bool,
    /// Initial prompt to guide transcription
    pub prompt: String,
    /// HuggingFace token for diarization
    pub hf_token: Option<String>,
    /// Response format (srt, vtt, txt, tsv, json, aud)
    pub response_format: Option<String>,
    /// Path to the uploaded audio file
    pub audio_file: Option<PathBuf>,
    /// Path to the folder containing the job files
    pub folder_path: Option<PathBuf>,
    /// Synchronous processing mode (true = wait for result, false = async job)
    pub sync: bool,
    /// Device for PyTorch inference (cuda or cpu)
    pub device: Option<String>,
    /// Device index for inference when using CUDA
    pub device_index: Option<String>,
}

impl TranscriptionParams {
    /// Validates the parameters and returns an error if any are invalid
    pub fn validate(&self) -> Result<(), String> {
        // Validate language code (should be 2-3 characters)
        if !self.language.is_empty() && (self.language.len() < 2 || self.language.len() > 3) {
            return Err(format!("Invalid language code: {}", self.language));
        }
        
        // Validate model name (should not be empty)
        if self.model.is_empty() {
            return Err("Model name cannot be empty".to_string());
        }
        
        // Validate prompt length
        if self.prompt.len() > 1000 {
            return Err("Prompt is too long (max 1000 characters)".to_string());
        }
        
        // Validate device if provided
        if let Some(device) = &self.device {
            if device != "cuda" && device != "cpu" {
                return Err(format!("Invalid device: {}. Must be 'cuda' or 'cpu'", device));
            }
        }
        
        // Validate device index if provided
        if let Some(device_index) = &self.device_index {
            if device_index.parse::<i32>().is_err() {
                return Err(format!("Invalid device index: {}", device_index));
            }
        }
        
        Ok(())
    }
}

/// Error response for API
#[derive(Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// Optional status information
    pub status: Option<String>,
}

/// Success response for API
#[derive(Serialize)]
pub struct SuccessResponse {
    /// Success flag
    pub success: bool,
    /// Message describing the successful operation
    pub message: String,
}

/// Status response for job status requests
#[derive(Serialize)]
pub struct StatusResponse {
    /// Current job status
    pub status: JobStatus,
    /// Position in the queue (1-based), if job is queued
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<usize>,
}
