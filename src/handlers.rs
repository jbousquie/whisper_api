// Whisper.cpp commands
// https://github.com/ggml-org/whisper.cpp?tab=readme-ov-file#quick-start

use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, post};
use futures::{StreamExt, TryStreamExt};
use serde_json::json;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

const WHISPER_CMD: &str = "/home/jerome/whspr";

#[post("/transcribe")]
async fn transcribe(mut form: Multipart) -> impl Responder {
    // Default values
    let mut language = String::from("fr");
    let mut model = String::from("large-v3");
    let mut audio_file = None;
    
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
            },
            "audio" => {
                // Create a temporary file with unique name
                let temp_file = match NamedTempFile::new() {
                    Ok(file) => file,
                    Err(e) => {
                        return HttpResponse::InternalServerError()
                            .json(json!({ "error": format!("Failed to create temp file: {}", e) }));
                    }
                };
                
                let file_path = temp_file.path().to_path_buf();
                let mut file_writer = temp_file;
                
                // Write file data to temp file
                while let Some(chunk) = field.next().await {
                    match chunk {
                        Ok(data) => {
                            if let Err(e) = file_writer.write_all(&data) {
                                return HttpResponse::InternalServerError()
                                    .json(json!({ "error": format!("Failed to write to temp file: {}", e) }));
                            }
                        },
                        Err(e) => {
                            return HttpResponse::BadRequest()
                                .json(json!({ "error": format!("Error processing file upload: {}", e) }));
                        }
                    }
                }
                
                audio_file = Some(file_path);
            },
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
            return HttpResponse::BadRequest().json(json!({ 
                "error": "No audio file provided in the request" 
            }));
        }
    };

    // Execute whisper command
    let output = Command::new(WHISPER_CMD)
        .arg("-m")
        .arg(format!("models/ggml-{}.bin", model))
        .arg("-l")
        .arg(&language)
        .arg("-f")
        .arg(&audio_path)
        .arg("--output-json")
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
                                return HttpResponse::Ok().json(json_value);
                            },
                            Err(_) => {
                                // If parsing as JSON fails, return the raw output
                                return HttpResponse::Ok().json(json!({
                                    "text": stdout.trim()
                                }));
                            }
                        }
                    },
                    Err(_) => {
                        return HttpResponse::InternalServerError().json(json!({
                            "error": "Failed to parse command output"
                        }));
                    }
                }
            } else {
                // Command executed but failed
                let error = String::from_utf8_lossy(&output.stderr).to_string();
                return HttpResponse::BadRequest().json(json!({
                    "error": error
                }));
            }
        },
        Err(e) => {
            // Failed to execute command
            return HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to execute whisper command: {}", e)
            }));
        }
    }
}


