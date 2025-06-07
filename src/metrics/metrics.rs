//! Metrics Module for Whisper API
//!
//! This module provides a pluggable metrics system that supports multiple monitoring
//! backends including Prometheus, StatsD, and other monitoring systems.

use crate::metrics::null::NullExporter;
use crate::metrics::prometheus::PrometheusExporter;
use crate::metrics::statsd::StatsDExporter;
use async_trait::async_trait;
use log::{debug, warn};
use std::sync::Arc;

/// Metrics exporter trait for pluggable monitoring systems
#[async_trait]
pub trait MetricsExporter: Send + Sync {
    /// Increment a counter metric
    async fn increment(&self, name: &str, labels: &[(&str, &str)]);

    /// Set a gauge metric value
    async fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Observe a value in a histogram metric
    async fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Export metrics in the format expected by the monitoring system
    async fn export(&self) -> Result<Vec<u8>, String>;
}

/// Metrics facade for the application
#[derive(Clone)]
pub struct Metrics {
    exporter: Arc<dyn MetricsExporter>,
}

impl Metrics {
    pub fn new(exporter: Arc<dyn MetricsExporter>) -> Self {
        Self { exporter }
    }

    /// Increment a counter metric
    pub async fn increment(&self, name: &str, labels: &[(&str, &str)]) {
        self.exporter.increment(name, labels).await
    }

    /// Set a gauge metric value
    pub async fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        self.exporter.set_gauge(name, value, labels).await
    }

    /// Observe a value in a histogram metric
    pub async fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        self.exporter.observe_histogram(name, value, labels).await
    }

    /// Export metrics in the format expected by the monitoring system
    pub async fn export(&self) -> Result<Vec<u8>, String> {
        self.exporter.export().await
    }

    // Convenience methods for common metrics

    /// Record HTTP request duration
    pub async fn record_http_request(
        &self,
        endpoint: &str,
        method: &str,
        status: &str,
        duration: f64,
    ) {
        self.observe_histogram(
            "http_request_duration_seconds",
            duration,
            &[
                ("endpoint", endpoint),
                ("method", method),
                ("status", status),
            ],
        )
        .await;

        self.increment(
            "http_requests_total",
            &[
                ("endpoint", endpoint),
                ("method", method),
                ("status", status),
            ],
        )
        .await;
    }

    /// Record job submission
    pub async fn record_job_submitted(&self, model: &str, language: &str) {
        self.increment(
            "jobs_submitted_total",
            &[("model", model), ("language", language)],
        )
        .await;
    }

    /// Record job completion
    pub async fn record_job_completed(
        &self,
        model: &str,
        language: &str,
        duration: f64,
        status: &str,
    ) {
        self.observe_histogram(
            "job_processing_duration_seconds",
            duration,
            &[("model", model), ("language", language), ("status", status)],
        )
        .await;

        self.increment(
            "jobs_completed_total",
            &[("model", model), ("language", language), ("status", status)],
        )
        .await;
    }

    /// Set current queue size
    pub async fn set_queue_size(&self, size: f64) {
        self.set_gauge("queue_size", size, &[]).await;
    }

    /// Set number of jobs currently processing
    pub async fn set_jobs_processing(&self, count: f64) {
        self.set_gauge("jobs_processing", count, &[]).await;
    }

    /// Record authentication attempt
    pub async fn record_auth_attempt(&self, status: &str) {
        self.increment("auth_attempts_total", &[("status", status)])
            .await;
    }

    /// Record file size
    pub async fn record_file_size(&self, size_bytes: f64) {
        self.observe_histogram("file_size_bytes", size_bytes, &[])
            .await;
    }
    /// Record job cancellation
    pub async fn record_job_cancelled(&self, model: &str, language: &str) {
        self.increment(
            "jobs_cancelled_total",
            &[("model", model), ("language", language)],
        )
        .await;
    }

    /// Record when a job starts processing
    pub async fn record_job_processing_start(&self) {
        self.increment("jobs_processing_started_total", &[]).await;
    }

    /// Record current queue size (alias for set_queue_size)
    pub async fn record_queue_size(&self, size: usize) {
        self.set_queue_size(size as f64).await;
    }
}

/// Factory function to create metrics exporter based on configuration
pub fn create_metrics_exporter(
    exporter_type: &str,
    endpoint: Option<&str>,
    prefix: Option<&str>,
    sample_rate: Option<f64>,
) -> Arc<dyn MetricsExporter> {
    match exporter_type.to_lowercase().as_str() {
        "prometheus" => {
            debug!("Initializing Prometheus metrics exporter");
            Arc::new(PrometheusExporter::new())
        }
        "statsd" => {
            let endpoint = endpoint.unwrap_or("localhost:8125");
            debug!(
                "Initializing StatsD metrics exporter with endpoint: {}, prefix: {:?}, sample_rate: {:?}",
                endpoint, prefix, sample_rate
            );
            match StatsDExporter::new(
                endpoint.to_string(),
                prefix.map(|s| s.to_string()),
                sample_rate,
            ) {
                Ok(exporter) => Arc::new(exporter),
                Err(e) => {
                    warn!("Failed to create StatsD exporter: {}, using null exporter", e);
                    Arc::new(NullExporter)
                }
            }
        }
        "none" | "disabled" => {
            debug!("Metrics disabled, using null exporter");
            Arc::new(NullExporter)
        }
        _ => {
            warn!(
                "Unknown metrics exporter type '{}', using null exporter",
                exporter_type
            );
            Arc::new(NullExporter)
        }
    }
}
