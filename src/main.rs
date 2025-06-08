use actix_web::{middleware::Logger, web, App, HttpServer};
use env_logger::Env;
use log::{info, warn};

// Import our modules
mod config;
mod config_loader;
mod error;
mod file_utils;
mod handlers;
mod metrics;
mod models;
mod queue_manager;

use crate::metrics::metrics::{create_metrics_exporter, Metrics};
use config::{HandlerConfig, MetricsConfig};

// Import the types we need
use config::HandlerConfig;

use handlers::{
    cancel_transcription, transcribe, transcription_result, transcription_status, Authentication,
};
use queue_manager::{QueueManager, WhisperConfig};

const DEFAULT_WHISPER_API_HOST: &str = "127.0.0.1";
const DEFAULT_WHISPER_API_PORT: &str = "8181";
const DEFAULT_WHISPER_API_TIMEOUT_SECONDS: u64 = 480; // 8 minutes
const DEFAULT_WHISPER_API_KEEPALIVE_SECONDS: u64 = 480; // 8 minutes

/// Metrics endpoint handler
async fn metrics_handler(metrics: web::Data<Metrics>) -> Result<HttpResponse, actix_web::Error> {
    match metrics.export().await {
        Ok(data) => Ok(HttpResponse::Ok()
            .content_type("text/plain; version=0.0.4; charset=utf-8")
            .body(data)),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(format!("Failed to export metrics: {}", e))
        ),
    }
}

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
    let whisper_config = WhisperConfig::default();    // Create metrics config with custom default
    let metrics_config = MetricsConfig::default();

    // Initialize metrics
    let metrics_exporter = create_metrics_exporter(
        &metrics_config.exporter_type,
        metrics_config.endpoint.as_deref(),
        metrics_config.prefix.as_deref(),
        metrics_config.sample_rate,
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let metrics = Metrics::new(metrics_exporter);

    // Create tmp directory if it doesn't exist
    let tmp_dir = &handler_config.temp_dir;
    if let Err(e) = std::fs::create_dir_all(tmp_dir) {
        warn!("Failed to create temp directory {}: {}", tmp_dir, e);
    }

    // Initialize the queue manager
    let command_path = &whisper_config.command_path.clone();
    let models_dir = whisper_config.models_dir.clone();
    let queue_manager = QueueManager::new(whisper_config, metrics.clone());

    // Server settings
    let host =
        std::env::var("WHISPER_API_HOST").unwrap_or_else(|_| DEFAULT_WHISPER_API_HOST.to_string());
    let port =
        std::env::var("WHISPER_API_PORT").unwrap_or_else(|_| DEFAULT_WHISPER_API_PORT.to_string());

    let timeout = std::time::Duration::from_secs(
        std::env::var("WHISPER_API_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_WHISPER_API_TIMEOUT_SECONDS),
    );
    let keep_alive = std::time::Duration::from_secs(
        std::env::var("WHISPER_API_KEEPALIVE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_WHISPER_API_KEEPALIVE_SECONDS),
    );

    info!("Starting Whisper API server on http://{}:{}", host, port);
    info!("Using temp directory: {}", tmp_dir);
    info!("WhisperX command: {}", command_path);
    info!("WhisperX models directory: {}", models_dir);
    info!("Metrics exporter: {}", metrics_config.exporter_type);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Authentication)
            .app_data(web::Data::new(queue_manager.clone()))
            .app_data(web::Data::new(handler_config.clone()))
            .app_data(web::Data::new(metrics.clone()))
            .service(web::resource("/metrics").route(web::get().to(metrics_handler)))
            .service(transcribe)
            .service(transcription_status)
            .service(transcription_result)
            .service(cancel_transcription)
    })
    .bind(format!("{}:{}", host, port))?
    .client_disconnect_timeout(timeout)
    .keep_alive(keep_alive)
    .run()
    .await
}
