//! Queue Manager for Whisper API
//!
//! This module implements a simple FIFO queue management system for processing audio transcription
//! requests asynchronously. It processes one job at a time to ensure WhisperX can use all available
//! physical resources for each transcription job.

use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};

/// Configuration for the WhisperX command
#[derive(Clone, Debug)]
pub struct WhisperConfig {
    /// Path to the WhisperX command
    pub command_path: String,
    /// Path to the models directory
    pub models_dir: String,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            command_path: std::env::var("WHISPER_CMD")
                .unwrap_or_else(|_| String::from("whisper-cli")),
            models_dir: std::env::var("WHISPER_MODELS_DIR")
                .unwrap_or_else(|_| String::from("/models")),
        }
    }
}

/// Job status enum for tracking the progress of transcription jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "data")]
pub enum JobStatus {
    /// Job is waiting in the queue
    Queued,
    /// Job is currently being processed
    Processing,
    /// Job is completed with transcription result
    Completed(String),
    /// Job failed with error message
    Failed(String),
}

/// Transcription job structure
#[derive(Debug, Clone)]
pub struct TranscriptionJob {
    /// Unique identifier for the job
    pub id: String,
    /// Path to the audio file
    pub audio_file: PathBuf,
    /// Path to the folder containing the job files
    pub folder_path: PathBuf,
    /// Language code for transcription
    pub language: String,
    /// Model name to use
    pub model: String,
}

/// Transcription result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Transcription text
    pub text: String,
    /// Language detected
    pub language: String,
    /// Segments with timestamps (if available)
    #[serde(default)]
    pub segments: Vec<TranscriptionSegment>,
}

/// Transcription segment with timestamp information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptionSegment {
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
    /// Segment text
    pub text: String,
}

/// Queue manager error types
#[derive(Error, Debug)]
pub enum QueueError {
    /// Job not found in the system
    #[error("Job not found: {0}")]
    JobNotFound(String),
    /// I/O error during file operations
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    /// Error while running the WhisperX command
    #[error("Transcription error: {0}")]
    TranscriptionError(String),
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// Internal error in the queue system
    #[error("Queue error: {0}")]
    QueueError(String),
}

/// Internal state of the queue manager
struct QueueState {
    /// Queue of jobs waiting to be processed
    queue: VecDeque<TranscriptionJob>,
    /// Map of job statuses by job ID
    statuses: HashMap<String, JobStatus>,
    /// Map of job results by job ID
    results: HashMap<String, TranscriptionResult>,
    /// Flag indicating if a job is currently being processed
    processing: bool,
}

/// Queue Manager for handling transcription jobs
pub struct QueueManager {
    /// Internal state protected by a mutex
    state: Arc<Mutex<QueueState>>,
    /// Job processor channel
    job_tx: mpsc::Sender<TranscriptionJob>,
    /// Configuration for WhisperX
    config: WhisperConfig,
}

impl QueueManager {
    /// Create a new queue manager instance and start background processing
    pub fn new(config: WhisperConfig) -> Arc<Mutex<Self>> {
        // Create channels for job processing
        let (job_tx, job_rx) = mpsc::channel(100);

        // Initialize internal state
        let state = Arc::new(Mutex::new(QueueState {
            queue: VecDeque::new(),
            statuses: HashMap::new(),
            results: HashMap::new(),
            processing: false,
        }));

        // Create the queue manager
        let manager = Arc::new(Mutex::new(Self {
            state: state.clone(),
            job_tx: job_tx.clone(),
            config: config.clone(),
        }));

        // Start the background processor
        Self::start_job_processor(job_rx, job_tx, state, config);

        manager
    }

    /// Start the background job processor
    fn start_job_processor(
        mut job_rx: mpsc::Receiver<TranscriptionJob>,
        job_tx: mpsc::Sender<TranscriptionJob>,
        state: Arc<Mutex<QueueState>>,
        config: WhisperConfig,
    ) {
        tokio::spawn(async move {
            info!("Job processor started");

            while let Some(job) = job_rx.recv().await {
                let job_id = job.id.clone();
                info!("Processing job: {}", job_id);

                // Update job status to processing
                {
                    let mut state_guard = state.lock().await;
                    state_guard
                        .statuses
                        .insert(job_id.clone(), JobStatus::Processing);
                }

                // Process the job
                let result = Self::process_transcription(job.clone(), &config).await;

                // Update job status based on result
                {
                    let mut state_guard = state.lock().await;
                    match result {
                        Ok(result) => {
                            info!("Job {} completed successfully", job_id);
                            state_guard
                                .statuses
                                .insert(job_id.clone(), JobStatus::Completed(result.text.clone()));
                            state_guard.results.insert(job_id.clone(), result);
                        }
                        Err(e) => {
                            error!("Job {} failed: {}", job_id, e);
                            state_guard
                                .statuses
                                .insert(job_id, JobStatus::Failed(e.to_string()));
                        }
                    }

                    // Mark job as no longer processing
                    state_guard.processing = false;
                }

                // Check if there are more jobs to process
                {
                    let mut state_guard = state.lock().await;
                    if !state_guard.queue.is_empty() {
                        let next_job = state_guard.queue.pop_front().unwrap();
                        state_guard.processing = true;
                        drop(state_guard); // Release lock before sending

                        if let Err(e) = job_tx.send(next_job).await {
                            error!("Failed to send next job to processor: {}", e);
                            // Reset processing flag if we failed to send the job
                            let mut state_guard = state.lock().await;
                            state_guard.processing = false;
                        }
                    }
                }
            }

            info!("Job processor stopped");
        });
    }

