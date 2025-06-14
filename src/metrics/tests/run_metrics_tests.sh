#!/bin/bash

# Comprehensive Metrics Test Script for Whisper API
# This script tests both Prometheus and StatsD metrics backends
# and provides comprehensive guidance for metrics validation.

# Configuration
API_HOST="${WHISPER_API_HOST:-127.0.0.1}"
API_PORT="${WHISPER_API_PORT:-8181}"
API_BASE_URL="http://${API_HOST}:${API_PORT}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored output
print_header() {
    echo -e "${CYAN}=========================================="
    echo -e "$1"
    echo -e "==========================================${NC}"
}

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

# Function to detect current metrics backend
detect_metrics_backend() {
    print_info "Detecting current metrics backend configuration..."
    
    # Try to access the metrics endpoint
    metrics_response=$(curl -s "${API_BASE_URL}/metrics" 2>/dev/null)
    curl_exit_code=$?
    
    if [ $curl_exit_code -eq 0 ]; then
        if echo "$metrics_response" | grep -q "# HELP"; then
            echo "prometheus"
            return 0
        elif [ -z "$metrics_response" ] || echo "$metrics_response" | grep -q "StatsD\|statsd"; then
            echo "statsd"
            return 0
        elif echo "$metrics_response" | grep -q "Failed to export metrics"; then
            echo "error"
            return 0
        fi
    fi
    
    # If metrics endpoint is not accessible, try to infer from environment
    if [ ! -z "$METRICS_BACKEND" ]; then
        echo "$METRICS_BACKEND"
        return 0
    fi
    
    if [ ! -z "$METRICS_EXPORTER" ]; then
        echo "$METRICS_EXPORTER"
        return 0
    fi
    
    echo "unknown"
    return 1
}

# Function to check API availability
check_api_status() {
    print_info "Checking Whisper API status..."
    
    if curl -s --connect-timeout 5 "${API_BASE_URL}/metrics" >/dev/null 2>&1; then
        print_success "API is running and accessible at $API_BASE_URL"
        return 0
    else
        print_error "API is not accessible at $API_BASE_URL"
        print_info "Please ensure the Whisper API is running."
        return 1
    fi
}

# Function to test Prometheus metrics
test_prometheus() {
    print_header "Testing Prometheus Metrics Backend"
    
    # Check if Prometheus test script exists
    if [ -f "./test_prometheus_metrics.sh" ]; then
        print_info "Running Prometheus-specific tests..."
        chmod +x "./test_prometheus_metrics.sh"
        ./test_prometheus_metrics.sh
    else
        print_warning "Prometheus test script not found. Running basic tests..."
        
        # Basic Prometheus tests
        print_info "Testing metrics endpoint..."
        metrics_content=$(curl -s "${API_BASE_URL}/metrics")
        
        if echo "$metrics_content" | grep -q "# HELP"; then
            print_success "Prometheus metrics format detected"
            
            # Count metrics
            help_lines=$(echo "$metrics_content" | grep -c "# HELP")
            type_lines=$(echo "$metrics_content" | grep -c "# TYPE")
            metric_lines=$(echo "$metrics_content" | grep -c "whisper_")
            
            print_info "Metrics summary:"
            print_info "  HELP lines: $help_lines"
            print_info "  TYPE lines: $type_lines"
            print_info "  Whisper metrics: $metric_lines"
            
        else
            print_error "No Prometheus metrics found"
        fi
    fi
}

# Function to test StatsD metrics
test_statsd() {
    print_header "Testing StatsD Metrics Backend"
    
    # Check if StatsD test script exists
    if [ -f "./test_statsd_metrics.sh" ]; then
        print_info "Running StatsD-specific tests..."
        chmod +x "./test_statsd_metrics.sh"
        ./test_statsd_metrics.sh
    else
        print_warning "StatsD test script not found. Running basic tests..."
        
        # Basic StatsD tests
        STATSD_HOST="${STATSD_HOST:-127.0.0.1}"
        STATSD_PORT="${STATSD_PORT:-8125}"
        
        print_info "Testing StatsD configuration..."
        print_info "StatsD Host: $STATSD_HOST"
        print_info "StatsD Port: $STATSD_PORT"
        
        # Try to test StatsD connectivity
        if command -v nc >/dev/null 2>&1; then
            if echo "test.metric:1|c" | nc -u -w1 "$STATSD_HOST" "$STATSD_PORT" >/dev/null 2>&1; then
                print_success "StatsD server is responsive"
            else
                print_warning "StatsD server may not be running"
            fi
        else
            print_warning "Cannot test StatsD connectivity (nc not available)"
        fi
    fi
}

