# MoQ-RS API Usage Alignment Analysis

## Executive Summary

This document analyzes the alignment between our FFI wrapper implementation (`moq_ffi/src/backend_moq.rs`) and the canonical usage patterns from the moq-rs source code, specifically examining `moq-relay-ietf`, `moq-pub`, and `moq-sub`.

**Overall Assessment:** ✅ **WELL-ALIGNED** with minor gaps and opportunities for improvement

## Analysis Methodology

1. Reviewed moq-rs source components:
   - `moq-relay-ietf/` - Production relay server implementation
   - `moq-pub/` - Publisher client example
   - `moq-sub/` - Subscriber client example
   - `moq-transport/src/session/` - Session API patterns
   - `moq-transport/src/serve/` - Track and data serving patterns

2. Compared against our FFI implementation:
   - `moq_ffi/src/backend_moq.rs` - Full MoQ transport integration
   - `moq_ffi/tests/cloudflare_relay_integration.rs` - Integration tests

3. Examined API usage for:
   - Session lifecycle (connect, accept, run)
   - Publisher patterns (announce, serve)
   - Subscriber patterns (subscribe, read)
   - Track and delivery modes (stream, datagram)

## Detailed Findings

### 1. Session Management

#### ✅ ALIGNED: Connection Pattern

**moq-rs pattern (moq-pub/src/main.rs):**
```rust
let session = quic.client.connect(&cli.url).await?;
let (session, mut publisher) = Publisher::connect(session).await?;

tokio::select! {
    res = session.run() => res.context("session error")?,
    res = publisher.announce(reader) => res.context("publisher error")?,
}
```

**Our FFI implementation (backend_moq.rs:519-544):**
```rust
let (moq_session, publisher, subscriber) = Session::connect(wt_session).await?;

// Store session and publisher/subscriber
inner.publisher = Some(publisher);
inner.subscriber = Some(subscriber);

// Spawn task to run the session
let task = RUNTIME.spawn(async move {
    if let Err(e) = moq_session.run().await {
        log::error!("MoQ session error: {}", e);
    }
});
inner.session_task = Some(task);
```

**Assessment:** ✅ Correctly follows the pattern:
- Uses `Session::connect()` to establish MoQ session over WebTransport
- Properly spawns a task to run `session.run()` in the background
- Stores publisher and subscriber handles for later use

#### ✅ ALIGNED: Role Configuration

**moq-rs pattern (moq-transport/src/session/mod.rs:68-73):**
```rust
pub async fn connect(
    session: web_transport::Session,
) -> Result<(Session, Publisher, Subscriber), SessionError> {
    Self::connect_role(session, setup::Role::Both).await
        .map(|(session, publisher, subscriber)| (session, publisher.unwrap(), subscriber.unwrap()))
}
```

**Our FFI implementation:**
```rust
// We use Session::connect() which defaults to Role::Both
let (moq_session, publisher, subscriber) = Session::connect(wt_session).await?;
```

**Assessment:** ✅ Correctly uses `Role::Both` (default in `Session::connect`)

### 2. Publisher API Usage

#### ✅ ALIGNED: Announce Pattern

**moq-rs pattern (moq-pub/src/main.rs:54-76):**
```rust
let (writer, _, reader) = serve::Tracks::new(Tuple::from_utf8_path(&cli.name)).produce();
// ... later ...
publisher.announce(reader).await?;
```

**moq-rs relay pattern (moq-relay-ietf/src/producer.rs:25-26):**
```rust
pub async fn announce(&mut self, tracks: TracksReader) -> Result<(), SessionError> {
    self.remote.announce(tracks).await
}
```

**Our FFI implementation (backend_moq.rs:799-813):**
```rust
// Create tracks for this namespace
let (tracks_writer, _tracks_request, tracks_reader) = 
    serve::Tracks::new(track_namespace.clone()).produce();

// Spawn task to announce and handle subscriptions
RUNTIME.spawn(async move {
    if let Err(e) = publisher.announce(tracks_reader).await {
        log::error!("Failed to announce namespace: {}", e);
        // Remove from announced namespaces on failure
        if let Ok(mut inner) = client_inner.lock() {
            inner.announced_namespaces.remove(&track_namespace_clone);
        }
    }
});

// Store the tracks writer for later use
inner.announced_namespaces.insert(track_namespace, tracks_writer);
```

