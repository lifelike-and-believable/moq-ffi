// Full backend with moq-transport integration
//
// This backend provides implementations of all FFI functions using the moq-transport library.
// The implementation uses moq-transport types and follows MoQ protocol patterns.
//
// Features:
// - Support for both IETF Draft 7 (CloudFlare production - PRIORITY) and Draft 14 (latest)
// - WebTransport over QUIC connection support (both drafts)
// - Stream and Datagram delivery modes
// - Full async runtime integration with proper FFI safety
//
// TODO(Draft 14): Future enhancements for Draft 14 implementation
// - Add raw QUIC connection support (without WebTransport protocol layer)
// - Implement direct Session creation from quinn::Connection
// - Add connection pooling and reuse capabilities
// - Optimize for lower latency scenarios
// - Add comprehensive integration tests with Draft 14 relay servers

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use tokio::runtime::Runtime;
use once_cell::sync::Lazy;

// Compile-time check: Ensure only one MoQ version feature is enabled
#[cfg(all(feature = "with_moq", feature = "with_moq_draft07"))]
compile_error!("Cannot enable both 'with_moq' and 'with_moq_draft07' features simultaneously. Choose one based on your relay server.");

// Import the appropriate moq-transport version based on feature flags
#[cfg(feature = "with_moq")]
use moq_transport as moq;

#[cfg(feature = "with_moq_draft07")]
use moq_transport_draft07 as moq;

// MoQ transport types - handle API differences between drafts
#[cfg(feature = "with_moq")]
use moq::coding::TrackNamespace;

#[cfg(feature = "with_moq_draft07")]
use moq::coding::Tuple as TrackNamespace;

// Global tokio runtime for async operations
// This runtime handles:
// - Async WebTransport/QUIC operations
// - Message processing tasks
// - Track reader/writer management
// - Callback invocations from async context
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("moq-ffi-worker")
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
});

// Thread-local error storage
thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn set_last_error(error: String) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = Some(error);
    });
}

fn get_last_error() -> Option<String> {
    LAST_ERROR.with(|e| e.borrow().clone())
}

/* ───────────────────────────────────────────────
 * Opaque Types
 * ─────────────────────────────────────────────── */

use moq::{
    serve::{self, TracksWriter},
    session::{Publisher as MoqTransportPublisher, Session, Subscriber as MoqTransportSubscriber},
};

struct ClientInner {
    connected: bool,
    url: Option<String>,
    session: Option<Session>,
    publisher: Option<MoqTransportPublisher>,
    subscriber: Option<MoqTransportSubscriber>,
    connection_callback: MoqConnectionCallback,
    connection_user_data: usize, // Store as usize for Send safety
    // Track announced namespaces
    announced_namespaces: HashMap<TrackNamespace, TracksWriter>,
    // Handle to session run task
    session_task: Option<tokio::task::JoinHandle<()>>,
}

#[repr(C)]
pub struct MoqClient {
    inner: Arc<Mutex<ClientInner>>,
}

enum PublisherMode {
    Stream(serve::StreamWriter),
    Datagrams(serve::DatagramsWriter),
}

struct PublisherInner {
    namespace: TrackNamespace,
    track_name: String,
    mode: PublisherMode,
    group_id_counter: std::sync::atomic::AtomicU64,
}

#[repr(C)]
pub struct MoqPublisher {
    inner: Arc<Mutex<PublisherInner>>,
}

struct SubscriberInner {
    namespace: TrackNamespace,
    track_name: String,
    data_callback: MoqDataCallback,
    user_data: usize, // Store as usize for Send safety
    track: serve::TrackReader,
    // Handle to data reading task
    reader_task: Option<tokio::task::JoinHandle<()>>,
}

#[repr(C)]
pub struct MoqSubscriber {
    inner: Arc<Mutex<SubscriberInner>>,
}

// Safety: We ensure thread safety through Arc<Mutex<>> wrappers
unsafe impl Send for MoqClient {}
unsafe impl Send for MoqPublisher {}
unsafe impl Send for MoqSubscriber {}

/* ───────────────────────────────────────────────
 * Enums
 * ─────────────────────────────────────────────── */

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MoqResultCode {
    MoqOk = 0,
    MoqErrorInvalidArgument = 1,
    MoqErrorConnectionFailed = 2,
    MoqErrorNotConnected = 3,
    MoqErrorTimeout = 4,
    MoqErrorInternal = 5,
    MoqErrorUnsupported = 6,
    MoqErrorBufferTooSmall = 7,
}

