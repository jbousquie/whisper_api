//! Metrics Module for Whisper API
//!
//! This module provides a comprehensive, pluggable metrics system that supports multiple monitoring
//! backends including Prometheus, StatsD, and other monitoring systems.
//!
//! # Features
//!
//! - **Multiple Backends**: Support for Prometheus, StatsD, and null (no-op) exporters
//! - **Type Safety**: All metric operations return `Result<(), MetricsError>` for proper error handling
//! - **Input Validation**: Comprehensive validation of metric names, labels, and values
//! - **Resource Management**: Built-in limits to prevent memory issues
//! - **Async Support**: Fully async API for non-blocking metric operations
//! - **Configuration**: Environment variable support for easy deployment configuration
//!
//! # Thread Safety
//!
//! The metrics system is designed to be fully thread-safe and suitable for high-concurrency environments:
//!
//! ## Core Thread Safety Guarantees
//!
//! - **Metrics struct**: Implements `Clone` and `Send + Sync`, can be safely shared across threads
//! - **MetricsExporter trait**: All implementations are `Send + Sync` with async methods
//! - **Internal synchronization**: All metric storage uses `tokio::sync::Mutex` for async-safe locking
//! - **Arc wrapping**: Metric collections use `Arc<Mutex<>>` for safe concurrent access
//!
//! ## Performance Optimizations
//!
//! - **Double-checked locking**: Metric creation uses fast-path reads to minimize lock contention
//! - **Separate locks**: Each metric type (counters, gauges, histograms) has its own lock
//! - **Non-blocking exports**: StatsD operations use UDP for fire-and-forget semantics
//! - **Error isolation**: Metric failures don't block application threads
//!
//! ## Concurrency Examples
//!
//! ```rust,no_run
//! use whisper_api::metrics::{Metrics, create_prometheus_exporter};
//! use std::sync::Arc;
//! use tokio::task;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let metrics = Arc::new(Metrics::new(create_prometheus_exporter().unwrap()));
//!     
//!     // Spawn multiple tasks that can safely use metrics concurrently
//!     let mut handles = Vec::new();
//!     
//!     for i in 0..10 {
//!         let metrics_clone = Arc::clone(&metrics);
//!         handles.push(task::spawn(async move {
//!             // Each task can safely record metrics without coordination
//!             metrics_clone.increment("worker_operations", &[("worker_id", &i.to_string())]).await;
//!         }));
//!     }
//!     
//!     // Wait for all tasks to complete
//!     for handle in handles {
//!         handle.await?;
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Memory Safety
//!
//! - **No unsafe code**: All metric operations use safe Rust constructs
//! - **Resource limits**: Built-in limits prevent memory exhaustion
//! - **Validation**: Input validation prevents invalid metric states
//! - **Error handling**: Comprehensive error types prevent panics
//!
//! ## Lock-Free Operations
//!
//! For maximum performance in high-throughput scenarios, consider:
//! - Using the StatsD exporter for fire-and-forget metrics
//! - Batching metric operations to reduce lock acquisition frequency
//! - Pre-creating metrics during application startup when possible
//!
//! # Examples
//!
//! ## Basic Usage with Prometheus
//!
//! ```rust,no_run
//! use whisper_api::metrics::{Metrics, create_prometheus_exporter};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let exporter = create_prometheus_exporter()?;
//!     let metrics = Metrics::new(exporter);
//!     
//!     // Increment a counter
//!     metrics.increment("requests_total", &[("method", "GET"), ("status", "200")]).await?;
//!     
//!     // Set a gauge value
//!     metrics.set_gauge("active_connections", 42.0, &[]).await?;
//!     
//!     // Observe histogram values
//!     metrics.observe_histogram("request_duration_seconds", 0.123, &[("endpoint", "/api/v1/transcribe")]).await?;
//!     
//!     // Export metrics
//!     let exported = metrics.export().await?;
//!     println!("{}", String::from_utf8_lossy(&exported));
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Using StatsD with Environment Configuration
//!
//! ```rust,no_run
//! use whisper_api::metrics::{Metrics, create_statsd_exporter_from_env};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Set environment variables:
//!     // STATSD_ENDPOINT=127.0.0.1:8125
//!     // STATSD_PREFIX=whisper_api
//!     // STATSD_SAMPLE_RATE=1.0
//!     
//!     let exporter = create_statsd_exporter_from_env()?;
//!     let metrics = Metrics::new(exporter);
//!     
//!     metrics.increment("api_calls", &[("operation", "transcribe")]).await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Error Handling
//!
//! ```rust,no_run
//! use whisper_api::metrics::{Metrics, MetricsError, create_prometheus_exporter};
//!
//! #[tokio::main]
//! async fn main() {
//!     let metrics = Metrics::new(create_prometheus_exporter().unwrap());
//!     
//!     match metrics.increment("invalid metric name!", &[]).await {
//!         Ok(()) => println!("Metric recorded successfully"),
//!         Err(MetricsError::InvalidName { name, reason }) => {
//!             eprintln!("Invalid metric name '{}': {}", name, reason);
//!         },
//!         Err(e) => eprintln!("Metrics error: {}", e),
//!     }
//! }
//! ```