**Assessment:** ✅ Correctly follows the pattern:
- Creates `Tracks` with namespace
- Calls `.produce()` to get writer/reader split
- Uses `publisher.announce(tracks_reader)` to announce namespace
- Properly stores `TracksWriter` for creating tracks later
- Spawns async task for announcement (fire-and-forget pattern like relay)

#### ✅ ALIGNED: Track Creation and Publishing

**moq-rs pattern (moq-transport/src/serve/tracks.rs:61-74):**
```rust
pub fn create(&mut self, track: &str) -> Option<TrackWriter> {
    let (writer, reader) = Track {
        namespace: self.namespace.clone(),
        name: track.to_owned(),
    }.produce();
    
    self.state.lock_mut()?.tracks.insert(track.to_owned(), reader);
    Some(writer)
}
```

**Our FFI implementation (backend_moq.rs:943-970):**
```rust
// Create track immediately
let track = match tracks_writer.create(&track_name_str) {
    Some(t) => t,
    None => {
        set_last_error("Failed to create track (all readers dropped)".to_string());
        return std::ptr::null_mut();
    }
};

// Create writer based on requested delivery mode
let mode = match delivery_mode {
    MoqDeliveryMode::MoqDeliveryDatagram => {
        match track.datagrams() {
            Ok(d) => PublisherMode::Datagrams(d),
            Err(e) => { /* error handling */ }
        }
    }
    MoqDeliveryMode::MoqDeliveryStream => {
        match track.stream(0) {
            Ok(s) => PublisherMode::Stream(s),
            Err(e) => { /* error handling */ }
        }
    }
};
```

**Assessment:** ✅ Correctly follows the pattern:
- Uses `TracksWriter.create()` to create track
- Calls `.stream()` or `.datagrams()` to select delivery mode
- Handles errors appropriately

#### ✅ ALIGNED: Data Publishing

**moq-rs patterns:**

Stream mode (moq-transport/src/serve/stream.rs):
```rust
pub fn create(&mut self, group_id: u64) -> Result<ObjectsWriter, ServeError> {
    // Creates group for writing objects
}
```

Datagram mode (moq-transport/src/serve/datagram.rs):
```rust
pub fn write(&mut self, datagram: Datagram) -> Result<(), ServeError> {
    // Writes datagram directly
}
```

**Our FFI implementation (backend_moq.rs:1098-1133):**
```rust
match &mut inner.mode {
    PublisherMode::Datagrams(datagrams) => {
        let datagram = serve::Datagram {
            group_id: 0,
            object_id: counter_val,
            priority: 0,
            payload: data_bytes,
        };
        datagrams.write(datagram)
    }
    PublisherMode::Stream(stream) => {
        stream.create(counter_val)
            .and_then(|mut group| {
                group.write(data_bytes)
            })
    }
}
```

**Assessment:** ✅ Correctly follows the pattern:
- Stream mode: Creates group with ID, then writes data to group
- Datagram mode: Creates `Datagram` struct and calls `write()`
- Properly handles both Draft 07 and Draft 14 differences in `Datagram` struct

### 3. Subscriber API Usage

#### ✅ ALIGNED: Subscribe Pattern

**moq-rs pattern (moq-sub/src/main.rs:32-44):**
```rust
let (session, subscriber) = moq_transport::session::Subscriber::connect(session).await?;

// Associate empty set of Tracks with provided namespace
let tracks = Tracks::new(Tuple::from_utf8_path(&config.name));

let mut media = Media::new(subscriber, tracks, out).await?;

tokio::select! {
    res = session.run() => res.context("session error")?,
    res = media.run() => res.context("media error")?,
}
```

**moq-rs relay pattern (moq-relay-ietf/src/consumer.rs:95-101):**
```rust
Some(track) = request.next() => {
    let mut remote = self.remote.clone();
    tasks.push(async move {
        if let Err(err) = remote.subscribe(track).await {
            log::warn!("failed forwarding subscribe: {:?}, error: {}", info, err)
        }
    }.boxed());
}
```

