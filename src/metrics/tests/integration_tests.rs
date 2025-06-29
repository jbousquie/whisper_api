//! Integration tests for the metrics system
//!
//! These tests verify that the metrics system works correctly across
//! different backends and handles errors appropriately.

use crate::metrics::{
    create_null_exporter, create_prometheus_exporter, create_statsd_exporter_from_env,
    error::MetricsError, Metrics,
};

#[tokio::test]
async fn test_prometheus_metrics_basic_operations() {
    let exporter = create_prometheus_exporter().expect("Failed to create Prometheus exporter");
    let metrics = Metrics::new(exporter);

    // Test counter increment
    metrics
        .increment("test_counter", &[("label", "value")])
        .await
        .expect("Failed to increment counter");

    // Test gauge setting
    metrics
        .set_gauge("test_gauge", 42.0, &[])
        .await
        .expect("Failed to set gauge");

    // Test histogram observation
    metrics
        .observe_histogram("test_histogram", 1.23, &[("type", "test")])
        .await
        .expect("Failed to observe histogram");

    // Test export
    let exported = metrics.export().await.expect("Failed to export metrics");
    let exported_str = String::from_utf8(exported).expect("Invalid UTF-8 in exported metrics");

    // Verify metrics are present in export
    assert!(exported_str.contains("test_counter"));
    assert!(exported_str.contains("test_gauge"));
    assert!(exported_str.contains("test_histogram"));
}

#[tokio::test]
async fn test_null_exporter_never_fails() {
    let metrics = Metrics::new(create_null_exporter());

    // All operations should succeed silently
    let _ = metrics.increment("any_name", &[]).await;
    let _ = metrics.set_gauge("any_gauge", f64::NAN, &[]).await; // Even NaN should be handled
    let _ = metrics.observe_histogram("any_histogram", -1.0, &[]).await; // Even negative values

    let exported = metrics
        .export()
        .await
        .expect("Null exporter should never fail");
    assert_eq!(exported, b""); // Null exporter returns empty bytes
}

#[tokio::test]
async fn test_metrics_validation_errors() {
    let exporter = create_prometheus_exporter().expect("Failed to create Prometheus exporter");

    // Test direct exporter calls to verify error handling
    let result = exporter.increment("", &[]).await;
    assert!(matches!(result, Err(MetricsError::InvalidName { .. })));

    let result = exporter.set_gauge("valid_name", f64::INFINITY, &[]).await;
    assert!(matches!(result, Err(MetricsError::InvalidValue { .. })));

    let result = exporter.increment("valid_name", &[("", "value")]).await;
    assert!(matches!(result, Err(MetricsError::InvalidLabel { .. })));
}

#[tokio::test]
async fn test_convenience_methods() {
    let metrics = Metrics::new(create_null_exporter());

    // Test HTTP request recording
    metrics
        .record_http_request("/api/v1/transcribe", "POST", "200", 0.123)
        .await;

    // Test queue metrics
    metrics.set_queue_size(100.0).await;
    metrics.record_queue_size(50).await;

    // Test transcription metrics (using generic increment/gauge methods)
    metrics
        .increment(
            "transcription_requests",
            &[("language", "en"), ("model", "whisper-large")],
        )
        .await
        .expect("Failed to increment transcription_requests");
    metrics
        .observe_histogram(
            "transcription_duration",
            2.5,
            &[("language", "en"), ("model", "whisper-large")],
        )
        .await
        .expect("Failed to observe transcription_duration");

    // All should complete without panicking
}

#[tokio::test]
async fn test_large_values_handling() {
    let metrics = Metrics::new(create_null_exporter());

    // Test very large queue size (should trigger conversion error but not panic)
    metrics.record_queue_size(usize::MAX).await;

    // Test large histogram values
    metrics.observe_histogram("large_values", 1e100, &[]).await.expect("Failed to observe large histogram value");

    // Should complete without issues
}

// Environment-dependent test (only runs if STATSD_ENDPOINT is set)
#[tokio::test]
#[ignore] // Ignore by default since it requires StatsD server
async fn test_statsd_from_env() {
    // This test requires:
    // STATSD_ENDPOINT=127.0.0.1:8125
    // And a running StatsD server

    if std::env::var("STATSD_ENDPOINT").is_ok() {
        let exporter = create_statsd_exporter_from_env();
        match exporter {
            Ok(exporter) => {
                let metrics = Metrics::new(exporter);
                metrics
                    .increment("test_statsd_counter", &[("source", "integration_test")])
                    .await
                    .expect("Failed to increment StatsD counter");
                // Note: StatsD is fire-and-forget, so we can't verify receipt :(
            }
            Err(e) => {
                eprintln!("StatsD configuration error (expected in CI): {}", e);
            }
        }
    }
}
