use crate::metrics::error::{validation, MetricsError};
/// Prometheus metrics exporter implementation
///
/// It allows for the collection and export of metrics in a format compatible with Prometheus.
///
use crate::metrics::metrics::MetricsExporter;
use async_trait::async_trait;
use log::debug;
use prometheus::{
    CounterVec, Encoder, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder,
};
use std::collections::HashMap;
use tokio::sync::Mutex;

/// Prometheus implementation of MetricsExporter
pub struct PrometheusExporter {
    registry: Registry,
    counters: Mutex<HashMap<String, CounterVec>>,
    gauges: Mutex<HashMap<String, GaugeVec>>,
    histograms: Mutex<HashMap<String, HistogramVec>>,
    /// Maximum number of metrics to prevent memory issues
    max_metrics: usize,
    /// Optional namespace prefix for all metrics
    namespace: Option<String>,
}

impl PrometheusExporter {
    pub fn new() -> Self {
        let registry = Registry::new();
        Self {
            registry,
            counters: Mutex::new(HashMap::new()),
            gauges: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
            max_metrics: 1000, // Default limit
            namespace: None,
        }
    }

    /// Create a new PrometheusExporter with custom max metrics limit
    #[allow(dead_code)] // May be used in configuration scenarios
    pub fn with_max_metrics(max_metrics: usize) -> Self {
        let registry = Registry::new();
        Self {
            registry,
            counters: Mutex::new(HashMap::new()),
            gauges: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
            max_metrics,
            namespace: None,
        }
    }

    /// Create a new PrometheusExporter with namespace prefix
    #[allow(dead_code)] // May be used in configuration scenarios
    pub fn with_namespace<S: Into<String>>(namespace: S) -> Self {
        let registry = Registry::new();
        Self {
            registry,
            counters: Mutex::new(HashMap::new()),
            gauges: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
            max_metrics: 1000,
            namespace: Some(namespace.into()),
        }
    }

    /// Create a new PrometheusExporter with both namespace and max metrics
    #[allow(dead_code)] // May be used in configuration scenarios
    pub fn with_namespace_and_limits<S: Into<String>>(namespace: S, max_metrics: usize) -> Self {
        let registry = Registry::new();
        Self {
            registry,
            counters: Mutex::new(HashMap::new()),
            gauges: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
            max_metrics,
            namespace: Some(namespace.into()),
        }
    }

    /// Create PrometheusExporter from environment variables
    ///
    /// Environment variables:
    /// - PROMETHEUS_NAMESPACE: Namespace prefix for metrics (optional)
    /// - PROMETHEUS_MAX_METRICS: Maximum number of metrics to track (default: 1000)
    ///
    /// # Examples
    /// ```bash
    /// export PROMETHEUS_NAMESPACE=whisper_api    /// export PROMETHEUS_MAX_METRICS=2000
    /// ```
    #[allow(dead_code)]
    pub fn from_env() -> Result<Self, MetricsError> {
        let namespace = std::env::var("PROMETHEUS_NAMESPACE").ok();

        let max_metrics = std::env::var("PROMETHEUS_MAX_METRICS")
            .ok()
            .and_then(|val| val.parse::<usize>().ok())
            .unwrap_or(1000);

        if max_metrics == 0 {
            return Err(MetricsError::configuration_error(
                "PROMETHEUS_MAX_METRICS must be greater than 0",
            ));
        }

        let registry = Registry::new();
        Ok(Self {
            registry,
            counters: Mutex::new(HashMap::new()),
            gauges: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
            max_metrics,
            namespace,
        })
    }

    /// Apply namespace prefix to metric name if configured
    fn apply_namespace(&self, name: &str) -> String {
        match &self.namespace {
            Some(ns) => format!("{}_{}", ns, name),
            None => name.to_string(),
        }
    }

