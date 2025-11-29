# MoQ FFI C/C++ Test Suite

This directory contains a comprehensive test suite for the MoQ FFI C/C++ wrapper API.

## Overview

The test suite is organized into two main categories:

### Unit Tests (C)
These tests verify individual FFI functions in isolation, testing for:
- Null pointer safety
- Invalid argument handling
- Error code correctness
- Memory safety
- Resource cleanup
- Edge cases

**Unit test files:**
- `test_initialization.c` - Initialization and version APIs
- `test_lifecycle.c` - Client/publisher/subscriber lifecycle management
- `test_connection.c` - Connection/disconnection functionality
- `test_publishing.c` - Publishing APIs and namespace announcements
- `test_subscribing.c` - Subscription APIs and data callbacks
- `test_track_discovery.c` - Catalog and track announcement APIs
- `test_error_handling.c` - Error handling and recovery
- `test_memory_safety.c` - Memory management and safety

### Integration Tests (C++)
These tests demonstrate complete workflows and serve as usage examples:
- Full publish/subscribe workflows
- Multiple packet transmission (text and binary data)
- Data integrity verification
- Catalog-based track discovery
- Multiple concurrent clients
- Cross-client communication

**Integration test files:**
- `test_pubsub_integration.cpp` - Publisher-subscriber workflows with data verification
- `test_catalog_integration.cpp` - Catalog subscription and track discovery
- `test_multi_client_integration.cpp` - Multiple concurrent clients and cross-client pub/sub

## Building the Tests

### Prerequisites

1. **Build the MoQ FFI library first:**
   ```bash
   cd ../  # Go to moq_ffi directory
   cargo build --release --features with_moq_draft07
   ```

2. **Install CMake** (version 3.15 or later)

3. **Install a C/C++ compiler:**
   - Linux: GCC or Clang
   - macOS: Xcode Command Line Tools
   - Windows: MSVC (Visual Studio 2019+)

### Build Steps

```bash
# From the c_tests directory
mkdir build
cd build

# Configure
cmake ..

# Build
cmake --build .

# Or with specific build type
cmake --build . --config Release
```

### Custom Library Paths

If your MoQ FFI library is in a non-standard location:

```bash
cmake .. \
  -DMOQFFI_LIB_DIR=/path/to/moq_ffi/target/release \
  -DMOQFFI_INCLUDE_DIR=/path/to/moq_ffi/include
```

## Running the Tests

### Run All Tests

```bash
# From the build directory
ctest --output-on-failure
```

### Run Specific Test

```bash
# Unit tests
./test_initialization
./test_lifecycle
./test_connection
./test_publishing
./test_subscribing
./test_track_discovery
./test_error_handling
./test_memory_safety

# Integration tests
./test_pubsub_integration
./test_catalog_integration
./test_multi_client_integration
```

### Run with Verbose Output

```bash
ctest --output-on-failure --verbose
```

## Test Categories

### Unit Tests (No Network Required)

Most unit tests can run without network access. They test:
- API contracts and null pointer handling
- Error code correctness
- Memory safety and cleanup
- Enum values and constants

Some connection tests will attempt to connect to the Cloudflare relay but will pass even if the connection fails.

### Integration Tests (Network Required)

Integration tests require access to the Cloudflare MoQ relay:
- **Relay URL:** `https://relay.cloudflare.mediaoverquic.com`
- These tests demonstrate real-world usage patterns
- They verify end-to-end functionality
- Tests are designed to be robust to relay unavailability

## Expected Test Output

### Successful Test Run

```
[PASS] moq_init() should succeed
[PASS] First moq_init() should succeed (idempotent)
[PASS] moq_version() should return non-null string
...
========== TEST SUMMARY ==========
Total:  25
Passed: 25
Failed: 0
==================================
```

### Failed Test Run

```
[PASS] moq_init() should succeed
[FAIL] test.c:42: Client should be created (got NULL)
...
========== TEST SUMMARY ==========
Total:  25
Passed: 24
Failed: 1
==================================
```

## Integration Test Examples

### Example 1: Basic Pub/Sub

