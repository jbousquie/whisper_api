//! Queue Manager for Whisper API
//!
//! This module implements a simple FIFO queue management system for processing audio transcription
//! requests asynchronously. It processes one job at a time to ensure WhisperX can use all available
//! physical resources for each transcription job.

use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};

const DEFAULT_WHISPER_CMD: &str = "whisperx";
const DEFAULT_WHISPER_MODELS_DIR: &str = "/models";
const DEFAULT_JOB_RETENTION_HOURS: u64 = 48;
const DEFAULT_CLEANUP_INTERVAL_HOURS: u64 = 1;

// Environment variable names
const ENV_JOB_RETENTION_HOURS: &str = "WHISPER_JOB_RETENTION_HOURS";
const ENV_CLEANUP_INTERVAL_HOURS: &str = "WHISPER_CLEANUP_INTERVAL_HOURS";

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
                .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_CMD)),
            models_dir: std::env::var("WHISPER_MODELS_DIR")
                .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_MODELS_DIR)),
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
    /// Whether to apply speaker diarization
    pub diarize: bool,
    /// Optional initial prompt for transcription
    pub prompt: String,
}

/// Job metadata saved separately from the job itself
#[derive(Debug, Clone)]
struct JobMetadata {
    /// Path to the folder containing job files
    folder_path: PathBuf,
    /// Timestamp when the job was created
    created_at: SystemTime,
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
    /// Cannot cancel job in its current state
    #[error("Cannot cancel job: {0}")]
    CannotCancelJob(String),
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
    /// This is the key mechanism that ensures only one job runs at a time.
    /// When true, no new jobs will be started from the queue.
    processing: bool,
    /// Map of job metadata by job ID
    job_metadata: HashMap<String, JobMetadata>,
}

/// Queue Manager for handling transcription jobs
pub struct QueueManager {
    /// Internal state protected by a mutex
    /// The mutex ensures that all state access is atomic and thread-safe,
    /// preventing race conditions when checking or updating the processing flag.
    state: Arc<Mutex<QueueState>>,
    /// Job processor channel
    /// This single channel is used to send jobs to a single background task,
    /// ensuring jobs are processed one at a time in the order they're received.
    job_tx: mpsc::Sender<TranscriptionJob>,
}

impl QueueManager {
    /// Create a new queue manager instance and start background processing
    pub fn new(config: WhisperConfig) -> Arc<Mutex<Self>> {
        // Create channels for job processing
        // This single channel approach ensures jobs are processed sequentially
        let (job_tx, job_rx) = mpsc::channel(100);

        // Initialize internal state
        let state = Arc::new(Mutex::new(QueueState {
            queue: VecDeque::new(),
            statuses: HashMap::new(),
            results: HashMap::new(),
            processing: false, // Initially not processing any job
            job_metadata: HashMap::new(),
        }));

        // Create the queue manager
        let manager = Arc::new(Mutex::new(Self {
            state: state.clone(),
            job_tx: job_tx.clone(),
        }));

        // Start the background processor - only one processor task is created
        // This single task listens to the job channel and processes jobs one at a time
        Self::start_job_processor(job_rx, job_tx, state.clone(), config);

        // Start the cleanup task
        Self::start_cleanup_task(Arc::clone(&manager));

        manager
    }

