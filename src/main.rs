use actix_web::{middleware::Logger, web, App, HttpServer};
use env_logger::Env;
use log::{info, warn};

// Import our modules
mod config;
mod config_loader;
mod error;
mod file_utils;
mod handlers;
mod models;
mod queue_manager;

// Import the types we need
use config::HandlerConfig;
use handlers::{
    cancel_transcription, transcribe, transcription_result, transcription_status, Authentication,
};
use queue_manager::{QueueManager, WhisperConfig};

const DEFAULT_WHISPER_API_HOST: &str = "127.0.0.1";
const DEFAULT_WHISPER_API_PORT: &str = "8181";
const DEFAULT_WHISPER_API_TIMEOUT: u64 = 480;
const DEFAULT_WHISPER_API_KEEPALIVE: u64 = 480;
const DEFAULT_HTTP_WORKER_NUMBER: usize = 0; // 0 means use number of CPU cores

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Load configuration from file and environment variables
    if config_loader::load_config() {
        info!("Configuration loaded from file");
    } else {
        info!("Using environment variables and defaults (no config file loaded)");
    }

    // Load configurations
    let handler_config = HandlerConfig::default();
    let whisper_config = WhisperConfig::default();

    // Create tmp directory if it doesn't exist
    let tmp_dir = &handler_config.temp_dir;
    if let Err(e) = std::fs::create_dir_all(tmp_dir) {
        warn!("Failed to create temp directory {}: {}", tmp_dir, e);
    }

    // Initialize the queue manager
    let command_path = &whisper_config.command_path.clone();
    let models_dir = whisper_config.models_dir.clone();
    let queue_manager = QueueManager::new(whisper_config);

    // Server settings
    let host =
        std::env::var("WHISPER_API_HOST").unwrap_or_else(|_| DEFAULT_WHISPER_API_HOST.to_string());
    let port =
        std::env::var("WHISPER_API_PORT").unwrap_or_else(|_| DEFAULT_WHISPER_API_PORT.to_string());
    let timeout = std::time::Duration::from_secs(
        std::env::var("WHISPER_API_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_WHISPER_API_TIMEOUT),
    );
    let keep_alive = std::time::Duration::from_secs(
        std::env::var("WHISPER_API_KEEPALIVE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_WHISPER_API_KEEPALIVE),
    );

    // Get the number of workers from configuration (0 = use number of CPU cores)
    let workers = std::env::var("HTTP_WORKER_NUMBER")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_HTTP_WORKER_NUMBER);

    // Number of available CPU cores
    let num_cpus = num_cpus::get();

    // Calculate actual workers to use:
    // - If workers is 0, use number of CPU cores
    // - Otherwise, use workers but cap at number of CPU cores
    let workers_to_use = if workers == 0 {
        num_cpus
    } else {
        std::cmp::min(workers, num_cpus)
    };

    info!(
        "Using {} HTTP worker(s) (system has {} CPU cores)",
        workers_to_use, num_cpus
    );

    info!("Starting Whisper API server on http://{}:{}", host, port);
    info!("Using temp directory: {}", tmp_dir);
    info!("WhisperX command: {}", command_path);
    info!("WhisperX models directory: {}", models_dir);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Authentication)
            .app_data(web::Data::new(queue_manager.clone()))
            .app_data(web::Data::new(handler_config.clone()))
            .service(transcribe)
            .service(transcription_status)
            .service(transcription_result)
            .service(cancel_transcription)
    })
    .bind(format!("{}:{}", host, port))?
    .client_disconnect_timeout(timeout)
    .keep_alive(keep_alive)
    .workers(workers_to_use)
    .run()
    .await
}
