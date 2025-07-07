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

/// Configuration for metrics system resource limits and behavior
#[derive(Debug, Clone)]
pub struct MetricsResourceConfig {
    /// Maximum number of metrics to track (prevents memory exhaustion)
    pub max_metrics: usize,
    /// Maximum number of label combinations per metric (prevents cardinality explosion)
    pub max_label_combinations: usize,
    /// Maximum length for metric names
    pub max_metric_name_length: usize,
    /// Maximum length for label names
    pub max_label_name_length: usize,
    /// Maximum length for label values
    pub max_label_value_length: usize,
    /// Maximum number of labels per metric
    pub max_labels_per_metric: usize,
    /// Enable automatic cleanup of unused metrics (when applicable)
    #[allow(dead_code)]
    pub enable_metric_cleanup: bool,
    /// Interval for metric cleanup checks (in seconds)
    pub cleanup_interval_seconds: u64,
}

impl Default for MetricsResourceConfig {
    fn default() -> Self {
        Self {
            max_metrics: 1000,
            max_label_combinations: 10000,
            max_metric_name_length: 128,
            max_label_name_length: 64,
            max_label_value_length: 256,
            max_labels_per_metric: 16,
            enable_metric_cleanup: false,
            cleanup_interval_seconds: 300, // 5 minutes
        }
    }
}

