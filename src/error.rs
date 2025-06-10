// Error handling for Whisper API
//
// This module defines error types and handling for the Whisper API.
// It centralizes error definitions and provides helpful conversion traits.

use std::io;
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::oneshot;

use actix_web::{HttpResponse, ResponseError};

use crate::models::ErrorResponse;

/// Errors that can occur in the Whisper API handlers
#[derive(Error, Debug)]
pub enum HandlerError {
    /// Error when processing multipart form data
    #[error("Form error: {0}")]
    FormError(String),

    /// Error when saving file data
    #[error("File error: {0}")]
    FileError(#[from] io::Error),

    /// Error when no audio file was provided
    #[error("No audio file provided in the request")]
    NoAudioFile,

    /// Error with an invalid output format
    #[error("Invalid output format: {0}. Valid formats are: srt, vtt, txt, tsv, json, aud")]
    InvalidOutputFormat(String),

    /// Error when a file is too large
    #[error("File too large: {0} bytes exceeds limit of {1} bytes")]
    FileTooLarge(usize, usize),

    /// Error when adding job to queue
    #[error("Queue error: {0}")]
    QueueError(String),

    /// Error when job is not found
    #[error("Job not found: {0}")]
    JobNotFound(String),

    /// Error when job cannot be canceled
    #[error("Cannot cancel job: {0}")]
    CannotCancelJob(String),
    
    /// Error when communication channel fails
    #[error("Communication error: channel closed")]
    ChannelError,
    
    /// Error when a synchronous job times out
    #[error("Synchronous processing timeout after {0} seconds")]
    SyncTimeout(u64),
}

impl HandlerError {
    /// Create a new FormError
    pub fn form_error<S: Into<String>>(msg: S) -> Self {
        Self::FormError(msg.into())
    }
    
    /// Helper to clean up a folder when error occurs
    pub fn with_cleanup(self, folder: Option<&PathBuf>) -> Self {
        if let Some(folder) = folder {
            crate::file_utils::cleanup_folder(folder);
        }
        self
    }
}

impl ResponseError for HandlerError {
    fn error_response(&self) -> HttpResponse {
        let error_response = ErrorResponse {
            error: self.to_string(),
            status: None,
        };

        match self {
            HandlerError::NoAudioFile
            | HandlerError::InvalidOutputFormat(_)
            | HandlerError::FormError(_) => HttpResponse::BadRequest().json(error_response),
            HandlerError::JobNotFound(_) => HttpResponse::NotFound().json(error_response),
            HandlerError::CannotCancelJob(_) => HttpResponse::BadRequest().json(error_response),
            HandlerError::FileTooLarge(_, _) => {
                HttpResponse::PayloadTooLarge().json(error_response)
            }
            _ => HttpResponse::InternalServerError().json(error_response),
        }
    }
}

/// Convert QueueError to HandlerError
impl From<crate::queue_manager::QueueError> for HandlerError {
    fn from(err: crate::queue_manager::QueueError) -> Self {
        use crate::queue_manager::QueueError;

        match err {
            QueueError::JobNotFound(id) => HandlerError::JobNotFound(id),
            QueueError::CannotCancelJob(reason) => HandlerError::CannotCancelJob(reason),
            QueueError::InvalidOutputFormat(format) => HandlerError::InvalidOutputFormat(format),
            QueueError::IoError(e) => HandlerError::FileError(e),
            QueueError::JsonError(e) => HandlerError::QueueError(e.to_string()),
            QueueError::TranscriptionError(e) => HandlerError::QueueError(e),
            QueueError::QueueError(e) => HandlerError::QueueError(e),
        }
    }
}

/// Convert oneshot::RecvError to HandlerError
impl From<oneshot::error::RecvError> for HandlerError {
    fn from(_: oneshot::error::RecvError) -> Self {
        HandlerError::ChannelError
    }
}
