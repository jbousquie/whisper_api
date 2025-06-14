// Whisper API Library
//
// This crate provides an HTTP API for audio transcription using WhisperX.
// It implements a queue-based architecture for processing transcription requests.

pub mod config;
pub mod config_loader;
pub mod config_validator;
pub mod error;
pub mod file_utils;
pub mod handlers;
pub mod metrics;
pub mod models;
pub mod queue_manager;

// Re-export common types for easier access
pub use config::HandlerConfig;
pub use config::MetricsConfig;
pub use config_loader::load_config;
pub use config_validator::{WhisperConfigValidator, ValidationResults};
pub use error::HandlerError;
pub use handlers::{cancel_transcription, transcribe, transcription_result, transcription_status};
pub use metrics::metrics::Metrics;
pub use models::{TranscriptionResponse, ErrorResponse, SuccessResponse};
pub use queue_manager::{QueueManager, TranscriptionJob, WhisperConfig, SyncCompletionSender};
