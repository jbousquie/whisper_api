# Whisper API Metrics Test Script for Windows PowerShell
# This script provides basic metrics testing functionality for Windows users

param(
    [string]$Backend = "auto",
    [string]$ApiHost = "127.0.0.1",
    [string]$ApiPort = "8181"
)

$ApiBaseUrl = "http://${ApiHost}:${ApiPort}"
$MetricsEndpoint = "${ApiBaseUrl}/metrics"

# Colors for output
$Colors = @{
    Red = "Red"
    Green = "Green"
    Yellow = "Yellow"
    Blue = "Blue"
    Cyan = "Cyan"
}

function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    Write-Host $Message -ForegroundColor $Color
}

function Write-Info {
    param([string]$Message)
    Write-ColorOutput "[INFO] $Message" $Colors.Blue
}

function Write-Success {
    param([string]$Message)
    Write-ColorOutput "[SUCCESS] $Message" $Colors.Green
}

function Write-Warning {
    param([string]$Message)
    Write-ColorOutput "[WARNING] $Message" $Colors.Yellow
}

function Write-Error {
    param([string]$Message)
    Write-ColorOutput "[ERROR] $Message" $Colors.Red
}

function Write-Header {
    param([string]$Message)
    Write-ColorOutput "`n==========================================" $Colors.Cyan
    Write-ColorOutput $Message $Colors.Cyan
    Write-ColorOutput "==========================================" $Colors.Cyan
}

function Test-ApiAvailability {
    Write-Info "Checking if Whisper API is running on $ApiBaseUrl"
    
    try {
        $response = Invoke-WebRequest -Uri $MetricsEndpoint -Method Get -TimeoutSec 5 -ErrorAction Stop
        Write-Success "API is accessible"
        return $true
    }
    catch {
        Write-Error "API is not accessible. Please ensure the Whisper API is running."
        Write-Info "Start the API with appropriate metrics backend configuration"
        return $false
    }
}

function Detect-MetricsBackend {
    Write-Info "Detecting current metrics backend configuration..."
    
    try {
        $response = Invoke-WebRequest -Uri $MetricsEndpoint -Method Get -ErrorAction Stop
        $content = $response.Content
        
        if ($content -match "# HELP") {
            return "prometheus"
        }
        elseif ([string]::IsNullOrWhiteSpace($content) -or $content -match "StatsD") {
            return "statsd"
        }
        else {
            return "unknown"
        }
    }
    catch {
        return "unknown"
    }
}

function Test-PrometheusMetrics {
    Write-Header "Testing Prometheus Metrics Backend"
    
    try {
        $response = Invoke-WebRequest -Uri $MetricsEndpoint -Method Get -ErrorAction Stop
        $content = $response.Content
        
        if ($content -match "# HELP") {
            Write-Success "Prometheus metrics format detected"
            
            # Count metrics
            $helpLines = ($content -split "`n" | Where-Object { $_ -match "# HELP" }).Count
            $typeLines = ($content -split "`n" | Where-Object { $_ -match "# TYPE" }).Count
            $whisperMetrics = ($content -split "`n" | Where-Object { $_ -match "whisper_" -and $_ -notmatch "#" }).Count
            
            Write-Info "Metrics summary:"
            Write-Info "  HELP lines: $helpLines"
            Write-Info "  TYPE lines: $typeLines"
            Write-Info "  Whisper metrics: $whisperMetrics"
            
            # Expected metrics
            $expectedMetrics = @(
                "whisper_jobs_submitted_total",
                "whisper_jobs_completed_total", 
                "whisper_jobs_cancelled_total",
                "whisper_jobs_failed_total",
                "whisper_queue_size",
                "whisper_jobs_processing",
                "whisper_job_duration_seconds",
                "whisper_http_requests_total",
                "whisper_http_request_duration_seconds"
            )
            
            Write-Info "Checking for expected metrics:"
            foreach ($metric in $expectedMetrics) {
                if ($content -match $metric) {
                    Write-Success "✓ $metric"
                }
                else {
                    Write-Warning "✗ $metric (not found)"
                }
            }
        }
        else {
            Write-Error "No Prometheus metrics found"
        }
    }
    catch {
        Write-Error "Failed to fetch Prometheus metrics: $($_.Exception.Message)"
    }
}

