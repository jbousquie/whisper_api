# Whisper API metrics testing

This directory contains comprehensive test scripts for validating the metrics functionality of the Whisper API.

## Test scripts

### 1. `run_metrics_tests.sh` - Main test suite

Comprehensive test suite that automatically detects the current metrics backend and runs appropriate tests.

**Usage:**

```bash
# Run all tests
./run_metrics_tests.sh

# Run specific backend tests
./run_metrics_tests.sh prometheus
./run_metrics_tests.sh statsd

# Show configuration guide
./run_metrics_tests.sh guide

# Analyze current metrics
./run_metrics_tests.sh analyze
```

### 2. `test_prometheus_metrics.sh` - Prometheus-specific tests

Detailed testing for Prometheus metrics backend including format validation, endpoint testing, and metric collection verification.

**Usage:**

```bash
# Ensure API is running with Prometheus backend
METRICS_BACKEND=prometheus ./whisper_api

# Run Prometheus tests
./test_prometheus_metrics.sh
```

**Features:**

- Validates Prometheus metrics format
- Tests `/metrics` endpoint availability
- Verifies expected Whisper API metrics exist
- Simulates job submissions to test metric collection
- Benchmarks metrics endpoint performance
- Saves metrics samples for analysis

### 3. `test_statsd_metrics.sh` - StatsD-specific tests

Comprehensive testing for StatsD metrics backend including traffic capture and metric validation.

**Usage:**

```bash
# Ensure API is running with StatsD backend
METRICS_BACKEND=statsd STATSD_HOST=127.0.0.1 STATSD_PORT=8125 ./whisper_api

# Run StatsD tests
./test_statsd_metrics.sh
```

**Features:**

- Tests StatsD server connectivity
- Captures UDP traffic for metric validation
- Simulates API workload to generate metrics
- Provides StatsD format validation
- Includes test StatsD server for development

## Prerequisites

### For all tests

- `curl` - For HTTP requests
- `bash` - Shell interpreter
- Running Whisper API instance

### For StatsD tests (optional but recommended)

- `netcat` (`nc`) - For UDP connectivity testing
- `tcpdump` - For network traffic capture (requires sudo)
- StatsD server (Graphite, InfluxDB, or simple test server)

### For enhanced testing

- `sudo` access (for network capture)
- Docker (for running test infrastructure)

## Setting up test environment

### Prometheus testing

```bash
# Start API with Prometheus metrics
METRICS_BACKEND=prometheus ./whisper_api

# Access metrics at
curl http://localhost:8181/metrics
```

### StatsD testing

#### Option 1: using docker Graphite+StatsD

```bash
# Start Graphite with built-in StatsD
docker run -d \
  --name graphite \
  -p 8080:80 \
  -p 8125:8125/udp \
  graphiteapp/graphite-statsd

# Start API with StatsD metrics
METRICS_BACKEND=statsd \
STATSD_HOST=127.0.0.1 \
STATSD_PORT=8125 \
STATSD_PREFIX=whisper_api \
./whisper_api

# View metrics at http://localhost:8080
```

#### Option 2: simple test server

```bash
# Start a simple UDP listener for testing
nc -u -l -p 8125 > statsd_metrics.log &

# Start API with StatsD metrics
METRICS_BACKEND=statsd ./whisper_api

# View captured metrics
tail -f statsd_metrics.log
```

#### Option 3: InfluxDB with StatsD plugin

```bash
# Configure InfluxDB with StatsD input plugin
# See InfluxDB documentation for configuration details

# Start API pointing to InfluxDB StatsD endpoint
METRICS_BACKEND=statsd \
STATSD_HOST=your-influxdb-host \
STATSD_PORT=8125 \
./whisper_api
```

## Test scenarios

The test scripts simulate various scenarios to validate metrics collection :

### 1. Job lifecycle metrics

- Job submission
- Job processing
- Job completion/failure
- Job cancellation

### 2. Queue metrics

- Queue size tracking
- Job position monitoring
- Processing count updates

### 3. HTTP request metrics

- Request counting by endpoint
- Response time measurement
- Status code tracking

### 4. System metrics

- Performance impact measurement
- Concurrent request handling
- Error rate monitoring

