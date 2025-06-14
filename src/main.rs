use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer};
use env_logger::Env;
use log::{error, info, warn};
use std::env;

// Import our modules
mod config;
mod config_loader;
mod config_validator;
mod error;
mod file_utils;
mod handlers;
mod metrics;
mod models;
mod queue_manager;

use crate::metrics::metrics::{create_metrics_exporter, Metrics};
use config::{HandlerConfig, MetricsConfig};
use config_validator::WhisperConfigValidator;

// Import the types we need
use handlers::{
    cancel_transcription, transcribe, transcription_result, transcription_status, Authentication,
};
use queue_manager::QueueManager;

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

    // Check for command-line arguments for documentation generation
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--generate-config" => {
                println!("{}", WhisperConfigValidator::generate_sample_config());
                return Ok(());
            }
            "--generate-docs" => {
                println!("{}", WhisperConfigValidator::generate_config_documentation());
                return Ok(());
            }
            "--help" => {
                println!("Usage: {} [OPTIONS]", args[0]);
                println!("Options:");
                println!("  --generate-config    Generate sample configuration file");
                println!("  --generate-docs      Generate configuration documentation");
                println!("  --help              Show this help message");
                return Ok(());
            }
            _ => {
                eprintln!("Unknown option: {}", args[1]);
                eprintln!("Use --help for available options");
                std::process::exit(1);
            }
        }
    }

    // Load configuration from file and environment variables
    if config_loader::load_config() {
        info!("Configuration loaded from file");
    } else {
        info!("Using environment variables and defaults (no config file loaded)");
    }    // Validate and load all configuration parameters with type safety
    info!("Validating and loading configuration...");
    let validated_config = match WhisperConfigValidator::validate_and_load() {
        Ok(config) => config,
        Err(_validation_results) => {
            error!("Configuration validation failed. Please fix the above errors before starting the server.");
            std::process::exit(1);
        }
    };

    // Perform critical validation as an additional safety check
    if let Err(_critical_results) = WhisperConfigValidator::validate_critical() {
        error!("Critical configuration validation failed. Cannot start server.");
        std::process::exit(1);
    }

    // Create legacy config structs from validated configuration for compatibility
    let handler_config = HandlerConfig::from_validated(&validated_config);
    let whisper_config = queue_manager::WhisperConfig::from_validated(&validated_config);
    let metrics_config = MetricsConfig::from_validated(&validated_config);

    // Log metrics configuration for debugging
    info!("Metrics configuration:");
    info!("  Exporter type: {}", metrics_config.exporter_type);
    info!("  Endpoint: {:?}", metrics_config.endpoint);
    info!("  Prefix: {:?}", metrics_config.prefix);
    info!("  Sample rate: {:?}", metrics_config.sample_rate);

    // Initialize metrics
    let metrics_exporter = create_metrics_exporter(
        &metrics_config.exporter_type,
        metrics_config.endpoint.as_deref(),
        metrics_config.prefix.as_deref(),
        metrics_config.sample_rate,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let metrics = Metrics::new(metrics_exporter);    // Create tmp directory if it doesn't exist
    let tmp_dir = &validated_config.tmp_dir;
    if let Err(e) = std::fs::create_dir_all(tmp_dir) {
        warn!("Failed to create temp directory {}: {}", tmp_dir, e);
    }

    // Initialize the queue manager
    let queue_manager = QueueManager::new(whisper_config, metrics.clone());

    // Use validated configuration for server settings
    let host = validated_config.host.to_string();
    let port = validated_config.port.to_string();

    let timeout = std::time::Duration::from_secs(validated_config.timeout as u64);
    let keep_alive = std::time::Duration::from_secs(validated_config.keepalive as u64);

    // Use validated workers configuration
    let workers = validated_config.workers;

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
    );    info!("Starting Whisper API server on http://{}:{}", host, port);
    info!("Using temp directory: {}", validated_config.tmp_dir);
    info!("WhisperX command: {}", validated_config.whisper_cmd);
    info!("WhisperX models directory: {}", validated_config.models_dir);
    info!("Metrics exporter: {}", validated_config.metrics_backend);

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
    .workers(workers_to_use)
    .run()
    .await
}