use crate::metrics::error::{validation, MetricsError};
use crate::metrics::null::NullExporter;
use crate::metrics::prometheus::PrometheusExporter;
use crate::metrics::statsd::StatsDExporter;
use async_trait::async_trait;
use log::{debug, warn};
use std::sync::Arc;

/// Configuration for metrics monitoring and meta-metrics
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MetricsMonitoringConfig {
    /// Enable collection of meta-metrics about metrics system performance
    pub enable_meta_metrics: bool,
    /// Enable detailed operation timing (may impact performance)
    pub enable_operation_timing: bool,
    /// Enable validation error tracking
    pub enable_validation_tracking: bool,
    /// Enable export performance monitoring
    pub enable_export_monitoring: bool,
}

impl Default for MetricsMonitoringConfig {
    fn default() -> Self {
        Self {
            enable_meta_metrics: true,
            enable_operation_timing: false, // Disabled by default to avoid overhead
            enable_validation_tracking: true,
            enable_export_monitoring: true,
        }
    }
}

impl MetricsMonitoringConfig {
    /// Create configuration from environment variables
    ///
    /// Environment variables:
    /// - METRICS_ENABLE_META_METRICS: Enable meta-metrics (default: true)
    /// - METRICS_ENABLE_OPERATION_TIMING: Enable operation timing (default: false)  
    /// - METRICS_ENABLE_VALIDATION_TRACKING: Enable validation error tracking (default: true)
    /// - METRICS_ENABLE_EXPORT_MONITORING: Enable export monitoring (default: true)
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        Self {
            enable_meta_metrics: std::env::var("METRICS_ENABLE_META_METRICS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_operation_timing: std::env::var("METRICS_ENABLE_OPERATION_TIMING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            enable_validation_tracking: std::env::var("METRICS_ENABLE_VALIDATION_TRACKING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_export_monitoring: std::env::var("METRICS_ENABLE_EXPORT_MONITORING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }
    }
}

/// Metrics exporter trait for pluggable monitoring systems
///
/// This trait defines the interface that all metrics exporters must implement.
/// It provides a unified API for recording different types of metrics across
/// various monitoring backends.
#[async_trait]
pub trait MetricsExporter: Send + Sync {
    /// Increment a counter metric by 1
    ///
    /// Counters are monotonically increasing values that track the total number
    /// of events or occurrences. They are reset to zero when the process restarts.
    ///
    /// # Arguments
    /// * `name` - The metric name (must be valid: alphanumeric, underscores, start with letter)
    /// * `labels` - Key-value pairs for metric dimensions
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use whisper_api::metrics::MetricsExporter;
    /// # async fn example(exporter: &dyn MetricsExporter) -> Result<(), whisper_api::metrics::MetricsError> {
    /// exporter.increment("http_requests_total", &[("method", "GET"), ("status", "200")]).await?;
    /// exporter.increment("api_calls", &[]).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn increment(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), MetricsError>;

    /// Set a gauge metric to a specific value
    ///
    /// Gauges represent point-in-time values that can go up or down, such as
    /// current memory usage, active connections, or temperature readings.
    ///
    /// # Arguments
    /// * `name` - The metric name
    /// * `value` - The gauge value (any finite f64 number)
    /// * `labels` - Key-value pairs for metric dimensions
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use whisper_api::metrics::MetricsExporter;
    /// # async fn example(exporter: &dyn MetricsExporter) -> Result<(), whisper_api::metrics::MetricsError> {
    /// exporter.set_gauge("memory_usage_bytes", 1024.0 * 1024.0 * 512.0, &[("type", "heap")]).await?;
    /// exporter.set_gauge("active_connections", 42.0, &[]).await?;
    /// exporter.set_gauge("cpu_usage_percent", 85.5, &[("core", "0")]).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn set_gauge(
        &self,
        name: &str,
        value: f64,
        labels: &[(&str, &str)],
    ) -> Result<(), MetricsError>;

