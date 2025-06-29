// API route handlers for Whisper API
//
// This module contains the route handlers for the Whisper API.
// It implements the actual HTTP endpoints for the API.

use crate::config::HandlerConfig;
use crate::error::HandlerError;
use crate::handlers::form::extract_form_data;
use crate::models::{StatusResponse, SuccessResponse, TranscriptionResponse};
use crate::queue_manager::{JobStatus, QueueManager, TranscriptionJob, TranscriptionResult};
use crate::Metrics;
use actix_multipart::Multipart;
use actix_web::{delete, get, options, post, web, HttpRequest, HttpResponse};
use log::{error, info, warn};
use serde::Serialize;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::{oneshot, Mutex};

/// Handler for transcription requests
///
/// This endpoint receives audio files and transcription parameters,
/// creates a transcription job, and adds it to the processing queue.
/// If HuggingFace token is not provided in the request, it will try to read it from the default file.
/// If no token is available and diarization is requested, diarization will be disabled.
///
/// When sync=true is specified, the endpoint waits for the transcription to complete before responding.
#[post("/transcription")]
pub async fn transcribe(
    form: Multipart,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
    config: web::Data<HandlerConfig>,
    metrics: web::Data<Metrics>,
) -> Result<HttpResponse, HandlerError> {
    let start_time = Instant::now();
    // Extract form data (audio file and parameters)
    let (params, job_paths) = extract_form_data(form, &config).await?;

    // Log diarization status
    if params.diarize {
        if params.hf_token.is_some() {
            info!(
                "Job {} will use speaker diarization with provided HF token",
                job_paths.id
            );
        } else {
            warn!(
                "Job {} requested diarization but no HF token is available",
                job_paths.id
            );
        }
    } else {
        info!("Job {} will not use speaker diarization", job_paths.id);
    }

    // Create a transcription job
    let job = TranscriptionJob {
        id: job_paths.id.clone(),
        audio_file: job_paths.audio_file,
        folder_path: job_paths.folder,
        language: params.language,
        model: params.model,
        diarize: params.diarize,
        prompt: params.prompt,
        hf_token: params.hf_token,
        output_format: params.response_format,
        device: params.device,
        device_index: params.device_index,
    };

    // Check if synchronous mode is requested
    if params.sync {
        info!("Job {} using synchronous mode", job_paths.id);
        let result =
            process_sync_job(job.clone(), &queue_manager, config.sync_request_timeout).await?;
        info!("Synchronous job {} completed", job_paths.id);
        return Ok(HttpResponse::Ok().json(result));
    }

    // Standard asynchronous mode processing
    // Add job to queue
    {
        let queue_manager = queue_manager.lock().await;
        queue_manager.add_job(job).await.map_err(|e| {
            error!("Failed to add job to queue: {}", e);
            HandlerError::from(e).with_cleanup(params.folder_path.as_ref())
        })?;
    }

    // Generate status URL for client to check progress
    let status_url = format!("/transcription/{}", job_paths.id); // Return job ID and status URL to client
    info!("Job {} added to queue", job_paths.id);

    let response = HttpResponse::Accepted().json(TranscriptionResponse {
        job_id: job_paths.id,
        status_url,
    });

    // Record HTTP request metrics
    let duration = start_time.elapsed().as_secs_f64();
    metrics
        .record_http_request("POST", "/transcribe", "202", duration)
        .await;

    Ok(response)
}

