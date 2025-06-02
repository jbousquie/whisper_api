// Whisper API HTTP handlers
//
// This module contains the HTTP handlers for the Whisper API.
// It handles the transcription requests and responses according to the
// architecture described in the README.

use crate::queue_manager::{QueueError, QueueManager, TranscriptionJob};
use actix_multipart::Multipart;
use actix_web::{delete, get, post, web, HttpResponse, Responder};
use futures::{StreamExt, TryStreamExt};
use log::{error, info, warn};
use serde::Serialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Configuration for the Whisper API handlers
#[derive(Clone, Debug)]
pub struct HandlerConfig {
    /// Directory to store temporary files
    pub temp_dir: String,
}

impl Default for HandlerConfig {
    fn default() -> Self {
        Self {
            temp_dir: std::env::var("WHISPER_TMP_FILES")
                .unwrap_or_else(|_| String::from("/tmp/whisper_api")),
        }
    }
}

/// Response for transcription request
#[derive(Serialize)]
pub struct TranscriptionResponse {
    /// Job ID assigned to the transcription request
    pub job_id: String,
    /// URL to check the status of the transcription
    pub status_url: String,
}

/// Generate a unique filename with UUID and create a subfolder for it
fn generate_unique_filename(
    base_dir: &str,
    prefix: &str,
    extension: &str,
) -> Result<(PathBuf, PathBuf, String), std::io::Error> {
    let uuid = Uuid::new_v4();
    let folder_name = uuid.to_string();
    let filename = format!("{}_{}.{}", prefix, uuid, extension);

    // Create full directory path
    let directory_path = Path::new(base_dir).join(&folder_name);

    // Create the directory
    fs::create_dir_all(&directory_path)?;

    // Full file path inside the new directory
    let file_path = directory_path.join(&filename);

    Ok((directory_path, file_path, uuid.to_string()))
}

/// Helper function to clean up the folder
pub fn cleanup_folder(folder_path: &Path) {
    if let Err(e) = fs::remove_dir_all(folder_path) {
        error!("Failed to clean up folder {}: {}", folder_path.display(), e);
    }
}

/// Handler for transcription requests
#[post("/transcribe")]
pub async fn transcribe(
    mut form: Multipart,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
    config: web::Data<HandlerConfig>,
) -> impl Responder {
    // Default values
    let mut language = String::from("fr");
    let mut model = String::from("large-v3");
    let mut audio_file: Option<PathBuf> = None;
    let mut folder_path: Option<PathBuf> = None;
    let mut job_id = String::new();

    // Create main tmp directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&config.temp_dir) {
        error!("Failed to create main tmp directory: {}", e);
        return HttpResponse::InternalServerError().json(
            serde_json::json!({ "error": format!("Failed to create main tmp directory: {}", e) }),
        );
    }

    // Process form data
    while let Ok(Some(mut field)) = form.try_next().await {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition
            .and_then(|cd| cd.get_name().map(|name| name.to_string()))
            .unwrap_or_default();

        match field_name.as_str() {
            "language" | "model" => {
                // Simplified reading of text parameters
                let mut value = String::new();
                while let Some(Ok(chunk)) = field.next().await {
                    if let Ok(s) = std::str::from_utf8(&chunk) {
                        value.push_str(s);
                    }
                }

                let value = value.trim().to_string();
                if !value.is_empty() {
                    if field_name.as_str() == "language" {
                        language = value;
                    } else {
                        model = value;
                    }
                }
            }
            "file" => {
                // Generate a unique folder and filename for the audio file
                let (dir_path, file_path, _id) = match generate_unique_filename(
                    &config.temp_dir,
                    "whisper",
                    "audio",
                ) {
                    Ok((folder, file, id)) => {
                        job_id = id.clone();
                        (folder, file, id)
                    }
                    Err(e) => {
                        error!("Failed to create unique directory: {}", e);
                        return HttpResponse::InternalServerError().json(
                            serde_json::json!({ "error": format!("Failed to create unique directory: {}", e) }),
                        );
                    }
                };

                // Store folder path for cleanup in case of error
                folder_path = Some(dir_path);

                // Open the file for writing
                let mut file = match File::create(&file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        // Clean up folder before returning error
                        if let Some(folder) = &folder_path {
                            cleanup_folder(folder);
                        }
                        error!("Failed to create file: {}", e);
                        return HttpResponse::InternalServerError().json(
                            serde_json::json!({ "error": format!("Failed to create file: {}", e) }),
                        );
                    }
                };

                // Write file data to the file
                while let Some(chunk) = field.next().await {
                    match chunk {
                        Ok(data) => {
                            if let Err(e) = file.write_all(&data) {
                                // Clean up folder before returning error
                                if let Some(folder) = &folder_path {
                                    cleanup_folder(folder);
                                }
                                error!("Failed to write to file: {}", e);
                                return HttpResponse::InternalServerError()
                                    .json(serde_json::json!({ "error": format!("Failed to write to file: {}", e) }));
                            }
                        }
                        Err(e) => {
                            // Clean up folder before returning error
                            if let Some(folder) = &folder_path {
                                cleanup_folder(folder);
                            }
                            error!("Error processing file upload: {}", e);
                            return HttpResponse::BadRequest().json(
                                serde_json::json!({ "error": format!("Error processing file upload: {}", e) }),
                            );
                        }
                    }
                }

                // Store the file path
                audio_file = Some(file_path);
            }
            _ => {
                // Skip unknown fields
                while let Some(_) = field.next().await {}
            }
        }
    }

    // Check if audio file was provided
    let audio_path = match audio_file {
        Some(path) => path,
        None => {
            // No cleanup needed here as no folder was created
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "No audio file provided in the request"
            }));
        }
    };

    // Create a transcription job and add it to the queue
    let job = TranscriptionJob {
        id: job_id.clone(),
        audio_file: audio_path.clone(),
        folder_path: folder_path.clone().unwrap(),
        language,
        model,
    };

    // Add job to queue
    {
        let queue_manager = queue_manager.lock().await;
        if let Err(e) = queue_manager.add_job(job).await {
            // Clean up folder before returning error
            if let Some(folder) = &folder_path {
                cleanup_folder(folder);
            }
            error!("Failed to add job to queue: {}", e);
            return HttpResponse::InternalServerError().json(
                serde_json::json!({ "error": format!("Failed to add job to queue: {}", e) }),
            );
        }
    }

    // Generate status URL for client to check progress
    let status_url = format!("/transcription/{}", job_id);

    // Return job ID and status URL to client
    info!("Job {} added to queue", job_id);
    HttpResponse::Accepted().json(TranscriptionResponse { job_id, status_url })
}

