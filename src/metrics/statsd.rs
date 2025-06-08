/// StatsD Exporter for Whisper API Metrics
///
/// This module provides a complete implementation for StatsD metrics export.
/// StatsD is a network daemon that listens for statistics sent over UDP and
/// aggregates them before sending to a backend service like Graphite.
///
/// The StatsD protocol supports the following metric types:
/// - Counters: `metric_name:value|c[|@sample_rate][|#tag1:value1,tag2:value2]`
/// - Gauges: `metric_name:value|g[|#tag1:value1,tag2:value2]`
/// - Timers/Histograms: `metric_name:value|ms[|@sample_rate][|#tag1:value1,tag2:value2]`
use crate::metrics::metrics::MetricsExporter;
use crate::metrics::error::MetricsError;
use async_trait::async_trait;
use log::{debug, error};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// StatsD metrics exporter implementation
pub struct StatsDExporter {
    /// StatsD server address (host:port)
    address: SocketAddr,
    /// Optional metric prefix for all metrics
    prefix: Option<String>,
    /// Sample rate for metrics (0.0 to 1.0)
    sample_rate: f64,
}

impl StatsDExporter {
    /// Create a new StatsD exporter
    ///
    /// # Arguments
    /// * `endpoint` - StatsD server endpoint in format "host:port"
    /// * `prefix` - Optional prefix for all metric names
    /// * `sample_rate` - Sample rate for metrics (default: 1.0)
    ///
    /// # Example
    /// ```
    /// let exporter = StatsDExporter::new("127.0.0.1:8125".to_string(), Some("whisper_api".to_string()), Some(1.0))?;
    /// ```
    pub fn new(
        endpoint: String,
        prefix: Option<String>,
        sample_rate: Option<f64>,
    ) -> Result<Self, String> {
        // Handle hostname resolution for localhost and other hostnames
        let address = if endpoint.starts_with("localhost:") {
            // Replace localhost with 127.0.0.1
            endpoint.replace("localhost:", "127.0.0.1:")
        } else {
            endpoint.clone()
        };

        let address = address
            .parse::<SocketAddr>()
            .map_err(|e| format!("Invalid StatsD endpoint '{}': {}", endpoint, e))?;

        let sample_rate = sample_rate.unwrap_or(1.0);
        if !(0.0..=1.0).contains(&sample_rate) {
            return Err("Sample rate must be between 0.0 and 1.0".to_string());
        }

        Ok(Self {
            address,
            prefix,
            sample_rate,
        })
    }

    /// Format metric name with optional prefix
    fn format_metric_name(&self, name: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}.{}", prefix, name),
            None => name.to_string(),
        }
    }

    /// Format labels as StatsD tags
    /// Converts labels to StatsD tag format: #tag1:value1,tag2:value2
    fn format_tags(&self, labels: &[(&str, &str)]) -> String {
        if labels.is_empty() {
            String::new()
        } else {
            let tags: Vec<String> = labels
                .iter()
                .map(|(key, value)| format!("{}:{}", key, value))
                .collect();
            format!("|#{}", tags.join(","))
        }
    }

    /// Format sample rate for StatsD message
    fn format_sample_rate(&self) -> String {
        if self.sample_rate < 1.0 {
            format!("|@{}", self.sample_rate)
        } else {
            String::new()
        }
    }

    /// Send a StatsD message via UDP
    async fn send_metric(&self, message: &str) {
        // Skip sending if sampling and random check fails
        if self.sample_rate < 1.0 && fastrand::f64() > self.sample_rate {
            return;
        }

        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(socket) => {
                if let Err(e) = socket.send_to(message.as_bytes(), &self.address).await {
                    error!("Failed to send StatsD metric: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to create UDP socket for StatsD: {}", e);
            }
        }

        debug!("Sent StatsD metric: {}", message);
    }
}

#[async_trait]
impl MetricsExporter for StatsDExporter {
    /// Increment a counter metric
    /// Format: metric_name:1|c[|@sample_rate][|#tags]
    async fn increment(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        let metric_name = self.format_metric_name(name);
        let tags = self.format_tags(labels);
        let sample_rate = self.format_sample_rate();

        let message = format!("{}:1|c{}{}", metric_name, sample_rate, tags);
        self.send_metric(&message).await;
        Ok(())
    }
    /// Set a gauge metric value
    /// Format: metric_name:value|g[|#tags]
    async fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        let metric_name = self.format_metric_name(name);
        let tags = self.format_tags(labels);

        let message = format!("{}:{}|g{}", metric_name, value, tags);
        self.send_metric(&message).await;
        Ok(())
    }
    /// Observe a value in a histogram/timer metric
    /// Format: metric_name:value|ms[|@sample_rate][|#tags]
    async fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        let metric_name = self.format_metric_name(name);
        let tags = self.format_tags(labels);
        let sample_rate = self.format_sample_rate();

        // Convert to milliseconds if the value looks like seconds (< 100)
        let value_ms = if value < 100.0 { value * 1000.0 } else { value };

        let message = format!("{}:{}|ms{}{}", metric_name, value_ms, sample_rate, tags);
        self.send_metric(&message).await;
        Ok(())
    }
    /// Export metrics - StatsD doesn't support pull-based exports
    /// This method returns an empty result since StatsD is push-based
    async fn export(&self) -> Result<Vec<u8>, MetricsError> {
        // StatsD is a push-based system, not pull-based like Prometheus
        // Return empty response as there's nothing to export
        debug!("StatsD export called - StatsD is push-based, no data to export");
        Ok(vec![])
    }
}
