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
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};
use crate::metrics::Metrics;

const DEFAULT_WHISPER_CMD: &str = "/home/llm/whisper_api/whisperx.sh";
const DEFAULT_WHISPER_MODELS_DIR: &str = "/home/llm/models/whisperx_models";
const DEFAULT_WHISPER_OUTPUT_DIR: &str = "/home/llm/whisper_api/output";
const DEFAULT_WHISPER_OUTPUT_FORMAT: &str = "txt";
const DEFAULT_JOB_RETENTION_HOURS: u64 = 48;
const DEFAULT_CLEANUP_INTERVAL_HOURS: u64 = 1;

// Valid output formats for WhisperX
const VALID_OUTPUT_FORMATS: [&str; 6] = ["srt", "vtt", "txt", "tsv", "json", "aud"];

// Environment variable names
const ENV_JOB_RETENTION_HOURS: &str = "WHISPER_JOB_RETENTION_HOURS";
const ENV_CLEANUP_INTERVAL_HOURS: &str = "WHISPER_CLEANUP_INTERVAL_HOURS";
const ENV_WHISPER_OUTPUT_DIR: &str = "WHISPER_OUTPUT_DIR";
const ENV_WHISPER_OUTPUT_FORMAT: &str = "WHISPER_OUTPUT_FORMAT";

/// Configuration for the WhisperX command
#[derive(Clone, Debug)]
pub struct WhisperConfig {
    /// Path to the WhisperX command
    pub command_path: String,
    /// Path to the models directory
    pub models_dir: String,
    /// Path to the output directory
    pub output_dir: String,
    /// Default output format
    pub output_format: String,
}

impl WhisperConfig {
    /// Validates if a given format is in the list of allowed output formats
    pub fn validate_output_format(format: &str) -> Result<(), QueueError> {
        if VALID_OUTPUT_FORMATS.contains(&format) {
            Ok(())
        } else {
            Err(QueueError::InvalidOutputFormat(format.to_string()))
        }
    }
}

impl Default for WhisperConfig {
    fn default() -> Self {
        let output_format = std::env::var(ENV_WHISPER_OUTPUT_FORMAT)
            .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_OUTPUT_FORMAT));

        // Validate the output format from env var
        if WhisperConfig::validate_output_format(output_format.as_str()).is_err() {
            warn!(
                "Invalid output format in environment variable: {}. Using default: {}",
                output_format, DEFAULT_WHISPER_OUTPUT_FORMAT
            );

            Self {
                command_path: std::env::var("WHISPER_CMD")
                    .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_CMD)),
                models_dir: std::env::var("WHISPER_MODELS_DIR")
                    .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_MODELS_DIR)),
                output_dir: std::env::var(ENV_WHISPER_OUTPUT_DIR)
                    .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_OUTPUT_DIR)),
                output_format: DEFAULT_WHISPER_OUTPUT_FORMAT.to_string(),
            }
        } else {
            Self {
                command_path: std::env::var("WHISPER_CMD")
                    .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_CMD)),
                models_dir: std::env::var("WHISPER_MODELS_DIR")
                    .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_MODELS_DIR)),
                output_dir: std::env::var(ENV_WHISPER_OUTPUT_DIR)
                    .unwrap_or_else(|_| String::from(DEFAULT_WHISPER_OUTPUT_DIR)),
                output_format,
            }
        }
    }
}

/// Job status enum for tracking the progress of transcription jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum JobStatus {
    /// Job is waiting in the queue
    Queued,
    /// Job is currently being processed
    Processing,
    /// Job is completed successfully
    Completed,
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
    /// Hugging Face token for accessing diarization models
    pub hf_token: Option<String>,
    /// Output format for transcription results
    pub output_format: Option<String>,
}

