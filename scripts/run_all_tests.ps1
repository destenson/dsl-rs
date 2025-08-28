# DSL-RS Test Runner - Windows PowerShell Wrapper
# Executes the Python test runner with proper environment setup

param(
    [Parameter(HelpMessage="Test profile to run (quick|standard|full|nightly)")]
    [ValidateSet('quick', 'standard', 'full', 'nightly')]
    [string]$Profile = 'standard',
    
    [Parameter(HelpMessage="Run tests sequentially instead of parallel")]
    [switch]$Sequential,
    
    [Parameter(HelpMessage="Don't generate HTML/JSON reports")]
    [switch]$NoReports,
    
    [Parameter(HelpMessage="Path to Python executable")]
    [string]$Python = 'python',
    
    [Parameter(HelpMessage="Show help message")]
    [switch]$Help,
    
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$AdditionalArgs
)

# Script configuration
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Get script and project directories
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

# Colors for output (using Write-Host)
function Write-Color {
    param(
        [string]$Message,
        [ConsoleColor]$Color = 'White'
    )
    Write-Host $Message -ForegroundColor $Color
}

# Display usage
function Show-Usage {
    Write-Host @"
DSL-RS Test Runner - Windows PowerShell Wrapper

Usage: .\run_all_tests.ps1 [OPTIONS] [-- ADDITIONAL_ARGS]

Options:
    -Profile <profile>    Test profile to run (quick|standard|full|nightly)
                         Default: standard
    -Sequential          Run tests sequentially instead of parallel
    -NoReports          Don't generate HTML/JSON reports
    -Python <path>      Path to Python executable
                         Default: python
    -Help               Show this help message

Examples:
    .\run_all_tests.ps1                        # Run standard tests
    .\run_all_tests.ps1 -Profile quick         # Run quick tests only
    .\run_all_tests.ps1 -Profile full          # Run full test suite
    .\run_all_tests.ps1 -Sequential            # Run tests sequentially
    .\run_all_tests.ps1 -- --chaos             # Pass additional args to test runner

"@
    exit 0
}

# Check prerequisites
function Test-Prerequisites {
    Write-Color "Checking prerequisites..." -Color Blue
    
    # Check Python
    try {
        $pythonVersion = & $Python --version 2>&1
        if ($pythonVersion -notmatch 'Python (\d+)\.(\d+)') {
            throw "Could not determine Python version"
        }
        $majorVersion = [int]$Matches[1]
        $minorVersion = [int]$Matches[2]
        
        if ($majorVersion -lt 3 -or ($majorVersion -eq 3 -and $minorVersion -lt 8)) {
            throw "Python 3.8 or higher is required (found $pythonVersion)"
        }
        Write-Color "  Python: $pythonVersion" -Color Green
    }
    catch {
        Write-Color "Error: Python is not installed or not in PATH" -Color Red
        Write-Color "  $_" -Color Red
        exit 1
    }
    
    # Check Rust and Cargo
    try {
        $cargoVersion = & cargo --version 2>&1
        Write-Color "  Cargo: $cargoVersion" -Color Green
    }
    catch {
        Write-Color "Error: Cargo is not installed or not in PATH" -Color Red
        exit 1
    }
    
    # Check for virtual environment
    $venvPath = Join-Path $ProjectRoot "venv"
    if (-not (Test-Path $venvPath)) {
        Write-Color "Virtual environment not found. Creating..." -Color Yellow
        & $Python -m venv $venvPath
        if ($LASTEXITCODE -ne 0) {
            Write-Color "Error: Failed to create virtual environment" -Color Red
            exit 1
        }
    }
    
    # Activate virtual environment
    $venvActivate = Join-Path $venvPath "Scripts\Activate.ps1"
    if (Test-Path $venvActivate) {
        & $venvActivate
    }
    else {
        Write-Color "Warning: Could not activate virtual environment" -Color Yellow
    }
    
    # Install Python dependencies
    $requirementsFile = Join-Path $ScriptDir "requirements.txt"
    if (Test-Path $requirementsFile) {
        Write-Color "Installing Python dependencies..." -Color Blue
        & pip install -q -r $requirementsFile
        if ($LASTEXITCODE -ne 0) {
            Write-Color "Warning: Some dependencies may not have installed correctly" -Color Yellow
        }
    }
    
    Write-Color "Prerequisites check completed" -Color Green
}

