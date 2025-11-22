// Stub backend for testing builds without moq-transport dependency
//
// This backend provides no-op implementations of all FFI functions,
// allowing the library to build and link successfully for testing
// the build toolchain and integration without requiring the full
// moq-transport dependency.

use std::ffi::CString;
use std::os::raw::c_char;

/* ───────────────────────────────────────────────
 * Opaque Types
 * ─────────────────────────────────────────────── */

#[repr(C)]
pub struct MoqClient {
    _dummy: u8,
}

#[repr(C)]
pub struct MoqPublisher {
    _dummy: u8,
}

#[repr(C)]
pub struct MoqSubscriber {
    _dummy: u8,
}

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

/// Creates a new MoQ client instance (stub implementation).
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
        Box::into_raw(Box::new(MoqClient { _dummy: 0 }))
    }).unwrap_or(std::ptr::null_mut())
}

/// Destroys a MoQ client and releases all associated resources (stub implementation).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null (null pointers are safely ignored)
/// - `client` must not be accessed after this function returns
/// - This function is thread-safe
///
/// # Parameters
/// - `client`: Pointer to the client to destroy, or null (null is safely ignored)
#[no_mangle]
pub unsafe extern "C" fn moq_client_destroy(client: *mut MoqClient) {
    let _ = std::panic::catch_unwind(|| {
        if !client.is_null() {
            let _ = Box::from_raw(client);
        }
    });
}

/// Connects to a MoQ relay server (stub implementation - always fails).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - `url` must be a valid null-terminated C string pointer
/// - `url` must not be null
/// - This function is thread-safe
///
/// # Returns
/// Always returns MoqErrorUnsupported in stub build
#[no_mangle]
pub unsafe extern "C" fn moq_connect(
    client: *mut MoqClient,
    url: *const c_char,
    _connection_callback: MoqConnectionCallback,
    _user_data: *mut std::ffi::c_void,
) -> MoqResult {
    std::panic::catch_unwind(|| {
        if client.is_null() || url.is_null() {
            return make_error_result(
                MoqResultCode::MoqErrorInvalidArgument,
                "Client or URL is null",
            );
        }

        // Stub: always return unsupported
        make_error_result(
            MoqResultCode::MoqErrorUnsupported,
            "Stub backend: MoQ transport not enabled. Rebuild with --features with_moq",
        )
    }).unwrap_or_else(|_| {
        make_error_result(MoqResultCode::MoqErrorInternal, "Internal panic occurred")
    })
}

/// Disconnects from the MoQ relay server (stub implementation).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` must not be null
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_disconnect(client: *mut MoqClient) -> MoqResult {
    std::panic::catch_unwind(|| {
        if client.is_null() {
            return make_error_result(MoqResultCode::MoqErrorInvalidArgument, "Client is null");
        }
        make_ok_result()
    }).unwrap_or_else(|_| {
        make_error_result(MoqResultCode::MoqErrorInternal, "Internal panic occurred")
    })
}

/// Checks if the client is currently connected (stub implementation - always returns false).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `client` may be null (returns false)
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_is_connected(client: *const MoqClient) -> bool {
    std::panic::catch_unwind(|| {
        if client.is_null() {
            return false;
        }
        false // Stub: never connected
    }).unwrap_or(false)
}

/* ───────────────────────────────────────────────
 * Publishing
 * ─────────────────────────────────────────────── */

/// Announces a namespace to the MoQ relay server (stub implementation).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `namespace` must be a valid null-terminated C string pointer
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_announce_namespace(
    client: *mut MoqClient,
    namespace: *const c_char,
) -> MoqResult {
    std::panic::catch_unwind(|| {
        if client.is_null() || namespace.is_null() {
            return make_error_result(
                MoqResultCode::MoqErrorInvalidArgument,
                "Client or namespace is null",
            );
        }

        make_error_result(
            MoqResultCode::MoqErrorUnsupported,
            "Stub backend: MoQ transport not enabled",
        )
    }).unwrap_or_else(|_| {
        make_error_result(MoqResultCode::MoqErrorInternal, "Internal panic occurred")
    })
}