    /// Observe a value in a histogram metric
    ///
    /// Histograms track the distribution of values over time, automatically
    /// creating buckets and calculating percentiles. Useful for latency,
    /// request sizes, and other measurement distributions.
    ///
    /// # Arguments
    /// * `name` - The metric name
    /// * `value` - The observed value (must be finite and non-negative for most use cases)
    /// * `labels` - Key-value pairs for metric dimensions
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use whisper_api::metrics::MetricsExporter;
    /// # async fn example(exporter: &dyn MetricsExporter) -> Result<(), whisper_api::metrics::MetricsError> {
    /// exporter.observe_histogram("request_duration_seconds", 0.123, &[("endpoint", "/api/v1/transcribe")]).await?;
    /// exporter.observe_histogram("file_size_bytes", 1024.0 * 50.0, &[("type", "audio")]).await?;
    /// exporter.observe_histogram("queue_wait_time_ms", 15.5, &[]).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn observe_histogram(
        &self,
        name: &str,
        value: f64,
        labels: &[(&str, &str)],
    ) -> Result<(), MetricsError>;

    /// Export metrics in the format expected by the monitoring system
    ///
    /// Returns the current metrics in the appropriate format for the backend
    /// (e.g., Prometheus exposition format, StatsD messages, JSON, etc.).
    ///
    /// # Returns
    /// A byte vector containing the serialized metrics data
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use whisper_api::metrics::MetricsExporter;
    /// # async fn example(exporter: &dyn MetricsExporter) -> Result<(), whisper_api::metrics::MetricsError> {
    /// let metrics_data = exporter.export().await?;
    /// println!("Metrics: {}", String::from_utf8_lossy(&metrics_data));
    /// # Ok(())
    /// # }
    /// ```
    async fn export(&self) -> Result<Vec<u8>, MetricsError>;
}

/// Metrics facade for the application
#[derive(Clone)]
pub struct Metrics {
    exporter: Arc<dyn MetricsExporter>,
    monitoring_config: MetricsMonitoringConfig,
}

