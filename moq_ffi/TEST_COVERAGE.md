# MoQ FFI Test Coverage Guide

## Overview

This document describes the test coverage for the MoQ FFI library and explains what passing tests mean for production confidence.

## Test Layers

The MoQ FFI library has three layers of testing:

### 1. Unit Tests (Rust)
**Location:** `moq_ffi/src/backend_moq.rs` and `moq_ffi/src/backend_stub.rs`  
**Run:** `cargo test --features with_moq_draft07`

These tests verify:
- ✅ FFI function signatures and ABI compatibility
- ✅ Null pointer handling and safety
- ✅ Panic protection at FFI boundary
- ✅ Error code consistency
- ✅ Memory management (allocation/deallocation)
- ✅ Thread-local error storage
- ✅ Enum value consistency with C header

**Production Confidence:** Unit tests ensure the FFI layer is **safe and correct** but do **not** verify network connectivity or protocol behavior.

### 2. C/C++ Tests
**Location:** `moq_ffi/c_tests/src/`  
**Run:** `cd moq_ffi/c_tests/build && ctest`

These tests verify:
- ✅ C header compatibility
- ✅ All API functions are callable from C/C++
- ✅ Callback mechanisms work correctly
- ✅ Memory safety from C caller perspective
- ✅ Error handling across FFI boundary
- ✅ Struct layout matches between Rust and C

**Production Confidence:** C tests ensure the library can be **integrated into C/C++ applications** correctly.

### 3. Integration Tests (Cloudflare Relay)
**Location:** `moq_ffi/tests/cloudflare_relay_integration.rs`  
**Run:** `cargo test --features with_moq_draft07 --test cloudflare_relay_integration -- --ignored --nocapture`

These tests verify:
- ✅ Real network connectivity to Cloudflare MoQ relay
- ✅ WebTransport/QUIC protocol handshake
- ✅ MoQ session establishment (Draft 07)
- ✅ Namespace announcement workflow
- ✅ Publisher creation and data publishing
- ✅ Subscriber creation and data reception
- ✅ Pub/Sub roundtrip data verification
- ✅ Multiple client handling
- ✅ Datagram and Stream delivery modes

**Production Confidence:** Integration tests verify the **end-to-end workflow** works with a real relay server.

## API Coverage Matrix

| Function | Unit Tests | C Tests | Integration Tests | Production Ready |
|----------|:----------:|:-------:|:-----------------:|:----------------:|
| `moq_init()` | ✅ | ✅ | ✅ | ✅ |
| `moq_client_create()` | ✅ | ✅ | ✅ | ✅ |
| `moq_client_destroy()` | ✅ | ✅ | ✅ | ✅ |
| `moq_connect()` | ✅ | ✅ | ✅ | ✅ |
| `moq_disconnect()` | ✅ | ✅ | ✅ | ✅ |
| `moq_is_connected()` | ✅ | ✅ | ✅ | ✅ |
| `moq_announce_namespace()` | ✅ | ✅ | ✅ | ✅ |
| `moq_create_publisher()` | ✅ | ✅ | ✅ | ✅ |
| `moq_create_publisher_ex()` | ✅ | ✅ | ✅ | ✅ |
| `moq_publisher_destroy()` | ✅ | ✅ | ✅ | ✅ |
| `moq_publish_data()` | ✅ | ✅ | ✅ | ✅ |
| `moq_subscribe()` | ✅ | ✅ | ✅ | ✅ |
| `moq_subscriber_destroy()` | ✅ | ✅ | ✅ | ✅ |
| `moq_unsubscribe()` | ✅ | ✅ | ⚠️ | ⚠️ |
| `moq_is_subscribed()` | ✅ | ✅ | ⚠️ | ⚠️ |
| `moq_subscribe_announces()` | ✅ | ✅ | ❌* | ⚠️* |
| `moq_subscribe_catalog()` | ⚠️ | ✅ | ❌ | ⚠️ |
| `moq_free_str()` | ✅ | ✅ | ✅ | ✅ |
| `moq_version()` | ✅ | ✅ | ✅ | ✅ |
| `moq_last_error()` | ✅ | ✅ | ✅ | ✅ |