# Function to run comprehensive API tests
run_api_tests() {
    print_header "Running Comprehensive API Tests"
    
    print_info "Testing various API endpoints to generate metrics..."
    
    # Test metrics endpoint
    curl -s "${API_BASE_URL}/metrics" >/dev/null 2>&1 || true
    
    # Test non-existent job status
    curl -s "${API_BASE_URL}/transcription/test-job-12345" >/dev/null 2>&1 || true
    
    # Test non-existent job result
    curl -s "${API_BASE_URL}/transcription/test-job-67890/result" >/dev/null 2>&1 || true
    
    # Create a small test file and try to submit a job
    test_file="/tmp/metrics_test_audio.wav"
    echo "fake audio content for testing metrics" > "$test_file"
    
    print_info "Submitting test transcription job..."
    job_response=$(curl -s -X POST \
        -F "audio=@${test_file}" \
        -F "language=en" \
        -F "model=tiny" \
        -F "sync=false" \
        "${API_BASE_URL}/transcription" 2>/dev/null || echo "failed")
    
    if [ "$job_response" != "failed" ]; then
        print_success "Test job submitted successfully"
        
        # Extract job ID if possible and test status endpoint
        job_id=$(echo "$job_response" | grep -o '"job_id":"[^"]*"' | cut -d'"' -f4 2>/dev/null || echo "")
        
        if [ ! -z "$job_id" ]; then
            print_info "Testing job status for job: $job_id"
            curl -s "${API_BASE_URL}/transcription/$job_id" >/dev/null 2>&1 || true
            
            # Wait a moment then try to cancel the job
            sleep 2
            curl -s -X DELETE "${API_BASE_URL}/transcription/$job_id" >/dev/null 2>&1 || true
        fi
    else
        print_warning "Test job submission failed (this is expected with fake audio)"
    fi
    
    # Cleanup
    rm -f "$test_file"
    
    print_success "API tests completed"
}

# Function to provide configuration guidance
print_configuration_guide() {
    print_header "Metrics Configuration Guide"
    
    echo "To test different metrics backends, start the Whisper API with:"
    echo
    echo -e "${GREEN}For Prometheus:${NC}"
    echo "  METRICS_BACKEND=prometheus ./whisper_api"
    echo "  Then access metrics at: ${API_BASE_URL}/metrics"
    echo
    echo -e "${GREEN}For StatsD:${NC}"
    echo "  METRICS_BACKEND=statsd STATSD_HOST=127.0.0.1 STATSD_PORT=8125 ./whisper_api"
    echo "  Requires a StatsD server running on the specified host/port"
    echo
    echo -e "${GREEN}For development (no metrics):${NC}"
    echo "  METRICS_BACKEND=null ./whisper_api"
    echo "  or simply: ./whisper_api (null is the default)"
    echo
    echo -e "${YELLOW}StatsD Integration Examples:${NC}"
    echo "  # Start Graphite with StatsD (Docker)"
    echo "  docker run -d --name graphite -p 8080:80 -p 8125:8125/udp graphiteapp/graphite-statsd"
    echo
    echo "  # Start InfluxDB with StatsD plugin"
    echo "  # (requires InfluxDB configuration for StatsD)"
    echo
    echo "  # Simple StatsD listener for testing"
    echo "  nc -u -l -p 8125"
}