#[repr(C)]
pub struct MoqResult {
    pub code: MoqResultCode,
    pub message: *const c_char,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MoqConnectionState {
    MoqStateDisconnected = 0,
    MoqStateConnecting = 1,
    MoqStateConnected = 2,
    MoqStateFailed = 3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MoqDeliveryMode {
    MoqDeliveryDatagram = 0,
    MoqDeliveryStream = 1,
}

/* ───────────────────────────────────────────────
 * Callbacks
 * ─────────────────────────────────────────────── */

pub type MoqConnectionCallback =
    Option<unsafe extern "C" fn(user_data: *mut std::ffi::c_void, state: MoqConnectionState)>;

pub type MoqDataCallback = Option<
    unsafe extern "C" fn(user_data: *mut std::ffi::c_void, data: *const u8, data_len: usize),
>;

pub type MoqTrackCallback = Option<
    unsafe extern "C" fn(
        user_data: *mut std::ffi::c_void,
        namespace: *const c_char,
        track_name: *const c_char,
    ),
>;

/* ───────────────────────────────────────────────
 * Helper Functions
 * ─────────────────────────────────────────────── */

fn make_ok_result() -> MoqResult {
    MoqResult {
        code: MoqResultCode::MoqOk,
        message: std::ptr::null(),
    }
}

fn make_error_result(code: MoqResultCode, message: &str) -> MoqResult {
    let c_message = CString::new(message).unwrap_or_else(|_| CString::new("Invalid message").unwrap());
    MoqResult {
        code,
        message: c_message.into_raw(),
    }
}

/* ───────────────────────────────────────────────
 * Client Management
 * ─────────────────────────────────────────────── */

/// Creates a new MoQ client instance.
///
/// # Returns
/// A pointer to the newly created client, or null on failure.
/// The client must be destroyed with `moq_client_destroy()` when no longer needed.
///
/// # Thread Safety
/// This function is thread-safe and can be called from any thread.
#[no_mangle]
pub extern "C" fn moq_client_create() -> *mut MoqClient {
    std::panic::catch_unwind(|| {
        let client = MoqClient {
            inner: Arc::new(Mutex::new(ClientInner {
                connected: false,
                url: None,
                session: None,
                publisher: None,
                subscriber: None,
                connection_callback: None,
                connection_user_data: 0,
                announced_namespaces: HashMap::new(),
                session_task: None,
            })),
        };
        Box::into_raw(Box::new(client))
    }).unwrap_or_else(|_| {
        log::error!("Panic in moq_client_create");
        set_last_error("Internal panic occurred in moq_client_create".to_string());
        std::ptr::null_mut()
    })
}

/// Destroys a MoQ client and releases all associated resources.
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null (null pointers are safely ignored)
/// - `client` must not be accessed after this function returns
/// - This function is thread-safe
/// - Active connections will be closed and async tasks will be aborted
///
/// # Parameters
/// - `client`: Pointer to the client to destroy, or null (null is safely ignored)
#[no_mangle]
pub unsafe extern "C" fn moq_client_destroy(client: *mut MoqClient) {
    let _ = std::panic::catch_unwind(|| {
        if !client.is_null() {
            let client_box = Box::from_raw(client);
            
            // Clean up resources properly
            if let Ok(mut inner) = client_box.inner.lock() {
                // Abort session task if running
                if let Some(task) = inner.session_task.take() {
                    task.abort();
                }
                // Clear all resources
                inner.announced_namespaces.clear();
                inner.publisher = None;
                inner.subscriber = None;
                inner.session = None;
                inner.connected = false;
            }
            
            drop(client_box);
        }
    });
    // Silently handle panics - destructor should not propagate panics
}

/// Connects to a MoQ relay server.
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - `url` must be a valid null-terminated C string pointer
/// - `url` must not be null
/// - `url` must be a valid HTTPS URL for WebTransport over QUIC
/// - `connection_callback` may be null (no callback will be invoked)
/// - `user_data` will be passed to the callback and may be null
/// - This function is thread-safe
///
/// # Parameters
/// - `client`: Pointer to the MoQ client
/// - `url`: HTTPS URL of the relay server (WebTransport over QUIC)
/// - `connection_callback`: Optional callback for connection state changes
/// - `user_data`: User data pointer passed to the callback
///
/// # Returns
/// `MoqResult` with status code and error message (if any)
#[no_mangle]
pub unsafe extern "C" fn moq_connect(
    client: *mut MoqClient,
    url: *const c_char,
    connection_callback: MoqConnectionCallback,
    user_data: *mut std::ffi::c_void,
) -> MoqResult {
    std::panic::catch_unwind(|| {
        moq_connect_impl(client, url, connection_callback, user_data)
    }).unwrap_or_else(|_| {
        log::error!("Panic in moq_connect");
        set_last_error("Internal panic occurred in moq_connect".to_string());
        make_error_result(
            MoqResultCode::MoqErrorInternal,
            "Internal panic occurred"
        )
    })
}

unsafe fn moq_connect_impl(
    client: *mut MoqClient,
    url: *const c_char,
    connection_callback: MoqConnectionCallback,
    user_data: *mut std::ffi::c_void,
) -> MoqResult {
    if client.is_null() || url.is_null() {
        set_last_error("Client or URL is null".to_string());
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "Client or URL is null",
        );
    }

    let url_str = match CStr::from_ptr(url).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid UTF-8 in URL".to_string());
            return make_error_result(
                MoqResultCode::MoqErrorInvalidArgument,
                "Invalid UTF-8 in URL",
            );
        }
    };

    let client_ref = &*client;
    let mut inner = match client_ref.inner.lock() {
        Ok(inner) => inner,
        Err(_) => {
            set_last_error("Failed to lock client mutex".to_string());
            return make_error_result(
                MoqResultCode::MoqErrorInternal,
                "Failed to lock client mutex",
            );
        }
    };

    // Validate URL format - must be HTTPS for WebTransport over QUIC
    // Draft 07 (CloudFlare): WebTransport over QUIC only (current priority)
    // Draft 14 (Latest): WebTransport over QUIC
    // 
    // TODO(Draft 14): Add support for raw QUIC connections (quic:// URLs)
    // - Accept both https:// (WebTransport) and quic:// (raw QUIC) for Draft 14
    // - Implement direct QUIC connection without HTTP/3 handshake
    // - Use moq-transport Session::connect() with raw QUIC streams
    // - Add ALPN negotiation for MoQ protocol identification
    // - Reference: https://github.com/moq-wg/moq-transport
    if !url_str.starts_with("https://") {
        set_last_error(format!("Invalid URL scheme: {}", url_str));
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "URL must start with https:// (WebTransport over QUIC)",
        );
    }

    // Store connection callback
    inner.connection_callback = connection_callback;
    inner.connection_user_data = user_data as usize;
    inner.url = Some(url_str.clone());

    // Notify connecting state (with panic protection)
    if let Some(callback) = connection_callback {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            callback(user_data, MoqConnectionState::MoqStateConnecting);
        }));
    }

    // Parse URL
    let parsed_url = match url::Url::parse(&url_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("Failed to parse URL: {}", e));
            if let Some(callback) = connection_callback {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    callback(user_data, MoqConnectionState::MoqStateFailed);
                }));
            }
            return make_error_result(
                MoqResultCode::MoqErrorInvalidArgument,
                "Invalid URL format",
            );
        }
    };

    // Establish WebTransport connection over QUIC asynchronously
    // Priority: Draft 07 (CloudFlare production relay)
    // Both Draft 07 and Draft 14 use WebTransport over QUIC
    //
    // TODO(Draft 14): Implement connection type detection and routing
    // When Draft 14 raw QUIC support is added:
    // 1. Check URL scheme (https:// vs quic://)
    // 2. Route to appropriate connection method:
    //    - https:// -> WebTransport (current implementation)
    //    - quic:// -> Raw QUIC (to be implemented)
    // 3. Both should result in a compatible session for moq-transport
    let client_inner = client_ref.inner.clone();
    let url_str_clone = url_str.clone();
    let result = RUNTIME.block_on(async move {
        // Create quinn endpoint for WebTransport over QUIC
        let bind_addr = "[::]:0".parse()
            .map_err(|e| format!("Failed to parse bind address: {}", e))?;
        let mut endpoint = quinn::Endpoint::client(bind_addr)
            .map_err(|e| format!("Failed to create endpoint: {}", e))?;

        // Configure TLS with native root certificates  
        let mut roots = rustls::RootCertStore::empty();
        let native_certs = rustls_native_certs::load_native_certs();
        
        // Log any errors that occurred while loading certificates
        for err in native_certs.errors {
            log::warn!("Failed to load native cert: {:?}", err);
        }
        
        // Add valid certificates to the store
        for cert in native_certs.certs {
            if let Err(e) = roots.add(cert) {
                log::warn!("Failed to add root cert: {:?}", e);
            }
        }

        let client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        let mut client_config = quinn::ClientConfig::new(std::sync::Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
                .map_err(|e| format!("Crypto config error: {}", e))?
        ));
        
        // Configure transport - enable datagrams for MoQ datagram delivery
        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_concurrent_bidi_streams(100u32.into());
        transport_config.max_concurrent_uni_streams(100u32.into());
        transport_config.datagram_receive_buffer_size(Some(1024 * 1024)); // 1MB buffer
        transport_config.datagram_send_buffer_size(1024 * 1024); // 1MB send buffer
        client_config.transport_config(std::sync::Arc::new(transport_config));
        
        endpoint.set_default_client_config(client_config);

        // Connect via WebTransport (HTTP/3 over QUIC)
        #[cfg(feature = "with_moq_draft07")]
        log::info!("Connecting via WebTransport over QUIC to {} (Draft 07 - CloudFlare)", url_str_clone);
        
        #[cfg(feature = "with_moq")]
        log::info!("Connecting via WebTransport over QUIC to {} (Draft 14 - Latest)", url_str_clone);
        
        use web_transport_quinn::connect as wt_connect;
        let wt_session_quinn = wt_connect(&endpoint, &parsed_url)
            .await
            .map_err(|e| format!("Failed to connect via WebTransport: {}", e))?;
        
        // Convert to generic web_transport::Session
        let wt_session = web_transport::Session::from(wt_session_quinn);

        log::info!("WebTransport session established to {}", url_str_clone);

        // Establish MoQ session over the transport
        let (moq_session, publisher, subscriber) = Session::connect(wt_session)
            .await
            .map_err(|e| format!("Failed to establish MoQ session: {}", e))?;

        log::info!("MoQ session established");

        // Store session and publisher/subscriber
        let mut inner = client_inner.lock()
            .map_err(|e| format!("Failed to lock client mutex: {}", e))?;
        inner.publisher = Some(publisher);
        inner.subscriber = Some(subscriber);
        inner.connected = true;

        // Notify connection success via callback (with panic protection)
        if let Some(callback) = inner.connection_callback {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback(inner.connection_user_data as *mut std::ffi::c_void, MoqConnectionState::MoqStateConnected);
            }));
        }

        // Spawn task to run the session
        let task = RUNTIME.spawn(async move {
            if let Err(e) = moq_session.run().await {
                log::error!("MoQ session error: {}", e);
            }
        });
        inner.session_task = Some(task);

        Ok::<(), String>(())
    });

    match result {
        Ok(()) => {
            log::info!("Connected to {} successfully", url_str);
            make_ok_result()
        }
        Err(e) => {
            log::error!("Connection failed: {}", e);
            set_last_error(e.clone());
            
            // Notify connection failure and clean up partial state
            let inner_result = client_ref.inner.lock();
            let mut inner = match inner_result {
                Ok(guard) => guard,
                Err(poisoned) => {
                    log::warn!("Mutex poisoned during connection failure, recovering");
                    poisoned.into_inner()
                }
            };
            inner.connected = false;
            inner.url = None;
            inner.connection_callback = None;
            inner.connection_user_data = 0;
            
            if let Some(callback) = connection_callback {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    callback(user_data, MoqConnectionState::MoqStateFailed);
                }));
            }
            
            make_error_result(
                MoqResultCode::MoqErrorConnectionFailed,
                &e,
            )
        }
    }
}

