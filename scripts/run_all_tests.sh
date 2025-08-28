#!/bin/bash

# DSL-RS Test Runner - Linux/macOS Wrapper
# Executes the Python test runner with proper environment setup

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Default values
PYTHON_CMD="python3"
TEST_PROFILE="standard"
PARALLEL=true
GENERATE_REPORTS=true

# Function to print colored output
print_color() {
    local color=$1
    shift
    echo -e "${color}$@${NC}"
}

# Function to check prerequisites
check_prerequisites() {
    print_color $BLUE "Checking prerequisites..."
    
    # Check Python
    if ! command -v $PYTHON_CMD &> /dev/null; then
        print_color $RED "Error: Python 3 is not installed"
        exit 1
    fi
    
    # Check Python version
    PYTHON_VERSION=$($PYTHON_CMD --version | cut -d' ' -f2 | cut -d'.' -f1,2)
    REQUIRED_VERSION="3.8"
    if [ "$(printf '%s\n' "$REQUIRED_VERSION" "$PYTHON_VERSION" | sort -V | head -n1)" != "$REQUIRED_VERSION" ]; then
        print_color $RED "Error: Python $REQUIRED_VERSION or higher is required (found $PYTHON_VERSION)"
        exit 1
    fi
    
    # Check Rust and Cargo
    if ! command -v cargo &> /dev/null; then
        print_color $RED "Error: Cargo is not installed"
        exit 1
    fi
    
    # Check GStreamer
    if ! pkg-config --exists gstreamer-1.0; then
        print_color $YELLOW "Warning: GStreamer may not be properly installed"
    fi
    
    # Check if virtual environment exists
    if [ ! -d "$PROJECT_ROOT/venv" ]; then
        print_color $YELLOW "Virtual environment not found. Creating..."
        $PYTHON_CMD -m venv "$PROJECT_ROOT/venv"
    fi
    
    # Activate virtual environment
    source "$PROJECT_ROOT/venv/bin/activate"
    
    # Install Python dependencies
    if [ -f "$SCRIPT_DIR/requirements.txt" ]; then
        print_color $BLUE "Installing Python dependencies..."
        pip install -q -r "$SCRIPT_DIR/requirements.txt"
    fi
    
    print_color $GREEN "Prerequisites check completed"
}

# Function to set environment variables
setup_environment() {
    print_color $BLUE "Setting up environment..."
    
    # Rust environment
    export RUST_BACKTRACE=1
    export RUST_LOG=${RUST_LOG:-info}
    
    # GStreamer environment
    export GST_DEBUG=${GST_DEBUG:-2}
    
    # Test runner environment
    export TEST_RUNNER_ROOT="$PROJECT_ROOT"
    export TEST_REPORTS_DIR="$PROJECT_ROOT/test-reports"
    export TEST_LOGS_DIR="$PROJECT_ROOT/test-logs"
    
    # Create necessary directories
    mkdir -p "$TEST_REPORTS_DIR"
    mkdir -p "$TEST_LOGS_DIR"
    
    # Platform-specific settings
    case "$(uname -s)" in
        Linux*)
            export TEST_PLATFORM="linux"
            export TEST_MAX_PARALLEL=$(nproc)
            ;;
        Darwin*)
            export TEST_PLATFORM="macos"
            export TEST_MAX_PARALLEL=$(sysctl -n hw.ncpu)
            ;;
        *)
            export TEST_PLATFORM="unknown"
            export TEST_MAX_PARALLEL=4
            ;;
    esac
    
    print_color $GREEN "Environment setup completed"
}

# Function to run tests
run_tests() {
    print_color $BLUE "Starting test execution..."
    
    # Build command
    CMD="$PYTHON_CMD $SCRIPT_DIR/test_runner.py"
    
    # Add test profile
    case $TEST_PROFILE in
        quick)
            CMD="$CMD --unit --integration"
            ;;
        standard)
            CMD="$CMD --all"
            ;;
        full)
            CMD="$CMD --all --benchmarks"
            ;;
        nightly)
            CMD="$CMD --all --benchmarks --matrix"
            ;;
        *)
            CMD="$CMD $TEST_PROFILE"
            ;;
    esac
    
    # Add parallel flag
    if [ "$PARALLEL" = true ]; then
        CMD="$CMD --parallel"
    fi
    
    # Add report generation
    if [ "$GENERATE_REPORTS" = true ]; then
        CMD="$CMD --json --html"
    fi
    
    # Add any additional arguments
    if [ $# -gt 0 ]; then
        CMD="$CMD $@"
    fi
    
    # Execute tests
    print_color $GREEN "Executing: $CMD"
    
    # Run with timestamp and log
    TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    LOG_FILE="$TEST_LOGS_DIR/test_run_${TIMESTAMP}.log"
    
    if $CMD 2>&1 | tee "$LOG_FILE"; then
        EXIT_CODE=0
        print_color $GREEN "Test execution completed successfully"
    else
        EXIT_CODE=$?
        print_color $RED "Test execution failed with exit code $EXIT_CODE"
    fi
    
    # Save log location
    print_color $BLUE "Log saved to: $LOG_FILE"
    
    return $EXIT_CODE
}

# Function to display usage
usage() {
    cat << EOF
Usage: $0 [OPTIONS] [ADDITIONAL_ARGS]

Options:
    -h, --help              Show this help message
    -p, --profile PROFILE   Test profile to run (quick|standard|full|nightly)
                           Default: standard
    -s, --sequential        Run tests sequentially instead of parallel
    -n, --no-reports       Don't generate HTML/JSON reports
    --python PATH          Path to Python executable
                           Default: python3

Examples:
    $0                      # Run standard tests
    $0 --profile quick      # Run quick tests only
    $0 --profile full       # Run full test suite
    $0 --sequential         # Run tests sequentially
    $0 -- --chaos           # Pass additional args to test runner

EOF
    exit 0
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            ;;
        -p|--profile)
            TEST_PROFILE="$2"
            shift 2
            ;;
        -s|--sequential)
            PARALLEL=false
            shift
            ;;
        -n|--no-reports)
            GENERATE_REPORTS=false
            shift
            ;;
        --python)
            PYTHON_CMD="$2"
            shift 2
            ;;
        --)
            shift
            break
            ;;
        *)
            break
            ;;
    esac
done

# Main execution
main() {
    print_color $BLUE "========================================="
    print_color $BLUE "       DSL-RS Test Runner"
    print_color $BLUE "========================================="
    
    # Check prerequisites
    check_prerequisites
    
    # Setup environment
    setup_environment
    
    # Run tests
    run_tests "$@"
    EXIT_CODE=$?
    
    # Deactivate virtual environment
    deactivate 2>/dev/null || true
    
    print_color $BLUE "========================================="
    if [ $EXIT_CODE -eq 0 ]; then
        print_color $GREEN "       All Tests Completed"
    else
        print_color $RED "       Tests Failed"
    fi
    print_color $BLUE "========================================="
    
    exit $EXIT_CODE
}

# Run main function
main "$@"