# Setup environment
function Set-TestEnvironment {
    Write-Color "Setting up environment..." -Color Blue
    
    # Rust environment
    $env:RUST_BACKTRACE = "1"
    if (-not $env:RUST_LOG) {
        $env:RUST_LOG = "info"
    }
    
    # GStreamer environment
    if (-not $env:GST_DEBUG) {
        $env:GST_DEBUG = "2"
    }
    
    # Test runner environment
    $env:TEST_RUNNER_ROOT = $ProjectRoot
    $env:TEST_REPORTS_DIR = Join-Path $ProjectRoot "test-reports"
    $env:TEST_LOGS_DIR = Join-Path $ProjectRoot "test-logs"
    
    # Create necessary directories
    New-Item -ItemType Directory -Force -Path $env:TEST_REPORTS_DIR | Out-Null
    New-Item -ItemType Directory -Force -Path $env:TEST_LOGS_DIR | Out-Null
    
    # Platform-specific settings
    $env:TEST_PLATFORM = "windows"
    $env:TEST_MAX_PARALLEL = (Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
    
    Write-Color "Environment setup completed" -Color Green
}

# Run tests
function Invoke-Tests {
    Write-Color "Starting test execution..." -Color Blue
    
    # Build command
    $testRunnerPath = Join-Path $ScriptDir "test_runner.py"
    $cmd = @($Python, $testRunnerPath)
    
    # Add test profile
    switch ($Profile) {
        'quick' {
            $cmd += '--unit', '--integration'
        }
        'standard' {
            $cmd += '--all'
        }
        'full' {
            $cmd += '--all', '--benchmarks'
        }
        'nightly' {
            $cmd += '--all', '--benchmarks', '--matrix'
        }
    }
    
    # Add parallel flag
    if (-not $Sequential) {
        $cmd += '--parallel'
    }
    
    # Add report generation
    if (-not $NoReports) {
        $cmd += '--json', '--html'
    }
    
    # Add additional arguments
    if ($AdditionalArgs) {
        $cmd += $AdditionalArgs
    }
    
    # Create log file
    $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
    $logFile = Join-Path $env:TEST_LOGS_DIR "test_run_$timestamp.log"
    
    Write-Color "Executing: $($cmd -join ' ')" -Color Green
    Write-Color "Log file: $logFile" -Color Blue
    
    # Execute tests with output capture
    try {
        # Use Start-Process for better output handling
        $process = Start-Process -FilePath $cmd[0] `
                                -ArgumentList $cmd[1..$cmd.Length] `
                                -WorkingDirectory $ProjectRoot `
                                -PassThru `
                                -NoNewWindow `
                                -RedirectStandardOutput "$logFile.stdout" `
                                -RedirectStandardError "$logFile.stderr"
        
        # Display output in real-time
        $outputJob = Start-Job -ScriptBlock {
            param($stdout, $stderr)
            
            # Monitor stdout
            if (Test-Path $stdout) {
                Get-Content $stdout -Wait -ErrorAction SilentlyContinue
            }
        } -ArgumentList "$logFile.stdout", "$logFile.stderr"
        
        # Wait for process to complete
        $process.WaitForExit()
        $exitCode = $process.ExitCode
        
        # Stop output monitoring
        Stop-Job $outputJob -ErrorAction SilentlyContinue
        Remove-Job $outputJob -ErrorAction SilentlyContinue
        
        # Combine stdout and stderr into single log file
        if (Test-Path "$logFile.stdout") {
            Get-Content "$logFile.stdout" | Out-File $logFile
            Remove-Item "$logFile.stdout" -ErrorAction SilentlyContinue
        }
        if (Test-Path "$logFile.stderr") {
            Get-Content "$logFile.stderr" | Out-File $logFile -Append
            Remove-Item "$logFile.stderr" -ErrorAction SilentlyContinue
        }
        
        if ($exitCode -eq 0) {
            Write-Color "Test execution completed successfully" -Color Green
        }
        else {
            Write-Color "Test execution failed with exit code $exitCode" -Color Red
        }
        
        return $exitCode
    }
    catch {
        Write-Color "Error executing tests: $_" -Color Red
        return 1
    }
}

# Main execution
function Main {
    # Show help if requested
    if ($Help) {
        Show-Usage
    }
    
    Write-Color "=========================================" -Color Blue
    Write-Color "       DSL-RS Test Runner" -Color Blue
    Write-Color "=========================================" -Color Blue
    
    # Check prerequisites
    Test-Prerequisites
    
    # Setup environment
    Set-TestEnvironment
    
    # Run tests
    $exitCode = Invoke-Tests
    
    Write-Color "=========================================" -Color Blue
    if ($exitCode -eq 0) {
        Write-Color "       All Tests Completed" -Color Green
    }
    else {
        Write-Color "       Tests Failed" -Color Red
    }
    Write-Color "=========================================" -Color Blue
    
    # Deactivate virtual environment if activated
    if (Get-Command deactivate -ErrorAction SilentlyContinue) {
        deactivate
    }
    
    exit $exitCode
}

# Run main function
Main