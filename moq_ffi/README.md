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

## Features

- `with_moq` - Enable full moq-transport integration (default: disabled)

## License

MIT
