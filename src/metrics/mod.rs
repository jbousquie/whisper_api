// Whisper API metrics for monitoring and observability
//
// This module contains the metrics for the Whisper API.
// It provides functionality to track and report various metrics

pub mod error;
pub mod metrics;
pub mod null;
pub mod prometheus;
pub mod statsd;

// Re-export main types and functions for convenience
// These are intentional re-exports for external use
#[allow(unused_imports)]
pub use metrics::{
    create_metrics_exporter, create_null_exporter, create_prometheus_exporter,
    create_prometheus_exporter_from_env, create_prometheus_exporter_with_namespace,
    create_statsd_exporter_from_env, Metrics, MetricsExporter,
};

// Integration tests (only compiled when testing)
#[cfg(test)]
mod tests {
    mod benchmarks;
    mod integration_tests;
    mod test_statsd;
}
