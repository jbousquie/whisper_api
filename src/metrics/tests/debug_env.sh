#!/bin/bash

# Whisper API Metrics Environment Variable Debug Script
# This script helps diagnose environment variable configuration issues

echo "=========================================="
echo "  Whisper API Metrics Environment Debug"
echo "=========================================="
echo

echo "Current Environment Variables:"
echo "------------------------------"

# Check metrics backend configuration
echo "Metrics Backend Configuration:"
if [ ! -z "$METRICS_BACKEND" ]; then
    echo "  METRICS_BACKEND = '$METRICS_BACKEND'"
else
    echo "  METRICS_BACKEND = (not set)"
fi

if [ ! -z "$METRICS_EXPORTER" ]; then
    echo "  METRICS_EXPORTER = '$METRICS_EXPORTER'"
else
    echo "  METRICS_EXPORTER = (not set)"
fi

echo

# Check StatsD configuration
echo "StatsD Configuration:"
if [ ! -z "$STATSD_HOST" ]; then
    echo "  STATSD_HOST = '$STATSD_HOST'"
else
    echo "  STATSD_HOST = (not set, default: 127.0.0.1)"
fi

if [ ! -z "$STATSD_PORT" ]; then
    echo "  STATSD_PORT = '$STATSD_PORT'"
else
    echo "  STATSD_PORT = (not set, default: 8125)"
fi

if [ ! -z "$STATSD_ENDPOINT" ]; then
    echo "  STATSD_ENDPOINT = '$STATSD_ENDPOINT'"
else
    echo "  STATSD_ENDPOINT = (not set)"
fi

if [ ! -z "$STATSD_PREFIX" ]; then
    echo "  STATSD_PREFIX = '$STATSD_PREFIX'"
else
    echo "  STATSD_PREFIX = (not set)"
fi

if [ ! -z "$STATSD_SAMPLE_RATE" ]; then
    echo "  STATSD_SAMPLE_RATE = '$STATSD_SAMPLE_RATE'"
else
    echo "  STATSD_SAMPLE_RATE = (not set, default: 1.0)"
fi

echo

# Check general metrics configuration
echo "General Metrics Configuration:"
if [ ! -z "$METRICS_ENDPOINT" ]; then
    echo "  METRICS_ENDPOINT = '$METRICS_ENDPOINT'"
else
    echo "  METRICS_ENDPOINT = (not set)"
fi

if [ ! -z "$METRICS_PREFIX" ]; then
    echo "  METRICS_PREFIX = '$METRICS_PREFIX'"
else
    echo "  METRICS_PREFIX = (not set)"
fi

if [ ! -z "$METRICS_SAMPLE_RATE" ]; then
    echo "  METRICS_SAMPLE_RATE = '$METRICS_SAMPLE_RATE'"
else
    echo "  METRICS_SAMPLE_RATE = (not set)"
fi

echo

# Check API configuration
echo "API Configuration:"
if [ ! -z "$WHISPER_API_HOST" ]; then
    echo "  WHISPER_API_HOST = '$WHISPER_API_HOST'"
else
    echo "  WHISPER_API_HOST = (not set, default: 127.0.0.1)"
fi

if [ ! -z "$WHISPER_API_PORT" ]; then
    echo "  WHISPER_API_PORT = '$WHISPER_API_PORT'"
else
    echo "  WHISPER_API_PORT = (not set, default: 8181)"
fi

echo

# Determine effective configuration
echo "Effective Configuration (what the API will use):"
echo "------------------------------------------------"

# Determine backend
if [ ! -z "$METRICS_BACKEND" ]; then
    EFFECTIVE_BACKEND="$METRICS_BACKEND"
elif [ ! -z "$METRICS_EXPORTER" ]; then
    EFFECTIVE_BACKEND="$METRICS_EXPORTER"
else
    EFFECTIVE_BACKEND="none"
fi
echo "Backend: $EFFECTIVE_BACKEND"