/// Job metadata saved separately from the job itself
#[derive(Debug, Clone)]
struct JobMetadata {
    /// Path to the folder containing job files
    folder_path: PathBuf,
    /// Timestamp when the job was created
    created_at: SystemTime,
    /// Path to the output file in the WhisperX output directory
    output_file_path: Option<PathBuf>,
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
    /// Invalid output format specified
    #[error("Invalid output format: {0}")]
    InvalidOutputFormat(String),
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
    /// Metrics collection and export
    metrics: Metrics,
}

impl QueueManager {    /// Create a new queue manager instance and start background processing
    pub fn new(config: WhisperConfig, metrics: Metrics) -> Arc<Mutex<Self>> {
        // Create channels for job processing
        // This single channel approach ensures jobs are processed sequentially
        let (job_tx, job_rx) = mpsc::channel(100);        // Initialize internal state
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
            metrics: metrics.clone(),
        }));

        // Start the background processor - only one processor task is created
        // This single task listens to the job channel and processes jobs one at a time
        Self::start_job_processor(job_rx, job_tx, state.clone(), config, metrics);

        // Start the cleanup task
        Self::start_cleanup_task(Arc::clone(&manager));

        manager
    }    /// Start the background job processor
    /// This creates a single processing task that handles one job at a time
    fn start_job_processor(
        mut job_rx: mpsc::Receiver<TranscriptionJob>,
        job_tx: mpsc::Sender<TranscriptionJob>,
        state: Arc<Mutex<QueueState>>,
        config: WhisperConfig,
        metrics: Metrics,
    ) {
        tokio::spawn(async move {
            info!("Job processor started");

            // Process jobs one at a time in the order they're received
            // This loop ensures sequential processing - one job must complete
            // before the next one is handled
            while let Some(job) = job_rx.recv().await {                let job_id = job.id.clone();
                info!("Processing job: {}", job_id);

                // Record job processing start and update queue size
                metrics.record_job_processing_start().await;
                
                // Update queue size metrics (job is now dequeued)
                {
                    let state_guard = state.lock().await;
                    metrics.record_queue_size(state_guard.queue.len()).await;
                }

                // Update job status to processing
                {
                    let mut state_guard = state.lock().await;
                    state_guard
                        .statuses
                        .insert(job_id.clone(), JobStatus::Processing);
                }// Process the job
                let result = Self::process_transcription(job.clone(), &config, state.clone(), &metrics).await;

                // Update job status based on result
                {
                    let mut state_guard = state.lock().await;
                    match &result {
                        Ok(result) => {
                            info!("Job {} completed successfully", job_id);
                            state_guard
                                .statuses
                                .insert(job_id.clone(), JobStatus::Completed);
                            state_guard.results.insert(job_id.clone(), result.clone());
                            
                            // The output file path is already set in process_transcription
                        }
                        Err(e) => {
                            error!("Job {} failed: {}", job_id, e);
                            state_guard
                                .statuses
                                .insert(job_id.clone(), JobStatus::Failed(e.to_string()));
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
    }    /// Process a transcription job by invoking WhisperX
    async fn process_transcription(
        job: TranscriptionJob,
        config: &WhisperConfig,
        state: Arc<Mutex<QueueState>>,
        metrics: &Metrics,
    ) -> Result<TranscriptionResult, QueueError> {
        let start_time = std::time::Instant::now();
        debug!("Processing job {}", job.id);

        // Record job processing start
        metrics.record_job_submitted(&job.model, &job.language).await;

        // Validate output format if provided
        // This determines the output file format (e.g., srt, vtt, txt, json)
        let output_format = match &job.output_format {
            Some(format) => {
                WhisperConfig::validate_output_format(format.as_str())?;
                format.as_str()
            }
            None => &config.output_format,
        };

        // Execute whisper command
        let mut command = Command::new(&config.command_path);

        command
            .arg(&job.audio_file)
            .arg("--model")
            .arg(&job.model)
            .arg("--model_dir")
            .arg(format!("{}/{}", config.models_dir, job.model))
            .arg("--language")
            .arg(&job.language)
            .arg("--output_dir")
            .arg(&config.output_dir)
            .arg("--output_format")
            .arg(output_format);

        // Add diarization if requested
        if job.diarize {
            command.arg("--diarize");

            // Add HF token if available for diarization models
            if let Some(token) = &job.hf_token {
                if !token.is_empty() {
                    command.arg("--hf_token").arg(token);
                }
            }
        }

        // Add initial prompt if provided
        if !job.prompt.is_empty() {
            command.arg("--initial_prompt").arg(&job.prompt);
        }

        // WhisperX will name the output file based on the input audio filename
        // For example, if input is "recording.wav", output will be "recording.srt"
        let audio_file_name = Path::new(&job.audio_file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let output_filename = format!("{}.{}", audio_file_name, output_format);
        let output_file_path = Path::new(&config.output_dir).join(&output_filename);

        // Store the output file path in the job metadata for later cleanup
        // This will be retrieved and used during cleanup to remove the output file
        let output_file_path_clone = output_file_path.clone();        let output = command
            .output()
            .map_err(|e| {
                let duration = start_time.elapsed().as_secs_f64();
                // We need to use blocking call here since we're in a sync error context
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        metrics.record_job_completed(&job.model, &job.language, duration, "failed").await;
                    })
                });
                QueueError::TranscriptionError(format!("Failed to run command: {}", e))
            })?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).to_string();
            let duration = start_time.elapsed().as_secs_f64();
            metrics.record_job_completed(&job.model, &job.language, duration, "failed").await;
            return Err(QueueError::TranscriptionError(error));
        }

        // Read the result from the output file
        let file_content = fs::read_to_string(&output_file_path).map_err(|e| {
            QueueError::TranscriptionError(format!("Failed to read output file: {}", e))
        })?;

        // Copy the transcription result to the job's folder for convenience and persistence
        // This ensures the result is available even if the main output directory is cleaned
        // The copied file is named "transcript.{format}" regardless of the input filename
        let job_result_path = job
            .folder_path
            .join(format!("transcript.{}", output_format));
        fs::write(&job_result_path, &file_content).map_err(|e| QueueError::IoError(e))?;
        
        // Update job metadata with output file path for later cleanup
        {
            let mut state_guard = state.lock().await;
            if let Some(metadata) = state_guard.job_metadata.get_mut(&job.id) {
                metadata.output_file_path = Some(output_file_path_clone);
            }
        }        // Create result with the file content as-is
        let result = TranscriptionResult {
            text: file_content,
            language: job.language.clone(),
            segments: Vec::new(),
        };

        // Record metrics for job completion
        let duration = start_time.elapsed().as_secs_f64();
        metrics.record_job_completed(&job.model, &job.language, duration, "success").await;

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
    }    /// Add a job to the queue
    /// This is the entry point for new transcription requests
    pub async fn add_job(&self, job: TranscriptionJob) -> Result<(), QueueError> {
        let job_id = job.id.clone();

        // Record file size metrics
        if let Ok(metadata) = std::fs::metadata(&job.audio_file) {
            self.metrics.record_file_size(metadata.len() as f64).await;
        }

        // Validate output format if provided
        if let Some(format) = &job.output_format {
            WhisperConfig::validate_output_format(format.as_str())?;
        }

        let queue_size = {
            // Acquire mutex lock to safely modify state
            // This ensures thread-safe access to the job queue and other state
            let mut state = self.state.lock().await;

            // Save job metadata before adding to queue
            state.job_metadata.insert(
                job_id.clone(),
                JobMetadata {
                    folder_path: job.folder_path.clone(),
                    created_at: SystemTime::now(),
                    output_file_path: None, // Will be set during processing
                },
            );

            // Add job to queue (FIFO order) and update status
            // Jobs are always added to the back of the queue
            state.queue.push_back(job.clone());
            state.statuses.insert(job_id.clone(), JobStatus::Queued);
            
            state.queue.len()
        }; // Lock is automatically released when state goes out of scope

        // Record metrics for job submission and queue size
        self.metrics.record_job_submitted(&job.model, &job.language).await;
        self.metrics.set_queue_size(queue_size as f64).await;

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

    /// Get the position of a job in the queue (1-based index)
    /// 
    /// Returns Some(position) if the job is waiting in the queue, None if it's not in the queue
    /// Position 1 means it's the next job to be processed after the current one
    pub async fn get_job_position(&self, job_id: &str) -> Result<Option<usize>, QueueError> {
        let state = self.state.lock().await;
        
        // First check if the job exists
        if !state.statuses.contains_key(job_id) {
            return Err(QueueError::JobNotFound(job_id.to_string()));
        }
        
        // If the job is not in Queued status, it's not in the queue
        if !matches!(state.statuses.get(job_id), Some(JobStatus::Queued)) {
            return Ok(None);
        }
        
        // Find the position of the job in the queue (1-based index)
        for (position, job) in state.queue.iter().enumerate() {
            if job.id == job_id {
                // Add 1 to make it 1-based instead of 0-based
                return Ok(Some(position + 1));
            }
        }
        
        // If we get here, the job is in Queued status but not in the queue
        // This should not happen, but we handle it just in case
        Ok(None)
    }

    /// Get the result of a completed job
    pub async fn get_job_result(&self, job_id: &str) -> Result<TranscriptionResult, QueueError> {
        let state = self.state.lock().await;

        // Check job status first
        match state.statuses.get(job_id) {
            Some(JobStatus::Completed) => state
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
            match status {                JobStatus::Queued => {
                    // Get job details for metrics before removal
                    let job_for_metrics = state.queue.iter().find(|job| job.id == job_id).cloned();
                    
                    // Remove the job from the queue if it's there
                    state.queue.retain(|job| job.id != job_id);

                    // Get folder path from job metadata and clone it
                    let folder_path = if let Some(metadata) = state.job_metadata.get(job_id) {
                        Some(metadata.folder_path.clone())
                    } else {
                        None
                    };

                    // Remove job from tracking
                    state.statuses.remove(job_id);
                    state.results.remove(job_id);
                    state.job_metadata.remove(job_id);

                    // Drop the lock before filesystem operations
                    drop(state);

                    // Record cancellation metrics if we found the job
                    if let Some(job) = job_for_metrics {
                        self.metrics.record_job_cancelled(&job.model, &job.language).await;
                    }

                    // Remove job files if we have a path
                    if let Some(path) = folder_path {
                        if let Err(e) = fs::remove_dir_all(&path) {
                            error!("Failed to remove folder for canceled job {}: {}", job_id, e);
                            // Continue with cancellation even if file removal fails
                        }
                    }

                    info!("Canceled job: {}", job_id);
                    Ok(())
                }
                JobStatus::Processing => Err(QueueError::CannotCancelJob(
                    "Cannot cancel a job that is currently processing".to_string(),
                )),
                JobStatus::Completed | JobStatus::Failed(_) => {
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
            if !matches!(status, JobStatus::Completed | JobStatus::Failed(_)) {
                return Err(QueueError::QueueError(
                    "Cannot clean up a job that is still in progress".to_string(),
                ));
            }

            // Get metadata from job
            let metadata = state.job_metadata.get(job_id);

            // Try to remove temp folder if we found a folder path
            if let Some(metadata) = metadata {
                // Clean up temporary folder
                if let Err(e) = fs::remove_dir_all(&metadata.folder_path) {
                    error!("Failed to remove job folder {}: {}", metadata.folder_path.display(), e);
                    // Don't return error here, we still want to clean up the job state
                }

                // Clean up output file if it exists
                if let Some(output_path) = &metadata.output_file_path {
                    if output_path.exists() {
                        if let Err(e) = fs::remove_file(output_path) {
                            error!("Failed to remove output file {}: {}", output_path.display(), e);
                            // Continue with cleanup even if file removal fails
                        } else {
                            debug!("Successfully removed output file: {}", output_path.display());
                        }
                    }
                }
            } else {
                warn!(
                    "No metadata found for job {}, skipping file cleanup",
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
