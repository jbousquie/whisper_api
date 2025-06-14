# Whisper API Metrics Testing

This directory contains comprehensive test scripts for validating the metrics functionality of the Whisper API.

## Test Scripts

### 1. `run_metrics_tests.sh` - Main Test Suite

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

### 2. `test_prometheus_metrics.sh` - Prometheus-Specific Tests

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

### 3. `test_statsd_metrics.sh` - StatsD-Specific Tests

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

### 4. `test_metrics.ps1` - PowerShell Script for Windows

Cross-platform PowerShell script for Windows users to test metrics functionality.

**Usage:**

```powershell
# Run all tests (auto-detect backend)
.\test_metrics.ps1

# Test specific backend
.\test_metrics.ps1 -Backend prometheus
.\test_metrics.ps1 -Backend statsd

# Show configuration guide
.\test_metrics.ps1 -Backend guide

# Analyze current metrics
.\test_metrics.ps1 -Backend analyze

# Custom API endpoint
.\test_metrics.ps1 -ApiHost "192.168.1.100" -ApiPort "8080"
```

### 5. `test_metrics.bat` - Windows Batch Script

Simple batch file for Windows users who prefer traditional batch scripts.

**Usage:**

```cmd
# Run the test
test_metrics.bat
```

**Note:** Requires `curl` to be installed and available in PATH.

### 6. `test_runner.sh` - Convenience Test Runner

Easy-to-use interface for running different test types.

**Usage:**

```bash
# Run comprehensive tests
./test_runner.sh all

# Test specific backends
./test_runner.sh prometheus
./test_runner.sh statsd

# Environment and setup
./test_runner.sh setup        # Set up test environment
./test_runner.sh status       # Show environment status
./test_runner.sh debug        # Debug environment variables and API connectivity
./test_runner.sh clean        # Clean up test files

# Custom API endpoint
./test_runner.sh all --host "192.168.1.100" --port "8080"
```

### 7. `debug_env.sh` - Environment Variable Debugger

Diagnostic script to help troubleshoot environment variable configuration issues.

**Usage:**

```bash
# Debug environment variables
./debug_env.sh

# Debug with API connectivity test
./debug_env.sh --test-api
```

**Features:**

- Shows all relevant environment variables
- Calculates effective configuration
- Provides recommendations for fixes
- Tests API connectivity when requested

## Prerequisites

### For All Tests

- `curl` - For HTTP requests
- `bash` - Shell interpreter
- Running Whisper API instance

### For StatsD Tests (Optional but Recommended)

- `netcat` (`nc`) - For UDP connectivity testing
- `tcpdump` - For network traffic capture (requires sudo)
- StatsD server (Graphite, InfluxDB, or simple test server)

### For Enhanced Testing

- `sudo` access (for network capture)
- Docker (for running test infrastructure)

## Setting Up Test Environment

### Prometheus Testing

```bash
# Start API with Prometheus metrics
METRICS_BACKEND=prometheus ./whisper_api

# Access metrics at
curl http://localhost:8181/metrics
```

### StatsD Testing

#### Option 1: Using Docker Graphite+StatsD

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

#### Option 2: Simple Test Server

```bash
# Start a simple UDP listener for testing
nc -u -l -p 8125 > statsd_metrics.log &

# Start API with StatsD metrics
METRICS_BACKEND=statsd ./whisper_api

# View captured metrics
tail -f statsd_metrics.log
```

#### Option 3: InfluxDB with StatsD Plugin

```bash
# Configure InfluxDB with StatsD input plugin
# See InfluxDB documentation for configuration details

# Start API pointing to InfluxDB StatsD endpoint
METRICS_BACKEND=statsd \
STATSD_HOST=your-influxdb-host \
STATSD_PORT=8125 \
./whisper_api
```

## Test Scenarios

The test scripts simulate various scenarios to validate metrics collection:

### 1. Job Lifecycle Metrics

- Job submission
- Job processing
- Job completion/failure
- Job cancellation

### 2. Queue Metrics

- Queue size tracking
- Job position monitoring
- Processing count updates

### 3. HTTP Request Metrics

- Request counting by endpoint
- Response time measurement
- Status code tracking

### 4. System Metrics

- Performance impact measurement
- Concurrent request handling
- Error rate monitoring

## Expected Metrics

### Prometheus Metrics

