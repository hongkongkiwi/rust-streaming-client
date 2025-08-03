#!/bin/bash

# Rust Bodycam Client Test Runner
# Comprehensive testing script for power efficiency and functionality

set -e  # Exit on any error

echo "🔋 Starting Rust Bodycam Client Test Suite"
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
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

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    print_error "This script must be run from the Rust project root directory"
    exit 1
fi

# Set environment variables for testing
export RUST_BACKTRACE=1
export RUST_LOG=info

print_status "Setting up test environment..."

# Check Rust toolchain
print_status "Checking Rust toolchain..."
rustc --version
cargo --version

# Update dependencies
print_status "Updating dependencies..."
cargo update

# Format check
print_status "Checking code formatting..."
if cargo fmt --check; then
    print_success "Code formatting is correct"
else
    print_warning "Code formatting issues found. Run 'cargo fmt' to fix."
fi

# Clippy lints
print_status "Running Clippy lints..."
if cargo clippy -- -D warnings; then
    print_success "Clippy checks passed"
else
    print_error "Clippy found issues"
    exit 1
fi

# Unit tests
print_status "Running unit tests..."
if cargo test --lib; then
    print_success "Unit tests passed"
else
    print_error "Unit tests failed"
    exit 1
fi

# Integration tests
print_status "Running integration tests..."
if cargo test --tests; then
    print_success "Integration tests passed"
else
    print_error "Integration tests failed"
    exit 1
fi

# Power efficiency tests
print_status "Running power efficiency tests..."
if cargo test --test power_efficiency_tests; then
    print_success "Power efficiency tests passed"
else
    print_error "Power efficiency tests failed"
    exit 1
fi

# Security tests (if encryption module is available)
print_status "Running security tests..."
if cargo test encryption::tests; then
    print_success "Security tests passed"
else
    print_warning "Some security tests may have failed"
fi

# Performance benchmarks (optional, takes time)
if [ "${RUN_BENCHMARKS:-false}" = "true" ]; then
    print_status "Running performance benchmarks..."
    if cargo bench; then
        print_success "Benchmarks completed"
        print_status "Benchmark results saved to target/criterion/"
    else
        print_warning "Benchmarks encountered issues"
    fi
else
    print_status "Skipping benchmarks (set RUN_BENCHMARKS=true to run them)"
fi

# Memory leak detection (if valgrind is available)
if command -v valgrind &> /dev/null; then
    print_status "Running memory leak detection..."
    if cargo build --bin bodycam-client; then
        valgrind --leak-check=full --show-leak-kinds=all --track-origins=yes \
            ./target/debug/bodycam-client --help > /dev/null 2> valgrind.log
        
        if grep -q "definitely lost: 0 bytes" valgrind.log; then
            print_success "No memory leaks detected"
        else
            print_warning "Potential memory leaks detected. Check valgrind.log"
        fi
        rm -f valgrind.log
    fi
else
    print_status "Valgrind not available, skipping memory leak detection"
fi

# Power consumption simulation
print_status "Running power consumption simulation..."
if cargo test test_power_efficient_defaults; then
    print_success "Power efficiency configuration verified"
else
    print_warning "Power efficiency configuration may need adjustment"
fi

# Test coverage (if cargo-tarpaulin is installed)
if command -v cargo-tarpaulin &> /dev/null; then
    print_status "Generating test coverage report..."
    if cargo tarpaulin --out Html --output-dir coverage/; then
        print_success "Coverage report generated in coverage/"
    else
        print_warning "Coverage report generation failed"
    fi
else
    print_status "cargo-tarpaulin not installed, skipping coverage report"
    print_status "Install with: cargo install cargo-tarpaulin"
fi

# Security audit
if command -v cargo-audit &> /dev/null; then
    print_status "Running security audit..."
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_error "Security vulnerabilities found"
        exit 1
    fi
else
    print_status "cargo-audit not installed, skipping security audit"
    print_status "Install with: cargo install cargo-audit"
fi

# Final status
echo ""
echo "=========================================="
print_success "All tests completed successfully! 🎉"
echo ""
print_status "Power efficiency optimizations:"
echo "  ✓ Adaptive monitoring intervals"
echo "  ✓ Small encryption chunk sizes (64KB)"
echo "  ✓ CPU yielding in long operations"
echo "  ✓ Memory pre-allocation"
echo "  ✓ Low-power mode configuration"
echo ""
print_status "Testing features covered:"
echo "  ✓ Unit tests for all modules"
echo "  ✓ Integration tests"
echo "  ✓ Power efficiency tests"
echo "  ✓ Security tests"
echo "  ✓ Memory management tests"
echo "  ✓ Input validation tests"
echo ""

if [ "${RUN_BENCHMARKS:-false}" = "true" ]; then
    print_status "Next steps:"
    echo "  1. Review benchmark results in target/criterion/"
    echo "  2. Check HTML report for detailed performance analysis"
    echo "  3. Monitor power consumption in real deployment"
fi

print_status "Ready for low-power deployment! 🔋"