**Our FFI implementation (backend_moq.rs:1247-1258):**
```rust
// Create track writer/reader pair for subscription
let (track_writer, track_reader) = 
    serve::Track::new(track_namespace.clone(), track_name_str.clone()).produce();

// Subscribe to the track with timeout
let subscribe_result = RUNTIME.block_on(async move {
    match timeout(Duration::from_secs(SUBSCRIBE_TIMEOUT_SECS), async {
        subscriber_impl.subscribe(track_writer).await
            .map_err(|e| format!("Failed to subscribe: {}", e))
    }).await {
        Ok(result) => result,
        Err(_) => Err(format!("Subscribe timeout after {} seconds", SUBSCRIBE_TIMEOUT_SECS)),
    }
});
```

**Assessment:** ✅ Correctly follows the pattern:
- Creates `Track` with namespace and name
- Calls `.produce()` to get writer/reader split
- Uses `subscriber.subscribe(track_writer)` to request subscription
- Properly stores `TrackReader` for reading data later
- Adds timeout protection (good FFI practice)

#### ✅ ALIGNED: Reading Data

**moq-rs reading patterns:**

Stream mode (moq-transport/src/serve/stream.rs):
```rust
pub async fn next(&mut self) -> Result<Option<GroupReader>, ServeError> {
    // Returns next group
}
// Then for group:
pub async fn next(&mut self) -> Result<Option<ObjectReader>, ServeError> {
    // Returns next object
}
// Then for object:
pub async fn read(&mut self) -> Result<Option<Bytes>, ServeError> {
    // Returns next chunk
}
```

Datagram mode (moq-transport/src/serve/datagram.rs):
```rust
pub async fn read(&mut self) -> Result<Option<Datagram>, ServeError> {
    // Returns next datagram
}
```

**Our FFI implementation (backend_moq.rs:1300-1428):**
```rust
let mode_result = track.mode().await;
match mode_result {
    Ok(mode) => {
        loop {
            let data_result = match &mode {
                TrackReaderMode::Stream(stream) => {
                    let mut stream = stream.clone();
                    match stream.next().await {  // Get group
                        Ok(Some(mut group)) => {
                            match group.next().await {  // Get object
                                Ok(Some(mut object)) => {
                                    let mut buffer = Vec::new();
                                    loop {
                                        match object.read().await {  // Get chunks
                                            Ok(Some(chunk)) => buffer.extend_from_slice(&chunk),
                                            Ok(None) => break,
                                            Err(_) => break,
                                        }
                                    }
                                    if !buffer.is_empty() { Some(buffer) } else { None }
                                }
                                _ => None,
                            }
                        }
                        Ok(None) => break,  // Stream ended
                        Err(e) => None,
                    }
                }
                TrackReaderMode::Datagrams(datagrams) => {
                    let mut datagrams = datagrams.clone();
                    match datagrams.read().await {
                        Ok(Some(datagram)) => Some(datagram.payload.to_vec()),
                        Ok(None) => break,  // Datagrams ended
                        Err(e) => None,
                    }
                }
            };
            
            // Invoke callback if we got data
            if let Some(buffer) = data_result {
                // ... callback invocation ...
            }
        }
    }
}
```

**Assessment:** ✅ Correctly follows the pattern:
- Stream mode: Properly chains `.next()` calls for stream → group → object → chunks
- Datagram mode: Directly calls `.read()` to get datagram payload
- Handles both Draft 07 and Draft 14 `Subgroups` vs `Stream` terminology
- Spawns async task to continuously read data (fire-and-forget pattern)
- Properly handles end-of-stream conditions

### 4. WebTransport Connection

#### ✅ ALIGNED: QUIC/WebTransport Setup

**moq-rs pattern (moq-native-ietf/quic.rs and moq-pub/src/main.rs):**
```rust
let quic = quic::Endpoint::new(quic::Config {
    bind: cli.bind,
    tls: tls.clone(),
})?;

let session = quic.client.connect(&cli.url).await?;
```

