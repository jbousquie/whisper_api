# StatsD integration guide

This document explains how to use and test the StatsD metrics functionality in the Whisper API.

## Overview

The StatsD exporter sends metrics via UDP to a StatsD server using the standard StatsD protocol. The implementation supports:

- **Counters** : track events like job submissions, HTTP requests, authentication attempts
- **Gauges** : track current values like queue size, active jobs
- **Histograms/Timers** : track durations like job processing time, HTTP response time

## Configuration

Configure StatsD using environment variables:

```bash
# Enable StatsD metrics
export METRICS_EXPORTER=statsd

# StatsD server endpoint (default: localhost:8125)
export METRICS_ENDPOINT=localhost:8125

# Optional: Prefix for all metrics (recommended for multi-service environments)
export METRICS_PREFIX=whisper_api

# Optional: Sample rate for metrics (0.0 to 1.0, default: 1.0)
export METRICS_SAMPLE_RATE=1.0
```

### Windows PowerShell example

```powershell
$env:METRICS_EXPORTER="statsd"
$env:METRICS_ENDPOINT="localhost:8125"
$env:METRICS_PREFIX="whisper_api"
```

## StatsD protocol format

The implementation sends metrics in standard StatsD format:

- **Counter**: `metric_name:1|c|#tag1:value1,tag2:value2`
- **Gauge**: `metric_name:value|g|#tag1:value1,tag2:value2`
- **Timer**: `metric_name:value|ms|#tag1:value1,tag2:value2`

Example messages sent:

```
whisper_api.jobs_submitted_total:1|c|#model:large-v3,language:en
whisper_api.queue_size:5|g
whisper_api.http_request_duration_seconds:1500|ms|#endpoint:/transcribe,method:POST,status:200
```

## Testing with netcat

To monitor StatsD messages during development:

### Windows

```powershell
# Install netcat if not available
# Then listen on UDP port 8125
nc -u -l -p 8125
```

### Linux/macOS

```bash
nc -u -l 8125
```

## Metrics collected

### HTTP metrics

- `http_requests_total` (counter) : total HTTP requests with endpoint, method, status labels
- `http_request_duration_seconds` (histogram) : request duration in milliseconds

### Job metrics

- `jobs_submitted_total` (counter) : jobs submitted with model, language labels
- `jobs_completed_total` (counter) : jobs completed with model, language, status labels
- `jobs_cancelled_total` (counter) : jobs cancelled with model, language labels
- `job_processing_duration_seconds` (histogram) : job processing time
- `jobs_processing_started_total` (counter) : jobs started processing

### Queue metrics

- `queue_size` (gauge) : current queue size
- `jobs_processing` (gauge) : number of jobs currently processing

### Authentication metrics

- `auth_attempts_total` (counter) : authentication attempts with status label

### File metrics

- `file_size_bytes` (histogram) : uploaded file sizes

## Integration with monitoring systems

### Graphite + StatsD

1. Install StatsD and Graphite
2. Configure StatsD to forward to Graphite
3. View metrics in Graphite dashboard

### DataDog

1. Install DataDog StatsD agent
2. Configure endpoint to DataDog agent
3. View metrics in DataDog dashboard

### Custom StatsD server

The implementation works with any StatsD-compatible server that accepts UDP messages on the configured port.

## Error handling

The StatsD exporter is designed to be non-blocking and fault-tolerant:

- UDP send failures are logged but don't affect application performance
- Invalid configurations fall back to null exporter
- Network errors are handled gracefully without application impact

## Performance considerations

- Uses async UDP for non-blocking metric transmission
- Supports sampling to reduce network traffic in high-throughput environments
- Minimal memory overhead with efficient string formatting
- No buffering - metrics are sent immediately

## Example usage in code

```rust
use whisper_api::metrics::metrics::{create_metrics_exporter, Metrics};

// Create StatsD exporter
let metrics_exporter = create_metrics_exporter(
    "statsd",
    Some("localhost:8125"),
    Some("my_app"),  // prefix
    Some(1.0),       // sample rate
);

let metrics = Metrics::new(metrics_exporter);

// Record metrics
metrics.increment("custom_counter", &[("tag", "value")]).await;
metrics.set_gauge("active_connections", 42.0, &[]).await;
metrics.observe_histogram("request_duration", 123.45, &[("endpoint", "/api")]).await;
```
