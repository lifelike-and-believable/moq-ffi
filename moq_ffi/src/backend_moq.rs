// Full backend with moq-transport integration
//
// This backend provides implementations of all FFI functions using the moq-transport library.
// The implementation uses moq-transport types and follows MoQ protocol patterns.
//
// Current state:
// - FFI layer is production-ready with proper error handling and memory safety
// - Infrastructure for async operations (Tokio runtime, thread-safe state) is complete
// - MoQ transport types are integrated for track namespaces and data structures
// - Connection simulation allows testing and integration without relay server
// - Ready for WebTransport/QUIC integration when relay endpoint is available

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use tokio::runtime::Runtime;
use once_cell::sync::Lazy;

// MoQ transport types
use moq_transport::coding::TrackNamespace;

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
    static LAST_ERROR: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
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

use moq_transport::{
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

#[no_mangle]
pub extern "C" fn moq_client_create() -> *mut MoqClient {
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
}

#[no_mangle]
pub unsafe extern "C" fn moq_client_destroy(client: *mut MoqClient) {
    if !client.is_null() {
        let _ = Box::from_raw(client);
    }
}

#[no_mangle]
pub unsafe extern "C" fn moq_connect(
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

    // Validate URL format
    if !url_str.starts_with("https://") {
        set_last_error(format!("Invalid URL scheme: {}", url_str));
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "URL must start with https:// (WebTransport requires HTTPS)",
        );
    }

    // Store connection callback
    inner.connection_callback = connection_callback;
    inner.connection_user_data = user_data as usize;
    inner.url = Some(url_str.clone());

    // Notify connecting state
    if let Some(callback) = connection_callback {
        callback(user_data, MoqConnectionState::MoqStateConnecting);
    }

    // Parse URL for WebTransport
    let parsed_url = match url::Url::parse(&url_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("Failed to parse URL: {}", e));
            if let Some(callback) = connection_callback {
                callback(user_data, MoqConnectionState::MoqStateFailed);
            }
            return make_error_result(
                MoqResultCode::MoqErrorInvalidArgument,
                "Invalid URL format",
            );
        }
    };

    // Establish WebTransport connection asynchronously
    // NOTE: This requires an actual MoQ relay server to be running at the URL.
    // For testing without a relay, this will fail gracefully.
    let client_inner = client_ref.inner.clone();
    let url_str_clone = url_str.clone();
    let result = RUNTIME.block_on(async move {
        // Create quinn endpoint for WebTransport
        // Using default client configuration with native certificates
        let mut endpoint = quinn::Endpoint::client("[::]:0".parse().unwrap())
            .map_err(|e| format!("Failed to create endpoint: {}", e))?;

        // Configure TLS with native root certificates  
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            roots.add(cert).ok();
        }

        let client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        let mut client_config = quinn::ClientConfig::new(std::sync::Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
                .map_err(|e| format!("Crypto config error: {}", e))?
        ));
        
        // Set HTTP/3 ALPN
        client_config.transport_config(std::sync::Arc::new(quinn::TransportConfig::default()));
        endpoint.set_default_client_config(client_config);

        // Connect via WebTransport using web_transport_quinn
        // web-transport 0.3 re-exports from web_transport_quinn, but we call it directly
        use web_transport_quinn::connect as wt_connect;
        let wt_session_quinn = wt_connect(&endpoint, &parsed_url)
            .await
            .map_err(|e| format!("Failed to connect via WebTransport: {}", e))?;
        
        // Convert to generic web_transport::Session
        let wt_session = web_transport::Session::from(wt_session_quinn);

        log::info!("WebTransport session established to {}", url_str_clone);

        // Establish MoQ session over WebTransport
        let (moq_session, publisher, subscriber) = Session::connect(wt_session)
            .await
            .map_err(|e| format!("Failed to establish MoQ session: {}", e))?;

        log::info!("MoQ session established");

        // Store session and publisher/subscriber
        let mut inner = client_inner.lock().unwrap();
        inner.publisher = Some(publisher);
        inner.subscriber = Some(subscriber);
        inner.connected = true;

        // Notify connection success via callback
        if let Some(callback) = inner.connection_callback {
            callback(inner.connection_user_data as *mut std::ffi::c_void, MoqConnectionState::MoqStateConnected);
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
            
            // Notify connection failure
            let mut inner = client_ref.inner.lock().unwrap();
            inner.connected = false;
            if let Some(callback) = connection_callback {
                callback(user_data, MoqConnectionState::MoqStateFailed);
            }
            
            make_error_result(
                MoqResultCode::MoqErrorConnectionFailed,
                &e,
            )
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn moq_disconnect(client: *mut MoqClient) -> MoqResult {
    if client.is_null() {
        set_last_error("Client is null".to_string());
        return make_error_result(MoqResultCode::MoqErrorInvalidArgument, "Client is null");
    }

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

    // Notify disconnected state
    if let Some(callback) = inner.connection_callback {
        callback(inner.connection_user_data as *mut std::ffi::c_void, MoqConnectionState::MoqStateDisconnected);
    }

    log::info!("Disconnected from MoQ server");
    make_ok_result()
}

#[no_mangle]
pub unsafe extern "C" fn moq_is_connected(client: *const MoqClient) -> bool {
    if client.is_null() {
        return false;
    }

    let client_ref = &*client;
    let inner = match client_ref.inner.lock() {
        Ok(inner) => inner,
        Err(_) => return false,
    };

    inner.connected
}

/* ───────────────────────────────────────────────
 * Publishing
 * ─────────────────────────────────────────────── */

#[no_mangle]
pub unsafe extern "C" fn moq_announce_namespace(
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
    let inner = match client_ref.inner.lock() {
        Ok(inner) => inner,
        Err(_) => {
            set_last_error("Failed to lock client mutex".to_string());
            return make_error_result(
                MoqResultCode::MoqErrorInternal,
                "Failed to lock client mutex",
            );
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

#[no_mangle]
pub unsafe extern "C" fn moq_create_publisher(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
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

    // Default to stream mode (can support choosing mode later)
    let mode = match track.stream(0) {
        Ok(s) => PublisherMode::Stream(s),
        Err(e) => {
            set_last_error(format!("Failed to create stream writer: {}", e));
            return std::ptr::null_mut();
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

    log::info!("Created publisher for {}/{}", namespace_str, track_name_str);
    Box::into_raw(Box::new(publisher))
}

#[no_mangle]
pub unsafe extern "C" fn moq_publisher_destroy(publisher: *mut MoqPublisher) {
    if !publisher.is_null() {
        let _ = Box::from_raw(publisher);
        log::debug!("Destroyed publisher");
    }
}

#[no_mangle]
pub unsafe extern "C" fn moq_publish_data(
    publisher: *mut MoqPublisher,
    data: *const u8,
    data_len: usize,
    _delivery_mode: MoqDeliveryMode,
) -> MoqResult {
    if publisher.is_null() || data.is_null() {
        set_last_error("Publisher or data is null".to_string());
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "Publisher or data is null",
        );
    }

    let publisher_ref = &*publisher;
    let mut inner = match publisher_ref.inner.lock() {
        Ok(inner) => inner,
        Err(_) => {
            set_last_error("Failed to lock publisher mutex".to_string());
            return make_error_result(
                MoqResultCode::MoqErrorInternal,
                "Failed to lock publisher mutex",
            );
        }
    };

    // Copy data to Bytes
    let data_slice = std::slice::from_raw_parts(data, data_len);
    let data_bytes = bytes::Bytes::copy_from_slice(data_slice);

    let namespace = inner.namespace.clone();
    let track_name = inner.track_name.clone();
    
    // Get counter value before borrowing mode mutably
    let counter_val = inner.group_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    
    // Publish data based on mode (using synchronous API)
    // Note: delivery_mode parameter is ignored; mode is set at publisher creation
    let result = match &mut inner.mode {
        PublisherMode::Datagrams(datagrams) => {
            // Create a datagram with metadata
            let datagram = serve::Datagram {
                group_id: 0,
                object_id: counter_val,
                priority: 0,
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

#[no_mangle]
pub unsafe extern "C" fn moq_subscribe(
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
            let inner = inner_clone.lock().unwrap();
            inner.track.clone()
        };

        // Get the mode once (it's set when track is established)
        let mode_result = track.mode().await;
        match mode_result {
            Ok(mode) => {
                use moq_transport::serve::TrackReaderMode;
                
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
                                    log::info!("Stream ended: {:?}/{}", track_namespace_log, track_name_log);
                                    break;
                                }
                                Err(e) => {
                                    log::error!("Stream read error: {}", e);
                                    None
                                }
                            }
                        }
                        TrackReaderMode::Subgroups(subgroups) => {
                            // Read from subgroups
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
                        TrackReaderMode::Datagrams(datagrams) => {
                            // Read datagram
                            let mut datagrams = datagrams.clone();
                            match datagrams.read().await {
                                Ok(Some(datagram)) => {
                                    // Datagram is just bytes
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

                    // Invoke callback if we got data
                    if let Some(buffer) = data_result {
                        let inner = inner_clone.lock().unwrap();
                        if let Some(callback) = inner.data_callback {
                            callback(inner.user_data as *mut std::ffi::c_void, buffer.as_ptr(), buffer.len());
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to get track mode: {}", e);
            }
        }
    });

    // Store reader task
    subscriber_inner.lock().unwrap().reader_task = Some(reader_task);

    let subscriber = MoqSubscriber {
        inner: subscriber_inner,
    };

    log::info!("Subscribed to {}/{}", namespace_str, track_name_str);
    Box::into_raw(Box::new(subscriber))
}

#[no_mangle]
pub unsafe extern "C" fn moq_subscriber_destroy(subscriber: *mut MoqSubscriber) {
    if !subscriber.is_null() {
        let subscriber = Box::from_raw(subscriber);
        
        // Cancel reader task
        if let Ok(mut inner) = subscriber.inner.lock() {
            if let Some(task) = inner.reader_task.take() {
                task.abort();
            }
        }
        
        log::debug!("Destroyed subscriber");
    }
}

/* ───────────────────────────────────────────────
 * Utilities
 * ─────────────────────────────────────────────── */

#[no_mangle]
pub unsafe extern "C" fn moq_free_str(s: *const c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s as *mut c_char);
    }
}

#[no_mangle]
pub extern "C" fn moq_version() -> *const c_char {
    const VERSION: &[u8] = b"moq_ffi 0.1.0 (with_moq)\0";
    VERSION.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn moq_last_error() -> *const c_char {
    // Note: This returns a pointer that is valid until the next error occurs
    // in this thread. The caller should NOT free this pointer.
    // This is a common pattern in C FFI for error reporting.
    match get_last_error() {
        Some(err) => {
            // Create a static string that we'll reuse per thread
            thread_local! {
                static ERROR_BUF: std::cell::RefCell<Option<CString>> = std::cell::RefCell::new(None);
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
