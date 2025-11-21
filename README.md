# MoQ FFI - C++ Wrapper for moq-rs (Media over QUIC)

This repository provides:
- A **Rust FFI library** that exposes a C ABI (`moq_ffi.h`) for the [moq-rs](https://github.com/cloudflare/moq-rs) `moq-transport` implementation
- **Cross-platform build workflows** for Windows (Win64), Linux, and macOS
- **Release artifacts** ready for integration as third-party libraries in Unreal Engine plugins and other C++ projects

## 🚀 Quick Start

### Building the FFI Library

The library supports two build modes:

**1. Full Build (with MoQ Transport) - Recommended**
```bash
cd moq_ffi
cargo build --release --features with_moq
```

**2. Stub Build (for testing build toolchain)**
```bash
cd moq_ffi
cargo build --release
```

**Build Artifacts (Windows MSVC):**
- DLL: `target/release/moq_ffi.dll`
- Import lib: `target/release/moq_ffi.dll.lib`
- PDB: `target/release/moq_ffi.pdb`
- Header: `include/moq_ffi.h`

**Build Artifacts (Linux):**
- Shared lib: `target/release/libmoq_ffi.so`
- Static lib: `target/release/libmoq_ffi.a`
- Header: `include/moq_ffi.h`

**Build Artifacts (macOS):**
- Dynamic lib: `target/release/libmoq_ffi.dylib`
- Static lib: `target/release/libmoq_ffi.a`
- Header: `include/moq_ffi.h`

**Requirements:**
- Rust 1.87.0+ with Cargo
- Windows: Visual Studio 2019+ with C++ tools (build from x64 Native Tools prompt)
- Linux: GCC 7+ or Clang 10+
- macOS: Xcode command line tools

## 📦 Release Deliverables

The GitHub Actions workflows automatically build and package release artifacts:

### Windows (Win64) - Primary Platform
- **SDK Package**: `moq-ffi-sdk-windows-x64.zip`
  - `include/` - C headers (moq_ffi.h)
  - `lib/Win64/Release/` - Import libraries (moq_ffi.dll.lib)
  - `bin/` - DLL and debug symbols (moq_ffi.dll, moq_ffi.pdb)

- **Plugin Layout Package**: `moq-ffi-plugin-windows-x64.zip`
  - Ready-to-use ThirdParty layout for Unreal Engine plugins
  - `ThirdParty/moq_ffi/include/` - Headers
  - `ThirdParty/moq_ffi/lib/Win64/Release/` - Import lib
  - `ThirdParty/moq_ffi/bin/Win64/Release/` - DLL + PDB

### Linux - Secondary Platform
- **SDK Package**: `moq-ffi-sdk-linux-x64.tar.gz`
  - `include/` - C headers
  - `lib/` - Shared (.so) and static (.a) libraries

### macOS - Secondary Platform
- **SDK Package**: `moq-ffi-sdk-macos-universal.tar.gz`
  - `include/` - C headers
  - `lib/` - Universal binaries (x86_64 + arm64) for dynamic (.dylib) and static (.a) libraries

## 🎮 Unreal Engine Integration

For Unreal Engine plugin development:

1. Download the `moq-ffi-plugin-windows-x64.zip` from the [Releases](https://github.com/lifelike-and-believable/moq-ffi/releases) page
2. Extract into your plugin's directory structure:
   ```
   Plugins/YourMoqPlugin/
       ThirdParty/moq_ffi/
           include/                 # headers (moq_ffi.h)
           lib/Win64/Release/       # import lib (moq_ffi.dll.lib)
           bin/Win64/Release/       # DLL + PDB (moq_ffi.dll, moq_ffi.pdb)
   ```
3. Configure your plugin's Build.cs to reference the headers and link the import library
4. Stage the DLL at runtime

## 🔌 C/C++ Usage Example

```cpp
#include "moq_ffi.h"
#include <stdio.h>

void on_connection_state(void* user_data, MoqConnectionState state) {
    printf("Connection state: %d\n", state);
}

void on_data_received(void* user_data, const uint8_t* data, size_t data_len) {
    printf("Received %zu bytes\n", data_len);
}

int main() {
    // Create client
    MoqClient* client = moq_client_create();
    
    // Connect to relay
    MoqResult result = moq_connect(
        client, 
        "https://relay.example.com:443",
        on_connection_state,
        NULL
    );
    
    if (result.code == MOQ_OK) {
        // Announce namespace and create publisher
        moq_announce_namespace(client, "my-namespace");
        MoqPublisher* pub = moq_create_publisher(client, "my-namespace", "my-track");
        
        // Publish data
        uint8_t data[256] = {0};
        moq_publish_data(pub, data, sizeof(data), MOQ_DELIVERY_STREAM);
        
        // Subscribe to track
        MoqSubscriber* sub = moq_subscribe(
            client,
            "remote-namespace",
            "remote-track",
            on_data_received,
            NULL
        );
        
        // Cleanup
        moq_subscriber_destroy(sub);
        moq_publisher_destroy(pub);
    } else {
        printf("Connection failed: %s\n", result.message);
        moq_free_str(result.message);
    }
    
    moq_disconnect(client);
    moq_client_destroy(client);
    return 0;
}
```

## 🏗️ Project Structure

```
moq-ffi/
├── moq_ffi/                 # Rust FFI crate
│   ├── src/
│   │   ├── lib.rs           # Main entry point
│   │   ├── backend_stub.rs  # Stub implementation (no moq-transport)
│   │   └── backend_moq.rs   # Full implementation (with moq-transport)
│   ├── include/
│   │   └── moq_ffi.h        # C API header
│   └── Cargo.toml           # Rust dependencies and build config
├── tools/
│   ├── package.ps1          # Package SDK artifacts (Windows)
│   └── package-plugin.ps1   # Package plugin layout (Windows)
├── .github/
│   └── workflows/
│       └── build-ffi.yml    # CI/CD workflow for all platforms
└── README.md
```

## 🔧 Development

### Local Build Commands

**Windows (PowerShell):**
```powershell
cd moq_ffi
cargo build --release --features with_moq

# Package SDK
pwsh ../tools/package.ps1 -CrateDir "." -OutDir "../artifacts/windows-x64"

# Package plugin layout
pwsh ../tools/package-plugin.ps1 -CrateDir "." -OutDir "../artifacts/plugin-windows-x64"
```

**Linux/macOS:**
```bash
cd moq_ffi
cargo build --release --features with_moq

# Manual packaging
mkdir -p ../artifacts/lib ../artifacts/include
cp target/release/libmoq_ffi.* ../artifacts/lib/
cp include/*.h ../artifacts/include/
```

### Testing the Build

```bash
# Test stub build (no dependencies)
cd moq_ffi
cargo build --release
cargo test

# Test full build (with moq-transport)
cargo build --release --features with_moq
```

## 📝 API Reference

See [`moq_ffi/include/moq_ffi.h`](moq_ffi/include/moq_ffi.h) for the complete C API documentation.

### Core Functions

- **Client Management**: `moq_client_create()`, `moq_client_destroy()`, `moq_connect()`, `moq_disconnect()`
- **Publishing**: `moq_announce_namespace()`, `moq_create_publisher()`, `moq_publish_data()`
- **Subscribing**: `moq_subscribe()`, `moq_subscriber_destroy()`
- **Utilities**: `moq_version()`, `moq_last_error()`, `moq_free_str()`

### Delivery Modes

- `MOQ_DELIVERY_DATAGRAM`: Lossy delivery for high-frequency updates (like real-time motion data)
- `MOQ_DELIVERY_STREAM`: Reliable delivery for critical data

## 🚦 CI/CD Workflows

The repository includes GitHub Actions workflows that build release artifacts for all platforms:

- **Windows (MSVC)**: Primary platform with full SDK and plugin layout packages
- **Linux (GNU)**: Secondary platform with SDK package
- **macOS (Universal)**: Secondary platform with universal binaries (x86_64 + arm64)

Workflows trigger on:
- Push to main branch (paths: `moq_ffi/**`, `tools/**`, `.github/workflows/**`)
- Pull requests
- Manual dispatch
- Release publication (automatically attaches artifacts to releases)

## 🤝 Contributing

Contributions are welcome! Please:
1. Check existing issues before creating new ones
2. Follow the existing code style
3. Test your changes on all supported platforms when possible
4. Update documentation as needed

## 📄 License

MIT License - see [LICENSE](LICENSE) file for details.

## 🔗 Related Projects

- [moq-rs](https://github.com/cloudflare/moq-rs) - The underlying Rust implementation of MoQ Transport
- [livekit-ffi-ue](https://github.com/lifelike-and-believable/livekit-ffi-ue) - Reference repository for the FFI + Unreal Engine integration pattern

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/lifelike-and-believable/moq-ffi/issues)
- **MoQ Specification**: [IETF MoQ Transport Draft](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/)

---

**Note**: This is an early-stage project. The full moq-transport backend integration is planned but not yet complete. The current implementation provides the infrastructure and stub backend for testing build toolchains and integration patterns.