/// Creates a publisher for a specific track (stub implementation - always returns null).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `namespace` must be a valid null-terminated C string pointer
/// - `track_name` must be a valid null-terminated C string pointer
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_create_publisher(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
) -> *mut MoqPublisher {
    std::panic::catch_unwind(|| {
        if client.is_null() || namespace.is_null() || track_name.is_null() {
            return std::ptr::null_mut();
        }

        std::ptr::null_mut() // Stub: can't create publisher
    }).unwrap_or(std::ptr::null_mut())
}

/// Destroys a publisher and releases its resources (stub implementation).
///
/// # Safety
/// - `publisher` must be a valid pointer returned from `moq_create_publisher()`
/// - `publisher` must not be null (null pointers are safely ignored)
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_publisher_destroy(publisher: *mut MoqPublisher) {
    let _ = std::panic::catch_unwind(|| {
        if !publisher.is_null() {
            // Note: In stub backend, moq_create_publisher always returns null,
            // so this path should never be reached. This is a placeholder for
            // the full implementation.
            let _ = Box::from_raw(publisher);
        }
    });
}

/// Publishes data to a track (stub implementation).
///
/// # Safety
/// - `publisher` must be a valid pointer returned from `moq_create_publisher()`
/// - `data` must be a valid pointer to a buffer of at least `_data_len` bytes
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_publish_data(
    publisher: *mut MoqPublisher,
    data: *const u8,
    _data_len: usize,
    _delivery_mode: MoqDeliveryMode,
) -> MoqResult {
    std::panic::catch_unwind(|| {
        if publisher.is_null() || data.is_null() {
            return make_error_result(
                MoqResultCode::MoqErrorInvalidArgument,
                "Publisher or data is null",
            );
        }

        make_error_result(
            MoqResultCode::MoqErrorUnsupported,
            "Stub backend: MoQ transport not enabled",
        )
    }).unwrap_or_else(|_| {
        make_error_result(MoqResultCode::MoqErrorInternal, "Internal panic occurred")
    })
}

/* ───────────────────────────────────────────────
 * Subscribing
 * ─────────────────────────────────────────────── */

/// Subscribes to a track on the MoQ relay server (stub implementation - always returns null).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `namespace` must be a valid null-terminated C string pointer
/// - `track_name` must be a valid null-terminated C string pointer
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_subscribe(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
    _data_callback: MoqDataCallback,
    _user_data: *mut std::ffi::c_void,
) -> *mut MoqSubscriber {
    std::panic::catch_unwind(|| {
        if client.is_null() || namespace.is_null() || track_name.is_null() {
            return std::ptr::null_mut();
        }

        std::ptr::null_mut() // Stub: can't create subscriber
    }).unwrap_or(std::ptr::null_mut())
}

/// Destroys a subscriber and releases its resources (stub implementation).
///
/// # Safety
/// - `subscriber` must be a valid pointer returned from `moq_subscribe()`
/// - `subscriber` must not be null (null pointers are safely ignored)
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_subscriber_destroy(subscriber: *mut MoqSubscriber) {
    let _ = std::panic::catch_unwind(|| {
        if !subscriber.is_null() {
            // Note: In stub backend, moq_subscribe always returns null,
            // so this path should never be reached. This is a placeholder for
            // the full implementation.
            let _ = Box::from_raw(subscriber);
        }
    });
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
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_free_str(s: *const c_char) {
    let _ = std::panic::catch_unwind(|| {
        if !s.is_null() {
            let _ = CString::from_raw(s as *mut c_char);
        }
    });
}

/// Returns the version string of the library (stub implementation).
///
/// # Returns
/// A pointer to a static null-terminated C string containing the version.
/// This string must NOT be freed - it is a static string.
///
/// # Thread Safety
/// This function is thread-safe.
#[no_mangle]
pub extern "C" fn moq_version() -> *const c_char {
    const VERSION: &[u8] = b"moq_ffi 0.1.0 (stub)\0";
    VERSION.as_ptr() as *const c_char
}

/// Returns the last error message for this thread (stub implementation).
///
/// # Returns
/// Always returns null in stub build (no error tracking).
///
/// # Thread Safety
/// This function is thread-safe.
#[no_mangle]
pub extern "C" fn moq_last_error() -> *const c_char {
    std::ptr::null() // Stub: no thread-local error tracking
}