/// Disconnects from the MoQ relay server.
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - This function is thread-safe
/// - Closes active connection and aborts async tasks
///
/// # Parameters
/// - `client`: Pointer to the MoQ client
///
/// # Returns
/// `MoqResult` with status code and error message (if any)
#[no_mangle]
pub unsafe extern "C" fn moq_disconnect(client: *mut MoqClient) -> MoqResult {
    std::panic::catch_unwind(|| {
        if client.is_null() {
            set_last_error("Client is null".to_string());
            return make_error_result(MoqResultCode::MoqErrorInvalidArgument, "Client is null");
        }

        let client_ref = &*client;
        let inner_result = client_ref.inner.lock();
        let mut inner = match inner_result {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Mutex poisoned during disconnect, recovering");
                poisoned.into_inner()
            }
        };

        // Abort session task if running
        if let Some(task) = inner.session_task.take() {
            task.abort();
        }

        // Clear session state
        inner.session = None;
        inner.publisher = None;
        inner.subscriber = None;
        inner.announced_namespaces.clear();
        inner.connected = false;
        inner.url = None;

        // Notify disconnected state (with panic protection)
        if let Some(callback) = inner.connection_callback {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback(inner.connection_user_data as *mut std::ffi::c_void, MoqConnectionState::MoqStateDisconnected);
            }));
        }

        log::info!("Disconnected from MoQ server");
        make_ok_result()
    }).unwrap_or_else(|_| {
        log::error!("Panic in moq_disconnect");
        set_last_error("Internal panic occurred in moq_disconnect".to_string());
        make_error_result(
            MoqResultCode::MoqErrorInternal,
            "Internal panic occurred"
        )
    })
}

