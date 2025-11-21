# MoQ FFI Examples

This directory contains example programs demonstrating how to use the moq_ffi library.

## test_client.c

A simple example demonstrating basic MoQ client operations:
- Creating a client
- Connecting to a relay server
- Announcing namespaces
- Publishing data (both reliable and unreliable modes)
- Subscribing to tracks
- Clean disconnection

### Building

**Linux/macOS:**
```bash
cd examples

# Build the moq_ffi library first if not already built
cd ../moq_ffi
cargo build --release --features with_moq
cd ../examples

# Compile the example
gcc -o test_client test_client.c \
    -I../moq_ffi/include \
    -L../moq_ffi/target/release \
    -lmoq_ffi -lpthread -ldl -lm

# Run it
LD_LIBRARY_PATH=../moq_ffi/target/release ./test_client
```

**macOS specific:**
```bash
DYLD_LIBRARY_PATH=../moq_ffi/target/release ./test_client
```

**Windows (MSVC):**
```cmd
REM Build the library first
cd ..\moq_ffi
cargo build --release --features with_moq
cd ..\examples

REM Compile (using Visual Studio tools)
cl test_client.c /I..\moq_ffi\include ^
   /link ..\moq_ffi\target\release\moq_ffi.dll.lib

REM Run (make sure moq_ffi.dll is in PATH or current directory)
copy ..\moq_ffi\target\release\moq_ffi.dll .
test_client.exe
```

### Usage

Run with default URL:
```bash
./test_client
```

Run with custom server URL:
```bash
./test_client https://your-relay-server.com:443
```

## Integration with Unreal Engine

To use this library in an Unreal Engine plugin:

1. Download the release artifacts (or build locally)
2. Extract to your plugin's ThirdParty directory
3. Configure your Build.cs file:

```csharp
// MyPlugin.Build.cs
using UnrealBuildTool;
using System.IO;

public class MyPlugin : ModuleRules
{
    public MyPlugin(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;

        PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine" });

        // Add MoQ FFI library
        string ThirdPartyPath = Path.Combine(ModuleDirectory, "../../ThirdParty");
        string MoqPath = Path.Combine(ThirdPartyPath, "moq_ffi");
        
        PublicIncludePaths.Add(Path.Combine(MoqPath, "include"));
        
        if (Target.Platform == UnrealTargetPlatform.Win64)
        {
            PublicAdditionalLibraries.Add(
                Path.Combine(MoqPath, "lib/Win64/Release/moq_ffi.dll.lib")
            );
            
            RuntimeDependencies.Add(
                Path.Combine(MoqPath, "bin/Win64/Release/moq_ffi.dll")
            );
        }
        else if (Target.Platform == UnrealTargetPlatform.Linux)
        {
            PublicAdditionalLibraries.Add(
                Path.Combine(MoqPath, "lib/libmoq_ffi.so")
            );
        }
        else if (Target.Platform == UnrealTargetPlatform.Mac)
        {
            PublicAdditionalLibraries.Add(
                Path.Combine(MoqPath, "lib/libmoq_ffi.dylib")
            );
        }
    }
}
```

4. Include the header in your C++ code:

```cpp
#include "moq_ffi.h"

class UMyMoqComponent : public UActorComponent
{
private:
    MoqClient* Client = nullptr;

public:
    void BeginPlay() override
    {
        Super::BeginPlay();
        
        Client = moq_client_create();
        MoqResult Result = moq_connect(Client, "https://relay.example.com:443", 
                                        &UMyMoqComponent::OnConnectionState, this);
        // ... handle result
    }

    void EndPlay(const EEndPlayReason::Type EndPlayReason) override
    {
        if (Client)
        {
            moq_disconnect(Client);
            moq_client_destroy(Client);
            Client = nullptr;
        }
        Super::EndPlay(EndPlayReason);
    }

    static void OnConnectionState(void* UserData, MoqConnectionState State)
    {
        // Handle connection state changes
    }
};
```

## Additional Resources

- [Main README](../README.md) - Repository overview
- [moq_ffi/README.md](../moq_ffi/README.md) - Library-specific documentation
- [C API Header](../moq_ffi/include/moq_ffi.h) - Complete API reference
- [moq-rs GitHub](https://github.com/cloudflare/moq-rs) - Underlying Rust implementation
