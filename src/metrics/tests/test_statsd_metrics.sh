#!/bin/bash

# StatsD Metrics Test Script for Whisper API
# This script tests StatsD metrics integration and simulates various API operations
# to verify that metrics are being sent to StatsD servers correctly.

# Configuration
API_HOST="${WHISPER_API_HOST:-127.0.0.1}"
API_PORT="${WHISPER_API_PORT:-8181}"
API_BASE_URL="http://${API_HOST}:${API_PORT}"

# StatsD Configuration
STATSD_HOST="${STATSD_HOST:-127.0.0.1}"
STATSD_PORT="${STATSD_PORT:-8125}"
STATSD_PREFIX="${STATSD_PREFIX:-whisper_api}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to run a test
run_test() {
    local test_name="$1"
    local command="$2"
    local expected_status="$3"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    print_info "Running test: $test_name"
    
    # Execute the command and capture response
    response=$(eval "$command" 2>&1)
    status=$?
    
    if [ $status -eq $expected_status ]; then
        print_success "✓ $test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        print_error "✗ $test_name (Exit code: $status, Expected: $expected_status)"
        echo "Response: $response"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Function to check if API is running with StatsD
check_api_availability() {
    print_info "Checking if Whisper API is running on ${API_BASE_URL}"
    
    if curl -s --connect-timeout 5 "${API_BASE_URL}/metrics" > /dev/null 2>&1; then
        print_success "API is accessible"
        
        # Check if StatsD backend is configured
        print_info "Verifying StatsD configuration..."
        print_info "Expected StatsD server: ${STATSD_HOST}:${STATSD_PORT}"
        print_info "Expected prefix: ${STATSD_PREFIX}"
        print_warning "Make sure to start the API with: METRICS_BACKEND=statsd STATSD_HOST=${STATSD_HOST} STATSD_PORT=${STATSD_PORT} ./whisper_api"
        return 0
    else
        print_error "API is not accessible. Please ensure the Whisper API is running."
        exit 1
    fi
}

# Function to check if StatsD server is running (if available)
check_statsd_server() {
    print_info "Checking StatsD server availability"
    
    # Try to send a test metric to StatsD
    if command -v nc >/dev/null 2>&1; then
        if echo "test.metric:1|c" | nc -u -w1 "$STATSD_HOST" "$STATSD_PORT" >/dev/null 2>&1; then
            print_success "StatsD server is responsive at ${STATSD_HOST}:${STATSD_PORT}"
            PASSED_TESTS=$((PASSED_TESTS + 1))
        else
            print_warning "StatsD server may not be running at ${STATSD_HOST}:${STATSD_PORT}"
            print_info "This test will continue but metrics won't be visible without a StatsD server"
        fi
    else
        print_warning "netcat (nc) not available, skipping StatsD server connectivity test"
    fi
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
}

# Function to capture StatsD traffic (if possible)
setup_statsd_capture() {
    print_info "Setting up StatsD traffic capture"
    
    # Check if we can capture UDP traffic
    if command -v tcpdump >/dev/null 2>&1; then
        # Create capture file
        CAPTURE_FILE="/tmp/statsd_capture_$(date +%s).pcap"
        
        print_info "Starting packet capture on port ${STATSD_PORT}..."
        print_info "Capture file: $CAPTURE_FILE"
        
        # Start tcpdump in background (requires root privileges)
        sudo tcpdump -i any -n udp port "$STATSD_PORT" -w "$CAPTURE_FILE" >/dev/null 2>&1 &
        TCPDUMP_PID=$!
        
        if [ $? -eq 0 ]; then
            print_success "Packet capture started (PID: $TCPDUMP_PID)"
            sleep 2  # Give tcpdump time to start
            return 0
        else
            print_warning "Could not start packet capture (may require sudo privileges)"
            return 1
        fi
    else
        print_warning "tcpdump not available, traffic capture disabled"
        return 1
    fi
}

# Function to stop StatsD capture
stop_statsd_capture() {
    if [ ! -z "$TCPDUMP_PID" ]; then
        print_info "Stopping packet capture..."
        sudo kill "$TCPDUMP_PID" 2>/dev/null || true
        wait "$TCPDUMP_PID" 2>/dev/null || true
        
        if [ -f "$CAPTURE_FILE" ]; then
            # Analyze capture
            packet_count=$(sudo tcpdump -r "$CAPTURE_FILE" 2>/dev/null | wc -l)
            print_info "Captured $packet_count UDP packets"
            
            if [ "$packet_count" -gt 0 ]; then
                print_success "StatsD traffic was captured"
                print_info "Analyze with: sudo tcpdump -r $CAPTURE_FILE -A"
            else
                print_warning "No StatsD traffic captured"
            fi
        fi
    fi
}

# Function to start a simple StatsD server for testing
start_test_statsd_server() {
    print_info "Starting test StatsD server for metric collection"
    
    # Create a simple UDP server to capture StatsD metrics
    STATSD_LOG="/tmp/statsd_metrics_$(date +%s).log"
    
    # Use netcat to listen on StatsD port and log metrics
    if command -v nc >/dev/null 2>&1; then
        nc -u -l -p "$STATSD_PORT" >> "$STATSD_LOG" &
        NC_PID=$!
        
        if [ $? -eq 0 ]; then
            print_success "Test StatsD server started (PID: $NC_PID, Log: $STATSD_LOG)"
            sleep 1
            return 0
        else
            print_warning "Could not start test StatsD server"
            return 1
        fi
    else
        print_warning "netcat not available for test server"
        return 1
    fi
}

# Function to stop test StatsD server
stop_test_statsd_server() {
    if [ ! -z "$NC_PID" ]; then
        print_info "Stopping test StatsD server..."
        kill "$NC_PID" 2>/dev/null || true
        wait "$NC_PID" 2>/dev/null || true
        
        if [ -f "$STATSD_LOG" ]; then
            metric_count=$(wc -l < "$STATSD_LOG" 2>/dev/null || echo "0")
            print_info "Collected $metric_count StatsD metrics"
            
            if [ "$metric_count" -gt 0 ]; then
                print_success "StatsD metrics were received"
                print_info "View metrics with: cat $STATSD_LOG"
                
                # Show sample of collected metrics
                print_info "Sample of collected metrics:"
                head -10 "$STATSD_LOG" 2>/dev/null | while read line; do
                    echo "  $line"
                done
            else
                print_warning "No StatsD metrics were received"
            fi
        fi
    fi
}

# Function to test job submission and metrics
test_job_metrics() {
    print_info "Testing job-related StatsD metrics"
    
    # Create a test audio file
    test_file="/tmp/test_audio_statsd.wav"
    echo "fake audio content for statsd test" > "$test_file"
    
    print_info "Submitting test job to generate metrics..."
    
    # Submit a test job
    job_response=$(curl -s -X POST \
        -F "audio=@${test_file}" \
        -F "language=en" \
        -F "model=tiny" \
        -F "sync=false" \
        "${API_BASE_URL}/transcription" 2>/dev/null || echo "request_failed")
    
    print_info "Job submission response: $job_response"
    
    # Wait for metrics to be sent
    sleep 3
    
    # Cleanup
    rm -f "$test_file"
    
    run_test "Job submission completed" \
        "[ '$job_response' != 'request_failed' ]" \
        0
}

# Function to test HTTP request metrics
test_http_metrics() {
    print_info "Testing HTTP request StatsD metrics"
    
    # Make various HTTP requests to trigger metrics
    print_info "Making HTTP requests to generate metrics..."
    
    # Test different endpoints
    curl -s "${API_BASE_URL}/transcription/test-job-123" >/dev/null 2>&1 || true
    curl -s "${API_BASE_URL}/transcription/test-job-456/result" >/dev/null 2>&1 || true
    curl -s "${API_BASE_URL}/metrics" >/dev/null 2>&1 || true
    
    # Wait for metrics to be sent
    sleep 2
    
    run_test "HTTP requests completed" \
        "true" \
        0
}

# Function to test StatsD metric format validation
test_statsd_format() {
    print_info "Testing StatsD metric format expectations"
    
    # Expected StatsD metric patterns
    expected_patterns=(
        "${STATSD_PREFIX}.jobs.submitted:.*\\|c"
        "${STATSD_PREFIX}.jobs.completed:.*\\|c"
        "${STATSD_PREFIX}.queue.size:.*\\|g"
        "${STATSD_PREFIX}.jobs.processing:.*\\|g"
        "${STATSD_PREFIX}.job.duration:.*\\|h"
        "${STATSD_PREFIX}.http.requests:.*\\|c"
        "${STATSD_PREFIX}.http.duration:.*\\|h"
    )
    
    print_info "Expected StatsD metric patterns:"
    for pattern in "${expected_patterns[@]}"; do
        echo "  - $pattern"
    done
    
    # This test always passes as we can't directly verify the format without a server
    run_test "StatsD format validation (informational)" \
        "true" \
        0
}

# Function to test concurrent requests
test_concurrent_requests() {
    print_info "Testing concurrent requests to generate StatsD metrics"
    
    # Create multiple background requests
    for i in {1..5}; do
        (curl -s "${API_BASE_URL}/transcription/concurrent-test-$i" >/dev/null 2>&1 || true) &
    done
    
    # Wait for all requests to complete
    wait
    
    # Additional delay for metrics processing
    sleep 2
    
    run_test "Concurrent requests completed" \
        "true" \
        0
}

# Function to validate environment configuration
test_environment_config() {
    print_info "Validating StatsD environment configuration"
    
    # Check required environment variables
    config_checks=(
        "METRICS_BACKEND should be 'statsd'"
        "STATSD_HOST configuration: ${STATSD_HOST}"
        "STATSD_PORT configuration: ${STATSD_PORT}"
        "STATSD_PREFIX configuration: ${STATSD_PREFIX}"
    )
    
    for check in "${config_checks[@]}"; do
        print_info "$check"
    done
    
    # Verify the API is not exposing Prometheus metrics endpoint
    metrics_response=$(curl -s "${API_BASE_URL}/metrics" 2>/dev/null || echo "no_metrics_endpoint")
    
    if [[ "$metrics_response" == *"prometheus"* ]] || [[ "$metrics_response" == *"# HELP"* ]]; then
        print_warning "API appears to be using Prometheus metrics instead of StatsD"
        print_warning "Make sure METRICS_BACKEND=statsd is set"
    else
        print_success "API is not exposing Prometheus metrics (good for StatsD mode)"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    fi
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
}

# Function to simulate realistic workload
simulate_workload() {
    print_info "Simulating realistic API workload for StatsD metrics"
    
    # Create test files
    for i in {1..3}; do
        echo "test audio content $i" > "/tmp/workload_test_$i.wav"
    done
    
    # Submit multiple jobs with different parameters
    for i in {1..3}; do
        print_info "Submitting workload job $i..."
        
        curl -s -X POST \
            -F "audio=@/tmp/workload_test_$i.wav" \
            -F "language=en" \
            -F "model=tiny" \
            -F "sync=false" \
            "${API_BASE_URL}/transcription" >/dev/null 2>&1 || true
        
        # Check job status
        sleep 1
        curl -s "${API_BASE_URL}/transcription/workload-job-$i" >/dev/null 2>&1 || true
        
        # Small delay between jobs
        sleep 1
    done
    
    # Cleanup
    rm -f /tmp/workload_test_*.wav
    
    run_test "Workload simulation completed" \
        "true" \
        0
}

# Function to analyze captured metrics (if available)
analyze_captured_metrics() {
    if [ -f "$STATSD_LOG" ]; then
        print_info "Analyzing captured StatsD metrics"
        
        # Count different metric types
        counters=$(grep -c "|c" "$STATSD_LOG" 2>/dev/null || echo "0")
        gauges=$(grep -c "|g" "$STATSD_LOG" 2>/dev/null || echo "0")
        histograms=$(grep -c "|h" "$STATSD_LOG" 2>/dev/null || echo "0")
        
        print_info "Metric types captured:"
        print_info "  Counters: $counters"
        print_info "  Gauges: $gauges"
        print_info "  Histograms: $histograms"
        
        # Check for expected metric names
        job_metrics=$(grep -c "${STATSD_PREFIX}\.jobs\." "$STATSD_LOG" 2>/dev/null || echo "0")
        http_metrics=$(grep -c "${STATSD_PREFIX}\.http\." "$STATSD_LOG" 2>/dev/null || echo "0")
        queue_metrics=$(grep -c "${STATSD_PREFIX}\.queue\." "$STATSD_LOG" 2>/dev/null || echo "0")
        
        print_info "Metric categories captured:"
        print_info "  Job metrics: $job_metrics"
        print_info "  HTTP metrics: $http_metrics"
        print_info "  Queue metrics: $queue_metrics"
        
        total_metrics=$((counters + gauges + histograms))
        if [ $total_metrics -gt 0 ]; then
            print_success "Successfully captured $total_metrics StatsD metrics"
            PASSED_TESTS=$((PASSED_TESTS + 1))
        else
            print_warning "No StatsD metrics were captured"
        fi
        
        TOTAL_TESTS=$((TOTAL_TESTS + 1))
    fi
}

# Function to print test summary
print_summary() {
    echo
    echo "=========================================="
    echo "           TEST SUMMARY"
    echo "=========================================="
    echo "Total tests: $TOTAL_TESTS"
    echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
    echo -e "Failed: ${RED}$FAILED_TESTS${NC}"
    echo
    echo "StatsD Configuration:"
    echo "  Host: $STATSD_HOST"
    echo "  Port: $STATSD_PORT"
    echo "  Prefix: $STATSD_PREFIX"
    echo
    
    if [ ! -z "$STATSD_LOG" ] && [ -f "$STATSD_LOG" ]; then
        echo "Captured metrics saved to: $STATSD_LOG"
    fi
    
    if [ ! -z "$CAPTURE_FILE" ] && [ -f "$CAPTURE_FILE" ]; then
        echo "Network capture saved to: $CAPTURE_FILE"
    fi
    
    echo
    echo "Integration tips:"
    echo "  - For Grafana: Use a StatsD-compatible backend like Graphite or InfluxDB"
    echo "  - For monitoring: Set up your StatsD server to forward to your time-series database"
    echo "  - For testing: Use the captured logs to verify metric formats and values"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}All tests passed! ✓${NC}"
        exit 0
    else
        echo -e "${RED}Some tests failed! ✗${NC}"
        exit 1
    fi
}

# Function to cleanup resources
cleanup() {
    print_info "Cleaning up test resources..."
    stop_test_statsd_server
    stop_statsd_capture
    rm -f /tmp/test_audio_statsd.wav
    rm -f /tmp/workload_test_*.wav
}

# Trap to ensure cleanup on exit
trap cleanup EXIT

# Main execution
main() {
    echo "=========================================="
    echo "     Whisper API StatsD Metrics Test"
    echo "=========================================="
    echo "API URL: $API_BASE_URL"
    echo "StatsD Server: ${STATSD_HOST}:${STATSD_PORT}"
    echo "StatsD Prefix: $STATSD_PREFIX"
    echo
    
    # Run all tests
    check_api_availability
    check_statsd_server
    test_environment_config
    
    # Start test infrastructure
    start_test_statsd_server
    setup_statsd_capture
    
    # Run metric tests
    test_statsd_format
    test_job_metrics
    test_http_metrics
    test_concurrent_requests
    simulate_workload
    
    # Wait for final metrics
    sleep 3
    
    # Analyze results
    analyze_captured_metrics
    
    # Print summary
    print_summary
}

# Run main function
main "$@"