/// Checks if the client is currently connected to a relay server.
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` may be null (returns false)
/// - This function is thread-safe
///
/// # Parameters
/// - `client`: Pointer to the MoQ client
///
/// # Returns
/// `true` if connected, `false` otherwise (including if client is null)
#[no_mangle]
pub unsafe extern "C" fn moq_is_connected(client: *const MoqClient) -> bool {
    std::panic::catch_unwind(|| {
        if client.is_null() {
            return false;
        }

        let client_ref = &*client;
        let inner_result = client_ref.inner.lock();
        let inner = match inner_result {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Mutex poisoned in moq_is_connected, recovering");
                poisoned.into_inner()
            }
        };

        inner.connected
    }).unwrap_or(false)
}

/* ───────────────────────────────────────────────
 * Publishing
 * ─────────────────────────────────────────────── */

/// Announces a namespace to the MoQ relay server.
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - `namespace` must be a valid null-terminated C string pointer
/// - `namespace` must not be null
/// - Client must be connected before calling this function
/// - This function is thread-safe
///
/// # Parameters
/// - `client`: Pointer to the MoQ client
/// - `namespace`: Namespace string (slash-separated path, e.g., "example/namespace")
///
/// # Returns
/// `MoqResult` with status code and error message (if any)
#[no_mangle]
pub unsafe extern "C" fn moq_announce_namespace(
    client: *mut MoqClient,
    namespace: *const c_char,
) -> MoqResult {
    std::panic::catch_unwind(|| {
        moq_announce_namespace_impl(client, namespace)
    }).unwrap_or_else(|_| {
        log::error!("Panic in moq_announce_namespace");
        set_last_error("Internal panic occurred in moq_announce_namespace".to_string());
        make_error_result(
            MoqResultCode::MoqErrorInternal,
            "Internal panic occurred"
        )
    })
}

unsafe fn moq_announce_namespace_impl(
    client: *mut MoqClient,
    namespace: *const c_char,
) -> MoqResult {
    if client.is_null() || namespace.is_null() {
        set_last_error("Client or namespace is null".to_string());
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "Client or namespace is null",
        );
    }

    let namespace_str = match CStr::from_ptr(namespace).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid UTF-8 in namespace".to_string());
            return make_error_result(
                MoqResultCode::MoqErrorInvalidArgument,
                "Invalid UTF-8 in namespace",
            );
        }
    };

    let client_ref = &*client;
    let inner_result = client_ref.inner.lock();
    let inner = match inner_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("Mutex poisoned in moq_announce_namespace, recovering");
            poisoned.into_inner()
        }
    };

    if !inner.connected {
        set_last_error("Not connected to MoQ server".to_string());
        return make_error_result(
            MoqResultCode::MoqErrorNotConnected,
            "Not connected to MoQ server",
        );
    }

    // Get mutable reference for announcing
    drop(inner);
    let inner_result = client_ref.inner.lock();
    let mut inner = match inner_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("Mutex poisoned in moq_announce_namespace, recovering");
            poisoned.into_inner()
        }
    };

    // Parse namespace from string (using slash-separated path)
    let track_namespace = TrackNamespace::from_utf8_path(&namespace_str);

    if inner.announced_namespaces.contains_key(&track_namespace) {
        set_last_error(format!("Namespace already announced: {}", namespace_str));
        return make_error_result(
            MoqResultCode::MoqErrorInternal,
            "Namespace already announced",
        );
    }

    // Get publisher
    let mut publisher = match inner.publisher.as_mut() {
        Some(p) => p.clone(),
        None => {
            set_last_error("Publisher not available".to_string());
            return make_error_result(
                MoqResultCode::MoqErrorNotConnected,
                "Publisher not available",
            );
        }
    };

    // Create tracks for this namespace
    let (tracks_writer, _tracks_request, tracks_reader) = serve::Tracks::new(track_namespace.clone()).produce();

    // Spawn task to announce and handle subscriptions
    let track_namespace_clone = track_namespace.clone();
    let client_inner = client_ref.inner.clone();
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

    log::info!("Announced namespace: {}", namespace_str);
    make_ok_result()
}

/// Creates a publisher for a specific track (stream mode by default).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - `namespace` must be a valid null-terminated C string pointer
/// - `namespace` must not be null
/// - `track_name` must be a valid null-terminated C string pointer
/// - `track_name` must not be null
/// - Namespace must be announced before creating publisher
/// - Client must be connected
/// - This function is thread-safe
///
/// # Parameters
/// - `client`: Pointer to the MoQ client
/// - `namespace`: Namespace string (must be previously announced)
/// - `track_name`: Track name string
///
/// # Returns
/// Pointer to the created publisher, or null on failure
#[no_mangle]
pub unsafe extern "C" fn moq_create_publisher(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
) -> *mut MoqPublisher {
    // Default to stream mode for backward compatibility
    moq_create_publisher_ex(client, namespace, track_name, MoqDeliveryMode::MoqDeliveryStream)
}

/// Creates a publisher for a specific track with explicit delivery mode.
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - `namespace` must be a valid null-terminated C string pointer
/// - `namespace` must not be null
/// - `track_name` must be a valid null-terminated C string pointer
/// - `track_name` must not be null
/// - Namespace must be announced before creating publisher
/// - Client must be connected
/// - This function is thread-safe
///
/// # Parameters
/// - `client`: Pointer to the MoQ client
/// - `namespace`: Namespace string (must be previously announced)
/// - `track_name`: Track name string
/// - `delivery_mode`: Delivery mode (stream or datagram)
///
/// # Returns
/// Pointer to the created publisher, or null on failure
#[no_mangle]
pub unsafe extern "C" fn moq_create_publisher_ex(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
    delivery_mode: MoqDeliveryMode,
) -> *mut MoqPublisher {
    std::panic::catch_unwind(|| {
        moq_create_publisher_ex_impl(client, namespace, track_name, delivery_mode)
    }).unwrap_or_else(|_| {
        log::error!("Panic in moq_create_publisher_ex");
        set_last_error("Internal panic occurred in moq_create_publisher_ex".to_string());
        std::ptr::null_mut()
    })
}

unsafe fn moq_create_publisher_ex_impl(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
    delivery_mode: MoqDeliveryMode,
) -> *mut MoqPublisher {
    if client.is_null() || namespace.is_null() || track_name.is_null() {
        set_last_error("Client, namespace, or track_name is null".to_string());
        return std::ptr::null_mut();
    }

    let namespace_str = match CStr::from_ptr(namespace).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid UTF-8 in namespace".to_string());
            return std::ptr::null_mut();
        }
    };

    let track_name_str = match CStr::from_ptr(track_name).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid UTF-8 in track_name".to_string());
            return std::ptr::null_mut();
        }
    };

    let client_ref = &*client;
    let mut inner = match client_ref.inner.lock() {
        Ok(inner) => inner,
        Err(_) => {
            set_last_error("Failed to lock client mutex".to_string());
            return std::ptr::null_mut();
        }
    };

    if !inner.connected {
        set_last_error("Not connected to MoQ server".to_string());
        return std::ptr::null_mut();
    }

    // Parse namespace
    let track_namespace = TrackNamespace::from_utf8_path(&namespace_str);

    // Get the tracks writer for this namespace
    let tracks_writer = match inner.announced_namespaces.get_mut(&track_namespace) {
        Some(tw) => tw,
        None => {
            set_last_error(format!("Namespace not announced: {}", namespace_str));
            return std::ptr::null_mut();
        }
    };

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
                Err(e) => {
                    set_last_error(format!("Failed to create datagram writer: {}", e));
                    return std::ptr::null_mut();
                }
            }
        }
        MoqDeliveryMode::MoqDeliveryStream => {
            match track.stream(0) {
                Ok(s) => PublisherMode::Stream(s),
                Err(e) => {
                    set_last_error(format!("Failed to create stream writer: {}", e));
                    return std::ptr::null_mut();
                }
            }
        }
    };

    drop(inner);

    // Create publisher
    let publisher = MoqPublisher {
        inner: Arc::new(Mutex::new(PublisherInner {
            namespace: track_namespace,
            track_name: track_name_str.clone(),
            mode,
            group_id_counter: std::sync::atomic::AtomicU64::new(0),
        })),
    };

    log::info!("Created publisher for {}/{} (mode: {:?})", namespace_str, track_name_str, delivery_mode);
    Box::into_raw(Box::new(publisher))
}

/// Destroys a publisher and releases its resources.
///
/// # Safety
/// - `publisher` must be a valid pointer returned from `moq_create_publisher()` or `moq_create_publisher_ex()`
/// - `publisher` must not be null (null pointers are safely ignored)
/// - `publisher` must not be accessed after this function returns
/// - This function is thread-safe
///
/// # Parameters
/// - `publisher`: Pointer to the publisher to destroy, or null (null is safely ignored)
#[no_mangle]
pub unsafe extern "C" fn moq_publisher_destroy(publisher: *mut MoqPublisher) {
    let _ = std::panic::catch_unwind(|| {
        if !publisher.is_null() {
            let _ = Box::from_raw(publisher);
            log::debug!("Destroyed publisher");
        }
    });
    // Silently handle panics - destructor should not propagate panics
}

/// Publishes data to a track.
///
/// # Safety
/// - `publisher` must be a valid pointer returned from `moq_create_publisher()` or `moq_create_publisher_ex()`
/// - `publisher` must not be null
/// - `data` must be a valid pointer to a buffer of at least `data_len` bytes
/// - `data` must not be null if `data_len` > 0
/// - `data` may be null if `data_len` is 0
/// - This function is thread-safe
/// - Data is copied, so the buffer can be freed after this function returns
///
/// # Parameters
/// - `publisher`: Pointer to the publisher
/// - `data`: Pointer to the data buffer
/// - `data_len`: Length of the data in bytes
/// - `_delivery_mode`: Ignored (delivery mode is set at publisher creation)
///
/// # Returns
/// `MoqResult` with status code and error message (if any)
#[no_mangle]
pub unsafe extern "C" fn moq_publish_data(
    publisher: *mut MoqPublisher,
    data: *const u8,
    data_len: usize,
    _delivery_mode: MoqDeliveryMode,
) -> MoqResult {
    std::panic::catch_unwind(|| {
        moq_publish_data_impl(publisher, data, data_len, _delivery_mode)
    }).unwrap_or_else(|_| {
        log::error!("Panic in moq_publish_data");
        set_last_error("Internal panic occurred in moq_publish_data".to_string());
        make_error_result(
            MoqResultCode::MoqErrorInternal,
            "Internal panic occurred"
        )
    })
}

unsafe fn moq_publish_data_impl(
    publisher: *mut MoqPublisher,
    data: *const u8,
    data_len: usize,
    _delivery_mode: MoqDeliveryMode,
) -> MoqResult {
    if publisher.is_null() {
        set_last_error("Publisher is null".to_string());
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "Publisher is null",
        );
    }

    // Validate data pointer with length check
    if data.is_null() && data_len > 0 {
        set_last_error("Data is null but data_len is non-zero".to_string());
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "Data is null but data_len is non-zero",
        );
    }

    let publisher_ref = &*publisher;
    let inner_result = publisher_ref.inner.lock();
    let mut inner = match inner_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("Mutex poisoned in moq_publish_data, recovering");
            poisoned.into_inner()
        }
    };

    // Copy data to Bytes (handle empty data case)
    // Note: data_len == 0 case already validated above (null with non-zero length rejected)
    let data_bytes = if data_len == 0 {
        bytes::Bytes::new()
    } else {
        let data_slice = std::slice::from_raw_parts(data, data_len);
        bytes::Bytes::copy_from_slice(data_slice)
    };

    let namespace = inner.namespace.clone();
    let track_name = inner.track_name.clone();
    
    // Get counter value before borrowing mode mutably
    let counter_val = inner.group_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    
    // Publish data based on mode (using synchronous API)
    // Note: delivery_mode parameter is ignored; mode is set at publisher creation
    let result = match &mut inner.mode {
        PublisherMode::Datagrams(datagrams) => {
            // Create a datagram with metadata
            #[cfg(feature = "with_moq")]
            let datagram = serve::Datagram {
                group_id: 0,
                object_id: counter_val,
                priority: 0,
                payload: data_bytes,
            };
            
            #[cfg(feature = "with_moq_draft07")]
            let datagram = serve::Datagram {
                group_id: 0,
                object_id: counter_val,
                priority: 0,
                status: moq::data::ObjectStatus::Object,
                payload: data_bytes,
            };
            
            datagrams.write(datagram)
                .map_err(|e| format!("Failed to write datagram: {}", e))
                .map(|_| {
                    log::debug!("Published {} bytes to {:?}/{} via datagram", data_len, namespace, track_name);
                })
        }
        PublisherMode::Stream(stream) => {
            stream.create(counter_val)
                .and_then(|mut group| {
                    group.write(data_bytes)
                })
                .map_err(|e| format!("Failed to write to stream: {}", e))
                .map(|_| {
                    log::debug!("Published {} bytes to {:?}/{} via stream", data_len, namespace, track_name);
                })
        }
    };

    match result {
        Ok(()) => make_ok_result(),
        Err(e) => {
            set_last_error(e.clone());
            make_error_result(MoqResultCode::MoqErrorInternal, &e)
        }
    }
}

/* ───────────────────────────────────────────────
 * Subscribing
 * ─────────────────────────────────────────────── */

/// Subscribes to a track on the MoQ relay server.
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - `namespace` must be a valid null-terminated C string pointer
/// - `namespace` must not be null
/// - `track_name` must be a valid null-terminated C string pointer
/// - `track_name` must not be null
/// - `data_callback` may be null (no data will be received)
/// - `user_data` will be passed to the callback and may be null
/// - Client must be connected
/// - This function is thread-safe
///
/// # Parameters
/// - `client`: Pointer to the MoQ client
/// - `namespace`: Namespace string (slash-separated path)
/// - `track_name`: Track name string
/// - `data_callback`: Optional callback for received data
/// - `user_data`: User data pointer passed to the callback
///
/// # Returns
/// Pointer to the created subscriber, or null on failure
#[no_mangle]
pub unsafe extern "C" fn moq_subscribe(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
    data_callback: MoqDataCallback,
    user_data: *mut std::ffi::c_void,
) -> *mut MoqSubscriber {
    std::panic::catch_unwind(|| {
        moq_subscribe_impl(client, namespace, track_name, data_callback, user_data)
    }).unwrap_or_else(|_| {
        log::error!("Panic in moq_subscribe");
        set_last_error("Internal panic occurred in moq_subscribe".to_string());
        std::ptr::null_mut()
    })
}

unsafe fn moq_subscribe_impl(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
    data_callback: MoqDataCallback,
    user_data: *mut std::ffi::c_void,
) -> *mut MoqSubscriber {
    if client.is_null() || namespace.is_null() || track_name.is_null() {
        set_last_error("Client, namespace, or track_name is null".to_string());
        return std::ptr::null_mut();
    }

    let namespace_str = match CStr::from_ptr(namespace).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid UTF-8 in namespace".to_string());
            return std::ptr::null_mut();
        }
    };

    let track_name_str = match CStr::from_ptr(track_name).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid UTF-8 in track_name".to_string());
            return std::ptr::null_mut();
        }
    };

    let client_ref = &*client;
    let inner_result = client_ref.inner.lock();
    let mut inner = match inner_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("Mutex poisoned in moq_subscribe, recovering");
            poisoned.into_inner()
        }
    };

    if !inner.connected {
        set_last_error("Not connected to MoQ server".to_string());
        return std::ptr::null_mut();
    }

    // Parse namespace
    let track_namespace = TrackNamespace::from_utf8_path(&namespace_str);

    // Get subscriber
    let mut subscriber_impl = match inner.subscriber.as_mut() {
        Some(s) => s.clone(),
        None => {
            set_last_error("Subscriber not available".to_string());
            return std::ptr::null_mut();
        }
    };

    drop(inner);

    // Create track writer/reader pair for subscription
    let (track_writer, track_reader) = serve::Track::new(track_namespace.clone(), track_name_str.clone()).produce();

    // Subscribe to the track
    let subscribe_result = RUNTIME.block_on(async move {
        subscriber_impl.subscribe(track_writer).await
            .map_err(|e| format!("Failed to subscribe: {}", e))
    });

    if let Err(e) = subscribe_result {
        set_last_error(e.clone());
        log::error!("Subscription failed: {}", e);
        return std::ptr::null_mut();
    }

    // Create subscriber and spawn task to read incoming data
    let subscriber_inner = Arc::new(Mutex::new(SubscriberInner {
        namespace: track_namespace.clone(),
        track_name: track_name_str.clone(),
        data_callback,
        user_data: user_data as usize,
        track: track_reader.clone(),
        reader_task: None,
    }));

    // Clone values for the async task
    let track_namespace_log = track_namespace.clone();
    let track_name_log = track_name_str.clone();
    
    // Spawn task to read data from track
    let inner_clone = subscriber_inner.clone();
    let reader_task = RUNTIME.spawn(async move {
        let track = {
            match inner_clone.lock() {
                Ok(inner) => inner.track.clone(),
                Err(e) => {
                    log::error!("Failed to lock subscriber mutex: {}", e);
                    return;
                }
            }
        };

        // Get the mode once (it's set when track is established)
        let mode_result = track.mode().await;
        match mode_result {
            Ok(mode) => {
                use moq::serve::TrackReaderMode;
                
                loop {
                    let data_result = match &mode {
                        TrackReaderMode::Stream(stream) => {
                            // Read from stream - groups contain objects
                            let mut stream = stream.clone();
                            match stream.next().await {
                                Ok(Some(mut group)) => {
                                    match group.next().await {
                                        Ok(Some(mut object)) => {
                                            // Read all chunks from object
                                            let mut buffer = Vec::new();
                                            loop {
                                                match object.read().await {
                                                    Ok(Some(chunk)) => {
                                                        buffer.extend_from_slice(&chunk);
                                                    }
                                                    Ok(None) => break,
                                                    Err(_) => break,
                                                }
                                            }
                                            if !buffer.is_empty() {
                                                Some(buffer)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    }
                                }
                                Ok(None) => {
                                    log::info!("Stream ended: {:?}/{}", track_namespace_log, track_name_log);
                                    break;
                                }
                                Err(e) => {
                                    log::error!("Stream read error: {}", e);
                                    None
                                }
                            }
                        }
                        #[cfg(feature = "with_moq")]
                        TrackReaderMode::Subgroups(subgroups) => {
                            // Read from subgroups (Draft 14)
                            let mut subgroups = subgroups.clone();
                            match subgroups.next().await {
                                Ok(Some(mut subgroup)) => {
                                    match subgroup.next().await {
                                        Ok(Some(mut object)) => {
                                            // Read all chunks from object
                                            let mut buffer = Vec::new();
                                            loop {
                                                match object.read().await {
                                                    Ok(Some(chunk)) => buffer.extend_from_slice(&chunk),
                                                    Ok(None) => break,
                                                    Err(_) => break,
                                                }
                                            }
                                            if !buffer.is_empty() {
                                                Some(buffer)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    }
                                }
                                Ok(None) => {
                                    log::info!("Subgroups ended: {:?}/{}", track_namespace_log, track_name_log);
                                    break;
                                }
                                Err(e) => {
                                    log::error!("Subgroups read error: {}", e);
                                    None
                                }
                            }
                        }
                        #[cfg(feature = "with_moq_draft07")]
                        TrackReaderMode::Subgroups(subgroups) => {
                            // Read from subgroups (Draft 07)
                            let mut subgroups = subgroups.clone();
                            match subgroups.next().await {
                                Ok(Some(mut group)) => {
                                    match group.next().await {
                                        Ok(Some(mut object)) => {
                                            // Read all chunks from object
                                            let mut buffer = Vec::new();
                                            loop {
                                                match object.read().await {
                                                    Ok(Some(chunk)) => buffer.extend_from_slice(&chunk),
                                                    Ok(None) => break,
                                                    Err(_) => break,
                                                }
                                            }
                                            if !buffer.is_empty() {
                                                Some(buffer)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    }
                                }
                                Ok(None) => {
                                    log::info!("Subgroups ended: {:?}/{}", track_namespace_log, track_name_log);
                                    break;
                                }
                                Err(e) => {
                                    log::error!("Subgroups read error: {}", e);
                                    None
                                }
                            }
                        }
                        TrackReaderMode::Datagrams(datagrams) => {
                            // Read datagram
                            let mut datagrams = datagrams.clone();
                            match datagrams.read().await {
                                Ok(Some(datagram)) => {
                                    // Datagram payload - API is same for both drafts
                                    Some(datagram.payload.to_vec())
                                }
                                Ok(None) => {
                                    log::info!("Datagrams ended: {:?}/{}", track_namespace_log, track_name_log);
                                    break;
                                }
                                Err(e) => {
                                    log::error!("Datagram read error: {}", e);
                                    None
                                }
                            }
                        }
                    };

                    // Invoke callback if we got data (with panic protection)
                    if let Some(buffer) = data_result {
                        let inner_result = inner_clone.lock();
                        let inner = match inner_result {
                            Ok(guard) => guard,
                            Err(poisoned) => {
                                log::warn!("Mutex poisoned in subscriber callback, recovering");
                                poisoned.into_inner()
                            }
                        };
                        if let Some(callback) = inner.data_callback {
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                callback(inner.user_data as *mut std::ffi::c_void, buffer.as_ptr(), buffer.len());
                            }));
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to get track mode: {}", e);
            }
        }
    });

    // Store reader task (with proper error handling)
    {
        let inner_result = subscriber_inner.lock();
        let mut inner = match inner_result {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Mutex poisoned when storing reader task, recovering");
                poisoned.into_inner()
            }
        };
        inner.reader_task = Some(reader_task);
    } // Drop the guard before moving subscriber_inner

    let subscriber = MoqSubscriber {
        inner: subscriber_inner,
    };

    log::info!("Subscribed to {}/{}", namespace_str, track_name_str);
    Box::into_raw(Box::new(subscriber))
}

/// Destroys a subscriber and releases its resources.
///
/// # Safety
/// - `subscriber` must be a valid pointer returned from `moq_subscribe()`
/// - `subscriber` must not be null (null pointers are safely ignored)
/// - `subscriber` must not be accessed after this function returns
/// - This function is thread-safe
/// - Active reader task will be aborted
///
/// # Parameters
/// - `subscriber`: Pointer to the subscriber to destroy, or null (null is safely ignored)
#[no_mangle]
pub unsafe extern "C" fn moq_subscriber_destroy(subscriber: *mut MoqSubscriber) {
    let _ = std::panic::catch_unwind(|| {
        if !subscriber.is_null() {
            let subscriber = Box::from_raw(subscriber);
            
            // Cancel reader task (with proper error handling)
            let inner_result = subscriber.inner.lock();
            let mut inner = match inner_result {
                Ok(guard) => guard,
                Err(poisoned) => {
                    log::warn!("Mutex poisoned in moq_subscriber_destroy, recovering");
                    poisoned.into_inner()
                }
            };
            if let Some(task) = inner.reader_task.take() {
                task.abort();
            }
            
            log::debug!("Destroyed subscriber for {:?}/{}", inner.namespace, inner.track_name);
        }
    });
    // Silently handle panics - destructor should not propagate panics
}

/* ───────────────────────────────────────────────
 * Utilities
 * ─────────────────────────────────────────────── */

/// Frees a string allocated by the FFI library.
///
/// # Safety
/// - `s` must be a valid pointer returned by a moq_ffi function that requires freeing
/// - `s` must not be null (null pointers are safely ignored)
/// - `s` must not be accessed after this function returns
/// - `s` must not be a static string (like those from moq_version or moq_last_error)
/// - This function is thread-safe
///
/// # Parameters
/// - `s`: Pointer to the string to free, or null (null is safely ignored)
///
/// # Note
/// Only use this for strings in MoqResult.message fields where code != MOQ_OK.
/// Do NOT use this for moq_version() or moq_last_error() return values.
#[no_mangle]
pub unsafe extern "C" fn moq_free_str(s: *const c_char) {
    let _ = std::panic::catch_unwind(|| {
        if !s.is_null() {
            let _ = CString::from_raw(s as *mut c_char);
        }
    });
    // Silently handle panics - free function should not propagate panics
}

/// Returns the version string of the library.
///
/// # Returns
/// A pointer to a static null-terminated C string containing the version.
/// This string must NOT be freed - it is a static string.
///
/// # Thread Safety
/// This function is thread-safe.
#[no_mangle]
pub extern "C" fn moq_version() -> *const c_char {
    #[cfg(feature = "with_moq")]
    {
        const VERSION: &[u8] = b"moq_ffi 0.1.0 (IETF Draft 14)\0";
        VERSION.as_ptr() as *const c_char
    }
    
    #[cfg(feature = "with_moq_draft07")]
    {
        const VERSION: &[u8] = b"moq_ffi 0.1.0 (IETF Draft 07)\0";
        VERSION.as_ptr() as *const c_char
    }
    
    #[cfg(not(any(feature = "with_moq", feature = "with_moq_draft07")))]
    {
        // Stub build - no MoQ transport
        const VERSION: &[u8] = b"moq_ffi 0.1.0 (stub)\0";
        VERSION.as_ptr() as *const c_char
    }
}

/// Returns the last error message for this thread.
///
/// # Returns
/// A pointer to a null-terminated C string containing the last error message,
/// or null if no error has occurred. The string is valid until the next error
/// occurs in this thread or the thread exits. The caller must NOT free this pointer.
///
/// # Thread Safety
/// This function is thread-safe. Each thread has its own error storage.
///
/// # Note
/// This pointer is thread-local and should NOT be freed with moq_free_str().
#[no_mangle]
pub extern "C" fn moq_last_error() -> *const c_char {
    // Note: This returns a pointer that is valid until the next error occurs
    // in this thread. The caller should NOT free this pointer.
    // This is a common pattern in C FFI for error reporting.
    match get_last_error() {
        Some(err) => {
            // Create a static string that we'll reuse per thread
            thread_local! {
                static ERROR_BUF: std::cell::RefCell<Option<CString>> = const { std::cell::RefCell::new(None) };
            }
            
            ERROR_BUF.with(|buf| {
                let c_str = CString::new(err).unwrap_or_else(|_| CString::new("Unknown error").unwrap());
                *buf.borrow_mut() = Some(c_str);
                buf.borrow().as_ref().unwrap().as_ptr()
            })
        }
        None => std::ptr::null(),
    }
}