```cpp
// From test_pubsub_integration.cpp
// 1. Connect to relay
moq_connect(client, CLOUDFLARE_RELAY_URL, connection_callback, &ctx);

// 2. Announce namespace
moq_announce_namespace(client, "my-namespace");

// 3. Create publisher
MoqPublisher* pub = moq_create_publisher_ex(
    client, "my-namespace", "my-track", MOQ_DELIVERY_STREAM);

// 4. Subscribe on different client
MoqSubscriber* sub = moq_subscribe(
    sub_client, "my-namespace", "my-track", data_callback, &user_data);

// 5. Publish data
const char* message = "Hello, MoQ!";
moq_publish_data(pub, (uint8_t*)message, strlen(message), MOQ_DELIVERY_STREAM);
```

### Example 2: Binary Data Transfer

```cpp
// Create binary data
std::vector<uint8_t> binary_data(256);
for (int i = 0; i < 256; i++) {
    binary_data[i] = (uint8_t)i;
}

// Publish binary data
moq_publish_data(publisher, binary_data.data(), binary_data.size(),
                 MOQ_DELIVERY_DATAGRAM);
```

### Example 3: Catalog Discovery

```cpp
// Subscribe to catalog
MoqSubscriber* catalog_sub = moq_subscribe_catalog(
    client, "namespace", "catalog", catalog_callback, &catalog_ctx);

// Catalog callback receives track information
void catalog_callback(void* user_data, const MoqTrackInfo* tracks, size_t count) {
    for (size_t i = 0; i < count; i++) {
        printf("Track: %s, Codec: %s, Resolution: %dx%d\n",
               tracks[i].name, tracks[i].codec,
               tracks[i].width, tracks[i].height);
    }
}
```

## Memory Leak Testing

The test suite is designed to be run with memory leak detection tools:

### Valgrind (Linux/macOS)

```bash
valgrind --leak-check=full --show-leak-kinds=all ./test_initialization
```

### AddressSanitizer (All platforms)

```bash
# Rebuild with ASAN
cd ..
cargo clean
RUSTFLAGS="-Z sanitizer=address" cargo build --release --features with_moq_draft07 -Z build-std --target x86_64-unknown-linux-gnu

cd c_tests/build
cmake .. -DCMAKE_C_FLAGS="-fsanitize=address" -DCMAKE_CXX_FLAGS="-fsanitize=address"
cmake --build .
./test_initialization
```

## CI Integration

These tests are integrated into the GitHub Actions CI workflow. See `.github/workflows/build-ffi.yml`.

The CI runs:
1. All unit tests on Linux, macOS, and Windows
2. Integration tests (with network access)
3. Memory leak detection with Valgrind/ASAN

## Test Framework

The tests use a simple custom test framework defined in `include/test_framework.h`:

```c
TEST_INIT();                                    // Initialize test statistics
TEST_ASSERT(condition, "message");              // Assert condition
TEST_ASSERT_EQ(actual, expected, "message");    // Assert equality
TEST_ASSERT_NOT_NULL(ptr, "message");           // Assert non-null
TEST_ASSERT_STR_EQ(str1, str2, "message");      // Assert string equality
TEST_ASSERT_MEM_EQ(mem1, mem2, size, "msg");    // Assert memory equality
TEST_SUMMARY();                                 // Print test summary
TEST_EXIT();                                    // Exit with pass/fail code
```

## Coverage

The test suite provides comprehensive coverage:

- **22 FFI functions** fully tested
- **4 callback types** tested with various user data scenarios
- **8 result codes** verified
- **Memory safety** extensively tested (null pointers, large buffers, cleanup)
- **Error handling** tested across all APIs
- **Real-world workflows** demonstrated in integration tests

## Troubleshooting

### Tests fail to build

- Ensure MoQ FFI library is built first: `cargo build --release --features with_moq_draft07`
- Check CMake version: `cmake --version` (need 3.15+)
- Verify library path is correct

### Integration tests timeout

- Check network connectivity
- Verify Cloudflare relay is accessible: `curl https://relay.cloudflare.mediaoverquic.com`
- Tests will gracefully handle unavailability

### Memory leak warnings

- Some Rust/system library allocations may appear as "still reachable"
- Focus on "definitely lost" leaks in Valgrind output
- Use provided suppression file: `valgrind --suppressions=../tools/valgrind-suppressions.supp`

## Contributing

When adding new tests:

1. Follow the existing naming convention: `test_<category>.c` or `test_<workflow>_integration.cpp`
2. Use the test framework macros consistently
3. Add new test executables to `CMakeLists.txt`
4. Document the test purpose in a comment block
5. Ensure tests clean up all resources (no leaks)
6. Make network-dependent tests robust to failures

## License

Same license as the parent MoQ FFI project.