**Our FFI implementation (backend_moq.rs:436-514):**
```rust
// Create quinn endpoint
let mut endpoint = match "[::]:0".parse::<std::net::SocketAddr>() {
    Ok(ipv6_addr) => quinn::Endpoint::client(ipv6_addr)
        .or_else(|_| quinn::Endpoint::client("0.0.0.0:0".parse()?))?,
    // ... fallback to IPv4 ...
};

// Configure TLS with native root certificates
let mut roots = rustls::RootCertStore::empty();
let native_certs = rustls_native_certs::load_native_certs();
for cert in native_certs.certs {
    roots.add(cert)?;
}

let client_crypto = rustls::ClientConfig::builder()
    .with_root_certificates(roots)
    .with_no_client_auth();

let client_config = quinn::ClientConfig::new(
    Arc::new(quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)?)
);

// Configure transport - enable datagrams
let mut transport_config = quinn::TransportConfig::default();
transport_config.datagram_receive_buffer_size(Some(1024 * 1024));
transport_config.datagram_send_buffer_size(1024 * 1024);
client_config.transport_config(Arc::new(transport_config));

endpoint.set_default_client_config(client_config);

// Connect via WebTransport
let wt_session_quinn = wt_connect(&endpoint, &parsed_url).await?;
let wt_session = web_transport::Session::from(wt_session_quinn);
```

**Assessment:** ✅ Correctly follows the pattern:
- Creates Quinn endpoint for client connections
- Configures TLS with native root certificates
- Enables QUIC datagrams (important for MoQ datagram delivery)
- Uses `web_transport_quinn::connect()` to establish WebTransport session
- Converts to generic `web_transport::Session` for moq-transport

#### ⚠️ GAP: IPv6/IPv4 Fallback Not in moq-rs

**Our implementation (backend_moq.rs:438-464):**
```rust
let mut endpoint = match "[::]:0".parse::<std::net::SocketAddr>() {
    Ok(ipv6_addr) => {
        match quinn::Endpoint::client(ipv6_addr) {
            Ok(ep) => ep,
            Err(e) => {
                // IPv6 not available, fall back to IPv4
                let ipv4_addr = "0.0.0.0:0".parse()?;
                quinn::Endpoint::client(ipv4_addr)?
            }
        }
    }
    // ... fallback logic ...
};
```

**Assessment:** ⚠️ This is a **custom enhancement** not found in moq-rs examples
- **Justification**: Good defensive programming for FFI library
- **Impact**: No compatibility issue, improves robustness
- **Recommendation**: Keep this enhancement

### 5. Async Runtime Management

#### ⚠️ DIFFERENCE: Global Runtime vs Application-Managed

**moq-rs pattern:**
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Application manages its own runtime via #[tokio::main]
}
```

**Our FFI implementation (backend_moq.rs:57-64):**
```rust
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("moq-ffi-worker")
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
});
```

**Assessment:** ⚠️ Different pattern, but **CORRECT for FFI**
- **Justification**: FFI libraries cannot assume calling code has async runtime
- **Pattern**: Common in FFI libraries (e.g., livekit-ffi, reqwest-blocking)
- **Trade-off**: Global runtime vs per-client runtime
- **Recommendation**: Keep current approach, it's appropriate for FFI

#### ✅ ALIGNED: Runtime Usage

**Our implementation:**
```rust
// Blocking bridge to async
RUNTIME.block_on(async move {
    // ... async operation ...
});

// Background task spawning
RUNTIME.spawn(async move {
    if let Err(e) = moq_session.run().await {
        log::error!("MoQ session error: {}", e);
    }
});
```

**Assessment:** ✅ Correct patterns for async-to-sync bridge in FFI

### 6. Error Handling

#### ✅ ALIGNED: Error Types

**moq-rs patterns:**
- Uses `anyhow::Error` for application-level errors
- Uses typed errors like `SessionError`, `ServeError`
- Logs errors with `log::error!`, `log::warn!`

**Our FFI implementation:**
- Converts all errors to `MoqResult` with error codes and messages
- Properly propagates error context
- Uses thread-local error storage
- Logs errors consistently

**Assessment:** ✅ Properly adapts moq-rs errors to FFI-safe C ABI

### 7. Delivery Modes

#### ✅ ALIGNED: Stream vs Datagram

**moq-rs API:**
```rust
// Track provides methods to select mode
pub fn stream(self, priority: u8) -> Result<StreamWriter, ServeError>
pub fn datagrams(self) -> Result<DatagramsWriter, ServeError>
```

**Our FFI implementation:**
```rust
let mode = match delivery_mode {
    MoqDeliveryMode::MoqDeliveryDatagram => {
        match track.datagrams() {
            Ok(d) => PublisherMode::Datagrams(d),
            Err(e) => { /* error */ }
        }
    }
    MoqDeliveryMode::MoqDeliveryStream => {
        match track.stream(0) {
            Ok(s) => PublisherMode::Stream(s),
            Err(e) => { /* error */ }
        }
    }
};
```

**Assessment:** ✅ Correctly uses the API
- Properly calls `.stream()` or `.datagrams()` on TrackWriter
- Passes priority (0 = highest for live streaming)
- Stores mode in publisher for later use

## Identified Gaps and Misalignments

### 🔴 GAP 1: No Support for Dynamic Track Creation (TracksRequest)

**moq-rs pattern (moq-relay-ietf/src/consumer.rs:92-104):**
```rust
let (_, mut request, reader) = Tracks::new(announce.namespace.clone()).produce();

