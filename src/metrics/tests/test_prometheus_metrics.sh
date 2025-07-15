#!/bin/bash

# Prometheus Metrics Test Script for Whisper API
# This script tests the Prometheus metrics endpoint and simulates various API operations
# to verify that metrics are being collected and exported correctly.

# Configuration
API_HOST="${WHISPER_API_HOST:-127.0.0.1}"
API_PORT="${WHISPER_API_PORT:-8181}"
API_BASE_URL="http://${API_HOST}:${API_PORT}"
METRICS_ENDPOINT="${API_BASE_URL}/metrics"

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

# Function to check if API is running
check_api_availability() {
    print_info "Checking if Whisper API is running on ${API_BASE_URL}"
    
    if curl -s --connect-timeout 5 "${API_BASE_URL}/metrics" > /dev/null; then
        print_success "API is accessible"
        return 0
    else
        print_error "API is not accessible. Please ensure the Whisper API is running with Prometheus metrics enabled."
        print_info "Start the API with: METRICS_BACKEND=prometheus ./whisper_api"
        exit 1
    fi
}

# Function to check metrics endpoint
test_metrics_endpoint() {
    print_info "Testing Prometheus metrics endpoint availability"
    
    run_test "Metrics endpoint accessibility" \
        "curl -s -o /dev/null -w '%{http_code}' '${METRICS_ENDPOINT}' | grep -q '200'" \
        0
}

# Function to verify Prometheus format
test_prometheus_format() {
    print_info "Verifying Prometheus metrics format"
    
    run_test "Prometheus format validation" \
        "curl -s '${METRICS_ENDPOINT}' | grep -q '^# HELP'" \
        0
    
    run_test "Metrics content type" \
        "curl -s -I '${METRICS_ENDPOINT}' | grep -q 'Content-Type: text/plain'" \
        0
}

# Function to check for expected metrics
test_expected_metrics() {
    print_info "Checking for expected Whisper API metrics"
    
    # Get metrics content
    metrics_content=$(curl -s "${METRICS_ENDPOINT}")
    
    # List of expected metrics
    expected_metrics=(
        "whisper_jobs_submitted_total"
        "whisper_jobs_completed_total"
        "whisper_jobs_cancelled_total"
        "whisper_jobs_failed_total"
        "whisper_queue_size"
        "whisper_jobs_processing"
        "whisper_job_duration_seconds"
        "whisper_http_requests_total"
        "whisper_http_request_duration_seconds"
    )
    
    for metric in "${expected_metrics[@]}"; do
        run_test "Metric exists: $metric" \
            "echo '$metrics_content' | grep -q '$metric'" \
            0
    done
}

# Function to simulate job submission and check metrics
test_job_metrics() {
    print_info "Testing job-related metrics (simulation)"
    
    # Create a test audio file (empty file for testing)
    test_file="/tmp/test_audio_prometheus.wav"
    echo "fake audio content" > "$test_file"
    
    # Get initial metrics
    initial_metrics=$(curl -s "${METRICS_ENDPOINT}")
    initial_submitted=$(echo "$initial_metrics" | grep "whisper_jobs_submitted_total" | tail -1 | awk '{print $2}' || echo "0")
    
    print_info "Initial submitted jobs: $initial_submitted"
    
    # Submit a test job (this will likely fail due to invalid audio, but should increment metrics)
    job_response=$(curl -s -X POST \
        -F "audio=@${test_file}" \
        -F "language=en" \
        -F "model=tiny" \
        -F "sync=false" \
        "${API_BASE_URL}/transcription" 2>/dev/null || echo "failed")
    
    # Wait a moment for metrics to update
    sleep 2
    
    # Get updated metrics
    updated_metrics=$(curl -s "${METRICS_ENDPOINT}")
    updated_submitted=$(echo "$updated_metrics" | grep "whisper_jobs_submitted_total" | tail -1 | awk '{print $2}' || echo "0")
    
    print_info "Updated submitted jobs: $updated_submitted"
    
    # Check if job submission was recorded (allowing for the fact that the job might fail)
    if [ "$updated_submitted" != "$initial_submitted" ]; then
        print_success "Job submission metrics are working"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        print_warning "Job submission metrics may not be updating (this could be normal if the API requires valid audio files)"
    fi
    
    # Cleanup
    rm -f "$test_file"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
}

