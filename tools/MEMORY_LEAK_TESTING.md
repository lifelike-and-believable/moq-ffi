# Memory Leak Testing Guide

This guide describes how to test for memory leaks in the moq-ffi library using various tools.

## Overview

Memory leak testing is critical for FFI libraries as they bridge the Rust memory safety model with C's manual memory management. The moq-ffi library includes tools to detect:

- Memory leaks (allocated memory that is never freed)
- Use-after-free errors
- Buffer overflows
- Double frees
- Invalid pointer dereferences

## Quick Start

### Linux/macOS

```bash
# Run all available tools
./tools/test-memory-leaks.sh

# Run only valgrind
./tools/test-memory-leaks.sh valgrind

# Run only AddressSanitizer
./tools/test-memory-leaks.sh asan
```

### Windows

```powershell
# Run all available tools
pwsh tools/test-memory-leaks.ps1

# Run only AddressSanitizer
pwsh tools/test-memory-leaks.ps1 -Mode asan

# Run only Application Verifier
pwsh tools/test-memory-leaks.ps1 -Mode appverifier
```

## Tools

### Valgrind (Linux/macOS)

Valgrind is the most comprehensive memory leak detection tool. It provides:

- Detailed leak reports with stack traces
- Detection of invalid memory access
- Detection of use-after-free
- Thread error detection

**Installation:**
```bash
# Ubuntu/Debian
sudo apt-get install valgrind

# macOS
brew install valgrind
```

**Manual usage:**
```bash
cd moq_ffi
cargo test --no-run --features with_moq
TEST_BINARY=$(find target/debug/deps -name "moq_ffi-*" -type f -executable | head -1)
valgrind --leak-check=full --show-leak-kinds=all "$TEST_BINARY"
```

**Known limitations:**
- Slower than ASAN (10-50x slower)
- May report false positives from Rust runtime and async tasks
- Use `tools/valgrind-suppressions.supp` to filter known false positives

### AddressSanitizer (All platforms)

AddressSanitizer (ASAN) is a fast memory error detector built into LLVM/Clang:

- Fast runtime (2x slower than normal)
- Detects use-after-free, buffer overflows, stack overflows
- Detects memory leaks (with LeakSanitizer)
- Best support on nightly Rust

**Requirements:**
```bash
# Install nightly Rust
rustup install nightly
```

**Manual usage:**
```bash
cd moq_ffi
export RUSTFLAGS="-Z sanitizer=address"
export ASAN_OPTIONS="detect_leaks=1"
cargo +nightly test --features with_moq --target x86_64-unknown-linux-gnu
```

**Known limitations:**
- Requires nightly Rust
- May have false positives with async runtimes
- Not all platforms support leak detection

### Application Verifier (Windows only)

Application Verifier is Microsoft's runtime verification tool:

- Heap corruption detection
- Handle leaks
- Lock verification
- Part of Windows SDK

**Installation:**
- Included with Windows SDK
- Available in Visual Studio installer

**Manual usage:**
```powershell
cd moq_ffi
cargo test --no-run --features with_moq
$binary = (Get-ChildItem "target\debug\deps\moq_ffi-*.exe" | Select-Object -First 1).FullName
appverif /enable Heaps Leak /for $binary
& $binary
appverif /disable * /for $binary
```

## Interpreting Results

### No Leaks Found

```
✓ Valgrind: No memory leaks detected
✓ AddressSanitizer: No issues detected
```

This means all allocated memory was properly freed.

### Leaks Detected

Example valgrind output:
```
==12345== LEAK SUMMARY:
==12345==    definitely lost: 40 bytes in 1 blocks
==12345==    indirectly lost: 200 bytes in 5 blocks
==12345==      possibly lost: 0 bytes in 0 blocks
```

**definitely lost**: Real memory leaks - these MUST be fixed
**indirectly lost**: Memory lost because parent was leaked
**possibly lost**: May or may not be a leak (often false positives)

### Common Issues

#### 1. Leaked Error Messages

**Symptom:** Memory leaks in functions that return `MoqResult` with error messages

**Fix:** Always call `moq_free_str()` on error messages:
```c
MoqResult result = moq_connect(client, url, NULL, NULL);
if (result.code != MOQ_OK) {
    printf("Error: %s\n", result.message);
    moq_free_str(result.message);  // Important!
}
```

#### 2. Leaked Client/Publisher/Subscriber

**Symptom:** Leaks in client lifecycle tests

**Fix:** Always call destroy functions:
```c
MoqClient* client = moq_client_create();
// ... use client ...
moq_client_destroy(client);  // Always call this
```

#### 3. Async Runtime Leaks

**Symptom:** Leaks reported in `tokio::runtime` or background tasks

**Analysis:** These are often false positives from long-lived async tasks. Check if:
- The session task is properly cleaned up in `moq_disconnect`
- Subscriber reader tasks are cancelled in `moq_subscriber_destroy`

#### 4. Rust Standard Library Leaks

**Symptom:** Leaks in `std::thread`, `std::sync`, or `once_cell`

**Analysis:** Usually false positives from static initialization. Add suppressions if confirmed benign.

## CI Integration

Memory leak testing should be run in CI to catch regressions. The tests are configured to:

1. Run on pull requests
2. Run on main branch pushes
3. Upload reports as artifacts
4. Fail the build if leaks are detected

### GitHub Actions Configuration

```yaml
jobs:
  memory-leak-test-linux:
    name: Memory Leak Tests (Linux)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install valgrind
        run: sudo apt-get install -y valgrind
      
      - name: Run memory leak tests
        run: ./tools/test-memory-leaks.sh
      
      - name: Upload reports
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: memory-leak-reports-linux
          path: |
            valgrind-report.txt
            asan-report.*
```

## Best Practices

1. **Run locally before committing**: Catch leaks early
2. **Test both stub and full builds**: Different code paths may have different issues
3. **Test error paths**: Most leaks occur in error handling
4. **Document suppressions**: If adding suppressions, explain why
5. **Update tests**: Add tests that exercise new code paths

## Troubleshooting

### Valgrind is too slow

- Run ASAN instead for quick checks
- Use valgrind only before major releases
- Run valgrind on specific test modules: `cargo test --features with_moq test_name`

### Too many false positives

- Update `valgrind-suppressions.supp` with patterns
- Check if leaks are from static initialization (usually safe)
- Verify async runtime cleanup is working

### ASAN not available

- Install nightly Rust: `rustup install nightly`
- Check platform support: ASAN works best on Linux
- Use platform-specific tools (Application Verifier on Windows)

### Tests fail in CI but pass locally

- Check platform differences (Linux vs Windows)
- Verify CI has required tools installed
- Check for race conditions in async code

## References

- [Valgrind User Manual](https://valgrind.org/docs/manual/manual.html)
- [AddressSanitizer Documentation](https://clang.llvm.org/docs/AddressSanitizer.html)
- [Rust FFI Guidelines](https://doc.rust-lang.org/nomicon/ffi.html)
- [Application Verifier](https://docs.microsoft.com/en-us/windows-hardware/drivers/debugger/application-verifier)

## Support

If you encounter issues with memory leak testing:

1. Check existing GitHub issues
2. Run with verbose logging: `RUST_LOG=debug ./tools/test-memory-leaks.sh`
3. Create a minimal reproduction case
4. Open an issue with tool versions and full output
