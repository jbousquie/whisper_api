//! Performance benchmarks for the metrics system
//!
//! These benchmarks test thread safety, lock contention, and overall performance
//! of the metrics system under various load conditions.

#[cfg(test)]
mod benchmarks {
    use crate::metrics::{create_null_exporter, create_prometheus_exporter, Metrics};
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::task;

    #[tokio::test]
    async fn bench_concurrent_counter_increments() {
        let metrics = Arc::new(Metrics::new(create_null_exporter()));
        let num_tasks = 100;
        let operations_per_task = 1000;

        let start = Instant::now();
        let mut handles = Vec::new();

        for task_id in 0..num_tasks {
            let metrics_clone = Arc::clone(&metrics);
            handles.push(task::spawn(async move {
                for op in 0..operations_per_task {
                    metrics_clone
                        .increment(
                            "bench_counter",
                            &[("task", &task_id.to_string()), ("op", &op.to_string())],
                        )
                        .await;
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let duration = start.elapsed();
        let total_operations = num_tasks * operations_per_task;
        let ops_per_sec = total_operations as f64 / duration.as_secs_f64();

        println!(
            "Concurrent counter benchmark: {} operations in {:.2}s ({:.0} ops/sec)",
            total_operations,
            duration.as_secs_f64(),
            ops_per_sec
        );

        // Assert reasonable performance (should handle at least 10k ops/sec even on slow machines)
        assert!(
            ops_per_sec > 10_000.0,
            "Performance too low: {:.0} ops/sec",
            ops_per_sec
        );
    }

    #[tokio::test]
    async fn bench_mixed_metric_types() {
        let metrics = Arc::new(Metrics::new(create_null_exporter()));
        let num_tasks = 50;

        let start = Instant::now();
        let mut handles = Vec::new();

        for task_id in 0..num_tasks {
            let metrics_clone = Arc::clone(&metrics);
            handles.push(task::spawn(async move {
                for i in 0..500 {
                    // Mix different metric types to test lock contention
                    metrics_clone
                        .increment("mixed_counter", &[("task", &task_id.to_string())])
                        .await;
                    metrics_clone
                        .set_gauge("mixed_gauge", i as f64, &[("task", &task_id.to_string())])
                        .await;
                    metrics_clone
                        .observe_histogram(
                            "mixed_histogram",
                            i as f64 * 0.1,
                            &[("task", &task_id.to_string())],
                        )
                        .await;
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let duration = start.elapsed();
        let total_operations = num_tasks * 500 * 3; // 3 operations per iteration
        let ops_per_sec = total_operations as f64 / duration.as_secs_f64();

        println!(
            "Mixed metrics benchmark: {} operations in {:.2}s ({:.0} ops/sec)",
            total_operations,
            duration.as_secs_f64(),
            ops_per_sec
        );

        // Should handle mixed workload efficiently
        assert!(
            ops_per_sec > 5_000.0,
            "Mixed workload performance too low: {:.0} ops/sec",
            ops_per_sec
        );
    }

    #[tokio::test]
    async fn bench_prometheus_export_performance() {
        let metrics = Arc::new(Metrics::new(create_prometheus_exporter().unwrap()));

        // Create a variety of metrics
        for i in 0..100 {
            metrics
                .increment("export_test_counter", &[("series", &i.to_string())])
                .await;
            metrics
                .set_gauge("export_test_gauge", i as f64, &[("series", &i.to_string())])
                .await;
            metrics
                .observe_histogram(
                    "export_test_histogram",
                    i as f64,
                    &[("series", &i.to_string())],
                )
                .await;
        }

        // Benchmark export performance
        let num_exports = 100;
        let start = Instant::now();

        for _ in 0..num_exports {
            let _export_data = metrics.export().await.unwrap();
        }

        let duration = start.elapsed();
        let exports_per_sec = num_exports as f64 / duration.as_secs_f64();

        println!(
            "Prometheus export benchmark: {} exports in {:.2}s ({:.0} exports/sec)",
            num_exports,
            duration.as_secs_f64(),
            exports_per_sec
        );

        // Should be able to export metrics frequently
        assert!(
            exports_per_sec > 50.0,
            "Export performance too low: {:.0} exports/sec",
            exports_per_sec
        );
    }

    #[tokio::test]
    async fn bench_metric_creation_overhead() {
        let metrics = Arc::new(Metrics::new(create_prometheus_exporter().unwrap()));
        let num_unique_metrics = 1000;

        let start = Instant::now();

        // Test creating many unique metrics (tests double-checked locking optimization)
        for i in 0..num_unique_metrics {
            metrics
                .increment(&format!("unique_metric_{}", i), &[])
                .await;
        }

        let duration = start.elapsed();
        let creations_per_sec = num_unique_metrics as f64 / duration.as_secs_f64();

        println!(
            "Metric creation benchmark: {} unique metrics in {:.2}s ({:.0} creations/sec)",
            num_unique_metrics,
            duration.as_secs_f64(),
            creations_per_sec
        );

        // Should handle metric creation efficiently
        assert!(
            creations_per_sec > 100.0,
            "Metric creation too slow: {:.0} creations/sec",
            creations_per_sec
        );
    }
}
