# MoQ FFI Crate

This crate provides a C ABI wrapper around the [moq-transport](https://crates.io/crates/moq-transport) library from [moq-rs](https://github.com/cloudflare/moq-rs).

## Building

**Stub Backend (no dependencies, for testing):**
```bash
cargo build --release
```

**Full Backend (with moq-transport):**
```bash
cargo build --release --features with_moq
```

## Outputs

- **Linux**: `target/release/libmoq_ffi.so` (shared), `target/release/libmoq_ffi.a` (static)
- **macOS**: `target/release/libmoq_ffi.dylib` (dynamic), `target/release/libmoq_ffi.a` (static)
- **Windows**: `target/release/moq_ffi.dll`, `target/release/moq_ffi.dll.lib` (import), `target/release/moq_ffi.pdb` (debug)

## C API

The C API is defined in `include/moq_ffi.h`. See the main repository README for usage examples.

## Testing

The library has three layers of testing to ensure production confidence. See [TEST_COVERAGE.md](TEST_COVERAGE.md) for detailed coverage information.

### Quick Reference

**Run unit tests (Rust, with Draft 07 backend):**
```bash
cargo test --features with_moq_draft07
```

**Run C/C++ tests:**
```bash
cd c_tests && mkdir build && cd build
cmake ..
cmake --build .
ctest --output-on-failure
```

**Run integration tests (requires network):**
```bash
cargo test --features with_moq_draft07 \
  --test cloudflare_relay_integration \
  -- --ignored --nocapture
```

### What Passing Tests Mean

| Test Layer | What It Verifies |
|------------|------------------|
| **Unit Tests** | FFI safety, null handling, panic protection, memory management |
| **C/C++ Tests** | C header compatibility, callback mechanisms, cross-language integration |
| **Integration Tests** | Real relay connectivity, protocol handshake, end-to-end pub/sub |

**If all CI tests pass, the API is safe to use in production.** The integration tests provide additional confidence that the protocol works end-to-end with real relays.

### Test Coverage Summary

- **81 Rust unit tests** covering all FFI functions
- **11 C/C++ test executables** covering API from C caller perspective
- **10 integration tests** verifying real relay connectivity (Cloudflare)

The test suite covers:
- Lifecycle management (create/destroy operations)
- Null pointer validation
- Error handling and error codes
- Panic protection at FFI boundaries
- Memory safety and resource cleanup
- Thread-local error storage
- Enum values and helper functions
- Real network pub/sub workflows

## Features

- `with_moq` - Enable full moq-transport integration (IETF Draft 14, default: disabled)
- `with_moq_draft07` - Enable Draft 07 integration (CloudFlare production relay, mutually exclusive with `with_moq`)

## License

MIT
