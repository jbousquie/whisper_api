use crate::metrics::error::{validation, MetricsError};
/// Prometheus metrics exporter implementation
///
/// It allows for the collection and export of metrics in a format compatible with Prometheus.
///
use crate::metrics::metrics::MetricsExporter;
use async_trait::async_trait;
use dashmap::DashMap;
use log::debug;
use prometheus::{
    CounterVec, Encoder, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Prometheus implementation of MetricsExporter
pub struct PrometheusExporter {
    registry: Registry,
    counters: DashMap<String, CounterVec>,
    gauges: DashMap<String, GaugeVec>,
    histograms: DashMap<String, HistogramVec>,
    /// Maximum number of metrics to prevent memory issues
    max_metrics: usize,
    /// Optional namespace prefix for all metrics
    namespace: Option<String>,
    /// Atomic counter for total metrics across all types
    metric_count: AtomicUsize,
}

impl PrometheusExporter {    pub fn new() -> Self {
        let registry = Registry::new();
        Self {
            registry,
            counters: DashMap::new(),
            gauges: DashMap::new(),
            histograms: DashMap::new(),
            max_metrics: 1000, // Default limit
            namespace: None,
            metric_count: AtomicUsize::new(0),
        }
    }    /// Create a new PrometheusExporter with custom max metrics limit
    #[allow(dead_code)] // May be used in configuration scenarios
    pub fn with_max_metrics(max_metrics: usize) -> Self {
        let registry = Registry::new();
        Self {
            registry,
            counters: DashMap::new(),
            gauges: DashMap::new(),
            histograms: DashMap::new(),
            max_metrics,
            namespace: None,
            metric_count: AtomicUsize::new(0),
        }
    }    /// Create a new PrometheusExporter with namespace prefix
    #[allow(dead_code)] // May be used in configuration scenarios
    pub fn with_namespace<S: Into<String>>(namespace: S) -> Self {
        let namespace = namespace.into();
        // Validate namespace using the same validation as metric names
        validation::validate_metric_name(&namespace).expect("Invalid namespace");
        let registry = Registry::new();
        Self {
            registry,
            counters: DashMap::new(),
            gauges: DashMap::new(),
            histograms: DashMap::new(),
            max_metrics: 1000,
            namespace: Some(namespace),
            metric_count: AtomicUsize::new(0),
        }
    }    /// Create a new PrometheusExporter with both namespace and max metrics
    #[allow(dead_code)] // May be used in configuration scenarios
    pub fn with_namespace_and_limits<S: Into<String>>(namespace: S, max_metrics: usize) -> Self {
        let namespace = namespace.into();
        // Validate namespace using the same validation as metric names
        validation::validate_metric_name(&namespace).expect("Invalid namespace");
        let registry = Registry::new();
        Self {
            registry,
            counters: DashMap::new(),
            gauges: DashMap::new(),
            histograms: DashMap::new(),
            max_metrics,
            namespace: Some(namespace),
            metric_count: AtomicUsize::new(0),
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
        }        let registry = Registry::new();
        Ok(Self {
            registry,
            counters: DashMap::new(),
            gauges: DashMap::new(),
            histograms: DashMap::new(),
            max_metrics,
            namespace,
            metric_count: AtomicUsize::new(0),
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
    }    fn check_resource_limits(&self) -> Result<(), MetricsError> {
        let current_count = self.metric_count.load(Ordering::SeqCst);
        if current_count >= self.max_metrics {
            return Err(MetricsError::resource_limit_exceeded(format!(
                "Maximum number of metrics ({}) exceeded",
                self.max_metrics
            )));
        }
        self.metric_count.fetch_add(1, Ordering::SeqCst);
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
    }    async fn get_or_create_counter(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<CounterVec, MetricsError> {
        let name_with_namespace = self.apply_namespace(name);

        // Fast path: check if metric already exists
        if let Some(counter) = self.counters.get(&name_with_namespace) {
            return Ok(counter.clone());
        }

        // Check resource limits before creating new metric
        self.check_resource_limits()?;

        let opts = Opts::new(&name_with_namespace, help);
        let counter = CounterVec::new(opts, label_names).map_err(|e| {
            MetricsError::registration_failed(name, format!("Failed to create counter: {}", e))
        })?;

        // Try to register with Prometheus, handling race conditions
        match self.registry.register(Box::new(counter.clone())) {
            Ok(_) => {
                self.counters.insert(name_with_namespace, counter.clone());
                Ok(counter)
            }
            Err(e) => {
                // Check if it's a duplicate registration error (race condition)
                if e.to_string().contains("duplicate") {
                    // Another thread registered it, fetch the existing one
                    if let Some(existing_counter) = self.counters.get(&name_with_namespace) {
                        Ok(existing_counter.clone())
                    } else {
                        Err(MetricsError::registration_failed(
                            name,
                            "Race condition in counter registration",
                        ))
                    }
                } else {
                    Err(MetricsError::registration_failed(
                        name,
                        format!("Failed to register counter: {}", e),
                    ))
                }
            }
        }
    }    async fn get_or_create_gauge(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<GaugeVec, MetricsError> {
        let name_with_namespace = self.apply_namespace(name);

        // Fast path: check if metric already exists
        if let Some(gauge) = self.gauges.get(&name_with_namespace) {
            return Ok(gauge.clone());
        }

        // Check resource limits before creating new metric
        self.check_resource_limits()?;

        let opts = Opts::new(&name_with_namespace, help);
        let gauge = GaugeVec::new(opts, label_names).map_err(|e| {
            MetricsError::registration_failed(name, format!("Failed to create gauge: {}", e))
        })?;

        // Try to register with Prometheus, handling race conditions
        match self.registry.register(Box::new(gauge.clone())) {
            Ok(_) => {
                self.gauges.insert(name_with_namespace, gauge.clone());
                Ok(gauge)
            }
            Err(e) => {
                // Check if it's a duplicate registration error (race condition)
                if e.to_string().contains("duplicate") {
                    // Another thread registered it, fetch the existing one
                    if let Some(existing_gauge) = self.gauges.get(&name_with_namespace) {
                        Ok(existing_gauge.clone())
                    } else {
                        Err(MetricsError::registration_failed(
                            name,
                            "Race condition in gauge registration",
                        ))
                    }
                } else {
                    Err(MetricsError::registration_failed(
                        name,
                        format!("Failed to register gauge: {}", e),
                    ))
                }
            }
        }
    }    async fn get_or_create_histogram(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<HistogramVec, MetricsError> {
        let name_with_namespace = self.apply_namespace(name);

        // Fast path: check if metric already exists
        if let Some(histogram) = self.histograms.get(&name_with_namespace) {
            return Ok(histogram.clone());
        }

        // Check resource limits before creating new metric
        self.check_resource_limits()?;

        // Configure explicit histogram buckets for better observability
        let buckets = Self::get_histogram_buckets(name);
        let opts = HistogramOpts::new(&name_with_namespace, help).buckets(buckets);
        let histogram = HistogramVec::new(opts, label_names).map_err(|e| {
            MetricsError::registration_failed(name, format!("Failed to create histogram: {}", e))
        })?;

        // Try to register with Prometheus, handling race conditions
        match self.registry.register(Box::new(histogram.clone())) {
            Ok(_) => {
                self.histograms.insert(name_with_namespace, histogram.clone());
                Ok(histogram)
            }
            Err(e) => {
                // Check if it's a duplicate registration error (race condition)
                if e.to_string().contains("duplicate") {
                    // Another thread registered it, fetch the existing one
                    if let Some(existing_histogram) = self.histograms.get(&name_with_namespace) {
                        Ok(existing_histogram.clone())
                    } else {
                        Err(MetricsError::registration_failed(
                            name,
                            "Race condition in histogram registration",
                        ))
                    }
                } else {
                    Err(MetricsError::registration_failed(
                        name,
                        format!("Failed to register histogram: {}", e),
                    ))
                }
            }
        }
    }fn extract_label_names_and_values<'a>(
        labels: &'a [(&'a str, &'a str)],
    ) -> Result<(Vec<&'a str>, Vec<&'a str>), MetricsError> {
        let label_names: Vec<&str> = labels.iter().map(|(k, _)| *k).collect();
        let unique_names: HashSet<&str> = label_names.iter().copied().collect();        if unique_names.len() != label_names.len() {
            return Err(MetricsError::invalid_label(
                "labels",
                "Duplicate label names detected",
            ));
        }
        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        Ok((label_names, label_values))
    }    /// Remove a counter metric. Use with caution, as removing metrics can disrupt Prometheus time-series data.
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn remove_counter(&self, name: &str) -> Result<(), MetricsError> {
        validation::validate_metric_name(name)?;

        let name_with_namespace = self.apply_namespace(name);

        if let Some((_, counter)) = self.counters.remove(&name_with_namespace) {
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
    }    /// Remove a gauge metric. Use with caution, as removing metrics can disrupt Prometheus time-series data.
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn remove_gauge(&self, name: &str) -> Result<(), MetricsError> {
        validation::validate_metric_name(name)?;

        let name_with_namespace = self.apply_namespace(name);

        if let Some((_, gauge)) = self.gauges.remove(&name_with_namespace) {
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
    }    /// Remove a histogram metric. Use with caution, as removing metrics can disrupt Prometheus time-series data.
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn remove_histogram(&self, name: &str) -> Result<(), MetricsError> {
        validation::validate_metric_name(name)?;

        let name_with_namespace = self.apply_namespace(name);

        if let Some((_, histogram)) = self.histograms.remove(&name_with_namespace) {
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
    }    /// Remove all metrics (cleanup method)
    #[allow(dead_code)] // May be used for cleanup scenarios
    pub async fn clear_all_metrics(&self) -> Result<(), MetricsError> {
        // Clear all internal collections
        self.counters.clear();
        self.gauges.clear();
        self.histograms.clear();

        // Reset the metric count
        self.metric_count.store(0, Ordering::SeqCst);

        // Note: Prometheus Registry doesn't have a clear_all method
        // So we create a new registry for complete cleanup
        debug!("Cleared all metrics from PrometheusExporter");
        Ok(())
    }
}

#[async_trait]
impl MetricsExporter for PrometheusExporter {    async fn increment(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        let (label_names, label_values) = Self::extract_label_names_and_values(labels)?;
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
    }    async fn set_gauge(
        &self,
        name: &str,
        value: f64,
        labels: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        validation::validate_numeric_value(value)?;
        let (label_names, label_values) = Self::extract_label_names_and_values(labels)?;
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
    }    async fn observe_histogram(
        &self,
        name: &str,
        value: f64,
        labels: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        validation::validate_numeric_value(value)?;
        let (label_names, label_values) = Self::extract_label_names_and_values(labels)?;
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
