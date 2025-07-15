#!/bin/bash

# Whisper API Metrics Test Runner
# Convenience script to run various metric tests

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
API_HOST="${WHISPER_API_HOST:-127.0.0.1}"
API_PORT="${WHISPER_API_PORT:-8181}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_usage() {
    echo "Whisper API Metrics Test Runner"
    echo
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo
    echo "Commands:"
    echo "  prometheus    Run Prometheus metrics tests"
    echo "  statsd        Run StatsD metrics tests"
    echo "  all           Run comprehensive test suite"
    echo "  setup         Set up test environment"
    echo "  clean         Clean up test files"
    echo "  status        Show test environment status"
    echo "  debug         Debug environment variables and API connectivity"
    echo "  help          Show this help message"
    echo
    echo "Options:"
    echo "  --host HOST   API host (default: $API_HOST)"
    echo "  --port PORT   API port (default: $API_PORT)"
    echo "  --verbose     Enable verbose output"
    echo
    echo "Examples:"
    echo "  $0 all                    # Run all tests"
    echo "  $0 prometheus             # Test Prometheus backend"
    echo "  $0 statsd --verbose       # Test StatsD with verbose output"
    echo "  $0 setup                  # Set up test environment"
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

check_dependencies() {
    print_info "Checking dependencies..."
    
    local missing_deps=()
    
    if ! command -v curl >/dev/null 2>&1; then
        missing_deps+=("curl")
    fi
    
    if ! command -v bash >/dev/null 2>&1; then
        missing_deps+=("bash")
    fi
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        print_error "Missing dependencies: ${missing_deps[*]}"
        print_info "Please install the missing dependencies and try again."
        exit 1
    fi
    
    print_success "All dependencies are available"
}

make_executable() {
    print_info "Making test scripts executable..."
    
    local scripts=(
        "run_metrics_tests.sh"
        "test_prometheus_metrics.sh"
        "test_statsd_metrics.sh"
    )
    
    for script in "${scripts[@]}"; do
        if [ -f "$SCRIPT_DIR/$script" ]; then
            chmod +x "$SCRIPT_DIR/$script"
            print_success "Made $script executable"
        else
            print_warning "$script not found"
        fi
    done
}

setup_environment() {
    print_info "Setting up test environment..."
    
    check_dependencies
    make_executable
    
    # Create temporary directory for test files
    mkdir -p /tmp/whisper_api_tests
    
    print_success "Test environment setup complete"
    print_info "You can now run tests with: $0 all"
}

run_prometheus_tests() {
    print_info "Running Prometheus metrics tests..."
    
    export WHISPER_API_HOST="$API_HOST"
    export WHISPER_API_PORT="$API_PORT"
    
    if [ -f "$SCRIPT_DIR/test_prometheus_metrics.sh" ]; then
        "$SCRIPT_DIR/test_prometheus_metrics.sh"
    else
        print_error "Prometheus test script not found"
        exit 1
    fi
}

run_statsd_tests() {
    print_info "Running StatsD metrics tests..."
    
    export WHISPER_API_HOST="$API_HOST"
    export WHISPER_API_PORT="$API_PORT"
    
    if [ -f "$SCRIPT_DIR/test_statsd_metrics.sh" ]; then
        "$SCRIPT_DIR/test_statsd_metrics.sh"
    else
        print_error "StatsD test script not found"
        exit 1
    fi
}

run_all_tests() {
    print_info "Running comprehensive metrics test suite..."
    
    export WHISPER_API_HOST="$API_HOST"
    export WHISPER_API_PORT="$API_PORT"
    
    if [ -f "$SCRIPT_DIR/run_metrics_tests.sh" ]; then
        "$SCRIPT_DIR/run_metrics_tests.sh"
    else
        print_error "Comprehensive test script not found"
        exit 1
    fi
}

clean_test_files() {
    print_info "Cleaning up test files..."
    
    # Remove temporary test files
    rm -f /tmp/test_audio_*.wav
    rm -f /tmp/whisper_metrics_*.txt
    rm -f /tmp/statsd_metrics_*.log
    rm -f /tmp/statsd_capture_*.pcap
    rm -rf /tmp/whisper_metrics_test_*
    rm -rf /tmp/whisper_api_tests
    
    # Remove local test files
    rm -f "$SCRIPT_DIR"/prometheus_metrics.txt
    rm -f "$SCRIPT_DIR"/temp_metrics.txt
    rm -f "$SCRIPT_DIR"/metrics_test_results_*.txt
    
    print_success "Test files cleaned up"
}

show_test_status() {
    print_info "Test Environment Status:"
    echo
    echo "API Configuration:"
    echo "  Host: $API_HOST"
    echo "  Port: $API_PORT"
    echo "  URL: http://$API_HOST:$API_PORT"
    echo
    
    echo "Available Scripts:"
    local scripts=(
        "run_metrics_tests.sh:Comprehensive test suite"
        "test_prometheus_metrics.sh:Prometheus-specific tests"
        "test_statsd_metrics.sh:StatsD-specific tests"
        "test_metrics.ps1:PowerShell script for Windows"
        "test_metrics.bat:Batch script for Windows"
    )
    
    for script_info in "${scripts[@]}"; do
        IFS=':' read -r script desc <<< "$script_info"
        if [ -f "$SCRIPT_DIR/$script" ]; then
            if [ -x "$SCRIPT_DIR/$script" ] || [[ "$script" == *.ps1 ]] || [[ "$script" == *.bat ]]; then
                print_success "✓ $script - $desc"
            else
                print_warning "! $script - $desc (not executable)"
            fi
        else
            print_error "✗ $script - $desc (missing)"
        fi
    done
    
    echo
    echo "Dependencies:"
    command -v curl >/dev/null 2>&1 && print_success "✓ curl" || print_error "✗ curl"
    command -v nc >/dev/null 2>&1 && print_success "✓ netcat (optional)" || print_warning "! netcat (optional)"
    command -v tcpdump >/dev/null 2>&1 && print_success "✓ tcpdump (optional)" || print_warning "! tcpdump (optional)"
}

# Parse command line arguments
VERBOSE=false
COMMAND=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --host)
            API_HOST="$2"
            shift 2
            ;;
        --port)
            API_PORT="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        prometheus|statsd|all|setup|clean|help|status|debug)
            COMMAND="$1"
            shift
            ;;
        *)
            print_error "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# Enable verbose output if requested
if [ "$VERBOSE" = true ]; then
    set -x
fi

# Execute command
case "$COMMAND" in
    prometheus)
        run_prometheus_tests
        ;;
    statsd)
        run_statsd_tests
        ;;
    all)
        run_all_tests
        ;;
    setup)
        setup_environment
        ;;
    clean)
        clean_test_files
        ;;
    status)
        show_test_status
        ;;
    debug)
        if [ -f "$SCRIPT_DIR/debug_env.sh" ]; then
            chmod +x "$SCRIPT_DIR/debug_env.sh"
            "$SCRIPT_DIR/debug_env.sh" --test-api
        else
            print_error "Debug script not found"
            exit 1
        fi
        ;;
    help|"")
        print_usage
        ;;
    *)
        print_error "Unknown command: $COMMAND"
        print_usage
        exit 1
        ;;
esac
