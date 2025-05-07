// Whisper.cpp commands
// https://github.com/ggml-org/whisper.cpp?tab=readme-ov-file#quick-start

use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, post};
use futures::{StreamExt, TryStreamExt};
use serde_json::json;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;
const WHISPER_CMD: &str = "/home/jerome/scripts/cpp/whisper.cpp/build/bin/whisper-cli";
const WHISPER_TMP_FILES: &str = "/home/llm/tmp";

// Generate a unique filename with UUID and create a subfolder for it
fn generate_unique_filename(prefix: &str, extension: &str) -> Result<(PathBuf, PathBuf), std::io::Error> {
    let uuid = Uuid::new_v4();
    let folder_name = uuid.to_string();
    let filename = format!("{}_{}.{}", prefix, uuid, extension);
    
    // Create full directory path
    let directory_path = Path::new(WHISPER_TMP_FILES).join(&folder_name);
    
    // Create the directory
    fs::create_dir_all(&directory_path)?;
    
    // Full file path inside the new directory
    let file_path = directory_path.join(&filename);
    
    Ok((directory_path, file_path))
}

// Helper function to clean up the folder
fn cleanup_folder(folder_path: &Path) {
    if let Err(e) = fs::remove_dir_all(folder_path) {
        eprintln!("Failed to clean up folder {}: {}", folder_path.display(), e);
    }
}

#[post("/transcribe")]
async fn transcribe(mut form: Multipart) -> impl Responder {
    // Default values
    let mut language = String::from("fr");
    let mut model = String::from("large-v3");
    let mut audio_file: Option<PathBuf> = None;
    let mut folder_path: Option<PathBuf> = None;

    // Create main tmp directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(WHISPER_TMP_FILES) {
        return HttpResponse::InternalServerError().json(
            json!({ "error": format!("Failed to create main tmp directory: {}", e) }),
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
            "audio" => {
                // Generate a unique folder and filename for the audio file
                let (dir_path, file_path) = match generate_unique_filename("whisper", "audio") {
                    Ok((folder, file)) => (folder, file),
                    Err(e) => {
                        return HttpResponse::InternalServerError().json(
                            json!({ "error": format!("Failed to create unique directory: {}", e) }),
                        );
                    }
                };
                
                // Store folder path for cleanup
                folder_path = Some(dir_path);
                
                // Open the file for writing
                let mut file = match File::create(&file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        // Clean up folder before returning error
                        if let Some(folder) = &folder_path {
                            cleanup_folder(folder);
                        }
                        return HttpResponse::InternalServerError().json(
                            json!({ "error": format!("Failed to create file: {}", e) }),
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
                                return HttpResponse::InternalServerError()
                                    .json(json!({ "error": format!("Failed to write to file: {}", e) }));
                            }
                        }
                        Err(e) => {
                            // Clean up folder before returning error
                            if let Some(folder) = &folder_path {
                                cleanup_folder(folder);
                            }
                            return HttpResponse::BadRequest().json(
                                json!({ "error": format!("Error processing file upload: {}", e) }),
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
            return HttpResponse::BadRequest().json(json!({
                "error": "No audio file provided in the request"
            }));
        }
    };

    // Create a response to return after cleanup
    let response = {
        // Execute whisper command
        let output = Command::new(WHISPER_CMD)
            .arg("-m")
            .arg(format!(
                "/home/jerome/scripts/cpp/whisper.cpp/models/ggml-{}.bin",
                model
            ))
            .arg("-l")
            .arg(&language)
            .arg("-f")
            .arg(&audio_path)
            .arg("-oj")
            .output();
        
        // Process command output
        match output {
            Ok(output) => {
                if output.status.success() {
                    // Try to parse output as JSON
                    match String::from_utf8(output.stdout) {
                        Ok(stdout) => {
                            match serde_json::from_str::<serde_json::Value>(&stdout) {
                                Ok(json_value) => {
                                    // Return the JSON output from whisper
                                    HttpResponse::Ok().json(json_value)
                                }
                                Err(_) => {
                                    // If parsing as JSON fails, return the raw output
                                    HttpResponse::Ok().json(json!({
                                        "text": stdout.trim()
                                    }))
                                }
                            }
                        }
                        Err(_) => {
                            HttpResponse::InternalServerError().json(json!({
                                "error": "Failed to parse command output"
                            }))
                        }
                    }
                } else {
                    // Command executed but failed
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    HttpResponse::BadRequest().json(json!({
                        "error": error
                    }))
                }
            }
            Err(e) => {
                // Failed to execute command
                HttpResponse::InternalServerError().json(json!({
                    "error": format!("Failed to execute whisper command: {}", e)
                }))
            }
        }
    };

    // Clean up the folder before returning the response
    if let Some(folder) = folder_path {
        cleanup_folder(&folder);
    }

    // Return the response
    response
}
