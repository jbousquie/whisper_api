/// StatsD integration test
///
/// This example demonstrates how to use the StatsD metrics exporter.
///
/// To run this example:
/// 1. Start a StatsD server (or use netcat to listen on UDP port 8125)
/// 2. Run: cargo run --example test_statsd
///
/// To monitor StatsD messages with netcat (Windows):
/// ```
/// nc -u -l -p 8125
/// ```
use std::time::Duration;
use tokio::time::sleep;
use whisper_api::metrics::metrics::{create_metrics_exporter, Metrics};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("Testing StatsD metrics integration...");

    // Create StatsD exporter pointing to localhost:8125
    let metrics_exporter = create_metrics_exporter(
        "statsd",
        Some("localhost:8125"),
        Some("whisper_api_test"), // prefix
        Some(1.0),                // sample rate
    );

    let metrics = Metrics::new(metrics_exporter);

    println!("Sending test metrics to StatsD server at localhost:8125...");
    println!("Monitor with: nc -u -l -p 8125");

    // Test counter metrics
    println!("1. Testing counter metrics...");
    metrics
        .increment("test_counter", &[("environment", "test")])
        .await;
    metrics.record_job_submitted("large-v3", "en").await;
    sleep(Duration::from_millis(100)).await;

    // Test gauge metrics
    println!("2. Testing gauge metrics...");
    metrics
        .set_gauge("test_gauge", 42.0, &[("type", "test")])
        .await;
    metrics.set_queue_size(5.0).await;
    metrics.set_jobs_processing(2.0).await;
    sleep(Duration::from_millis(100)).await;

    // Test histogram metrics
    println!("3. Testing histogram metrics...");
    metrics
        .observe_histogram("test_histogram", 123.45, &[("bucket", "test")])
        .await;
    metrics
        .record_http_request("/transcribe", "POST", "200", 1.5)
        .await;
    sleep(Duration::from_millis(100)).await;

    // Test application-specific metrics
    println!("4. Testing application-specific metrics...");
    metrics
        .record_job_completed("large-v3", "en", 45.2, "success")
        .await;
    metrics.record_auth_attempt("success").await;
    metrics.record_file_size(1024000.0).await;
    sleep(Duration::from_millis(100)).await;

    println!("Test completed! Check your StatsD receiver for the metrics.");

    Ok(())
}