# Function to analyze current metrics
analyze_current_metrics() {
    print_header "Current Metrics Analysis"
    
    backend=$(detect_metrics_backend)
    
    case $backend in
        "prometheus")
            print_success "Detected Prometheus metrics backend"
            
            metrics_content=$(curl -s "${API_BASE_URL}/metrics")
            
            # Analyze Prometheus metrics
            echo "Available metrics:"
            echo "$metrics_content" | grep "# HELP" | while read line; do
                metric_name=$(echo "$line" | awk '{print $3}')
                echo "  - $metric_name"
            done
            
            echo
            echo "Sample metrics values:"
            echo "$metrics_content" | grep "whisper_" | grep -v "# " | head -10 | while read line; do
                echo "  $line"
            done
            ;;
            
        "statsd")
            print_success "Detected StatsD metrics backend"
            print_info "StatsD metrics are sent via UDP and not directly visible via HTTP"
            print_info "Check your StatsD server logs or dashboard for metrics"
            ;;
            
        "unknown")
            print_warning "Could not determine metrics backend"
            print_info "The API might be using the 'null' backend or may not be running"
            ;;
    esac
}

# Function to run performance tests
run_performance_tests() {
    print_header "Performance Impact Analysis"
    
    print_info "Testing metrics collection performance impact..."
    
    # Time multiple requests to measure overhead
    total_time=0
    requests=10
    
    for i in $(seq 1 $requests); do
        start_time=$(date +%s%N)
        curl -s "${API_BASE_URL}/metrics" >/dev/null 2>&1 || curl -s "${API_BASE_URL}/transcription/perf-test-$i" >/dev/null 2>&1 || true
        end_time=$(date +%s%N)
        
        request_time=$((($end_time - $start_time) / 1000000)) # Convert to milliseconds
        total_time=$(($total_time + $request_time))
    done
    
    average_time=$(($total_time / $requests))
    print_info "Average request time: ${average_time}ms over $requests requests"
    
    if [ $average_time -lt 100 ]; then
        print_success "Low latency impact from metrics collection"
    elif [ $average_time -lt 500 ]; then
        print_warning "Moderate latency impact from metrics collection"
    else
        print_error "High latency impact from metrics collection"
    fi
}

# Function to save test results
save_test_results() {
    timestamp=$(date +"%Y%m%d_%H%M%S")
    results_dir="/tmp/whisper_metrics_test_$timestamp"
    mkdir -p "$results_dir"
    
    print_info "Saving test results to: $results_dir"
    
    # Save API info
    echo "API Base URL: $API_BASE_URL" > "$results_dir/test_info.txt"
    echo "Test Timestamp: $timestamp" >> "$results_dir/test_info.txt"
    echo "Detected Backend: $(detect_metrics_backend)" >> "$results_dir/test_info.txt"
    
    # Save current metrics (if Prometheus)
    if [ "$(detect_metrics_backend)" = "prometheus" ]; then
        curl -s "${API_BASE_URL}/metrics" > "$results_dir/prometheus_metrics.txt" 2>/dev/null || true
    fi
    
    # Save environment variables
    env | grep -E "(WHISPER|METRICS|STATSD)" > "$results_dir/environment.txt" || true
    
    print_success "Test results saved to: $results_dir"
}

# Main execution function
main() {
    print_header "Whisper API Metrics Comprehensive Test Suite"
    
    echo "API URL: $API_BASE_URL"
    echo "Test Started: $(date)"
    echo
    
    # Check API availability first
    if ! check_api_status; then
        print_error "Cannot proceed without API access"
        exit 1
    fi
    
    # Detect and display current backend
    backend=$(detect_metrics_backend)
    print_info "Detected metrics backend: $backend"
    
    # Run comprehensive tests
    run_api_tests
    
    # Wait for metrics to be processed
    sleep 3
    
    # Run backend-specific tests
    case $backend in
        "prometheus")
            test_prometheus
            ;;
        "statsd")
            test_statsd
            ;;
        "unknown")
            print_warning "Running generic tests for unknown backend"
            ;;
    esac
    
    # Analyze current state
    analyze_current_metrics
    
    # Performance testing
    run_performance_tests
    
    # Save results
    save_test_results
    
    # Show configuration guide
    print_configuration_guide
    
    print_header "Test Suite Complete"
    print_success "All tests have been executed. Check the output above for detailed results."
}

# Handle script arguments
case "${1:-}" in
    "prometheus")
        test_prometheus
        ;;
    "statsd")
        test_statsd
        ;;
    "guide")
        print_configuration_guide
        ;;
    "analyze")
        analyze_current_metrics
        ;;
    *)
        main
        ;;
esac