function Test-StatsD {
    Write-Header "Testing StatsD Metrics Backend"
    
    $statsdHost = $env:STATSD_HOST
    if (-not $statsdHost) { $statsdHost = "127.0.0.1" }
    
    $statsdPort = $env:STATSD_PORT
    if (-not $statsdPort) { $statsdPort = "8125" }
    
    $statsdPrefix = $env:STATSD_PREFIX
    if (-not $statsdPrefix) { $statsdPrefix = "whisper_api" }
    
    Write-Info "StatsD Configuration:"
    Write-Info "  Host: $statsdHost"
    Write-Info "  Port: $statsdPort"
    Write-Info "  Prefix: $statsdPrefix"
    
    Write-Success "StatsD metrics are sent via UDP and not directly visible via HTTP"
    Write-Info "Check your StatsD server logs or dashboard for metrics"
    
    # Test UDP connectivity if possible
    try {
        $udpClient = New-Object System.Net.Sockets.UdpClient
        $udpClient.Connect($statsdHost, $statsdPort)
        $testMetric = [System.Text.Encoding]::ASCII.GetBytes("test.metric:1|c")
        $udpClient.Send($testMetric, $testMetric.Length)
        $udpClient.Close()
        Write-Success "Successfully sent test metric to StatsD server"
    }
    catch {
        Write-Warning "Could not send test metric to StatsD server: $($_.Exception.Message)"
        Write-Info "This may be normal if no StatsD server is running"
    }
}

function Test-ApiEndpoints {
    Write-Header "Testing API Endpoints to Generate Metrics"
    
    $endpoints = @(
        @{ Method = "GET"; Path = "/metrics"; Description = "Metrics endpoint" },
        @{ Method = "GET"; Path = "/transcription/test-job-123"; Description = "Job status (non-existent)" },
        @{ Method = "GET"; Path = "/transcription/test-job-456/result"; Description = "Job result (non-existent)" }
    )
    
    foreach ($endpoint in $endpoints) {
        Write-Info "Testing $($endpoint.Description)..."
        try {
            $uri = "$ApiBaseUrl$($endpoint.Path)"
            if ($endpoint.Method -eq "GET") {
                $response = Invoke-WebRequest -Uri $uri -Method Get -ErrorAction SilentlyContinue
            }
            Write-Info "  Status: $($response.StatusCode) (Expected for test endpoints)"
        }
        catch {
            Write-Info "  Request completed (may have returned error status as expected)"
        }
        Start-Sleep -Milliseconds 500
    }
    
    Write-Success "API endpoint testing completed"
}

function Save-MetricsSnapshot {
    Write-Header "Saving Metrics Snapshot"
    
    $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
    $outputDir = "$env:TEMP\whisper_metrics_test_$timestamp"
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    
    Write-Info "Saving test results to: $outputDir"
    
    # Save test info
    $testInfo = @"
API Base URL: $ApiBaseUrl
Test Timestamp: $timestamp
Detected Backend: $(Detect-MetricsBackend)
PowerShell Version: $($PSVersionTable.PSVersion)
"@
    $testInfo | Out-File -FilePath "$outputDir\test_info.txt"
    
    # Save current metrics if Prometheus
    $backend = Detect-MetricsBackend
    if ($backend -eq "prometheus") {
        try {
            $response = Invoke-WebRequest -Uri $MetricsEndpoint -Method Get
            $response.Content | Out-File -FilePath "$outputDir\prometheus_metrics.txt"
            Write-Success "Prometheus metrics saved"
        }
        catch {
            Write-Warning "Could not save Prometheus metrics"
        }
    }
    
    # Save environment variables
    Get-ChildItem Env: | Where-Object { $_.Name -match "(WHISPER|METRICS|STATSD)" } | 
        ForEach-Object { "$($_.Name)=$($_.Value)" } | 
        Out-File -FilePath "$outputDir\environment.txt"
    
    Write-Success "Test results saved to: $outputDir"
}