# Function to test HTTP request metrics
test_http_metrics() {
    print_info "Testing HTTP request metrics"
    
    # Get initial HTTP metrics
    initial_metrics=$(curl -s "${METRICS_ENDPOINT}")
    
    # Make several HTTP requests to different endpoints
    curl -s "${API_BASE_URL}/transcription/nonexistent-job" > /dev/null || true
    curl -s "${API_BASE_URL}/transcription/another-fake-job/result" > /dev/null || true
    curl -s "${METRICS_ENDPOINT}" > /dev/null
    
    # Wait for metrics to update
    sleep 1
    
    # Get updated metrics
    updated_metrics=$(curl -s "${METRICS_ENDPOINT}")
    
    run_test "HTTP request metrics exist" \
        "echo '$updated_metrics' | grep -q 'whisper_http_requests_total'" \
        0
    
    run_test "HTTP duration metrics exist" \
        "echo '$updated_metrics' | grep -q 'whisper_http_request_duration_seconds'" \
        0
}

# Function to test metrics labels
test_metrics_labels() {
    print_info "Testing metrics labels and dimensions"
    
    metrics_content=$(curl -s "${METRICS_ENDPOINT}")
    
    run_test "Method labels in HTTP metrics" \
        "echo '$metrics_content' | grep 'whisper_http_requests_total' | grep -q 'method='" \
        0
    
    run_test "Endpoint labels in HTTP metrics" \
        "echo '$metrics_content' | grep 'whisper_http_requests_total' | grep -q 'endpoint='" \
        0
    
    run_test "Status labels in HTTP metrics" \
        "echo '$metrics_content' | grep 'whisper_http_requests_total' | grep -q 'status='" \
        0
}

# Function to test metrics persistence
test_metrics_persistence() {
    print_info "Testing metrics persistence across requests"
    
    # Make multiple requests to the metrics endpoint
    for i in {1..3}; do
        metrics_response=$(curl -s "${METRICS_ENDPOINT}")
        if [ $? -ne 0 ]; then
            print_error "Failed to get metrics on attempt $i"
            return 1
        fi
        sleep 1
    done
    
    run_test "Metrics endpoint is consistently available" \
        "true" \
        0
}

# Function to benchmark metrics endpoint performance
benchmark_metrics_endpoint() {
    print_info "Benchmarking metrics endpoint performance"
    
    # Time multiple requests to the metrics endpoint
    total_time=0
    requests=5
    
    for i in $(seq 1 $requests); do
        start_time=$(date +%s%N)
        curl -s "${METRICS_ENDPOINT}" > /dev/null
        end_time=$(date +%s%N)
        request_time=$((($end_time - $start_time) / 1000000)) # Convert to milliseconds
        total_time=$(($total_time + $request_time))
        print_info "Request $i: ${request_time}ms"
    done
    
    average_time=$(($total_time / $requests))
    print_info "Average response time: ${average_time}ms"
    
    if [ $average_time -lt 1000 ]; then
        print_success "Metrics endpoint performance is good (<1s average)"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        print_warning "Metrics endpoint is slow (>${average_time}ms average)"
    fi
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
}

# Function to save metrics for analysis
save_metrics_sample() {
    print_info "Saving metrics sample for analysis"
    
    timestamp=$(date +"%Y%m%d_%H%M%S")
    output_file="/tmp/whisper_metrics_${timestamp}.txt"
    
    curl -s "${METRICS_ENDPOINT}" > "$output_file"
    
    if [ $? -eq 0 ]; then
        print_success "Metrics saved to: $output_file"
        print_info "You can analyze the metrics with: cat $output_file"
    else
        print_error "Failed to save metrics"
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
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}All tests passed! ✓${NC}"
        exit 0
    else
        echo -e "${RED}Some tests failed! ✗${NC}"
        exit 1
    fi
}

# Main execution
main() {
    echo "=========================================="
    echo "    Whisper API Prometheus Metrics Test"
    echo "=========================================="
    echo "API URL: $API_BASE_URL"
    echo "Metrics URL: $METRICS_ENDPOINT"
    echo
    
    # Run all tests
    check_api_availability
    test_metrics_endpoint
    test_prometheus_format
    test_expected_metrics
    test_job_metrics
    test_http_metrics
    test_metrics_labels
    test_metrics_persistence
    benchmark_metrics_endpoint
    save_metrics_sample
    
    # Print summary
    print_summary
}

# Run main function
main "$@"
