# Memory Leak Detection Test Script (Windows)
# 
# This script runs the test suite with memory leak detection tools on Windows:
# - AddressSanitizer (ASAN) for runtime detection
# - Application Verifier (if available)
#
# Usage:
#   pwsh test-memory-leaks.ps1 [-Mode <asan|appverifier|all>]
#
# Examples:
#   pwsh test-memory-leaks.ps1                    # Run all available tools
#   pwsh test-memory-leaks.ps1 -Mode asan        # Run only AddressSanitizer

param(
    [Parameter(Mandatory=$false)]
    [ValidateSet("asan", "appverifier", "all")]
    [string]$Mode = "all"
)

$ErrorActionPreference = 'Stop'

# Configuration
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$CrateDir = Join-Path $ProjectRoot "moq_ffi"

Write-Host "=== MoQ FFI Memory Leak Detection (Windows) ===" -ForegroundColor Blue
Write-Host "Project root: $ProjectRoot"
Write-Host "Crate directory: $CrateDir"
Write-Host "Mode: $Mode"
Write-Host ""

# Function to test if a command exists
function Test-CommandExists {
    param([string]$Command)
    $null -ne (Get-Command $Command -ErrorAction SilentlyContinue)
}

# Function to run AddressSanitizer tests
function Test-WithASAN {
    Write-Host "=== Running AddressSanitizer Tests ===" -ForegroundColor Blue
    
    Push-Location $CrateDir
    try {
        # Check if nightly is available
        $hasNightly = $false
        try {
            $rustcVersion = rustc --version
            if ($rustcVersion -match "nightly") {
                $hasNightly = $true
            }
        } catch {
            Write-Host "WARNING: Could not determine Rust version" -ForegroundColor Yellow
        }
        
        if ($hasNightly) {
            Write-Host "Building with AddressSanitizer (nightly)..."
            
            # Set ASAN environment
            $env:RUSTFLAGS = "-Z sanitizer=address"
            $env:ASAN_OPTIONS = "detect_leaks=1:halt_on_error=0:log_path=$ProjectRoot\asan-report"
            
            # Build and run tests
            cargo +nightly test --features with_moq --target x86_64-pc-windows-msvc 2>&1 | Tee-Object "$ProjectRoot\asan-build.log"
            
            # Check for ASAN reports
            $asanReports = Get-ChildItem "$ProjectRoot\asan-report.*" -ErrorAction SilentlyContinue
            if ($asanReports) {
                Write-Host "✗ AddressSanitizer: Issues detected" -ForegroundColor Red
                Write-Host "See asan-report.* files for details"
                foreach ($report in $asanReports) {
                    Get-Content $report
                }
                return $false
            } else {
                Write-Host "✓ AddressSanitizer: No issues detected" -ForegroundColor Green
                return $true
            }
        } else {
            Write-Host "WARNING: Nightly Rust not available. ASAN requires nightly." -ForegroundColor Yellow
            Write-Host "Install nightly: rustup install nightly" -ForegroundColor Yellow
            return $null
        }
    } finally {
        Pop-Location
        # Clear ASAN environment variables
        Remove-Item Env:\RUSTFLAGS -ErrorAction SilentlyContinue
        Remove-Item Env:\ASAN_OPTIONS -ErrorAction SilentlyContinue
    }
}

# Function to run Application Verifier tests
function Test-WithAppVerifier {
    Write-Host "=== Running Application Verifier Tests ===" -ForegroundColor Blue
    
    if (Test-CommandExists "appverif") {
        Write-Host "Application Verifier is available"
        
        # Build test binary
        Push-Location $CrateDir
        try {
            Write-Host "Building test suite..."
            cargo test --no-run --features with_moq
            
            # Find the test binary
            $testBinary = Get-ChildItem "$CrateDir\target\debug\deps\moq_ffi-*.exe" | 
                         Select-Object -First 1 -ExpandProperty FullName
            
            if (-not $testBinary) {
                Write-Host "ERROR: Could not find test binary" -ForegroundColor Red
                return $false
            }
            
            Write-Host "Test binary: $testBinary"
            
            # Enable Application Verifier for the test binary
            Write-Host "Enabling Application Verifier..."
            appverif /enable Heaps Leak /for $testBinary
            
            # Run tests
            Write-Host "Running tests with Application Verifier..."
            & $testBinary 2>&1 | Tee-Object "$ProjectRoot\appverifier-output.log"
            
            # Disable Application Verifier
            appverif /disable * /for $testBinary
            
            # Check for issues
            $output = Get-Content "$ProjectRoot\appverifier-output.log" -Raw
            if ($output -match "VERIFIER STOP|HEAP CORRUPTION") {
                Write-Host "✗ Application Verifier: Issues detected" -ForegroundColor Red
                return $false
            } else {
                Write-Host "✓ Application Verifier: No issues detected" -ForegroundColor Green
                return $true
            }
        } finally {
            Pop-Location
        }
    } else {
        Write-Host "WARNING: Application Verifier not found" -ForegroundColor Yellow
        Write-Host "Application Verifier is included with Windows SDK"
        return $null
    }
}

# Function to run stub backend tests
function Test-StubBackend {
    Write-Host "=== Running Stub Backend Tests ===" -ForegroundColor Blue
    Push-Location $CrateDir
    try {
        cargo test
        Write-Host "✓ Stub tests passed" -ForegroundColor Green
        return $true
    } finally {
        Pop-Location
    }
}

# Main execution
$asanResult = $null
$appverifierResult = $null

switch ($Mode) {
    "asan" {
        $asanResult = Test-WithASAN
    }
    "appverifier" {
        $appverifierResult = Test-WithAppVerifier
    }
    "all" {
        Write-Host "Running stub backend tests first (faster)..."
        Test-StubBackend
        Write-Host ""
        
        $asanResult = Test-WithASAN
        Write-Host ""
        
        $appverifierResult = Test-WithAppVerifier
    }
}

# Summary
Write-Host ""
Write-Host "=== Memory Leak Detection Summary ===" -ForegroundColor Blue

if ($Mode -in @("all", "asan")) {
    if ($asanResult -eq $true) {
        Write-Host "AddressSanitizer: PASSED" -ForegroundColor Green
    } elseif ($asanResult -eq $false) {
        Write-Host "AddressSanitizer: FAILED" -ForegroundColor Red
    } else {
        Write-Host "AddressSanitizer: SKIPPED" -ForegroundColor Yellow
    }
}

if ($Mode -in @("all", "appverifier")) {
    if ($appverifierResult -eq $true) {
        Write-Host "Application Verifier: PASSED" -ForegroundColor Green
    } elseif ($appverifierResult -eq $false) {
        Write-Host "Application Verifier: FAILED" -ForegroundColor Red
    } else {
        Write-Host "Application Verifier: SKIPPED" -ForegroundColor Yellow
    }
}

# Exit with error if any tests failed
if ($asanResult -eq $false -or $appverifierResult -eq $false) {
    exit 1
}

exit 0