loop {
    tokio::select! {
        // Wait for the next subscriber and serve the track.
        Some(track) = request.next() => {
            let mut remote = self.remote.clone();
            tasks.push(async move {
                if let Err(err) = remote.subscribe(track).await {
                    log::warn!("failed forwarding subscribe: {:?}, error: {}", info, err)
                }
            }.boxed());
        },
        // ...
    }
}
```

**Our FFI implementation (backend_moq.rs:800):**
```rust
let (tracks_writer, _tracks_request, tracks_reader) = 
    serve::Tracks::new(track_namespace.clone()).produce();
// TracksRequest is discarded with `_`
```

**Impact:** 🟡 **MEDIUM**
- We only support **static track creation** (pre-defined tracks via `moq_create_publisher`)
- moq-rs supports **dynamic track creation** (tracks created on-demand when subscribers request them)
- This limits flexibility for scenarios where track names aren't known in advance

**Recommendation:**
1. Add FFI API for registering a "track request callback"
2. Implement `TracksRequest.next()` handler that invokes callback
3. Allow C code to dynamically create publishers in response to subscription requests

**Example proposed API:**
```c
typedef void (*MoqTrackRequestCallback)(
    void* user_data,
    const char* namespace,
    const char* track_name
);

MOQ_API MoqResult moq_set_track_request_callback(
    MoqClient* client,
    const char* namespace,
    MoqTrackRequestCallback callback,
    void* user_data
);
```

### 🟡 GAP 2: No Graceful Session Closure

**moq-rs pattern:**
```rust
// Session naturally closes when dropped or when an error occurs
// Applications can cleanly shut down by dropping handles
drop(publisher);
drop(subscriber);
// session.run() will complete gracefully
```

**Our FFI implementation:**
```rust
// moq_disconnect() aborts the session task
if let Some(task) = inner.session_task.take() {
    task.abort();  // Abrupt termination
}
```

**Impact:** 🟡 **MEDIUM**
- We use `.abort()` which forcefully terminates the session task
- This may not allow proper cleanup or graceful session closure
- Could cause connection resets or incomplete state synchronization

**Recommendation:**
1. Implement graceful shutdown pattern
2. Add a "closing" flag to signal session should terminate
3. Wait for session.run() to complete naturally (with timeout)
4. Only abort after timeout expires

**Example:**
```rust
// Signal shutdown
inner.closing = true;
drop(inner.publisher);
drop(inner.subscriber);