/// Process a job synchronously, waiting for its completion
///
/// This function adds a job to the queue and waits for it to complete,
/// returning the result directly to the client.
async fn process_sync_job(
    job: TranscriptionJob,
    queue_manager: &web::Data<Arc<Mutex<QueueManager>>>,
    timeout_seconds: u64,
) -> Result<TranscriptionResult, HandlerError> {
    // Add the job to the queue
    let job_id = job.id.clone();
    {
        let queue_manager = queue_manager.lock().await;
        queue_manager.add_job(job).await?;
    }

    // Create a channel for notification when job completes
    let (tx, rx) = oneshot::channel::<TranscriptionResult>();

    // Register the channel for this job
    {
        let queue_manager = queue_manager.lock().await;
        queue_manager.register_sync_channel(&job_id, tx).await?;
    }

    // Wait for job completion (with timeout)
    if timeout_seconds > 0 {
        match tokio::time::timeout(Duration::from_secs(timeout_seconds), rx).await {
            Ok(result) => {
                // Job completed successfully
                // Clean up (same as in result endpoint)
                if let Err(e) = queue_manager.lock().await.cleanup_job(&job_id).await {
                    warn!("Failed to clean up synchronized job {}: {}", job_id, e);
                }
                return Ok(result?);
            }
            Err(_) => {
                // Timeout
                // Remove the channel
                {
                    let queue_manager = queue_manager.lock().await;
                    queue_manager.remove_sync_channel(&job_id).await;
                }

                return Err(HandlerError::SyncTimeout(timeout_seconds));
            }
        }
    } else {
        // No timeout - wait indefinitely
        match rx.await {
            Ok(result) => {
                // Clean up
                if let Err(e) = queue_manager.lock().await.cleanup_job(&job_id).await {
                    warn!("Failed to clean up synchronized job {}: {}", job_id, e);
                }
                return Ok(result);
            }
            Err(e) => {
                // Channel error
                return Err(HandlerError::from(e));
            }
        }
    }
}