- `whisper_jobs_submitted_total{model, language}`
- `whisper_jobs_completed_total{model, language, status}`
- `whisper_jobs_cancelled_total{model, language}`
- `whisper_jobs_failed_total{model, language}`
- `whisper_queue_size`
- `whisper_jobs_processing`
- `whisper_job_duration_seconds{model, language}`
- `whisper_http_requests_total{method, endpoint, status}`
- `whisper_http_request_duration_seconds{method, endpoint}`

### StatsD Metrics

- `whisper_api.jobs.submitted:1|c|#model:large,language:en`
- `whisper_api.jobs.completed:1|c|#model:large,language:en,status:success`
- `whisper_api.queue.size:5|g`
- `whisper_api.jobs.processing:2|g`
- `whisper_api.job.duration:45.23|h|#model:large,language:en`
- `whisper_api.http.requests:1|c|#method:POST,endpoint:/transcription,status:202`
- `whisper_api.http.duration:120.5|h|#method:POST,endpoint:/transcription`

## Troubleshooting

### Common Issues

1. **API Not Accessible**

   ```bash
   # Check if API is running
   curl http://localhost:8181/metrics
   
   # Check API logs
   ./whisper_api
   ```

2. **Backend Not Detected**

   ```bash
   # Check environment variables
   echo $METRICS_BACKEND
   echo $METRICS_EXPORTER
   
   # Check API configuration logs
   # The API should log metrics configuration at startup
   
   # Try both variable names for compatibility
   export METRICS_BACKEND=prometheus
   # or
   export METRICS_EXPORTER=prometheus
   
   # Restart the API and check logs
   ./whisper_api
   ```

3. **Prometheus Metrics Empty**

   ```bash
   # Ensure Prometheus backend is configured
   METRICS_BACKEND=prometheus ./whisper_api
   
   # Generate some activity
   curl -X POST -F "audio=@test.wav" http://localhost:8181/transcription
   ```

4. **StatsD Metrics Not Received**

   ```bash
   # Test StatsD connectivity
   echo "test:1|c" | nc -u localhost 8125
   
   # Check StatsD server logs
   # Verify STATSD_HOST and STATSD_PORT configuration
   
   # Try both configuration methods:
   # Method 1: Separate host/port
   export STATSD_HOST=127.0.0.1
   export STATSD_PORT=8125
   
   # Method 2: Combined endpoint
   export STATSD_ENDPOINT=127.0.0.1:8125
   ```

5. **Permission Denied for tcpdump**

   ```bash
   # Run with sudo or add user to appropriate group
   sudo ./test_statsd_metrics.sh
   
   # Or disable traffic capture in the script
   ```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WHISPER_API_HOST` | `127.0.0.1` | API host address |
| `WHISPER_API_PORT` | `8181` | API port |
| `METRICS_BACKEND` | `none` | Metrics backend (`prometheus`, `statsd`, `none`, `null`) |
| `METRICS_EXPORTER` | `none` | Alternative name for METRICS_BACKEND (deprecated) |
| `STATSD_HOST` | `127.0.0.1` | StatsD server host |
| `STATSD_PORT` | `8125` | StatsD server port |
| `STATSD_ENDPOINT` | (computed) | Alternative: `host:port` format |
| `STATSD_PREFIX` | (none) | StatsD metric prefix |
| `STATSD_SAMPLE_RATE` | `1.0` | StatsD sampling rate |

**Note:** The application supports multiple environment variable names for backward compatibility:

- `METRICS_BACKEND` or `METRICS_EXPORTER` for backend selection
- `STATSD_HOST`/`STATSD_PORT` or `STATSD_ENDPOINT` for StatsD configuration
- `STATSD_PREFIX` or `METRICS_PREFIX` for metric prefixes

## Integration Examples

### Grafana Dashboard

After collecting metrics with Prometheus or StatsD, create Grafana dashboards:

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

### Alerting Rules

Set up alerts for important metrics:

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

When adding new metrics or modifying existing ones:

1. Update the expected metrics lists in test scripts
2. Add validation for new metric formats
3. Include new metrics in the documentation
4. Test with both Prometheus and StatsD backends

## Files Generated During Testing

The test scripts may create the following temporary files:

- `/tmp/whisper_metrics_*.txt` - Prometheus metrics samples
- `/tmp/statsd_metrics_*.log` - Captured StatsD metrics
- `/tmp/statsd_capture_*.pcap` - Network traffic captures
- `/tmp/whisper_metrics_test_*` - Test result directories
- `/tmp/test_audio_*.wav` - Temporary test files

These files are automatically cleaned up after tests complete.
