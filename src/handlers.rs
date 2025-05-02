// Whisper.cpp commands
// https://github.com/ggml-org/whisper.cpp?tab=readme-ov-file#quick-start

use actix_web::{HttpResponse, Responder, post, web};
use std::process::Command; // Example usage of `std::process::Command` to call external programs

const WHISPER_CMD: &str = "/home/jerome/whspr";

#[post("/transcribe")]
async fn transcribe(data: web::Json<TranscribeRequest>) -> impl Responder {
    println!("{:?}", data);
    let output = Command::new(WHISPER_CMD)
        .arg("--model")
        .arg(data.model.clone())
        .arg("--language")
        .arg(data.language.clone())
        .arg("-f")
        .arg(data.audio_file.clone())
        .output()
        .expect("Failed to execute process");

    let result = String::from_utf8(output.stdout).unwrap(); // Convert output to string, handle errors accordingly
    HttpResponse::Ok().body(result)
}
// Define a struct to represent the expected JSON body for the transcribe endpoint
#[derive(serde::Deserialize, Debug)]
struct TranscribeRequest {
    model: String,
    language: String,
    audio_file: String,
}