/// API status endpoint
///
/// This endpoint provides information about the API configuration and current queue state,
/// including server configuration, processing mode, and queue statistics.
#[get("/status")]
pub async fn api_status(
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
) -> Result<HttpResponse, HandlerError> {
    // Define a struct to hold API status information
    #[derive(Serialize)]
    struct ApiStatusResponse {
        server: ServerConfig,
        processing: ProcessingConfig,
        resources: ResourceConfig,
        security: SecurityConfig,
        queue_state: Option<QueueState>,
        error: Option<String>,
    }

    #[derive(Serialize)]
    struct ServerConfig {
        host: String,
        port: String,
        timeout: u64,
        keepalive: u64,
        worker_number: usize,
    }

    #[derive(Serialize)]
    struct ProcessingConfig {
        concurrent_mode: bool,
        max_concurrent_jobs: usize,
        device: String,
        device_index: String,
        default_output_format: String,
        default_sync_mode: bool,
        sync_timeout: u64,
    }

    #[derive(Serialize)]
    struct ResourceConfig {
        max_file_size: usize,
        job_retention_hours: u64,
        cleanup_interval_hours: u64,
    }

    #[derive(Serialize)]
    struct SecurityConfig {
        authorization_enabled: bool,
    }

    #[derive(Serialize)]
    struct QueueState {
        queued_jobs: usize,
        processing_jobs: usize,
    }

    // Get server configuration
    let host = env::var("WHISPER_API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("WHISPER_API_PORT").unwrap_or_else(|_| "8181".to_string());
    let timeout = env::var("WHISPER_API_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(480);
    let keepalive = env::var("WHISPER_API_KEEPALIVE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(480);
    let worker_number = env::var("HTTP_WORKER_NUMBER")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Get processing configuration
    let concurrent_mode = env::var("ENABLE_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(false);
    let max_concurrent_jobs = env::var("MAX_CONCURRENT_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let device = env::var("WHISPER_DEVICE").unwrap_or_else(|_| "cuda".to_string());
    let device_index = env::var("WHISPER_DEVICE_INDEX").unwrap_or_else(|_| "0".to_string());
    let default_output_format =
        env::var("WHISPER_OUTPUT_FORMAT").unwrap_or_else(|_| "txt".to_string());
    let default_sync_mode = env::var("DEFAULT_SYNC_MODE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(false);
    let sync_timeout = env::var("SYNC_REQUEST_TIMEOUT_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1800);

    // Get resource configuration
    let max_file_size = env::var("MAX_FILE_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(536870912); // 512MB default
    let job_retention_hours = env::var("WHISPER_JOB_RETENTION_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(48);
    let cleanup_interval_hours = env::var("WHISPER_CLEANUP_INTERVAL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    // Get security configuration
    let authorization_enabled = env::var("ENABLE_AUTHORIZATION")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(true);

    // Get queue state information
    let error_message = None;

    // Try to get queue statistics
    let manager = queue_manager.lock().await;

    // Get queue information
    let queued_jobs = manager.get_queue_size().await;
    let processing_jobs = manager.get_processing_count().await;

    // Create queue state info
    let queue_state = Some(QueueState {
        queued_jobs,
        processing_jobs,
    });

    // Create the response structure
    let response = ApiStatusResponse {
        server: ServerConfig {
            host,
            port,
            timeout,
            keepalive,
            worker_number,
        },
        processing: ProcessingConfig {
            concurrent_mode,
            max_concurrent_jobs,
            device,
            device_index,
            default_output_format,
            default_sync_mode,
            sync_timeout,
        },
        resources: ResourceConfig {
            max_file_size,
            job_retention_hours,
            cleanup_interval_hours,
        },
        security: SecurityConfig {
            authorization_enabled,
        },
        queue_state,
        error: error_message,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Handler for transcription status requests
///
/// This endpoint allows clients to check the status of a transcription job.
/// It also provides the queue position for jobs that are still waiting.
#[get("/transcription/{job_id}")]
pub async fn transcription_status(
    job_id: web::Path<String>,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
    metrics: web::Data<Metrics>,
) -> Result<HttpResponse, HandlerError> {
    let start_time = Instant::now();
    let job_id = job_id.into_inner();

    // Create lock scope to minimize lock duration
    let (status, queue_position) = {
        let queue_manager = queue_manager.lock().await;

        // Get job status
        let status = queue_manager.get_job_status(&job_id).await?;

        // Get queue position if job is in Queued status
        let position = if matches!(status, JobStatus::Queued) {
            queue_manager.get_job_position(&job_id).await?
        } else {
            None
        };

        (status, position)
    };

    // Create response with status and optional queue position
    let response = StatusResponse {
        status,
        queue_position,
    };

    // Record HTTP request metrics
    let duration = start_time.elapsed().as_secs_f64();
    metrics
        .record_http_request("GET", "/transcription/{job_id}", "200", duration)
        .await;

    Ok(HttpResponse::Ok().json(response))
}

/// Handler for completed transcription results
///
/// This endpoint allows clients to retrieve the final transcription result.
/// It also triggers cleanup of job files once the result is delivered.
#[get("/transcription/{job_id}/result")]
pub async fn transcription_result(
    job_id: web::Path<String>,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
    metrics: web::Data<Metrics>,
) -> Result<HttpResponse, HandlerError> {
    let start_time = Instant::now();
    let job_id = job_id.into_inner();

    // Get the job result
    let queue_manager = queue_manager.lock().await;
    let result = queue_manager.get_job_result(&job_id).await?; // Clean up the job files after delivering the result
    if let Err(e) = queue_manager.cleanup_job(&job_id).await {
        warn!("Failed to clean up job {}: {}", job_id, e);
    }

    // Record HTTP request metrics
    let duration = start_time.elapsed().as_secs_f64();
    metrics
        .record_http_request("GET", "/transcription/{job_id}/result", "200", duration)
        .await;

    Ok(HttpResponse::Ok().json(result))
}

/// Handler for OPTIONS requests to the transcription endpoint
///
/// This endpoint returns the available HTTP methods and CORS headers for the /transcription resource.
/// It respects the authorization configuration, ensuring consistent security across all endpoint methods.
#[options("/transcription")]
pub async fn transcription_options(_req: HttpRequest) -> HttpResponse {
    // List all supported methods for this endpoint
    let allowed_methods = "OPTIONS, POST, GET, DELETE";

    // Return appropriate headers
    HttpResponse::Ok()
        .append_header(("Allow", allowed_methods))
        .append_header(("Access-Control-Allow-Methods", allowed_methods))
        .append_header((
            "Access-Control-Allow-Headers",
            "Authorization, Content-Type",
        ))
        .append_header(("Access-Control-Max-Age", "86400")) // Cache preflight for 24 hours
        .finish()
}

/// Handler for canceling a transcription job
///
/// This endpoint allows clients to cancel a pending or in-progress job.
#[delete("/transcription/{job_id}")]
pub async fn cancel_transcription(
    job_id: web::Path<String>,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
    metrics: web::Data<Metrics>,
) -> Result<HttpResponse, HandlerError> {
    let start_time = Instant::now();
    let job_id = job_id.into_inner();

    // Cancel the job
    let queue_manager = queue_manager.lock().await;
    queue_manager.cancel_job(&job_id).await?;
    info!("Successfully canceled job: {}", job_id);

    // Record HTTP request metrics
    let duration = start_time.elapsed().as_secs_f64();
    metrics
        .record_http_request("DELETE", "/transcription/{job_id}", "200", duration)
        .await;

    Ok(HttpResponse::Ok().json(SuccessResponse {
        success: true,
        message: "Job canceled successfully".to_string(),
    }))
}