    /// Check if there are jobs waiting and start the next one if not currently processing
    async fn check_and_start_next_job(&self) -> Result<(), QueueError> {
        // Check if we have jobs waiting and we're not currently processing one
        let next_job = {
            let mut state_guard = self.state.lock().await;
            if !state_guard.processing && !state_guard.queue.is_empty() {
                state_guard.processing = true;
                state_guard.queue.pop_front()
            } else {
                None
            }
        };

        // If we have a job to process, send it to the processor
        if let Some(job) = next_job {
            debug!("Starting next job: {}", job.id);
            if let Err(e) = self.job_tx.send(job).await {
                error!("Failed to send job to processor: {}", e);

                // Reset processing flag if we failed to send the job
                let mut state_guard = self.state.lock().await;
                state_guard.processing = false;
                return Err(QueueError::QueueError(format!("Failed to send job: {}", e)));
            }
        }

        Ok(())
    }

    /// Process a transcription job by invoking WhisperX
    async fn process_transcription(
        job: TranscriptionJob,
        config: &WhisperConfig,
    ) -> Result<TranscriptionResult, QueueError> {
        debug!("Processing job {}", job.id);

        // Execute whisper command
        let output = Command::new(&config.command_path)
            .arg("-m")
            .arg(format!("{}/ggml-{}.bin", config.models_dir, job.model))
            .arg("-l")
            .arg(&job.language)
            .arg("-f")
            .arg(&job.audio_file)
            .arg("-oj")
            .output()
            .map_err(|e| QueueError::TranscriptionError(format!("Failed to run command: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(QueueError::TranscriptionError(error));
        }

        // Parse the JSON output
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let result: TranscriptionResult = serde_json::from_str(&stdout)
            .map_err(|e| QueueError::TranscriptionError(format!("Failed to parse JSON: {}", e)))?;

        // Save result to a file in the job folder
        let result_path = job.folder_path.join("transcript.json");
        let result_json =
            serde_json::to_string_pretty(&result).map_err(|e| QueueError::JsonError(e))?;
        fs::write(&result_path, result_json).map_err(|e| QueueError::IoError(e))?;

        Ok(result)
    }

    /// Add a job to the queue
    pub async fn add_job(&self, job: TranscriptionJob) -> Result<(), QueueError> {
        let job_id = job.id.clone();

        {
            let mut state = self.state.lock().await;
            state.queue.push_back(job);
            state.statuses.insert(job_id.clone(), JobStatus::Queued);
        }

        // Check if we need to start processing
        if let Err(e) = self.check_and_start_next_job().await {
            error!("Failed to start next job: {}", e);
        }

        info!("Job {} added to queue", job_id);
        Ok(())
    }

    /// Get the status of a job
    pub async fn get_job_status(&self, job_id: &str) -> Result<JobStatus, QueueError> {
        let state = self.state.lock().await;
        state
            .statuses
            .get(job_id)
            .cloned()
            .ok_or_else(|| QueueError::JobNotFound(job_id.to_string()))
    }

    /// Get the result of a completed job
    pub async fn get_job_result(&self, job_id: &str) -> Result<TranscriptionResult, QueueError> {
        let state = self.state.lock().await;

        // Check job status first
        match state.statuses.get(job_id) {
            Some(JobStatus::Completed(_)) => state
                .results
                .get(job_id)
                .cloned()
                .ok_or_else(|| QueueError::JobNotFound(job_id.to_string())),
            Some(JobStatus::Failed(err)) => Err(QueueError::TranscriptionError(err.clone())),
            Some(_) => Err(QueueError::QueueError("Job not completed yet".to_string())),
            None => Err(QueueError::JobNotFound(job_id.to_string())),
        }
    }

    /// Clean up a job and its resources
    pub async fn cleanup_job(&self, job_id: &str) -> Result<(), QueueError> {
        let mut state = self.state.lock().await;

        // Check if job exists and is in a terminal state
        if let Some(status) = state.statuses.get(job_id) {
            if !matches!(status, JobStatus::Completed(_) | JobStatus::Failed(_)) {
                return Err(QueueError::QueueError(
                    "Cannot clean up a job that is still in progress".to_string(),
                ));
            }

            // Find job folder path
            for job in &state.queue {
                if job.id == job_id {
                    // Remove job files
                    if let Err(e) = fs::remove_dir_all(&job.folder_path) {
                        error!(
                            "Failed to remove job folder {}: {}",
                            job.folder_path.display(),
                            e
                        );
                        return Err(QueueError::IoError(e));
                    }
                    break;
                }
            }

            // Remove job from state tracking
            state.statuses.remove(job_id);
            state.results.remove(job_id);

            Ok(())
        } else {
            Err(QueueError::JobNotFound(job_id.to_string()))
        }
    }
}