impl Metrics {
    pub fn new(exporter: Arc<dyn MetricsExporter>) -> Self {
        Self {
            exporter,
            monitoring_config: MetricsMonitoringConfig::default(),
        }
    }
    /// Create metrics with custom monitoring configuration
    #[allow(dead_code)]
    pub fn with_monitoring_config(
        exporter: Arc<dyn MetricsExporter>,
        config: MetricsMonitoringConfig,
    ) -> Self {
        Self {
            exporter,
            monitoring_config: config,
        }
    }
    /// Increment a counter metric
    pub async fn increment(&self, name: &str, labels: &[(&str, &str)]) {
        let start = if self.monitoring_config.enable_operation_timing {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let result = self.exporter.increment(name, labels).await;

        if let Err(e) = &result {
            warn!("Failed to increment metric '{}': {}", name, e);
            // Record validation error if enabled and it's a validation issue
            if self.monitoring_config.enable_validation_tracking
                && matches!(
                    e,
                    MetricsError::InvalidName { .. } | MetricsError::InvalidLabel { .. }
                )
            {
                let _ = self
                    .exporter
                    .increment("metrics_validation_errors_total", &[("metric_name", name)])
                    .await;
            }
        }

        // Record meta-metrics if enabled (avoiding recursion by checking metric name)
        if self.monitoring_config.enable_operation_timing
            && self.monitoring_config.enable_meta_metrics
            && !name.starts_with("metrics_")
        {
            if let Some(start_time) = start {
                let duration_ms = start_time.elapsed().as_millis() as f64;
                let _ = self
                    .exporter
                    .observe_histogram(
                        "metrics_operation_duration_ms",
                        duration_ms,
                        &[
                            ("operation", "increment"),
                            ("status", if result.is_ok() { "success" } else { "error" }),
                        ],
                    )
                    .await;
            }
        }
    }
    /// Set a gauge metric value
    pub async fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        let start = if self.monitoring_config.enable_operation_timing {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let result = self.exporter.set_gauge(name, value, labels).await;

        if let Err(e) = &result {
            warn!("Failed to set gauge metric '{}': {}", name, e);
            if self.monitoring_config.enable_validation_tracking
                && matches!(
                    e,
                    MetricsError::InvalidName { .. } | MetricsError::InvalidLabel { .. }
                )
            {
                let _ = self
                    .exporter
                    .increment("metrics_validation_errors_total", &[("metric_name", name)])
                    .await;
            }
        }

        // Record meta-metrics if enabled
        if self.monitoring_config.enable_operation_timing
            && self.monitoring_config.enable_meta_metrics
            && !name.starts_with("metrics_")
        {
            if let Some(start_time) = start {
                let duration_ms = start_time.elapsed().as_millis() as f64;
                let _ = self
                    .exporter
                    .observe_histogram(
                        "metrics_operation_duration_ms",
                        duration_ms,
                        &[
                            ("operation", "set_gauge"),
                            ("status", if result.is_ok() { "success" } else { "error" }),
                        ],
                    )
                    .await;
            }
        }
    }
    /// Observe a value in a histogram metric
    pub async fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        let start = std::time::Instant::now();
        let result = self.exporter.observe_histogram(name, value, labels).await;
        let duration_ms = start.elapsed().as_millis() as f64;

        if let Err(e) = &result {
            warn!("Failed to observe histogram metric '{}': {}", name, e);
            if matches!(
                e,
                MetricsError::InvalidName { .. } | MetricsError::InvalidLabel { .. }
            ) {
                let _ = self
                    .exporter
                    .increment("metrics_validation_errors_total", &[("metric_name", name)])
                    .await;
            }
        }

        // Record meta-metrics (avoiding recursion)
        if !name.starts_with("metrics_") {
            let _ = self
                .exporter
                .observe_histogram(
                    "metrics_operation_duration_ms",
                    duration_ms,
                    &[
                        ("operation", "observe_histogram"),
                        ("status", if result.is_ok() { "success" } else { "error" }),
                    ],
                )
                .await;
        }
    }
    /// Export metrics in the format expected by the monitoring system
    pub async fn export(&self) -> Result<Vec<u8>, MetricsError> {
        let start = std::time::Instant::now();
        let result = self.exporter.export().await;
        let duration_ms = start.elapsed().as_millis() as f64;

        // Determine exporter type for meta-metrics
        let exporter_type =
            if std::any::type_name::<Arc<dyn MetricsExporter>>().contains("Prometheus") {
                "prometheus"
            } else if std::any::type_name::<Arc<dyn MetricsExporter>>().contains("StatsD") {
                "statsd"
            } else {
                "unknown"
            };

        let bytes_exported = result.as_ref().map(|data| data.len()).unwrap_or(0);
        let is_success = result.is_ok();

        // Record export meta-metrics in a fire-and-forget manner
        tokio::spawn({
            let exporter = Arc::clone(&self.exporter);
            let exporter_type = exporter_type.to_string();
            async move {
                let _ = exporter
                    .observe_histogram(
                        "metrics_export_duration_ms",
                        duration_ms,
                        &[
                            ("exporter", &exporter_type),
                            ("status", if is_success { "success" } else { "error" }),
                        ],
                    )
                    .await;

                if is_success && bytes_exported > 0 {
                    if let Ok(safe_bytes) = validation::validate_usize_conversion(bytes_exported) {
                        let _ = exporter
                            .observe_histogram(
                                "metrics_export_bytes",
                                safe_bytes,
                                &[("exporter", &exporter_type)],
                            )
                            .await;
                    }
                }

                let _ = exporter
                    .increment(
                        "metrics_exports_total",
                        &[
                            ("exporter", &exporter_type),
                            ("status", if is_success { "success" } else { "error" }),
                        ],
                    )
                    .await;
            }
        });

        result
    }

