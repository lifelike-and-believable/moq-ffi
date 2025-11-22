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

The crate includes comprehensive unit tests covering all FFI functions:

**Run tests (stub backend):**
```bash
cargo test
```

**Run tests (full backend - Draft 14):**
```bash
cargo test --features with_moq
```

**Run tests (full backend - Draft 07):**
```bash
cargo test --features with_moq_draft07
```

**Code Coverage:**
```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Run coverage (stub backend)
cargo llvm-cov --lib

# Run coverage (full backend)
cargo llvm-cov --lib --features with_moq
```

**Coverage Results:**
- Stub backend: 93%+ line coverage (63 tests)
- Full backend: 69%+ line coverage (68 tests)
- Total: 131 unit tests

The test suite covers:
- Lifecycle management (create/destroy operations)
- Null pointer validation
- Error handling and error codes
- Panic protection at FFI boundaries
- Memory safety and resource cleanup
- Thread-local error storage
- Enum values and helper functions

## Features

- `with_moq` - Enable full moq-transport integration (IETF Draft 14, default: disabled)
- `with_moq_draft07` - Enable Draft 07 integration (CloudFlare production relay, mutually exclusive with `with_moq`)

## License

MIT