**Legend:**
- ✅ Fully tested
- ⚠️ Partially tested (unit tests only, or relay limitation)
- ❌ Not tested at this layer
- (*) Relay limitation: Cloudflare relay does not forward announcements (documented limitation)

## What "All Tests Pass" Means

### When CI Passes (Linux/Windows/macOS)

If the CI workflow passes, you can be confident that:

1. **The library builds correctly** on all platforms (Linux, Windows MSVC, macOS)
2. **The C ABI is stable** and matches the header file
3. **All FFI functions are safe** (no panics escape to C)
4. **Null pointers are handled safely** without crashing
5. **Memory management is correct** (no leaks in basic scenarios)
6. **C/C++ integration works** on all platforms

### When Integration Tests Pass

If integration tests pass (run manually or in the `cloudflare-integration-tests` job), you can be confident that:

1. **Real relay connectivity works** with Cloudflare's production MoQ relay
2. **The MoQ protocol handshake succeeds** (Draft 07)
3. **Publishing data works** end-to-end
4. **Subscribing and receiving data works** end-to-end
5. **Multiple clients can operate simultaneously**
6. **Both datagram and stream delivery modes work**

## Recommended Testing Strategy

### Before Deploying to Production

1. **Verify CI passes** - This ensures basic correctness
2. **Run integration tests manually** - This verifies relay connectivity:
   ```bash
   cd moq_ffi
   cargo test --features with_moq_draft07 \
     --test cloudflare_relay_integration \
     -- --ignored --nocapture
   ```
3. **Test your specific use case** - The integration tests cover common patterns, but you should test your specific workflow

### Known Limitations

1. **`moq_subscribe_announces()`** - Works correctly but Cloudflare relay does not forward announcements (protocol limitation, not a bug)

2. **`moq_subscribe_catalog()`** - The function works correctly but requires a publisher to be actively publishing catalog data in the expected format

3. **Datagram delivery** - Uses "latest value" semantics, not a queue. High-frequency writes may overwrite before delivery (by design for real-time sensor data)

## CI Workflow Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                         build-ffi.yml                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  windows-msvc   │  │   linux-gnu     │  │ macos-universal │ │
│  │                 │  │                 │  │                 │ │
│  │ • cargo build   │  │ • cargo build   │  │ • cargo build   │ │
│  │ • cargo test    │  │ • cargo test    │  │   (x86+arm64)   │ │
│  │ • package SDK   │  │ • package SDK   │  │ • package SDK   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │ c-cpp-tests     │  │ c-cpp-tests     │  │ c-cpp-tests     │ │
│  │ (Linux)         │  │ (Windows)       │  │ (macOS)         │ │
│  │                 │  │                 │  │                 │ │
│  │ • cmake build   │  │ • cmake build   │  │ • cmake build   │ │
│  │ • ctest run     │  │ • ctest run     │  │ • ctest run     │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
│  ┌─────────────────────────────────────┐  ┌─────────────────┐  │
│  │   cloudflare-integration-tests     │  │ memory-leak-    │  │
│  │   (continue-on-error: true)        │  │ tests (Linux)   │  │
│  │                                     │  │                 │  │
│  │ • Real relay connectivity          │  │ • Valgrind      │  │
│  │ • Pub/Sub roundtrip                │  │ • ASAN          │  │
│  │ • Network-dependent                │  │ • Leak check    │  │
│  └─────────────────────────────────────┘  └─────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Summary

**If all CI tests pass, the API is safe to use.** The integration tests provide additional confidence that the protocol works end-to-end with real relays.

For production deployment:
1. Ensure CI passes ✅
2. Run integration tests against your target relay ✅
3. Test your specific use case ✅

The MoQ FFI library follows FFI best practices:
- All FFI functions catch panics
- All pointers are validated before use
- Memory ownership is clearly documented
- Thread safety is ensured through proper synchronization
