/// Prometheus metrics exporter implementation
///
/// It allows for the collection and export of metrics in a format compatible with Prometheus.
///
use crate::metrics::metrics::MetricsExporter;
use crate::metrics::error::{MetricsError, validation};
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
    }    async fn get_or_create_counter(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<CounterVec, MetricsError> {
        let mut counters = self.counters.lock().await;
        if let Some(counter) = counters.get(name) {
            return Ok(counter.clone());
        }

        let opts = Opts::new(name, help);
        let counter = CounterVec::new(opts, label_names)
            .map_err(|e| MetricsError::registration_failed(name, format!("Failed to create counter: {}", e)))?;

        self.registry.register(Box::new(counter.clone()))
            .map_err(|e| MetricsError::registration_failed(name, format!("Failed to register counter: {}", e)))?;

        counters.insert(name.to_string(), counter.clone());
        Ok(counter)
    }    async fn get_or_create_gauge(&self, name: &str, help: &str, label_names: &[&str]) -> Result<GaugeVec, MetricsError> {
        let mut gauges = self.gauges.lock().await;
        if let Some(gauge) = gauges.get(name) {
            return Ok(gauge.clone());
        }

        let opts = Opts::new(name, help);
        let gauge = GaugeVec::new(opts, label_names)
            .map_err(|e| MetricsError::registration_failed(name, format!("Failed to create gauge: {}", e)))?;

        self.registry.register(Box::new(gauge.clone()))
            .map_err(|e| MetricsError::registration_failed(name, format!("Failed to register gauge: {}", e)))?;

        gauges.insert(name.to_string(), gauge.clone());
        Ok(gauge)
    }    async fn get_or_create_histogram(
        &self,
        name: &str,
        help: &str,
        label_names: &[&str],
    ) -> Result<HistogramVec, MetricsError> {
        let mut histograms = self.histograms.lock().await;
        if let Some(histogram) = histograms.get(name) {
            return Ok(histogram.clone());
        }

        let opts = HistogramOpts::new(name, help);
        let histogram = HistogramVec::new(opts, label_names)
            .map_err(|e| MetricsError::registration_failed(name, format!("Failed to create histogram: {}", e)))?;

        self.registry.register(Box::new(histogram.clone()))
            .map_err(|e| MetricsError::registration_failed(name, format!("Failed to register histogram: {}", e)))?;

        histograms.insert(name.to_string(), histogram.clone());
        Ok(histogram)
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
impl MetricsExporter for PrometheusExporter {    async fn increment(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;

        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let counter = self
            .get_or_create_counter(name, "Counter metric", &label_names)
            .await?;

        // Safely handle label values with proper error handling
        let metric = if label_names.is_empty() {
            counter.with_label_values(&[] as &[&str])
        } else {
            counter.with_label_values(&label_values)
        };

        metric.inc();
        debug!("Incremented counter {} with labels {:?}", name, labels);
        Ok(())
    }    async fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        validation::validate_numeric_value(value)?;

        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let gauge = self
            .get_or_create_gauge(name, "Gauge metric", &label_names)
            .await?;

        // Safely handle label values with proper error handling
        let metric = if label_names.is_empty() {
            gauge.with_label_values(&[] as &[&str])
        } else {
            gauge.with_label_values(&label_values)
        };

        metric.set(value);
        debug!("Set gauge {} to {} with labels {:?}", name, value, labels);
        Ok(())
    }    async fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        // Validate inputs first
        validation::validate_metric_name(name)?;
        validation::validate_labels(labels)?;
        validation::validate_numeric_value(value)?;

        let (label_names, label_values) = Self::extract_label_names_and_values(labels);
        let histogram = self
            .get_or_create_histogram(name, "Histogram metric", &label_names)
            .await?;

        // Safely handle label values with proper error handling
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
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|e| MetricsError::invalid_name("export", format!("Failed to encode metrics: {}", e)))?;
        Ok(buffer)
    }
}
