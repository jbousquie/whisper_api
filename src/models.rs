// Whisper API data models
//
// This module contains the data models used for the Whisper API.
// It includes request and response types used across the API.

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
    /// Output format (srt, vtt, txt, tsv, json, aud)
    pub output_format: Option<String>,
    /// Path to the uploaded audio file
    pub audio_file: Option<PathBuf>,
    /// Path to the folder containing the job files
    pub folder_path: Option<PathBuf>,
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
