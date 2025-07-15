@echo off
setlocal enabledelayedexpansion

REM Whisper API Metrics Test Script for Windows
REM This batch script provides basic metrics testing functionality

set API_HOST=127.0.0.1
set API_PORT=8181
set API_BASE_URL=http://!API_HOST!:!API_PORT!
set METRICS_ENDPOINT=!API_BASE_URL!/metrics

echo ==========================================
echo    Whisper API Metrics Test Suite
echo ==========================================
echo API URL: !API_BASE_URL!
echo Test Started: %date% %time%
echo.

REM Check if curl is available
curl --version >nul 2>&1
if !errorlevel! neq 0 (
    echo [ERROR] curl is not available. Please install curl or use PowerShell script instead.
    echo You can download curl from: https://curl.se/download.html
    pause
    exit /b 1
)

echo [INFO] Checking API availability...
curl -s --connect-timeout 5 "!METRICS_ENDPOINT!" >nul 2>&1
if !errorlevel! neq 0 (
    echo [ERROR] API is not accessible at !API_BASE_URL!
    echo [INFO] Please ensure the Whisper API is running.
    echo [INFO] Start with: METRICS_BACKEND=prometheus whisper_api.exe
    pause
    exit /b 1
)

echo [SUCCESS] API is accessible

REM Detect metrics backend
echo [INFO] Detecting metrics backend...
curl -s "!METRICS_ENDPOINT!" > temp_metrics.txt 2>nul
if !errorlevel! neq 0 (
    echo [WARNING] Could not fetch metrics
    set BACKEND=unknown
) else (
    findstr /c:"# HELP" temp_metrics.txt >nul 2>&1
    if !errorlevel! equ 0 (
        set BACKEND=prometheus
    ) else (
        set BACKEND=statsd
    )
)

echo [INFO] Detected backend: !BACKEND!

REM Test different endpoints to generate metrics
echo [INFO] Testing API endpoints to generate metrics...
curl -s "!API_BASE_URL!/transcription/test-job-123" >nul 2>&1
curl -s "!API_BASE_URL!/transcription/test-job-456/result" >nul 2>&1
curl -s "!METRICS_ENDPOINT!" >nul 2>&1

REM Wait for metrics to update
timeout /t 3 /nobreak >nul

if "!BACKEND!"=="prometheus" (
    echo.
    echo ==========================================
    echo      Testing Prometheus Metrics
    echo ==========================================
    
    curl -s "!METRICS_ENDPOINT!" > prometheus_metrics.txt
    if !errorlevel! equ 0 (
        echo [SUCCESS] Prometheus metrics retrieved
        
        REM Count metrics
        for /f %%i in ('findstr /c:"# HELP" prometheus_metrics.txt') do set HELP_COUNT=%%i
        for /f %%i in ('findstr /c:"whisper_" prometheus_metrics.txt ^| findstr /v "#"') do set METRIC_COUNT=%%i
        
        echo [INFO] HELP lines: !HELP_COUNT!
        echo [INFO] Whisper metrics: !METRIC_COUNT!
        
        echo [INFO] Checking for expected metrics:
        findstr /c:"whisper_jobs_submitted_total" prometheus_metrics.txt >nul && echo [SUCCESS] ✓ whisper_jobs_submitted_total || echo [WARNING] ✗ whisper_jobs_submitted_total
        findstr /c:"whisper_jobs_completed_total" prometheus_metrics.txt >nul && echo [SUCCESS] ✓ whisper_jobs_completed_total || echo [WARNING] ✗ whisper_jobs_completed_total
        findstr /c:"whisper_queue_size" prometheus_metrics.txt >nul && echo [SUCCESS] ✓ whisper_queue_size || echo [WARNING] ✗ whisper_queue_size
        findstr /c:"whisper_jobs_processing" prometheus_metrics.txt >nul && echo [SUCCESS] ✓ whisper_jobs_processing || echo [WARNING] ✗ whisper_jobs_processing
        findstr /c:"whisper_http_requests_total" prometheus_metrics.txt >nul && echo [SUCCESS] ✓ whisper_http_requests_total || echo [WARNING] ✗ whisper_http_requests_total
        
        echo [SUCCESS] Prometheus metrics test completed
        echo [INFO] Full metrics saved to: prometheus_metrics.txt
    ) else (
        echo [ERROR] Failed to retrieve Prometheus metrics
    )
) else if "!BACKEND!"=="statsd" (
    echo.
    echo ==========================================
    echo        Testing StatsD Metrics
    echo ==========================================
    
    if not defined STATSD_HOST set STATSD_HOST=127.0.0.1
    if not defined STATSD_PORT set STATSD_PORT=8125
    if not defined STATSD_PREFIX set STATSD_PREFIX=whisper_api
    
    echo [INFO] StatsD Configuration:
    echo [INFO]   Host: !STATSD_HOST!
    echo [INFO]   Port: !STATSD_PORT!
    echo [INFO]   Prefix: !STATSD_PREFIX!
    echo [SUCCESS] StatsD metrics are sent via UDP and not directly visible
    echo [INFO] Check your StatsD server logs or dashboard for metrics
    
    REM Try to test connectivity with a simple echo
    echo test.metric:1^|c | nslookup !STATSD_HOST! >nul 2>&1
    if !errorlevel! equ 0 (
        echo [SUCCESS] StatsD host is reachable
    ) else (
        echo [WARNING] StatsD host may not be reachable
    )
) else (
    echo [WARNING] Unknown metrics backend
    echo [INFO] The API might be using 'null' backend or configuration issue
)

REM Save test results
set TIMESTAMP=%date:~-4%%date:~3,2%%date:~0,2%_%time:~0,2%%time:~3,2%%time:~6,2%
set TIMESTAMP=!TIMESTAMP: =0!
set RESULTS_FILE=metrics_test_results_!TIMESTAMP!.txt

echo Test Results - !date! !time! > !RESULTS_FILE!
echo API URL: !API_BASE_URL! >> !RESULTS_FILE!
echo Detected Backend: !BACKEND! >> !RESULTS_FILE!
echo. >> !RESULTS_FILE!

if exist prometheus_metrics.txt (
    echo Prometheus Metrics: >> !RESULTS_FILE!
    type prometheus_metrics.txt >> !RESULTS_FILE!
)

echo Environment Variables: >> !RESULTS_FILE!
set | findstr /i "WHISPER METRICS STATSD" >> !RESULTS_FILE! 2>nul

echo.
echo ==========================================
echo        Configuration Guide
echo ==========================================
echo.
echo To test different metrics backends:
echo.
echo For Prometheus:
echo   set METRICS_BACKEND=prometheus
echo   whisper_api.exe
echo   Access metrics at: !API_BASE_URL!/metrics
echo.
echo For StatsD:
echo   set METRICS_BACKEND=statsd
echo   set STATSD_HOST=127.0.0.1
echo   set STATSD_PORT=8125
echo   whisper_api.exe
echo   Requires StatsD server running
echo.
echo For development (no metrics):
echo   set METRICS_BACKEND=null
echo   whisper_api.exe
echo.

echo [SUCCESS] Test completed! Results saved to: !RESULTS_FILE!

REM Cleanup temp files
if exist temp_metrics.txt del temp_metrics.txt

echo.
echo Press any key to exit...
pause >nul
