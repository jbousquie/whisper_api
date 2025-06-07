// API route handlers for Whisper API
//
// This module contains the route handlers for the Whisper API.
// It implements the actual HTTP endpoints for the API.

use actix_multipart::Multipart;
use actix_web::{delete, get, post, web, HttpResponse};
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::config::HandlerConfig;
use crate::error::HandlerError;
use crate::handlers::form::extract_form_data;
use crate::models::{SuccessResponse, TranscriptionResponse, StatusResponse};
use crate::queue_manager::{JobStatus, QueueManager, TranscriptionJob};
use crate::metrics::Metrics;

/// Handler for transcription requests
///
/// This endpoint receives audio files and transcription parameters,
/// creates a transcription job, and adds it to the processing queue.
/// If HuggingFace token is not provided in the request, it will try to read it from the default file.
/// If no token is available and diarization is requested, diarization will be disabled.
#[post("/transcribe")]
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
            info!("Job {} will use speaker diarization with provided HF token", job_paths.id);
        } else {
            warn!("Job {} requested diarization but no HF token is available", job_paths.id);
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
        output_format: params.output_format,
    };

    // Add job to queue
    {
        let queue_manager = queue_manager.lock().await;
        queue_manager.add_job(job).await.map_err(|e| {
            error!("Failed to add job to queue: {}", e);
            HandlerError::from(e).with_cleanup(params.folder_path.as_ref())
        })?;
    }

    // Generate status URL for client to check progress
    let status_url = format!("/transcription/{}", job_paths.id);    // Return job ID and status URL to client
    info!("Job {} added to queue", job_paths.id);
    
    let response = HttpResponse::Accepted().json(TranscriptionResponse {
        job_id: job_paths.id,
        status_url,
    });
    
    // Record HTTP request metrics
    let duration = start_time.elapsed().as_secs_f64();
    metrics.record_http_request("POST", "/transcribe", "202", duration).await;
    
    Ok(response)
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
    metrics.record_http_request("GET", "/transcription/{job_id}", "200", duration).await;
    
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
    let result = queue_manager.get_job_result(&job_id).await?;    // Clean up the job files after delivering the result
    if let Err(e) = queue_manager.cleanup_job(&job_id).await {
        warn!("Failed to clean up job {}: {}", job_id, e);
    }

    // Record HTTP request metrics
    let duration = start_time.elapsed().as_secs_f64();
    metrics.record_http_request("GET", "/transcription/{job_id}/result", "200", duration).await;

    Ok(HttpResponse::Ok().json(result))
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
    queue_manager.cancel_job(&job_id).await?;    info!("Successfully canceled job: {}", job_id);
    
    // Record HTTP request metrics
    let duration = start_time.elapsed().as_secs_f64();
    metrics.record_http_request("DELETE", "/transcription/{job_id}", "200", duration).await;
    
    Ok(HttpResponse::Ok().json(SuccessResponse {
        success: true,
        message: "Job canceled successfully".to_string(),
    }))
}