## Expected metrics

### Prometheus metrics

- `whisper_jobs_submitted_total{model, language}`
- `whisper_jobs_completed_total{model, language, status}`
- `whisper_jobs_cancelled_total{model, language}`
- `whisper_jobs_failed_total{model, language}`
- `whisper_queue_size`
- `whisper_jobs_processing`
- `whisper_job_duration_seconds{model, language}`
- `whisper_http_requests_total{method, endpoint, status}`
- `whisper_http_request_duration_seconds{method, endpoint}`

### StatsD metrics

- `whisper_api.jobs.submitted:1|c|#model:large,language:en`
- `whisper_api.jobs.completed:1|c|#model:large,language:en,status:success`
- `whisper_api.queue.size:5|g`
- `whisper_api.jobs.processing:2|g`
- `whisper_api.job.duration:45.23|h|#model:large,language:en`
- `whisper_api.http.requests:1|c|#method:POST,endpoint:/transcription,status:202`
- `whisper_api.http.duration:120.5|h|#method:POST,endpoint:/transcription`

## Troubleshooting

### Common issues

1. **API not accessible**

   ```bash
   # Check if API is running
   curl http://localhost:8181/metrics
   
   # Check API logs
   ./whisper_api
   ```

2. **Prometheus Metrics Empty**

   ```bash
   # Ensure Prometheus backend is configured
   METRICS_BACKEND=prometheus ./whisper_api
   
   # Generate some activity
   curl -X POST -F "audio=@test.wav" http://localhost:8181/transcription
   ```

3. **StatsD metrics not received**

   ```bash
   # Test StatsD connectivity
   echo "test:1|c" | nc -u localhost 8125
   
   # Check StatsD server logs
   # Verify STATSD_HOST and STATSD_PORT configuration
   ```

4. **Permission denied for tcpdump**

   ```bash
   # Run with sudo or add user to appropriate group
   sudo ./test_statsd_metrics.sh
   
   # Or disable traffic capture in the script
   ```

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WHISPER_API_HOST` | `127.0.0.1` | API host address |
| `WHISPER_API_PORT` | `8181` | API port |
| `METRICS_BACKEND` | `null` | Metrics backend (`prometheus`, `statsd`, `null`) |
| `STATSD_HOST` | `127.0.0.1` | StatsD server host |
| `STATSD_PORT` | `8125` | StatsD server port |
| `STATSD_PREFIX` | `whisper_api` | StatsD metric prefix |

## Integration examples

### Grafana dashboard

After collecting metrics with Prometheus or StatsD, create Grafana dashboards :

```json
{
  "dashboard": {
    "title": "Whisper API Metrics",
    "panels": [
      {
        "title": "Jobs Submitted Rate",
        "targets": [
          {
            "expr": "rate(whisper_jobs_submitted_total[5m])"
          }
        ]
      },
      {
        "title": "Queue Size",
        "targets": [
          {
            "expr": "whisper_queue_size"
          }
        ]
      }
    ]
  }
}
```

### Alerting rules

Set up alerts for important metrics :

```yaml
groups:
  - name: whisper_api
    rules:
      - alert: HighQueueSize
        expr: whisper_queue_size > 10
        for: 5m
        annotations:
          summary: "Whisper API queue is growing large"
          
      - alert: HighFailureRate
        expr: rate(whisper_jobs_failed_total[5m]) > 0.1
        for: 2m
        annotations:
          summary: "High job failure rate detected"
```

## Contributing

When adding new metrics or modifying existing ones :

1. Update the expected metrics lists in test scripts
2. Add validation for new metric formats
3. Include new metrics in the documentation
4. Test with both Prometheus and StatsD backends

## Files Generated during testing

The test scripts may create the following temporary files :

- `/tmp/whisper_metrics_*.txt` - Prometheus metrics samples
- `/tmp/statsd_metrics_*.log` - Captured StatsD metrics
- `/tmp/statsd_capture_*.pcap` - Network traffic captures
- `/tmp/whisper_metrics_test_*` - Test result directories
- `/tmp/test_audio_*.wav` - Temporary test files

These files are automatically cleaned up after tests complete.
