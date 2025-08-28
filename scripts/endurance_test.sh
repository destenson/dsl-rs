#!/bin/bash
#
# DSL-RS Endurance Test Script
# Runs long-duration tests to validate 24/7 stability
#

set -e

# Configuration
DURATION_HOURS=${1:-24}
INTERVAL_MINUTES=30
LOG_DIR="endurance-logs"

echo "Starting DSL-RS endurance test for $DURATION_HOURS hours"

# Create log directory
mkdir -p "$LOG_DIR"

# Start timestamp
START_TIME=$(date +%s)
END_TIME=$((START_TIME + DURATION_HOURS * 3600))

# Initialize counters
ITERATION=0
FAILURES=0

# Run tests in a loop
while [ $(date +%s) -lt $END_TIME ]; do
    ITERATION=$((ITERATION + 1))
    echo "Iteration $ITERATION at $(date)"
    
    # Run tests and capture metrics
    if cargo test --release > "$LOG_DIR/test_$ITERATION.log" 2>&1; then
        echo "  Tests passed"
    else
        echo "  Tests failed!"
        FAILURES=$((FAILURES + 1))
    fi
    
    # Check resource usage
    if command -v ps &> /dev/null; then
        ps aux | grep dsl-rs | head -n 1 >> "$LOG_DIR/resources.log"
    fi
    
    # Check for memory leaks
    if command -v valgrind &> /dev/null; then
        timeout 60 valgrind --leak-check=quick cargo test --release --test memory_leak 2>&1 | \
            grep "definitely lost" >> "$LOG_DIR/leaks.log" || true
    fi
    
    # Sleep until next interval
    sleep $((INTERVAL_MINUTES * 60))
done

# Summary
echo "\nEndurance Test Complete"
echo "Duration: $DURATION_HOURS hours"
echo "Iterations: $ITERATION"
echo "Failures: $FAILURES"
echo "Success Rate: $((100 * (ITERATION - FAILURES) / ITERATION))%"

exit $FAILURES