impl MetricsResourceConfig {
    /// Create configuration from environment variables
    ///
    /// Environment variables:
    /// - METRICS_MAX_METRICS: Maximum number of metrics (default: 1000)
    /// - METRICS_MAX_LABEL_COMBINATIONS: Maximum label combinations (default: 10000)
    /// - METRICS_MAX_METRIC_NAME_LENGTH: Maximum metric name length (default: 128)
    /// - METRICS_MAX_LABEL_NAME_LENGTH: Maximum label name length (default: 64)
    /// - METRICS_MAX_LABEL_VALUE_LENGTH: Maximum label value length (default: 256)
    /// - METRICS_MAX_LABELS_PER_METRIC: Maximum labels per metric (default: 16)
    /// - METRICS_ENABLE_CLEANUP: Enable metric cleanup (default: false)
    /// - METRICS_CLEANUP_INTERVAL_SECONDS: Cleanup interval (default: 300)
    pub fn from_env() -> Self {
        Self {
            max_metrics: std::env::var("METRICS_MAX_METRICS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            max_label_combinations: std::env::var("METRICS_MAX_LABEL_COMBINATIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10000),
            max_metric_name_length: std::env::var("METRICS_MAX_METRIC_NAME_LENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(128),
            max_label_name_length: std::env::var("METRICS_MAX_LABEL_NAME_LENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(64),
            max_label_value_length: std::env::var("METRICS_MAX_LABEL_VALUE_LENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(256),
            max_labels_per_metric: std::env::var("METRICS_MAX_LABELS_PER_METRIC")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(16),
            enable_metric_cleanup: std::env::var("METRICS_ENABLE_CLEANUP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            cleanup_interval_seconds: std::env::var("METRICS_CLEANUP_INTERVAL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        }
    }

    /// Validate the configuration values
    pub fn validate(&self) -> Result<(), MetricsError> {
        if self.max_metrics == 0 {
            return Err(MetricsError::configuration_error(
                "max_metrics must be greater than 0"
            ));
        }
        if self.max_label_combinations == 0 {
            return Err(MetricsError::configuration_error(
                "max_label_combinations must be greater than 0"
            ));
        }
        if self.max_metric_name_length == 0 {
            return Err(MetricsError::configuration_error(
                "max_metric_name_length must be greater than 0"
            ));
        }
        if self.max_label_name_length == 0 {
            return Err(MetricsError::configuration_error(
                "max_label_name_length must be greater than 0"
            ));
        }
        if self.max_label_value_length == 0 {
            return Err(MetricsError::configuration_error(
                "max_label_value_length must be greater than 0"
            ));
        }
        if self.max_labels_per_metric == 0 {
            return Err(MetricsError::configuration_error(
                "max_labels_per_metric must be greater than 0"
            ));
        }
        if self.cleanup_interval_seconds == 0 {
            return Err(MetricsError::configuration_error(
                "cleanup_interval_seconds must be greater than 0"
            ));
        }
        Ok(())
    }
}

/// Enhanced error handling configuration for metrics operations
#[derive(Debug, Clone)]
pub struct MetricsErrorConfig {
    /// Log validation errors to the configured logger
    pub log_validation_errors: bool,
    /// Log resource limit errors
    pub log_resource_errors: bool,
    /// Log export errors
    pub log_export_errors: bool,
    /// Log configuration errors
    pub log_config_errors: bool,
    /// Propagate errors to calling code (vs. silent failure)
    pub propagate_errors: bool,
    /// Enable detailed error context in logs
    pub detailed_error_context: bool,
    /// Log level for metrics errors (used with external logging integration)
    pub error_log_level: String,
}

impl Default for MetricsErrorConfig {
    fn default() -> Self {
        Self {
            log_validation_errors: true,
            log_resource_errors: true,
            log_export_errors: true,
            log_config_errors: true,
            propagate_errors: false, // Silent failure by default for metrics
            detailed_error_context: true,
            error_log_level: "warn".to_string(),
        }
    }
}

impl MetricsErrorConfig {
    /// Create configuration from environment variables
    ///
    /// Environment variables:
    /// - METRICS_LOG_VALIDATION_ERRORS: Log validation errors (default: true)
    /// - METRICS_LOG_RESOURCE_ERRORS: Log resource errors (default: true)
    /// - METRICS_LOG_EXPORT_ERRORS: Log export errors (default: true)
    /// - METRICS_LOG_CONFIG_ERRORS: Log config errors (default: true)
    /// - METRICS_PROPAGATE_ERRORS: Propagate errors to caller (default: false)
    /// - METRICS_DETAILED_ERROR_CONTEXT: Include detailed context (default: true)
    /// - METRICS_ERROR_LOG_LEVEL: Log level for errors (default: warn)
    pub fn from_env() -> Self {
        Self {
            log_validation_errors: std::env::var("METRICS_LOG_VALIDATION_ERRORS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            log_resource_errors: std::env::var("METRICS_LOG_RESOURCE_ERRORS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            log_export_errors: std::env::var("METRICS_LOG_EXPORT_ERRORS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            log_config_errors: std::env::var("METRICS_LOG_CONFIG_ERRORS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            propagate_errors: std::env::var("METRICS_PROPAGATE_ERRORS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            detailed_error_context: std::env::var("METRICS_DETAILED_ERROR_CONTEXT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            error_log_level: std::env::var("METRICS_ERROR_LOG_LEVEL")
                .unwrap_or_else(|_| "warn".to_string()),
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
    resource_config: MetricsResourceConfig,
    error_config: MetricsErrorConfig,
}

impl Metrics {
    pub fn new(exporter: Arc<dyn MetricsExporter>) -> Self {
        Self {
            exporter,
            monitoring_config: MetricsMonitoringConfig::default(),
            resource_config: MetricsResourceConfig::default(),
            error_config: MetricsErrorConfig::default(),
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
            resource_config: MetricsResourceConfig::default(),
            error_config: MetricsErrorConfig::default(),
        }
    }
    /// Create metrics with full configuration control
    #[allow(dead_code)]
    pub fn with_full_config(
        exporter: Arc<dyn MetricsExporter>,
        monitoring_config: MetricsMonitoringConfig,
        resource_config: MetricsResourceConfig,
        error_config: MetricsErrorConfig,
    ) -> Result<Self, MetricsError> {
        // Validate resource configuration
        resource_config.validate()?;
        
        Ok(Self {
            exporter,
            monitoring_config,
            resource_config,
            error_config,
        })
    }

    /// Create metrics from environment variables
    #[allow(dead_code)]
    pub fn from_env(exporter: Arc<dyn MetricsExporter>) -> Result<Self, MetricsError> {
        let monitoring_config = MetricsMonitoringConfig::from_env();
        let resource_config = MetricsResourceConfig::from_env();
        let error_config = MetricsErrorConfig::from_env();
        
        // Validate configurations
        resource_config.validate()?;
        
        Ok(Self {
            exporter,
            monitoring_config,
            resource_config,
            error_config,
        })
    }

    /// Get current resource configuration
    #[allow(dead_code)]
    pub fn resource_config(&self) -> &MetricsResourceConfig {
        &self.resource_config
    }

    /// Get current error handling configuration
    #[allow(dead_code)]
    pub fn error_config(&self) -> &MetricsErrorConfig {
        &self.error_config
    }    /// Increment a counter metric
    pub async fn increment(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        let start = if self.monitoring_config.enable_operation_timing {
            Some(std::time::Instant::now())
        } else {
            None
        };

        // Validate metric constraints first
        if let Err(e) = self.validate_metric_constraints(name, labels) {
            let should_propagate = self.handle_metrics_error(&e, "increment", name).await;
            if should_propagate {
                return Err(e);
            }
            return Ok(()); // Silent failure if not propagating
        }

        let result = self.exporter.increment(name, labels).await;

        if let Err(e) = &result {
            let should_propagate = self.handle_metrics_error(e, "increment", name).await;
            if !should_propagate {
                // Silent failure - return Ok but continue with meta-metrics
            } else if self.error_config.propagate_errors {
                // Record meta-metrics first, then propagate error
                if let Some(start_time) = start {
                    self.record_operation_meta_metrics("increment", start_time, false).await;
                }
                return Err(e.clone());
            }
        }

        // Record meta-metrics if enabled (avoiding recursion by checking metric name)
        if let Some(start_time) = start {
            self.record_operation_meta_metrics("increment", start_time, result.is_ok()).await;
        }

        if self.error_config.propagate_errors {
            result
        } else {
            Ok(()) // Always return Ok if not propagating errors
        }
    }    /// Set a gauge metric value
    pub async fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        let start = if self.monitoring_config.enable_operation_timing {
            Some(std::time::Instant::now())
        } else {
            None
        };

        // Validate metric constraints first
        if let Err(e) = self.validate_metric_constraints(name, labels) {
            let should_propagate = self.handle_metrics_error(&e, "set_gauge", name).await;
            if should_propagate {
                return Err(e);
            }
            return Ok(()); // Silent failure if not propagating
        }

        let result = self.exporter.set_gauge(name, value, labels).await;

        if let Err(e) = &result {
            let should_propagate = self.handle_metrics_error(e, "set_gauge", name).await;
            if !should_propagate {
                // Silent failure - return Ok but continue with meta-metrics
            } else if self.error_config.propagate_errors {
                // Record meta-metrics first, then propagate error
                if let Some(start_time) = start {
                    self.record_operation_meta_metrics("set_gauge", start_time, false).await;
                }
                return Err(e.clone());
            }
        }

        // Record meta-metrics if enabled
        if let Some(start_time) = start {
            self.record_operation_meta_metrics("set_gauge", start_time, result.is_ok()).await;
        }

        if self.error_config.propagate_errors {
            result
        } else {
            Ok(()) // Always return Ok if not propagating errors
        }
    }    /// Observe a value in a histogram metric
    pub async fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        let start = if self.monitoring_config.enable_operation_timing {
            Some(std::time::Instant::now())
        } else {
            None
        };

        // Validate metric constraints first
        if let Err(e) = self.validate_metric_constraints(name, labels) {
            let should_propagate = self.handle_metrics_error(&e, "observe_histogram", name).await;
            if should_propagate {
                return Err(e);
            }
            return Ok(()); // Silent failure if not propagating
        }

        let result = self.exporter.observe_histogram(name, value, labels).await;

        if let Err(e) = &result {
            let should_propagate = self.handle_metrics_error(e, "observe_histogram", name).await;
            if !should_propagate {
                // Silent failure - return Ok but continue with meta-metrics
            } else if self.error_config.propagate_errors {
                // Record meta-metrics first, then propagate error
                if let Some(start_time) = start {
                    self.record_operation_meta_metrics("observe_histogram", start_time, false).await;
                }
                return Err(e.clone());
            }
        }

        // Record meta-metrics if enabled
        if let Some(start_time) = start {
            self.record_operation_meta_metrics("observe_histogram", start_time, result.is_ok()).await;
        }

        if self.error_config.propagate_errors {
            result
        } else {
            Ok(()) // Always return Ok if not propagating errors
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
    }    // Convenience methods for common metrics    /// Record HTTP request duration
    pub async fn record_http_request(
        &self,
        endpoint: &str,
        method: &str,
        status: &str,
        duration: f64,
    ) {
        self.observe_histogram_internal(
            "http_request_duration_seconds",
            duration,
            &[
                ("endpoint", endpoint),
                ("method", method),
                ("status", status),
            ],
        )
        .await;

        self.increment_internal(
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
        self.increment_internal(
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
        self.observe_histogram_internal(
            "job_processing_duration_seconds",
            duration,
            &[("model", model), ("language", language), ("status", status)],
        )
        .await;

        self.increment_internal(
            "jobs_completed_total",
            &[("model", model), ("language", language), ("status", status)],
        )
        .await;
    }

    /// Set current queue size
    pub async fn set_queue_size(&self, size: f64) {
        self.set_gauge_internal("queue_size", size, &[]).await;
    }

    /// Set number of jobs currently processing
    pub async fn set_jobs_processing(&self, count: f64) {
        self.set_gauge_internal("jobs_processing", count, &[]).await;
    }
    /// Record authentication attempt
    pub async fn record_auth_attempt(&self, status: &str) {
        self.increment_internal("auth_attempts_total", &[("status", status)])
            .await;
    }

    /// Record file size
    #[allow(dead_code)]
    pub async fn record_file_size(&self, size_bytes: f64) {
        self.observe_histogram_internal("file_size_bytes", size_bytes, &[])
            .await;
    }
    /// Record job cancellation
    pub async fn record_job_cancelled(&self, model: &str, language: &str) {
        self.increment_internal(
            "jobs_cancelled_total",
            &[("model", model), ("language", language)],
        )
        .await;
    }

    /// Record when a job starts processing
    pub async fn record_job_processing_start(&self) {
        self.increment_internal("jobs_processing_started_total", &[]).await;
    }    /// Record current queue size (alias for set_queue_size)
    pub async fn record_queue_size(&self, size: usize) {
        // Use safe conversion to avoid precision loss for large queue sizes
        match validation::validate_usize_conversion(size) {
            Ok(safe_size) => self.set_queue_size(safe_size).await,
            Err(e) => {
                warn!("Failed to record queue size {}: {}", size, e);
                // Record the error but continue operation
                self.increment_internal("queue_size_conversion_errors_total", &[])
                    .await;
            }
        }
    }
    /// Record internal metrics system performance
    #[allow(dead_code)]
    pub async fn record_metrics_operation(&self, operation: &str, duration_ms: f64, success: bool) {
        let status = if success { "success" } else { "error" };

        // Track metrics system performance
        let _ = self.observe_histogram(
            "metrics_operation_duration_ms",
            duration_ms,
            &[("operation", operation), ("status", status)],
        )
        .await;

        // Count operations
        let _ = self.increment(
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
        let _ = self.observe_histogram(
            "metrics_export_duration_ms",
            duration_ms,
            &[("exporter", exporter_type), ("status", status)],
        )
        .await;

        // Track export size
        if success {
            match validation::validate_usize_conversion(bytes_exported) {
                Ok(safe_bytes) => {
                    let _ = self.observe_histogram(
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
        let _ = self.increment(
            "metrics_exports_total",
            &[("exporter", exporter_type), ("status", status)],
        )
        .await;
    }
    /// Record metrics validation errors
    #[allow(dead_code)]
    pub async fn record_metrics_validation_error(&self, error_type: &str, metric_name: &str) {
        let _ = self.increment(
            "metrics_validation_errors_total",
            &[("error_type", error_type), ("metric_name", metric_name)],
        )
        .await;
    }

    /// Enhanced error handling with configurable logging and propagation
    async fn handle_metrics_error(&self, error: &MetricsError, operation: &str, metric_name: &str) -> bool {
        let should_log = match error {
            MetricsError::InvalidName { .. } | MetricsError::InvalidLabel { .. } => {
                self.error_config.log_validation_errors
            },
            MetricsError::ResourceLimitExceeded { .. } => {
                self.error_config.log_resource_errors
            },
            MetricsError::ExportFailed { .. } => {
                self.error_config.log_export_errors
            },
            MetricsError::ConfigurationError { .. } => {
                self.error_config.log_config_errors
            },
            _ => true, // Log other errors by default
        };

        if should_log {
            if self.error_config.detailed_error_context {
                match self.error_config.error_log_level.as_str() {
                    "error" => log::error!("Metrics {} operation failed for '{}': {} (Context: {})", 
                                          operation, metric_name, error, self.get_error_context()),
                    "warn" => log::warn!("Metrics {} operation failed for '{}': {} (Context: {})", 
                                        operation, metric_name, error, self.get_error_context()),
                    "info" => log::info!("Metrics {} operation failed for '{}': {} (Context: {})", 
                                        operation, metric_name, error, self.get_error_context()),
                    "debug" => log::debug!("Metrics {} operation failed for '{}': {} (Context: {})", 
                                          operation, metric_name, error, self.get_error_context()),
                    _ => log::warn!("Metrics {} operation failed for '{}': {} (Context: {})", 
                                   operation, metric_name, error, self.get_error_context()),
                }
            } else {
                match self.error_config.error_log_level.as_str() {
                    "error" => log::error!("Metrics {} operation failed for '{}': {}", operation, metric_name, error),
                    "warn" => log::warn!("Metrics {} operation failed for '{}': {}", operation, metric_name, error),
                    "info" => log::info!("Metrics {} operation failed for '{}': {}", operation, metric_name, error),
                    "debug" => log::debug!("Metrics {} operation failed for '{}': {}", operation, metric_name, error),
                    _ => log::warn!("Metrics {} operation failed for '{}': {}", operation, metric_name, error),
                }
            }
        }

        // Record validation error if enabled and it's a validation issue
        if self.monitoring_config.enable_validation_tracking
            && matches!(
                error,
                MetricsError::InvalidName { .. } | MetricsError::InvalidLabel { .. }
            )
        {
            let error_type = match error {
                MetricsError::InvalidName { .. } => "invalid_name",
                MetricsError::InvalidLabel { .. } => "invalid_label",
                _ => "unknown",
            };
            let _ = self
                .exporter
                .increment("metrics_validation_errors_total", &[
                    ("metric_name", metric_name),
                    ("operation", operation),
                    ("error_type", error_type),
                ])
                .await;
        }

        // Return whether errors should be propagated
        self.error_config.propagate_errors
    }

    /// Get error context for detailed logging
    fn get_error_context(&self) -> String {
        format!(
            "max_metrics={}, max_labels_per_metric={}, monitoring_enabled={},
            error_propagation={}", 
            self.resource_config.max_metrics,
            self.resource_config.max_labels_per_metric,
            self.monitoring_config.enable_meta_metrics,
            self.error_config.propagate_errors
        )
    }

    /// Validate metric constraints before processing
    fn validate_metric_constraints(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        // Check metric name length
        if name.len() > self.resource_config.max_metric_name_length {
            return Err(MetricsError::invalid_name(
                name,
                format!("Metric name length {} exceeds maximum {}", 
                       name.len(), self.resource_config.max_metric_name_length)
            ));
        }

        // Check number of labels
        if labels.len() > self.resource_config.max_labels_per_metric {
            return Err(MetricsError::invalid_label(
                "labels",
                format!("Number of labels {} exceeds maximum {}", 
                       labels.len(), self.resource_config.max_labels_per_metric)
            ));
        }

        // Check label name and value lengths
        for (label_name, label_value) in labels {            if label_name.len() > self.resource_config.max_label_name_length {
                return Err(MetricsError::invalid_label(
                    *label_name,
                    format!("Label name length {} exceeds maximum {}", 
                           label_name.len(), self.resource_config.max_label_name_length)
                ));
            }
            if label_value.len() > self.resource_config.max_label_value_length {
                return Err(MetricsError::invalid_label(
                    *label_name,
                    format!("Label value length {} exceeds maximum {}", 
                           label_value.len(), self.resource_config.max_label_value_length)
                ));
            }
        }

        Ok(())
    }

    /// Record operation meta-metrics
    async fn record_operation_meta_metrics(&self, operation: &str, start_time: std::time::Instant, success: bool) {
        if self.monitoring_config.enable_operation_timing
            && self.monitoring_config.enable_meta_metrics
        {
            let duration_ms = start_time.elapsed().as_millis() as f64;
            let _ = self
                .exporter
                .observe_histogram(
                    "metrics_operation_duration_ms",
                    duration_ms,
                    &[
                        ("operation", operation),
                        ("status", if success { "success" } else { "error" }),
                    ],
                )
                .await;
        }
    }

    /// Internal increment that doesn't propagate errors (for convenience methods)
    async fn increment_internal(&self, name: &str, labels: &[(&str, &str)]) {
        if let Err(e) = self.increment(name, labels).await {
            log::warn!("Failed to increment metric '{}': {}", name, e);
        }
    }

    /// Internal set_gauge that doesn't propagate errors (for convenience methods)
    async fn set_gauge_internal(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        if let Err(e) = self.set_gauge(name, value, labels).await {
            log::warn!("Failed to set gauge '{}': {}", name, e);
        }
    }

    /// Internal observe_histogram that doesn't propagate errors (for convenience methods)
    async fn observe_histogram_internal(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        if let Err(e) = self.observe_histogram(name, value, labels).await {
            log::warn!("Failed to observe histogram '{}': {}", name, e);
        }
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
            debug!(
                "Unknown metrics exporter type '{}', using null exporter",
                exporter_type
            );
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

/// Factory function to create metrics with comprehensive configuration from environment
#[allow(dead_code)]
pub fn create_metrics_from_env() -> Result<Metrics, MetricsError> {
    // Determine exporter type from environment
    let exporter_type = std::env::var("METRICS_EXPORTER").unwrap_or_else(|_| "prometheus".to_string());
    
    // Create exporter based on type
    let exporter = match exporter_type.to_lowercase().as_str() {
        "prometheus" => {
            debug!("Creating Prometheus exporter from environment");
            Arc::new(PrometheusExporter::from_env()?) as Arc<dyn MetricsExporter>
        },
        "statsd" => {
            debug!("Creating StatsD exporter from environment");
            Arc::new(StatsDExporter::from_env()?) as Arc<dyn MetricsExporter>
        },
        "none" | "null" | "disabled" => {
            debug!("Creating null exporter (metrics disabled)");
            Arc::new(NullExporter) as Arc<dyn MetricsExporter>
        },
        _ => {
            debug!("Unknown exporter type '{}', falling back to Prometheus", exporter_type);
            Arc::new(PrometheusExporter::from_env()?) as Arc<dyn MetricsExporter>
        }
    };

    // Create metrics with full environment configuration
    Metrics::from_env(exporter)
}

/// Factory function to create metrics with custom resource limits
#[allow(dead_code)]
pub fn create_metrics_with_limits(
    exporter: Arc<dyn MetricsExporter>,
    max_metrics: usize,
    max_labels_per_metric: usize,
) -> Result<Metrics, MetricsError> {
    let mut resource_config = MetricsResourceConfig::default();
    resource_config.max_metrics = max_metrics;
    resource_config.max_labels_per_metric = max_labels_per_metric;
    
    Metrics::with_full_config(
        exporter,
        MetricsMonitoringConfig::default(),
        resource_config,
        MetricsErrorConfig::default(),
    )
}

/// Factory function to create metrics with error propagation enabled
#[allow(dead_code)]
pub fn create_metrics_with_error_propagation(
    exporter: Arc<dyn MetricsExporter>,
) -> Result<Metrics, MetricsError> {
    let mut error_config = MetricsErrorConfig::default();
    error_config.propagate_errors = true;
    
    Metrics::with_full_config(
        exporter,
        MetricsMonitoringConfig::default(),
        MetricsResourceConfig::default(),
        error_config,
    )
}