# Determine endpoint for StatsD
if [ "$EFFECTIVE_BACKEND" = "statsd" ]; then
    if [ ! -z "$METRICS_ENDPOINT" ]; then
        EFFECTIVE_ENDPOINT="$METRICS_ENDPOINT"
    elif [ ! -z "$STATSD_ENDPOINT" ]; then
        EFFECTIVE_ENDPOINT="$STATSD_ENDPOINT"
    else
        STATSD_HOST_VAL="${STATSD_HOST:-127.0.0.1}"
        STATSD_PORT_VAL="${STATSD_PORT:-8125}"
        EFFECTIVE_ENDPOINT="${STATSD_HOST_VAL}:${STATSD_PORT_VAL}"
    fi
    echo "StatsD Endpoint: $EFFECTIVE_ENDPOINT"
fi

# Determine prefix
if [ ! -z "$METRICS_PREFIX" ]; then
    EFFECTIVE_PREFIX="$METRICS_PREFIX"
elif [ ! -z "$STATSD_PREFIX" ]; then
    EFFECTIVE_PREFIX="$STATSD_PREFIX"
else
    EFFECTIVE_PREFIX="(none)"
fi
echo "Prefix: $EFFECTIVE_PREFIX"

# Determine sample rate
if [ ! -z "$METRICS_SAMPLE_RATE" ]; then
    EFFECTIVE_SAMPLE_RATE="$METRICS_SAMPLE_RATE"
elif [ ! -z "$STATSD_SAMPLE_RATE" ]; then
    EFFECTIVE_SAMPLE_RATE="$STATSD_SAMPLE_RATE"
else
    EFFECTIVE_SAMPLE_RATE="1.0"
fi
echo "Sample Rate: $EFFECTIVE_SAMPLE_RATE"

echo

# Provide recommendations
echo "Recommendations:"
echo "---------------"

case "$EFFECTIVE_BACKEND" in
    "prometheus")
        echo "✓ Prometheus backend is configured"
        echo "  - Start API with: ./whisper_api"
        echo "  - Access metrics at: http://${WHISPER_API_HOST:-127.0.0.1}:${WHISPER_API_PORT:-8181}/metrics"
        ;;
    "statsd")
        echo "✓ StatsD backend is configured"
        echo "  - Make sure StatsD server is running at: $EFFECTIVE_ENDPOINT"
        echo "  - Test connectivity: echo 'test:1|c' | nc -u ${EFFECTIVE_ENDPOINT%:*} ${EFFECTIVE_ENDPOINT#*:}"
        echo "  - Start API with: ./whisper_api"
        ;;
    "none"|"null"|"disabled")
        echo "! Metrics are disabled"
        echo "  - To enable Prometheus: export METRICS_BACKEND=prometheus"
        echo "  - To enable StatsD: export METRICS_BACKEND=statsd"
        ;;
    *)
        echo "⚠ Unknown or invalid backend: $EFFECTIVE_BACKEND"
        echo "  - Valid options: prometheus, statsd, none, null"
        echo "  - Set METRICS_BACKEND to one of the valid options"
        ;;
esac

echo

# Test API connectivity if requested
if [ "$1" = "--test-api" ]; then
    echo "Testing API Connectivity:"
    echo "------------------------"
    
    API_URL="http://${WHISPER_API_HOST:-127.0.0.1}:${WHISPER_API_PORT:-8181}"
    
    if command -v curl >/dev/null 2>&1; then
        echo -n "Testing API endpoint... "
        if curl -s --connect-timeout 5 "$API_URL/metrics" >/dev/null 2>&1; then
            echo "✓ API is accessible"
            
            echo -n "Detecting actual backend... "
            RESPONSE=$(curl -s "$API_URL/metrics" 2>/dev/null)
            if echo "$RESPONSE" | grep -q "# HELP"; then
                echo "✓ Prometheus (metrics endpoint returns Prometheus format)"
            elif [ -z "$RESPONSE" ]; then
                echo "? Possibly StatsD (empty response from metrics endpoint)"
            else
                echo "? Unknown (unexpected response from metrics endpoint)"
            fi
        else
            echo "✗ API is not accessible"
            echo "  Make sure the Whisper API is running"
        fi
    else
        echo "curl not available - cannot test API connectivity"
    fi
fi

echo
echo "=========================================="
echo "Debug complete. Use --test-api to test API connectivity."