function Show-ConfigurationGuide {
    Write-Header "Metrics Configuration Guide"
    
    Write-Host ""
    Write-ColorOutput "To test different metrics backends, start the Whisper API with:" $Colors.Green
    Write-Host ""
    Write-ColorOutput "For Prometheus:" $Colors.Green
    Write-Host "  set METRICS_BACKEND=prometheus"
    Write-Host "  whisper_api.exe"
    Write-Host "  Then access metrics at: $ApiBaseUrl/metrics"
    Write-Host ""
    Write-ColorOutput "For StatsD:" $Colors.Green
    Write-Host "  set METRICS_BACKEND=statsd"
    Write-Host "  set STATSD_HOST=127.0.0.1"
    Write-Host "  set STATSD_PORT=8125"
    Write-Host "  whisper_api.exe"
    Write-Host "  Requires a StatsD server running on the specified host/port"
    Write-Host ""
    Write-ColorOutput "For development (no metrics):" $Colors.Green
    Write-Host "  set METRICS_BACKEND=null"
    Write-Host "  whisper_api.exe"
    Write-Host "  or simply: whisper_api.exe (null is the default)"
    Write-Host ""
    Write-ColorOutput "StatsD Integration Examples:" $Colors.Yellow
    Write-Host "  # Start Graphite with StatsD (Docker)"
    Write-Host "  docker run -d --name graphite -p 8080:80 -p 8125:8125/udp graphiteapp/graphite-statsd"
    Write-Host ""
    Write-Host "  # Simple StatsD listener for testing (requires WSL or Linux tools)"
    Write-Host "  nc -u -l -p 8125"
}

function Main {
    Write-Header "Whisper API Metrics Test Suite (PowerShell)"
    
    Write-Host "API URL: $ApiBaseUrl"
    Write-Host "Test Started: $(Get-Date)"
    Write-Host ""
    
    # Check API availability
    if (-not (Test-ApiAvailability)) {
        Write-Error "Cannot proceed without API access"
        exit 1
    }
    
    # Detect backend
    $detectedBackend = Detect-MetricsBackend
    Write-Info "Detected metrics backend: $detectedBackend"
    
    # Test API endpoints to generate metrics
    Test-ApiEndpoints
    
    # Wait for metrics to be processed
    Start-Sleep -Seconds 3
    
    # Run backend-specific tests
    switch ($Backend) {
        "prometheus" { Test-PrometheusMetrics }
        "statsd" { Test-StatsD }
        "auto" {
            switch ($detectedBackend) {
                "prometheus" { Test-PrometheusMetrics }
                "statsd" { Test-StatsD }
                "unknown" { 
                    Write-Warning "Running generic tests for unknown backend"
                    Test-PrometheusMetrics
                }
            }
        }
        default { 
            Write-Warning "Unknown backend specified. Running auto-detection."
            Test-PrometheusMetrics
        }
    }
    
    # Save results
    Save-MetricsSnapshot
    
    # Show configuration guide
    Show-ConfigurationGuide
    
    Write-Header "Test Suite Complete"
    Write-Success "All tests have been executed. Check the output above for detailed results."
}

# Handle script parameters
switch ($Backend.ToLower()) {
    "prometheus" { Test-PrometheusMetrics; return }
    "statsd" { Test-StatsD; return }
    "guide" { Show-ConfigurationGuide; return }
    "analyze" { 
        $backend = Detect-MetricsBackend
        Write-Info "Current backend: $backend"
        switch ($backend) {
            "prometheus" { Test-PrometheusMetrics }
            "statsd" { Test-StatsD }
            default { Write-Warning "Unknown backend detected" }
        }
        return 
    }
    default { Main }
}