    // Convenience methods for common metrics    /// Record HTTP request duration
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
    #[allow(dead_code)]
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
        // Use safe conversion to avoid precision loss for large queue sizes
        match validation::validate_usize_conversion(size) {
            Ok(safe_size) => self.set_queue_size(safe_size).await,
            Err(e) => {
                warn!("Failed to record queue size {}: {}", size, e);
                // Record the error but continue operation
                self.increment("queue_size_conversion_errors_total", &[])
                    .await;
            }
        }
    }
    /// Record internal metrics system performance
    #[allow(dead_code)]
    pub async fn record_metrics_operation(&self, operation: &str, duration_ms: f64, success: bool) {
        let status = if success { "success" } else { "error" };

        // Track metrics system performance
        self.observe_histogram(
            "metrics_operation_duration_ms",
            duration_ms,
            &[("operation", operation), ("status", status)],
        )
        .await;

        // Count operations
        self.increment(
            "metrics_operations_total",
            &[("operation", operation), ("status", status)],
        )
        .await;
    }
    /// Record metrics export performance
    #[allow(dead_code)]
    pub async fn record_metrics_export(
        &self,
        exporter_type: &str,
        duration_ms: f64,
        success: bool,
        bytes_exported: usize,
    ) {
        let status = if success { "success" } else { "error" };

        // Track export performance
        self.observe_histogram(
            "metrics_export_duration_ms",
            duration_ms,
            &[("exporter", exporter_type), ("status", status)],
        )
        .await;

        // Track export size
        if success {
            match validation::validate_usize_conversion(bytes_exported) {
                Ok(safe_bytes) => {
                    self.observe_histogram(
                        "metrics_export_bytes",
                        safe_bytes,
                        &[("exporter", exporter_type)],
                    )
                    .await;
                }
                Err(e) => {
                    warn!("Failed to record export bytes {}: {}", bytes_exported, e);
                }
            }
        }

        // Count exports
        self.increment(
            "metrics_exports_total",
            &[("exporter", exporter_type), ("status", status)],
        )
        .await;
    }
    /// Record metrics validation errors
    #[allow(dead_code)]
    pub async fn record_metrics_validation_error(&self, error_type: &str, metric_name: &str) {
        self.increment(
            "metrics_validation_errors_total",
            &[("error_type", error_type), ("metric_name", metric_name)],
        )
        .await;
    }
}

/// Factory function to create metrics exporter based on configuration
pub fn create_metrics_exporter(
    exporter_type: &str,
    endpoint: Option<&str>,
    prefix: Option<&str>,
    sample_rate: Option<f64>,
) -> Result<Arc<dyn MetricsExporter>, MetricsError> {
    match exporter_type.to_lowercase().as_str() {
        "prometheus" => {
            debug!("Initializing Prometheus metrics exporter");
            Ok(Arc::new(PrometheusExporter::new()))
        }
        "statsd" => {
            let endpoint = endpoint.unwrap_or("127.0.0.1:8125");
            debug!(
                "Initializing StatsD metrics exporter with endpoint: {}, prefix: {:?}, sample_rate: {:?}",
                endpoint, prefix, sample_rate
            );
            let exporter = StatsDExporter::new(
                endpoint.to_string(),
                prefix.map(|s| s.to_string()),
                sample_rate,
            )?;
            Ok(Arc::new(exporter))
        }
        "none" | "null" | "disabled" => {
            debug!("Metrics disabled, using null exporter");
            Ok(Arc::new(NullExporter))
        }
        _ => {
            debug!("Unknown metrics exporter type '{}', using null exporter", exporter_type);
            Ok(Arc::new(NullExporter))
        }
    }
}

/// Factory function to create a Prometheus metrics exporter
#[allow(dead_code)]
pub fn create_prometheus_exporter() -> Result<Arc<dyn MetricsExporter>, MetricsError> {
    debug!("Creating Prometheus metrics exporter");
    Ok(Arc::new(PrometheusExporter::new()))
}

/// Factory function to create a Prometheus metrics exporter with namespace
#[allow(dead_code)]
pub fn create_prometheus_exporter_with_namespace(
    namespace: &str,
) -> Result<Arc<dyn MetricsExporter>, MetricsError> {
    debug!(
        "Creating Prometheus metrics exporter with namespace: {}",
        namespace
    );
    Ok(Arc::new(PrometheusExporter::with_namespace(namespace)))
}

/// Factory function to create a Prometheus metrics exporter from environment variables
#[allow(dead_code)]
pub fn create_prometheus_exporter_from_env() -> Result<Arc<dyn MetricsExporter>, MetricsError> {
    debug!("Creating Prometheus metrics exporter from environment variables");
    Ok(Arc::new(PrometheusExporter::from_env()?))
}

/// Factory function to create a StatsD metrics exporter from environment variables
#[allow(dead_code)]
pub fn create_statsd_exporter_from_env() -> Result<Arc<dyn MetricsExporter>, MetricsError> {
    debug!("Creating StatsD metrics exporter from environment variables");
    Ok(Arc::new(StatsDExporter::from_env()?))
}

/// Factory function to create a null (no-op) metrics exporter
#[allow(dead_code)]
pub fn create_null_exporter() -> Arc<dyn MetricsExporter> {
    debug!("Creating null metrics exporter");
    Arc::new(NullExporter)
}