// Wait for graceful shutdown with timeout
if let Some(task) = inner.session_task.take() {
    match tokio::time::timeout(Duration::from_secs(5), task).await {
        Ok(_) => log::info!("Session closed gracefully"),
        Err(_) => {
            log::warn!("Session shutdown timeout, aborting");
            // Task already dropped, no need to abort
        }
    }
}
```

### 🟡 GAP 3: Limited Track Status Handling

**moq-rs pattern (moq-transport/src/session/publisher.rs:99-125):**
```rust
res = announce.track_status_requested(), if !status_done => {
    match res? {
        Some(requested) => {
            let tracks = tracks.clone();
            status_tasks.push(async move {
                let info = requested.info.clone();
                if let Err(err) = Self::serve_track_status(requested, tracks).await {
                    log::warn!("failed serving track status: {:?}, error: {}", info, err)
                }
            });
        },
        None => status_done = true,
    }
}
```

**Our FFI implementation:**
- No handling of `track_status_requested()` messages
- No API to query or respond to track status requests

**Impact:** 🟢 **LOW**
- Track status is an optional MoQ protocol feature
- Most applications work without it
- Primarily used for advanced relay routing

**Recommendation:**
- **SHORT-TERM**: Document this limitation
- **LONG-TERM**: Add optional FFI API if needed by use cases

### 🟢 GAP 4: No Announced Callback for Subscribers

**moq-rs pattern (moq-relay-ietf/src/consumer.rs:38-48):**
```rust
loop {
    tokio::select! {
        Some(announce) = self.remote.announced() => {
            let this = self.clone();
            tasks.push(async move {
                let info = announce.clone();
                log::info!("serving announce: {:?}", info);
                if let Err(err) = this.serve(announce).await {
                    log::warn!("failed serving announce: {:?}, error: {}", info, err)
                }
            });
        },
        // ...
    }
}
```

**Our FFI implementation:**
- No handling of incoming announcements
- Subscriber must know track names in advance

**Impact:** 🟢 **LOW**
- Current use case assumes known track names
- Announcements are more relevant for relay/routing scenarios
- Direct client-to-client communication doesn't need this

**Recommendation:**
- **DOCUMENT**: Current limitation in README
- **FUTURE**: Add `MoqAnnouncedCallback` if dynamic discovery is needed

### 🟢 GAP 5: No Support for Subgroup-Level Publishing (Draft 14)

**moq-rs pattern (moq-transport/src/serve/track.rs:86-95):**
```rust
pub fn groups(self) -> Result<SubgroupsWriter, ServeError> {
    let (writer, reader) = Subgroups {
        track: self.info.clone(),
    }.produce();
    
    let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
    state.mode = Some(reader.into());
    Ok(writer)
}
```

**Our FFI implementation:**
- Only supports `.stream()` and `.datagrams()` modes
- No API to use `.groups()` / subgroups mode

**Impact:** 🟢 **LOW** (Draft 14 only)
- Subgroups provide finer-grained delivery control
- Most use cases covered by stream and datagram modes
- Draft 07 doesn't have subgroups

**Recommendation:**
- **DOCUMENT**: Limitation for Draft 14
- **DEFER**: Add if specific use case emerges

## Draft 07 vs Draft 14 Compatibility

### ✅ Handled Correctly

Our implementation properly handles differences between drafts:

1. **Namespace Type:**
   ```rust
   #[cfg(feature = "with_moq")]
   use moq::coding::TrackNamespace;
   
   #[cfg(feature = "with_moq_draft07")]
   use moq::coding::Tuple as TrackNamespace;
   ```

2. **Datagram Status Field (Draft 07 only):**
   ```rust
   #[cfg(feature = "with_moq_draft07")]
   let datagram = serve::Datagram {
       group_id: 0,
       object_id: counter_val,
       priority: 0,
       status: moq::data::ObjectStatus::Object,  // Draft 07 only
       payload: data_bytes,
   };
   ```

3. **Subgroups vs Groups Terminology:**
   ```rust
   #[cfg(feature = "with_moq")]
   TrackReaderMode::Subgroups(subgroups) => { /* Draft 14 */ }
   
   #[cfg(feature = "with_moq_draft07")]
   TrackReaderMode::Subgroups(subgroups) => { /* Draft 07, but called subgroups */ }
   ```

### ⚠️ Potential Issue: Version Negotiation

**moq-rs pattern (moq-transport/src/session/mod.rs:84):**
```rust
let versions: setup::Versions = [setup::Version::DRAFT_07].into();
```

**Our implementation:**
- Relies on moq-transport's default version negotiation
- No explicit control over which draft version to advertise

**Impact:** 🟡 **MEDIUM**
- Works correctly as long as relay supports the version
- Could cause confusion if version mismatch occurs
- Error messages might not be clear about version incompatibility

**Recommendation:**
- **DOCUMENT**: Which draft version is supported by each build
- **IMPROVE ERROR**: Detect version mismatch errors and provide clear message
- **FUTURE**: Add FFI API to query negotiated version

## Best Practices Followed

### ✅ FFI Safety

1. **Panic Protection:** All FFI entry points wrapped in `catch_unwind`
2. **Null Pointer Validation:** All pointer parameters validated before use
3. **Mutex Poisoning Recovery:** Handles poisoned mutexes gracefully
4. **Callback Protection:** Callbacks invoked within `catch_unwind`

### ✅ Memory Management

1. **Ownership Transfer:** Clear documentation of who owns memory
2. **String Handling:** Proper `CString::into_raw()` / `from_raw()` usage
3. **Cleanup:** Proper resource cleanup in destroy functions
4. **Task Management:** Async tasks properly aborted on cleanup

### ✅ Error Handling

1. **Error Conversion:** All moq-rs errors converted to FFI-safe codes
2. **Error Messages:** Helpful error messages with context
3. **Thread-Local Errors:** Per-thread error storage
4. **Logging:** Comprehensive logging for debugging

### ✅ Async Bridge

1. **Global Runtime:** Appropriate for FFI library
2. **Timeouts:** Added to prevent indefinite blocking
3. **Task Spawning:** Background tasks for long-running operations
4. **Blocking Bridge:** `block_on` used only for operations that must be synchronous

## Recommendations

### Priority 1 (High): Address Functional Gaps

1. **Implement Dynamic Track Creation** (GAP 1)
   - Add track request callback API
   - Use `TracksRequest.next()` properly
   - Allow on-demand track creation

2. **Implement Graceful Shutdown** (GAP 2)
   - Replace `.abort()` with graceful closure
   - Add timeout for clean shutdown
   - Improve connection state reporting

### Priority 2 (Medium): Improve Robustness

3. **Add Version Negotiation Awareness**
   - Detect version mismatch errors
   - Provide clear error messages
   - Consider exposing negotiated version

4. **Enhance Error Context**
   - Include more diagnostic information in errors
   - Map specific moq-rs error types to FFI codes
   - Add error code for version mismatch

### Priority 3 (Low): Optional Enhancements

5. **Add Announced Callback** (GAP 4)
   - If dynamic track discovery is needed
   - Would enable more relay-like behavior

6. **Add Track Status Support** (GAP 3)
   - If advanced routing is needed
   - Currently low priority

7. **Add Subgroups Mode** (GAP 5)
   - If finer-grained control is needed (Draft 14 only)
   - Currently low priority

### Priority 4 (Documentation): Clarify Limitations

8. **Document Known Limitations**
   - Static track creation only
   - No announced callback for subscribers
   - No track status handling
   - No subgroups mode (Draft 14)

9. **Add API Usage Examples**
   - Example showing proper lifecycle
   - Example showing error handling
   - Example showing both delivery modes

10. **Create Migration Guide**
    - If upgrading from Draft 07 to Draft 14
    - Highlight API differences
    - Provide upgrade checklist

## Conclusion

### Overall Assessment: ✅ **WELL-ALIGNED**

Our FFI implementation follows moq-rs API patterns correctly and aligns well with canonical usage from:
- ✅ Session management (connect, run)
- ✅ Publisher API (announce, create tracks, publish data)
- ✅ Subscriber API (subscribe, read data)
- ✅ Delivery modes (stream, datagram)
- ✅ WebTransport connection setup
- ✅ Error handling patterns

### Identified Gaps:

**High Impact:**
- 🔴 No dynamic track creation (TracksRequest)
- 🟡 No graceful session closure

**Medium Impact:**
- 🟡 No version negotiation awareness
- 🟡 Limited track status handling

**Low Impact:**
- 🟢 No announced callback for subscribers
- 🟢 No subgroups mode support (Draft 14)

### Key Strengths:

1. **Correct Core Patterns:** Session, Publisher, Subscriber APIs used correctly
2. **Robust FFI Safety:** Comprehensive panic protection and error handling
3. **Draft Compatibility:** Properly handles differences between Draft 07 and Draft 14
4. **Appropriate Adaptations:** Global runtime and timeouts suitable for FFI

### Next Steps:

1. Implement dynamic track creation (GAP 1) - **High Priority**
2. Implement graceful shutdown (GAP 2) - **High Priority**
3. Add version mismatch detection - **Medium Priority**
4. Document current limitations - **Required**
5. Create comprehensive usage examples - **Recommended**

### Risk Assessment:

**Overall Risk: 🟢 LOW**

The implementation is production-ready for the current use case (static tracks, known topology). Gaps are primarily related to advanced features and dynamic scenarios that may not be required for initial deployment.

**Compatibility Risk: 🟢 LOW**

The implementation correctly uses moq-rs APIs. Future moq-rs updates are unlikely to break our usage patterns since we follow canonical examples from moq-relay-ietf, moq-pub, and moq-sub.

---

**Analysis Date:** 2025-11-23  
**moq-rs Commit:** ebc843de8504e37d36c3134a1181513ebdf7a34a  
**Analyzer:** GitHub Copilot Code Review Agent