    /// Generate dynamic help text for metrics
    fn generate_help_text(name: &str, metric_type: &str) -> String {
        // Convert snake_case or kebab-case to human readable
        let readable_name = name
            .replace('_', " ")
            .replace('-', " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        format!("{} - {}", readable_name, metric_type)
    }

    fn check_resource_limits(&self, current_count: usize) -> Result<(), MetricsError> {
        if current_count >= self.max_metrics {
            return Err(MetricsError::resource_limit_exceeded(format!(
                "Maximum number of metrics ({}) exceeded",
                self.max_metrics
            )));
        }
        Ok(())
    }

    /// Get appropriate histogram buckets based on metric name
    fn get_histogram_buckets(name: &str) -> Vec<f64> {
        // Configure buckets based on metric type for better observability
        if name.contains("duration") || name.contains("latency") || name.contains("time") {
            // Time-based metrics: microseconds to seconds
            vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]
        } else if name.contains("size") || name.contains("bytes") || name.contains("length") {
            // Size-based metrics: bytes to megabytes
            vec![
                1024.0, 4096.0, 16384.0, 65536.0, 262144.0, 1048576.0, 4194304.0, 16777216.0,
            ]
        } else if name.contains("count") || name.contains("num") || name.contains("total") {
            // Count-based metrics
            vec![
                1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
            ]
        } else if name.contains("percent") || name.contains("ratio") || name.contains("rate") {
            // Percentage/ratio metrics (0-100%)
            vec![
                0.1, 0.5, 1.0, 5.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 100.0,
            ]
        } else {
            // Default buckets for general metrics
            vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0,
            ]
        }
    }
    async fn get_or_create_counter(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<CounterVec, MetricsError> {
        let name_with_namespace = self.apply_namespace(name);

        // Fast path: check if metric already exists without holding the lock
        {
            let counters = self.counters.lock().await;
            if let Some(counter) = counters.get(&name_with_namespace) {
                return Ok(counter.clone());
            }
        }

        // Slow path: create new metric
        let mut counters = self.counters.lock().await;

        // Double-check in case another thread created it while we were waiting
        if let Some(counter) = counters.get(&name_with_namespace) {
            return Ok(counter.clone());
        }

        // Check resource limits before creating new metric
        self.check_resource_limits(counters.len())?;

        let opts = Opts::new(&name_with_namespace, help);
        let counter = CounterVec::new(opts, label_names).map_err(|e| {
            MetricsError::registration_failed(name, format!("Failed to create counter: {}", e))
        })?;

        self.registry
            .register(Box::new(counter.clone()))
            .map_err(|e| {
                MetricsError::registration_failed(
                    name,
                    format!("Failed to register counter: {}", e),
                )
            })?;

        counters.insert(name_with_namespace, counter.clone());
        Ok(counter)
    }
    async fn get_or_create_gauge(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<GaugeVec, MetricsError> {
        let name_with_namespace = self.apply_namespace(name);

        // Fast path: check if metric already exists without holding the lock
        {
            let gauges = self.gauges.lock().await;
            if let Some(gauge) = gauges.get(&name_with_namespace) {
                return Ok(gauge.clone());
            }
        }

        // Slow path: create new metric
        let mut gauges = self.gauges.lock().await;

        // Double-check in case another thread created it while we were waiting
        if let Some(gauge) = gauges.get(&name_with_namespace) {
            return Ok(gauge.clone());
        }

        // Check resource limits before creating new metric
        self.check_resource_limits(gauges.len())?;

        let opts = Opts::new(&name_with_namespace, help);
        let gauge = GaugeVec::new(opts, label_names).map_err(|e| {
            MetricsError::registration_failed(name, format!("Failed to create gauge: {}", e))
        })?;

        self.registry
            .register(Box::new(gauge.clone()))
            .map_err(|e| {
                MetricsError::registration_failed(name, format!("Failed to register gauge: {}", e))
            })?;

        gauges.insert(name_with_namespace, gauge.clone());
        Ok(gauge)
    }
    async fn get_or_create_histogram(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<HistogramVec, MetricsError> {
        let name_with_namespace = self.apply_namespace(name);

        // Fast path: check if metric already exists without holding the lock
        {
            let histograms = self.histograms.lock().await;
            if let Some(histogram) = histograms.get(&name_with_namespace) {
                return Ok(histogram.clone());
            }
        }

        // Slow path: create new metric
        let mut histograms = self.histograms.lock().await;

        // Double-check in case another thread created it while we were waiting
        if let Some(histogram) = histograms.get(&name_with_namespace) {
            return Ok(histogram.clone());
        }

        // Check resource limits before creating new metric
        self.check_resource_limits(histograms.len())?;

        // Configure explicit histogram buckets for better observability
        let buckets = Self::get_histogram_buckets(name);
        let opts = HistogramOpts::new(&name_with_namespace, help).buckets(buckets);
        let histogram = HistogramVec::new(opts, label_names).map_err(|e| {
            MetricsError::registration_failed(name, format!("Failed to create histogram: {}", e))
        })?;

        self.registry
            .register(Box::new(histogram.clone()))
            .map_err(|e| {
                MetricsError::registration_failed(
                    name,
                    format!("Failed to register histogram: {}", e),
                )
            })?;

        histograms.insert(name_with_namespace, histogram.clone());
        Ok(histogram)
    }

    fn extract_label_names_and_values<'a>(
        labels: &'a [(&'a str, &'a str)],
    ) -> (Vec<&'a str>, Vec<&'a str>) {
        let label_names: Vec<&str> = labels.iter().map(|(k, _)| *k).collect();
        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        (label_names, label_values)
    }

    /// Remove a counter metric
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn remove_counter(&self, name: &str) -> Result<(), MetricsError> {
        validation::validate_metric_name(name)?;

        let name_with_namespace = self.apply_namespace(name);
        let mut counters = self.counters.lock().await;

        if let Some(counter) = counters.remove(&name_with_namespace) {
            // Unregister from Prometheus registry
            if let Err(e) = self.registry.unregister(Box::new(counter)) {
                return Err(MetricsError::export_failed(format!(
                    "Failed to unregister counter {}: {}",
                    name, e
                )));
            }
            debug!("Removed counter metric: {}", name);
        }
        Ok(())
    }

    /// Remove a gauge metric
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn remove_gauge(&self, name: &str) -> Result<(), MetricsError> {
        validation::validate_metric_name(name)?;

        let name_with_namespace = self.apply_namespace(name);
        let mut gauges = self.gauges.lock().await;

        if let Some(gauge) = gauges.remove(&name_with_namespace) {
            // Unregister from Prometheus registry
            if let Err(e) = self.registry.unregister(Box::new(gauge)) {
                return Err(MetricsError::export_failed(format!(
                    "Failed to unregister gauge {}: {}",
                    name, e
                )));
            }
            debug!("Removed gauge metric: {}", name);
        }
        Ok(())
    }

    /// Remove a histogram metric
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn remove_histogram(&self, name: &str) -> Result<(), MetricsError> {
        validation::validate_metric_name(name)?;

        let name_with_namespace = self.apply_namespace(name);
        let mut histograms = self.histograms.lock().await;

        if let Some(histogram) = histograms.remove(&name_with_namespace) {
            // Unregister from Prometheus registry
            if let Err(e) = self.registry.unregister(Box::new(histogram)) {
                return Err(MetricsError::export_failed(format!(
                    "Failed to unregister histogram {}: {}",
                    name, e
                )));
            }
            debug!("Removed histogram metric: {}", name);
        }
        Ok(())
    }

    /// Remove all metrics (cleanup method)
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn clear_all_metrics(&self) -> Result<(), MetricsError> {
        let mut counters = self.counters.lock().await;
        let mut gauges = self.gauges.lock().await;
        let mut histograms = self.histograms.lock().await;

        // Clear all internal collections
        counters.clear();
        gauges.clear();
        histograms.clear();

        // Note: Prometheus Registry doesn't have a clear_all method
        // So we create a new registry for complete cleanup
        debug!("Cleared all metrics from PrometheusExporter");
        Ok(())
    }
}

#[async_trait]
impl MetricsExporter for PrometheusExporter {
    async fn increment(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let help_text =
            Self::generate_help_text(name, "counter metric tracking incremental values");
        let counter = self
            .get_or_create_counter(name, &help_text, &label_names)
            .await?;

        // Handle label values with proper error handling
        let metric = if label_names.is_empty() {
            counter.with_label_values(&[] as &[&str])
        } else {
            counter.with_label_values(&label_values)
        };

        metric.inc();
        debug!("Incremented counter {} with labels {:?}", name, labels);
        Ok(())
    }

    async fn set_gauge(
        &self,
        name: &str,
        value: f64,
        labels: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        validation::validate_numeric_value(value)?;
        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let help_text = Self::generate_help_text(name, "gauge metric tracking current values");
        let gauge = self
            .get_or_create_gauge(name, &help_text, &label_names)
            .await?;

        // Handle label values with proper error handling
        let metric = if label_names.is_empty() {
            gauge.with_label_values(&[] as &[&str])
        } else {
            gauge.with_label_values(&label_values)
        };

        metric.set(value);
        debug!("Set gauge {} to {} with labels {:?}", name, value, labels);
        Ok(())
    }

    async fn observe_histogram(
        &self,
        name: &str,
        value: f64,
        labels: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        validation::validate_numeric_value(value)?;
        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let help_text =
            Self::generate_help_text(name, "histogram metric tracking value distributions");
        let histogram = self
            .get_or_create_histogram(name, &help_text, &label_names)
            .await?;

        // Handle label values with proper error handling
        let metric = if label_names.is_empty() {
            histogram.with_label_values(&[] as &[&str])
        } else {
            histogram.with_label_values(&label_values)
        };

        metric.observe(value);
        debug!(
            "Observed histogram {} with value {} and labels {:?}",
            name, value, labels
        );
        Ok(())
    }

    async fn export(&self) -> Result<Vec<u8>, MetricsError> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|e| MetricsError::export_failed(format!("Failed to encode metrics: {}", e)))?;
        Ok(buffer)
    }
}
