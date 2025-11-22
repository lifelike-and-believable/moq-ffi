#!/bin/bash
# Memory Leak Detection Test Script
# 
# This script runs the test suite with memory leak detection tools:
# - Valgrind (if available) for comprehensive leak detection
# - AddressSanitizer (ASAN) for fast runtime detection
#
# Usage:
#   ./test-memory-leaks.sh [valgrind|asan|all]
#
# Examples:
#   ./test-memory-leaks.sh          # Run all available tools
#   ./test-memory-leaks.sh valgrind # Run only valgrind
#   ./test-memory-leaks.sh asan     # Run only AddressSanitizer

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE_DIR="$PROJECT_ROOT/moq_ffi"

MODE="${1:-all}"

echo -e "${BLUE}=== MoQ FFI Memory Leak Detection ===${NC}"
echo "Project root: $PROJECT_ROOT"
echo "Crate directory: $CRATE_DIR"
echo "Mode: $MODE"
echo ""

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to run valgrind tests
run_valgrind_tests() {
    echo -e "${BLUE}=== Running Valgrind Tests ===${NC}"
    
    if ! command_exists valgrind; then
        echo -e "${YELLOW}WARNING: valgrind not found. Skipping valgrind tests.${NC}"
        echo "Install valgrind: sudo apt-get install valgrind (Ubuntu/Debian)"
        return 1
    fi
    
    echo "Building test suite..."
    cd "$CRATE_DIR"
    cargo test --no-run --features with_moq
    
    # Find the test binary
    TEST_BINARY=$(find target/debug/deps -maxdepth 1 -name "moq_ffi-*" -type f -executable | head -1)
    
    if [ -z "$TEST_BINARY" ]; then
        echo -e "${RED}ERROR: Could not find test binary${NC}"
        return 1
    fi
    
    echo "Running tests with valgrind: $TEST_BINARY"
    echo ""
    
    # Run valgrind with detailed leak checking
    # Note: We use --error-exitcode=1 to fail on leaks
    valgrind \
        --leak-check=full \
        --show-leak-kinds=all \
        --track-origins=yes \
        --verbose \
        --error-exitcode=1 \
        --suppressions="$SCRIPT_DIR/valgrind-suppressions.supp" 2>/dev/null || true \
        "$TEST_BINARY" \
        2>&1 | tee "$PROJECT_ROOT/valgrind-report.txt"
    
    # Check for leaks in the output
    if grep -q "ERROR SUMMARY: 0 errors" "$PROJECT_ROOT/valgrind-report.txt"; then
        echo -e "${GREEN}✓ Valgrind: No memory leaks detected${NC}"
        return 0
    else
        echo -e "${RED}✗ Valgrind: Memory leaks or errors detected${NC}"
        echo "See valgrind-report.txt for details"
        return 1
    fi
}

# Function to run AddressSanitizer tests
run_asan_tests() {
    echo -e "${BLUE}=== Running AddressSanitizer Tests ===${NC}"
    
    cd "$CRATE_DIR"
    
    # Check if we can build with ASAN
    if ! rustc --version | grep -q "nightly"; then
        echo -e "${YELLOW}WARNING: ASAN works best with nightly Rust${NC}"
    fi
    
    echo "Building with AddressSanitizer..."
    
    # Set ASAN flags
    export RUSTFLAGS="-Z sanitizer=address"
    export ASAN_OPTIONS="detect_leaks=1:halt_on_error=0:log_path=$PROJECT_ROOT/asan-report"
    
    # Build and run tests with ASAN
    # Note: Requires nightly Rust
    if rustc --version | grep -q "nightly"; then
        cargo +nightly test --features with_moq --target x86_64-unknown-linux-gnu \
            2>&1 | tee "$PROJECT_ROOT/asan-build.log"
        
        ASAN_RESULT=$?
        
        if [ $ASAN_RESULT -eq 0 ]; then
            # Check if any ASAN reports were generated
            if ls "$PROJECT_ROOT"/asan-report.* 1> /dev/null 2>&1; then
                echo -e "${RED}✗ AddressSanitizer: Issues detected${NC}"
                echo "See asan-report.* files for details"
                cat "$PROJECT_ROOT"/asan-report.*
                return 1
            else
                echo -e "${GREEN}✓ AddressSanitizer: No issues detected${NC}"
                return 0
            fi
        else
            echo -e "${RED}✗ AddressSanitizer: Build or test failed${NC}"
            return 1
        fi
    else
        # Fallback for stable Rust - use LeakSanitizer via LSAN
        echo "Attempting with LeakSanitizer (LSAN) on stable..."
        export RUSTFLAGS="-C link-arg=-fsanitize=leak"
        
        cargo test --features with_moq 2>&1 | tee "$PROJECT_ROOT/lsan-output.log" || true
        
        if grep -q "LeakSanitizer" "$PROJECT_ROOT/lsan-output.log"; then
            if grep -q "0 leaks" "$PROJECT_ROOT/lsan-output.log"; then
                echo -e "${GREEN}✓ LeakSanitizer: No leaks detected${NC}"
                return 0
            else
                echo -e "${RED}✗ LeakSanitizer: Leaks detected${NC}"
                return 1
            fi
        else
            echo -e "${YELLOW}WARNING: LeakSanitizer not available on this platform${NC}"
            return 1
        fi
    fi
}

# Function to run stub backend tests (lighter weight)
run_stub_tests() {
    echo -e "${BLUE}=== Running Stub Backend Tests ===${NC}"
    cd "$CRATE_DIR"
    cargo test
    echo -e "${GREEN}✓ Stub tests passed${NC}"
}

# Main execution
VALGRIND_RESULT=0
ASAN_RESULT=0

case "$MODE" in
    valgrind)
        run_valgrind_tests || VALGRIND_RESULT=$?
        ;;
    asan)
        run_asan_tests || ASAN_RESULT=$?
        ;;
    all)
        echo "Running stub backend tests first (faster)..."
        run_stub_tests
        echo ""
        
        run_valgrind_tests || VALGRIND_RESULT=$?
        echo ""
        
        run_asan_tests || ASAN_RESULT=$?
        ;;
    *)
        echo -e "${RED}ERROR: Invalid mode '$MODE'${NC}"
        echo "Usage: $0 [valgrind|asan|all]"
        exit 1
        ;;
esac

# Summary
echo ""
echo -e "${BLUE}=== Memory Leak Detection Summary ===${NC}"
if [ "$MODE" = "all" ] || [ "$MODE" = "valgrind" ]; then
    if [ $VALGRIND_RESULT -eq 0 ]; then
        echo -e "Valgrind: ${GREEN}PASSED${NC}"
    else
        echo -e "Valgrind: ${RED}FAILED${NC}"
    fi
fi

if [ "$MODE" = "all" ] || [ "$MODE" = "asan" ]; then
    if [ $ASAN_RESULT -eq 0 ]; then
        echo -e "AddressSanitizer: ${GREEN}PASSED${NC}"
    else
        echo -e "AddressSanitizer: ${RED}FAILED${NC}"
    fi
fi

# Exit with error if any tests failed
if [ $VALGRIND_RESULT -ne 0 ] || [ $ASAN_RESULT -ne 0 ]; then
    exit 1
fi

exit 0