    /// Start the background job processor
    /// This creates a single processing task that handles one job at a time
    fn start_job_processor(
        mut job_rx: mpsc::Receiver<TranscriptionJob>,
        job_tx: mpsc::Sender<TranscriptionJob>,
        state: Arc<Mutex<QueueState>>,
        config: WhisperConfig,
    ) {
        tokio::spawn(async move {
            info!("Job processor started");

            // Process jobs one at a time in the order they're received
            // This loop ensures sequential processing - one job must complete
            // before the next one is handled
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

                    // Mark job as no longer processing - this is critical for the sequential processing
                    // The flag must be cleared after a job finishes so the next job can start
                    state_guard.processing = false;
                }

                // Sequential job chaining: after completing one job, check if there are more jobs
                // and start the next one, maintaining the FIFO order
                {
                    let mut state_guard = state.lock().await;
                    if !state_guard.queue.is_empty() {
                        let next_job = state_guard.queue.pop_front().unwrap();
                        // Set processing flag to true for the next job
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
    /// This is the job starting logic that enforces sequential processing
    async fn check_and_start_next_job(&self) -> Result<(), QueueError> {
        // Check if we have jobs waiting and we're not currently processing one
        // The critical part is checking !state_guard.processing which ensures
        // we never start a new job if one is already running
        let next_job = {
            let mut state_guard = self.state.lock().await;
            if !state_guard.processing && !state_guard.queue.is_empty() {
                // Set flag BEFORE starting job to prevent concurrent starts
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
        let mut command = Command::new(&config.command_path);
        
        command
            .arg("-m")
            .arg(format!("{}/ggml-{}.bin", config.models_dir, job.model))
            .arg("-l")
            .arg(&job.language)
            .arg("-f")
            .arg(&job.audio_file)
            .arg("-oj");
            
        // Add diarization if requested
        if job.diarize {
            command.arg("--diarize");
        }
        
        // Add initial prompt if provided
        if !job.prompt.is_empty() {
            command.arg("--initial_prompt").arg(&job.prompt);
        }
        
        let output = command.output()
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

    /// Start a background task to periodically clean up old jobs
    fn start_cleanup_task(queue_manager: Arc<Mutex<Self>>) {
        // Get cleanup interval from environment variable or use default
        let cleanup_interval_hours = std::env::var(ENV_CLEANUP_INTERVAL_HOURS)
            .ok()
            .and_then(|val| val.parse::<u64>().ok())
            .unwrap_or(DEFAULT_CLEANUP_INTERVAL_HOURS);
        let cleanup_interval = Duration::from_secs(cleanup_interval_hours * 3600);

        // Get retention period from environment variable or use default
        let job_retention_hours = std::env::var(ENV_JOB_RETENTION_HOURS)
            .ok()
            .and_then(|val| val.parse::<u64>().ok())
            .unwrap_or(DEFAULT_JOB_RETENTION_HOURS);
        let max_age = Duration::from_secs(job_retention_hours * 3600);

        info!(
            "Starting cleanup task: retention period {} hours, interval {} hours",
            job_retention_hours, cleanup_interval_hours
        );

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;
                info!("Running scheduled cleanup of old jobs");

                // Get a copy of the QueueManager
                let queue_manager = queue_manager.lock().await;

                // Find and clean up old jobs
                match queue_manager.cleanup_old_jobs(max_age).await {
                    Ok(count) => {
                        if count > 0 {
                            info!("Cleaned up {} expired jobs", count);
                        } else {
                            debug!("No expired jobs to clean up");
                        }
                    }
                    Err(e) => {
                        error!("Error during scheduled cleanup: {}", e);
                    }
                }
            }
        });
    }

    /// Clean up jobs older than the specified duration
    async fn cleanup_old_jobs(&self, max_age: Duration) -> Result<usize, QueueError> {
        let now = SystemTime::now();
        let mut cleanup_count = 0;

        // Collect job IDs to clean up
        let job_ids_to_clean: Vec<String> = {
            let state = self.state.lock().await;
            state
                .job_metadata
                .iter()
                .filter_map(
                    |(job_id, metadata)| match now.duration_since(metadata.created_at) {
                        Ok(age) if age > max_age => Some(job_id.clone()),
                        _ => None,
                    },
                )
                .collect()
        };

        // Clean up each expired job
        for job_id in job_ids_to_clean {
            match self.cleanup_job(&job_id).await {
                Ok(_) => {
                    info!("Cleaned up expired job: {}", job_id);
                    cleanup_count += 1;
                }
                Err(e) => {
                    warn!("Failed to clean up expired job {}: {}", job_id, e);
                }
            }
        }

        Ok(cleanup_count)
    }

    /// Add a job to the queue
    /// This is the entry point for new transcription requests
    pub async fn add_job(&self, job: TranscriptionJob) -> Result<(), QueueError> {
        let job_id = job.id.clone();

        {
            // Acquire mutex lock to safely modify state
            // This ensures thread-safe access to the job queue and other state
            let mut state = self.state.lock().await;

            // Save job metadata before adding to queue
            state.job_metadata.insert(
                job_id.clone(),
                JobMetadata {
                    folder_path: job.folder_path.clone(),
                    created_at: SystemTime::now(),
                },
            );

            // Add job to queue (FIFO order) and update status
            // Jobs are always added to the back of the queue
            state.queue.push_back(job);
            state.statuses.insert(job_id.clone(), JobStatus::Queued);
        } // Lock is automatically released when state goes out of scope

        // Check if we need to start processing
        // This will only start a job if nothing is currently processing
        // which enforces the sequential processing requirement
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

    /// Cancel a pending job and remove its resources
    pub async fn cancel_job(&self, job_id: &str) -> Result<(), QueueError> {
        let mut state = self.state.lock().await;

        // Check if job exists
        if let Some(status) = state.statuses.get(job_id) {
            // Only allow cancellation of queued jobs, not processing or completed jobs
            match status {
                JobStatus::Queued => {
                    // Remove the job from the queue if it's there
                    state.queue.retain(|job| job.id != job_id);

                    // Get folder path from job metadata
                    if let Some(metadata) = state.job_metadata.get(job_id) {
                        let folder_path = metadata.folder_path.clone();
                        drop(state); // Release lock before filesystem operations

                        // Remove job files
                        if let Err(e) = fs::remove_dir_all(&folder_path) {
                            error!("Failed to remove folder for canceled job {}: {}", job_id, e);
                            // Continue with cancellation even if file removal fails
                        }

                        // Re-acquire lock to update state
                        let mut state = self.state.lock().await;
                        // Remove job from tracking
                        state.statuses.remove(job_id);
                        state.results.remove(job_id);
                        state.job_metadata.remove(job_id);
                    } else {
                        // No folder path found, but still remove job from tracking
                        state.statuses.remove(job_id);
                        state.results.remove(job_id);
                        state.job_metadata.remove(job_id);
                    }

                    info!("Canceled job: {}", job_id);
                    Ok(())
                }
                JobStatus::Processing => Err(QueueError::CannotCancelJob(
                    "Cannot cancel a job that is currently processing".to_string(),
                )),
                JobStatus::Completed(_) | JobStatus::Failed(_) => {
                    // For completed/failed jobs, we'll just clean them up
                    drop(state); // Release lock before calling another method
                    self.cleanup_job(job_id).await
                }
            }
        } else {
            Err(QueueError::JobNotFound(job_id.to_string()))
        }
    }

    /// Clean up a job and its resources
    pub async fn cleanup_job(&self, job_id: &str) -> Result<(), QueueError> {
        let mut state = self.state.lock().await;

        // Check if job exists
        if let Some(status) = state.statuses.get(job_id) {
            // Only enforce "completed" check for manual cleanup, not for expiration cleanup
            if !matches!(status, JobStatus::Completed(_) | JobStatus::Failed(_)) {
                return Err(QueueError::QueueError(
                    "Cannot clean up a job that is still in progress".to_string(),
                ));
            }

            // Get folder path from job metadata
            let folder_path = state
                .job_metadata
                .get(job_id)
                .map(|metadata| &metadata.folder_path);

            // Try to remove files if we found a folder path
            if let Some(path) = folder_path {
                if let Err(e) = fs::remove_dir_all(path) {
                    error!("Failed to remove job folder {}: {}", path.display(), e);
                    // Don't return error here, we still want to clean up the job state
                }
            } else {
                warn!(
                    "No folder path found for job {}, skipping file cleanup",
                    job_id
                );
            }

            // Remove job from state tracking
            state.statuses.remove(job_id);
            state.results.remove(job_id);
            state.job_metadata.remove(job_id);

            Ok(())
        } else {
            Err(QueueError::JobNotFound(job_id.to_string()))
        }
    }
}
