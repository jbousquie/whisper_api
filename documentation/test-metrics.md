# Testing Whisper API Metrics with CLI Tools

This guide shows how to test the metrics system implemented in the Whisper API using command-line tools like curl, PowerShell, and other utilities.

## Prerequisites

1. **Start the Whisper API server**:
```bash
# Build and run the server
cargo run --release
```

2. **Verify server is running**:
```bash
curl http://127.0.0.1:8181/api/status
```

## Testing Prometheus Metrics

### 1. Basic Metrics Endpoint Test

**Using curl (Linux/macOS/WSL):**
```bash
# Get all metrics in Prometheus format
curl -s http://127.0.0.1:8181/metrics

# Pretty print with basic formatting
curl -s http://127.0.0.1:8181/metrics | grep -E '^(whisper_|# HELP|# TYPE)'
```

**Using PowerShell (Windows):**
```powershell
# Get all metrics
$response = Invoke-RestMethod -Uri "http://127.0.0.1:8181/metrics" -Method GET
Write-Output $response

# Filter for whisper metrics only
$response = Invoke-WebRequest -Uri "http://127.0.0.1:8181/metrics" -Method GET
$response.Content -split "`n" | Where-Object { $_ -match "^(whisper_|# HELP|# TYPE)" }
```

### 2. Expected Metrics Output

You should see metrics like:
```
# HELP whisper_jobs_submitted_total Total number of transcription jobs submitted
# TYPE whisper_jobs_submitted_total counter
whisper_jobs_submitted_total{model="base",language="auto"} 0

# HELP whisper_jobs_completed_total Total number of transcription jobs completed
# TYPE whisper_jobs_completed_total counter
whisper_jobs_completed_total{model="base",language="auto",status="success"} 0

# HELP whisper_queue_size Current number of jobs in the queue
# TYPE whisper_queue_size gauge
whisper_queue_size 0

# HELP whisper_jobs_processing Current number of jobs being processed
# TYPE whisper_jobs_processing gauge
whisper_jobs_processing 0

# HELP whisper_job_duration_seconds Time taken to process transcription jobs
# TYPE whisper_job_duration_seconds histogram
whisper_job_duration_seconds_bucket{model="base",language="auto",le="1"} 0
whisper_job_duration_seconds_bucket{model="base",language="auto",le="5"} 0
# ... more buckets
```

## Testing with Actual Transcription Jobs

### 1. Submit a Test Job

**Create a test audio file** (or use an existing one):
```bash
# Download a test audio file (optional)
curl -o test.wav "https://www2.cs.uic.edu/~i101/SoundFiles/taunt.wav"
```

**Submit transcription job:**
```bash
# Using curl with form data
curl -X POST http://127.0.0.1:8181/transcribe \
  -F "audio=@test.wav" \
  -F "model=base" \
  -F "language=auto" \
  -F "output_format=txt"
```

**Using PowerShell:**
```powershell
# Create multipart form data
$uri = "http://127.0.0.1:8181/transcribe"
$filePath = "C:\path\to\your\test.wav"

$form = @{
    audio = Get-Item -Path $filePath
    model = "base"
    language = "auto"
    output_format = "txt"
}

$response = Invoke-RestMethod -Uri $uri -Method POST -Form $form
Write-Output $response
```

### 2. Monitor Metrics Changes

**Watch metrics in real-time:**
```bash
# Linux/macOS - watch metrics every 2 seconds
watch -n 2 'curl -s http://127.0.0.1:8181/metrics | grep whisper_'

# Windows PowerShell - monitor metrics
while ($true) {
    Clear-Host
    $response = Invoke-WebRequest -Uri "http://127.0.0.1:8181/metrics"
    $response.Content -split "`n" | Where-Object { $_ -match "whisper_" }
    Start-Sleep -Seconds 2
}
```

### 3. Check Specific Metrics

**Queue size:**
```bash
curl -s http://127.0.0.1:8181/metrics | grep "whisper_queue_size"
```

**Jobs submitted counter:**
```bash
curl -s http://127.0.0.1:8181/metrics | grep "whisper_jobs_submitted_total"
```

**Processing jobs gauge:**
```bash
curl -s http://127.0.0.1:8181/metrics | grep "whisper_jobs_processing"
```

## Testing StatsD Metrics

### 1. Configure StatsD Backend

Set environment variables:
```bash
export METRICS_BACKEND=statsd
export STATSD_HOST=127.0.0.1
export STATSD_PORT=8125
export METRICS_PREFIX=whisper_api
```

### 2. Setup StatsD Server for Testing

**Using netcat to capture StatsD packets:**
```bash
# Listen on UDP port 8125 to see StatsD messages
nc -u -l 8125
```

**Using Docker with Graphite/StatsD:**
```bash
docker run -d \
  --name graphite \
  --restart=always \
  -p 80:80 \
  -p 2003-2004:2003-2004 \
  -p 2023-2024:2023-2024 \
  -p 8125:8125/udp \
  -p 8126:8126 \
  graphiteapp/graphite-statsd
```

### 3. Test StatsD Metrics

When you submit jobs, you should see UDP packets like:
```
whisper_api.jobs_submitted:1|c
whisper_api.queue_size:1|g
whisper_api.jobs_processing:1|g
whisper_api.job_duration:123.45|h
```

## Advanced Testing Scenarios

### 1. Load Testing with Multiple Jobs

**Bash script for multiple submissions:**
```bash
#!/bin/bash
for i in {1..10}; do
    curl -X POST http://127.0.0.1:8181/transcribe \
      -F "audio=@test.wav" \
      -F "model=base" \
      -F "language=auto" \
      -F "output_format=txt" &
done
wait
```

**PowerShell script:**
```powershell
1..10 | ForEach-Object -Parallel {
    $form = @{
        audio = Get-Item -Path "C:\path\to\test.wav"
        model = "base"
        language = "auto"
        output_format = "txt"
    }
    Invoke-RestMethod -Uri "http://127.0.0.1:8181/transcribe" -Method POST -Form $form
} -ThrottleLimit 5
```

### 2. Testing Error Scenarios

**Submit invalid requests to test error metrics:**
```bash
# Missing audio file
curl -X POST http://127.0.0.1:8181/transcribe -F "model=base"

# Invalid model
curl -X POST http://127.0.0.1:8181/transcribe \
  -F "audio=@test.wav" \
  -F "model=invalid_model"
```

### 3. Metrics Validation Script

**Create a validation script:**
```bash
#!/bin/bash
# test-metrics-validation.sh

echo "Testing Metrics Endpoint..."
METRICS_URL="http://127.0.0.1:8181/metrics"

# Test 1: Endpoint accessibility
if curl -s -f "$METRICS_URL" > /dev/null; then
    echo "✓ Metrics endpoint is accessible"
else
    echo "✗ Metrics endpoint is not accessible"
    exit 1
fi

# Test 2: Check for required metrics
REQUIRED_METRICS=("whisper_jobs_submitted_total" "whisper_queue_size" "whisper_jobs_processing")

for metric in "${REQUIRED_METRICS[@]}"; do
    if curl -s "$METRICS_URL" | grep -q "$metric"; then
        echo "✓ Found metric: $metric"
    else
        echo "✗ Missing metric: $metric"
    fi
done

# Test 3: Prometheus format validation
if curl -s "$METRICS_URL" | grep -q "# HELP"; then
    echo "✓ Metrics are in Prometheus format"
else
    echo "✗ Metrics are not in proper Prometheus format"
fi

echo "Metrics testing complete!"
```

## Troubleshooting

### Common Issues:

1. **Metrics endpoint returns 404:**
   - Verify the server is running with metrics enabled
   - Check `WHISPER_API_METRICS_ENABLED=true`

2. **Empty metrics response:**
   - Check metrics backend configuration
   - Verify `METRICS_BACKEND` environment variable

3. **StatsD metrics not received:**
   - Check UDP port 8125 is open
   - Verify StatsD server is running
   - Check firewall settings

4. **Missing Whisper-specific metrics after job submission:**
   - Check server logs for metric recording errors: `cargo run --release 2>&1 | grep -i "Failed to.*metric"`
   - Verify job was actually processed (check job status endpoints)
   - Look for validation errors in metrics system

### Debug Commands

```bash
# Check server logs for metric errors
cargo run --release 2>&1 | grep -E "(Failed to.*metric|warn.*metric)"

# Verify environment variables
env | grep -E "(METRICS|WHISPER_API)"

# Test server connectivity
telnet 127.0.0.1 8181

# Check if jobs are being processed
curl -s http://127.0.0.1:8181/transcription/status/{JOB_ID}
```

### Debugging Missing Whisper Metrics

If you see system metrics but no whisper metrics after submitting jobs:

```bash
# 1. Submit a job and check the response
curl -X POST http://127.0.0.1:8181/transcribe \
  -F "audio=@test.wav" \
  -F "model=base" \
  -F "language=auto" \
  -F "output_format=txt"

# 2. Check server logs immediately for errors
# Look for lines containing "Failed to increment metric" or similar

# 3. Check job status
curl http://127.0.0.1:8181/transcription/status/{JOB_ID_FROM_STEP_1}

# 4. Look for whisper metrics specifically
curl -s http://127.0.0.1:8181/metrics | grep -i whisper

# 5. If still no whisper metrics, check if the job actually processed
curl http://127.0.0.1:8181/transcription/result/{JOB_ID_FROM_STEP_1}
```

## Integration with Monitoring Tools

### 1. Prometheus Server Configuration

Add to `prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'whisper-api'
    static_configs:
      - targets: ['127.0.0.1:8181']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### 2. Grafana Dashboard Queries

Example PromQL queries:
```promql
# Job submission rate
rate(whisper_jobs_submitted_total[5m])

# Queue size over time
whisper_queue_size

# Processing time percentiles
histogram_quantile(0.95, rate(whisper_job_duration_seconds_bucket[5m]))

# Success rate
rate(whisper_jobs_completed_total{status="success"}[5m]) / rate(whisper_jobs_completed_total[5m])
```

This comprehensive testing guide should help you validate that your metrics implementation is working correctly!
