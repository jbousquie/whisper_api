# Whisper API metrics module 

## General architecture of the metrics system 

The Whisper API metrics system is designed with a modular architecture that enables the collection, processing, and export of performance data to various monitoring systems. The main module is located in `metrics` and uses a trait-based pattern to support multiple export backends.

Currently supported products:

- Prometheus
- StatsD
- null

Planned/TODO list:

- Grafana
- NewRelic (K8s, complex, no crate, via OpenTelemetry)
- InfluxDB
- Graphite (redundant with statsd)
- Datadog (requires an API key)

## Structure of the metrics module 

### Main module (`metrics.rs`) 

The core of the system is the `Metrics` struct which implements the `MetricsCollector` trait. This struct:

1. Collects metrics asynchronously: all metric operations are non-blocking thanks to *tokio*
2. Is thread-safe: uses `Arc<dyn MetricsCollector + Send + Sync>` to allow sharing between threads
3. Robust error handling: includes safety validations to avoid numeric overflows

#### Supported metric types 

- Counters: for cumulative events (jobs submitted, completed, canceled)
- Gauges: for instantaneous values (queue size, jobs in progress)
- Histograms: for time distributions (transcription durations)

#### Integration with the whisper API 

The metrics system integrates at several levels within the application:

 1. In the queue manager (`queue_manager.rs`)

```rust
// Recording job submission
self.metrics.record_job_submitted(&job_model, &job_language).await;

// Updating queue size
self.metrics.set_queue_size(safe_size).await;

// Tracking the number of jobs in progress
self.metrics.set_jobs_processing(safe_count).await;

// Recording job completion
metrics.record_job_completed(&job.model, &job.language, duration, "success").await;
```

1. In the main application (`main.rs`)

- HTTP metrics for API requests
- Exposing a `/metrics` endpoint for Prometheus
- Configuring the metrics backend based on environment variables

## Export modules 

### Prometheus module (`prometheus.rs`) 

The Prometheus module implements exporting to the Prometheus monitoring system:

**Features**:

- **Metrics Registry**: Uses `prometheus::Registry` to organize all metrics
- **Prometheus Metric Types**:
  - `CounterVec`: For counters with labels (model, language)
  - `GaugeVec`: For gauges with dimensions
  - `HistogramVec`: For time distributions with configurable buckets

**Exposed Metrics**:

- `whisper_jobs_submitted_total`: Total number of submitted jobs
- `whisper_jobs_completed_total`: Total number of completed jobs
- `whisper_queue_size`: Current queue size
- `whisper_jobs_processing`: Number of jobs currently being processed
- `whisper_job_duration_seconds`: Histogram of transcription durations

**Configuration**:

- Customizable histogram buckets via environment variables
- Automatic labels for model and language
- Exposed via HTTP endpoint `/metrics`

### StatsD module (`statsd.rs`) 

The StatsD module enables sending metrics to compatible StatsD servers:

**Protocol**:

- **UDP**: Non-blocking metrics transmission via UDP
- **StatsD Format**: Follows the standard protocol `metric_name:value|type|@sample_rate`
- **Supported Types**: `c` (counter), `g` (gauge), `h` (histogram)

**Features**:

- **Asynchronous Transmission**: Uses `tokio::net::UdpSocket` for non-blocking sending
- **Error Handling**: Logs errors without interrupting the application
- **Automatic Formatting**: Converts metrics to StatsD format

**Configuration**:

```rust
// Environment variables
STATSD_HOST=localhost
STATSD_PORT=8125
STATSD_PREFIX=whisper_api
```

**Detailed Documentation**: The `STATSD.md` file provides a comprehensive guide to integration with different backends (Graphite, InfluxDB, DataDog).

### Null module (`null.rs`) 

The Null module implements a "no-op" backend for metrics:

**Use Cases**:

- **Test Environments**: Disables metrics without modifying code
- **Performance**: Avoids metrics overhead in production if not necessary
- **Development**: Allows development without monitoring infrastructure

**Implementation**:

```rust
impl MetricsCollector for NullMetrics {
    async fn record_job_submitted(&self, _model: &str, _language: &str) {
        // No operation - silently ignore
    }
    // ... all other methods do nothing
}
```

## Configuration and backend selection 

The system uses environment variables to select the backend:

```rust
// Automatic configuration based on environment
let metrics = match std::env::var("METRICS_BACKEND").as_deref() {
    Ok("prometheus") => Metrics::new_prometheus(),
    Ok("statsd") => Metrics::new_statsd(),
    _ => Metrics::new_null(), // Default backend
};
```

## System benefits 

1. **Modularity**: Makes it easy to add new metrics backends
2. **Performance**: Non-blocking asynchronous operations
3. **Reliability**: Robust error handling that doesn't impact the main application
4. **Flexibility**: Support for multiple monitoring systems simultaneously
5. **Observability**: Detailed metrics on all aspects of the API

## Collected metrics 

The system monitors in real-time:

- **Throughput**: Number of jobs submitted, processed, failed
- **Latency**: Processing time by model and language
- **Load**: Queue size, active jobs
- **Quality**: Success rate, error types
- **Resources**: Usage according to concurrency configuration

This architecture enables complete observability of the Whisper API, essential for production monitoring and performance optimization.