/// Handler for transcription status requests
#[get("/transcription/{job_id}")]
pub async fn transcription_status(
    job_id: web::Path<String>,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
) -> impl Responder {
    let job_id = job_id.into_inner();

    // Check job status
    let queue_manager = queue_manager.lock().await;
    match queue_manager.get_job_status(&job_id).await {
        Ok(status) => HttpResponse::Ok().json(status),
        Err(e) => match e {
            QueueError::JobNotFound(_) => {
                info!("Status requested for non-existent job {}: {}", job_id, e);
                HttpResponse::NotFound()
                    .json(serde_json::json!({ "error": format!("Job not found: {}", e) }))
            }
            _ => {
                error!("Failed to get job status: {}", e);
                HttpResponse::InternalServerError().json(
                    serde_json::json!({ "error": format!("Error retrieving job status: {}", e) }),
                )
            }
        },
    }
}

/// Handler for completed transcription results
#[get("/transcription/{job_id}/result")]
pub async fn transcription_result(
    job_id: web::Path<String>,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
) -> impl Responder {
    let job_id = job_id.into_inner();

    // Get the job result
    let queue_manager = queue_manager.lock().await;
    match queue_manager.get_job_result(&job_id).await {
        Ok(result) => {
            // Clean up the job files after delivering the result
            if let Err(e) = queue_manager.cleanup_job(&job_id).await {
                warn!("Failed to clean up job {}: {}", job_id, e);
            }
            HttpResponse::Ok().json(result)
        }
        Err(e) => match e {
            QueueError::JobNotFound(_) => {
                info!("Result requested for non-existent job {}: {}", job_id, e);
                HttpResponse::NotFound()
                    .json(serde_json::json!({ "error": format!("Job not found: {}", e) }))
            }
            QueueError::QueueError(ref msg) if msg.contains("not completed yet") => {
                info!("Result requested for incomplete job {}: {}", job_id, e);
                HttpResponse::Accepted().json(serde_json::json!({
                    "error": "Job is still processing",
                    "status": "pending"
                }))
            }
            _ => {
                error!("Failed to get job result: {}", e);
                HttpResponse::InternalServerError().json(
                    serde_json::json!({ "error": format!("Error retrieving job result: {}", e) }),
                )
            }
        },
    }
}

/// Handler for canceling a transcription job
#[delete("/transcription/{job_id}")]
pub async fn cancel_transcription(
    job_id: web::Path<String>,
    queue_manager: web::Data<Arc<Mutex<QueueManager>>>,
) -> impl Responder {
    let job_id = job_id.into_inner();

    // Cancel the job
    let queue_manager = queue_manager.lock().await;
    match queue_manager.cancel_job(&job_id).await {
        Ok(_) => {
            info!("Successfully canceled job: {}", job_id);
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Job canceled successfully"
            }))
        }
        Err(e) => match e {
            QueueError::JobNotFound(_) => {
                info!(
                    "Cancellation requested for non-existent job {}: {}",
                    job_id, e
                );
                HttpResponse::NotFound()
                    .json(serde_json::json!({ "error": format!("Job not found: {}", e) }))
            }
            QueueError::CannotCancelJob(reason) => {
                info!("Cannot cancel job {}: {}", job_id, reason);
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Cannot cancel job: {}", reason)
                }))
            }
            _ => {
                error!("Failed to cancel job {}: {}", job_id, e);
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": format!("Error canceling job: {}", e) }))
            }
        },
    }
}
