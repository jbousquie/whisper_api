// Whisper API HTTP handlers
//
// This module contains the HTTP handlers for the Whisper API.
// It provides the interface between HTTP requests and the transcription queue.

pub mod authentication;
pub mod form;
pub mod routes;

// Re-export handlers for easier access
pub use self::routes::{
    cancel_transcription, transcribe, transcription_result, transcription_status,
};
// Re-export authentication middleware
pub use self::authentication::Authentication;
