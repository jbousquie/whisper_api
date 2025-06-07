//! Metrics Module for Whisper API
//!
//! This module provides a pluggable metrics system that supports multiple monitoring
//! backends including Prometheus, StatsD, and other monitoring systems.

use async_trait::async_trait;
use log::{debug, warn};
use prometheus::{
    CounterVec, Encoder, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

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

/// Prometheus implementation of MetricsExporter
pub struct PrometheusExporter {
    registry: Registry,
    counters: Mutex<HashMap<String, CounterVec>>,
    gauges: Mutex<HashMap<String, GaugeVec>>,
    histograms: Mutex<HashMap<String, HistogramVec>>,
}

impl PrometheusExporter {
    pub fn new() -> Self {
        let registry = Registry::new();
        Self {
            registry,
            counters: Mutex::new(HashMap::new()),
            gauges: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
        }
    }

    async fn get_or_create_counter(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> CounterVec {
        let mut counters = self.counters.lock().await;
        if let Some(counter) = counters.get(name) {
            return counter.clone();
        }

        let opts = Opts::new(name, help);
        let counter = CounterVec::new(opts, label_names).unwrap();

        if let Err(e) = self.registry.register(Box::new(counter.clone())) {
            warn!("Failed to register counter metric {}: {}", name, e);
        }

        counters.insert(name.to_string(), counter.clone());
        counter
    }

    async fn get_or_create_gauge(&self, name: &str, help: &str, label_names: &[&str]) -> GaugeVec {
        let mut gauges = self.gauges.lock().await;
        if let Some(gauge) = gauges.get(name) {
            return gauge.clone();
        }

        let opts = Opts::new(name, help);
        let gauge = GaugeVec::new(opts, label_names).unwrap();

        if let Err(e) = self.registry.register(Box::new(gauge.clone())) {
            warn!("Failed to register gauge metric {}: {}", name, e);
        }

        gauges.insert(name.to_string(), gauge.clone());
        gauge
    }

    async fn get_or_create_histogram(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> HistogramVec {
        let mut histograms = self.histograms.lock().await;
        if let Some(histogram) = histograms.get(name) {
            return histogram.clone();
        }

        let opts = HistogramOpts::new(name, help);
        let histogram = HistogramVec::new(opts, label_names).unwrap();

        if let Err(e) = self.registry.register(Box::new(histogram.clone())) {
            warn!("Failed to register histogram metric {}: {}", name, e);
        }

        histograms.insert(name.to_string(), histogram.clone());
        histogram
    }

    fn extract_label_names_and_values<'a>(
        labels: &'a [(&'a str, &'a str)],
    ) -> (Vec<&'a str>, Vec<&'a str>) {
        let label_names: Vec<&str> = labels.iter().map(|(k, _)| *k).collect();
        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        (label_names, label_values)
    }
}

#[async_trait]
impl MetricsExporter for PrometheusExporter {
    async fn increment(&self, name: &str, labels: &[(&str, &str)]) {
        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let counter = self
            .get_or_create_counter(name, "Counter metric", &label_names)
            .await;

        if label_names.is_empty() {
            counter.with_label_values(&[]).inc();
        } else {
            counter.with_label_values(&label_values).inc();
        }

        debug!("Incremented counter {} with labels {:?}", name, labels);
    }

    async fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let gauge = self
            .get_or_create_gauge(name, "Gauge metric", &label_names)
            .await;

        if label_names.is_empty() {
            gauge.with_label_values(&[]).set(value);
        } else {
            gauge.with_label_values(&label_values).set(value);
        }

        debug!("Set gauge {} to {} with labels {:?}", name, value, labels);
    }

    async fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let histogram = self
            .get_or_create_histogram(name, "Histogram metric", &label_names)
            .await;

        if label_names.is_empty() {
            histogram.with_label_values(&[]).observe(value);
        } else {
            histogram.with_label_values(&label_values).observe(value);
        }

        debug!(
            "Observed histogram {} with value {} and labels {:?}",
            name, value, labels
        );
    }

    async fn export(&self) -> Result<Vec<u8>, String> {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|e| format!("Failed to encode metrics: {}", e))?;
        Ok(buffer)
    }
}

/// Null exporter for testing or when metrics are disabled
pub struct NullExporter;

#[async_trait]
impl MetricsExporter for NullExporter {
    async fn increment(&self, _name: &str, _labels: &[(&str, &str)]) {
        // No-op
    }

    async fn set_gauge(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // No-op
    }

    async fn observe_histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // No-op
    }

    async fn export(&self) -> Result<Vec<u8>, String> {
        Ok(vec![])
    }
}

/// StatsD implementation (placeholder for future implementation)
pub struct StatsDExporter {
    // Future implementation for StatsD
    _endpoint: String,
}

impl StatsDExporter {
    pub fn new(endpoint: String) -> Self {
        Self {
            _endpoint: endpoint,
        }
    }
}

#[async_trait]
impl MetricsExporter for StatsDExporter {
    async fn increment(&self, _name: &str, _labels: &[(&str, &str)]) {
        // TODO: Implement StatsD counter increment
        debug!("StatsD increment not yet implemented");
    }

    async fn set_gauge(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // TODO: Implement StatsD gauge set
        debug!("StatsD gauge set not yet implemented");
    }

    async fn observe_histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // TODO: Implement StatsD histogram observe
        debug!("StatsD histogram observe not yet implemented");
    }

    async fn export(&self) -> Result<Vec<u8>, String> {
        // StatsD doesn't export, it pushes metrics
        Ok(vec![])
    }
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
) -> Arc<dyn MetricsExporter> {
    match exporter_type.to_lowercase().as_str() {
        "prometheus" => {
            debug!("Initializing Prometheus metrics exporter");
            Arc::new(PrometheusExporter::new())
        }
        "statsd" => {
            let endpoint = endpoint.unwrap_or("localhost:8125");
            debug!(
                "Initializing StatsD metrics exporter with endpoint: {}",
                endpoint
            );
            Arc::new(StatsDExporter::new(endpoint.to_string()))
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
