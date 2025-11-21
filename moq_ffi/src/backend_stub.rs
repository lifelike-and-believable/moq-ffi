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

#[no_mangle]
pub extern "C" fn moq_client_create() -> *mut MoqClient {
    Box::into_raw(Box::new(MoqClient { _dummy: 0 }))
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
    _connection_callback: MoqConnectionCallback,
    _user_data: *mut std::ffi::c_void,
) -> MoqResult {
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
}

#[no_mangle]
pub unsafe extern "C" fn moq_disconnect(client: *mut MoqClient) -> MoqResult {
    if client.is_null() {
        return make_error_result(MoqResultCode::MoqErrorInvalidArgument, "Client is null");
    }
    make_ok_result()
}

#[no_mangle]
pub unsafe extern "C" fn moq_is_connected(client: *const MoqClient) -> bool {
    if client.is_null() {
        return false;
    }
    false // Stub: never connected
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
        return make_error_result(
            MoqResultCode::MoqErrorInvalidArgument,
            "Client or namespace is null",
        );
    }

    make_error_result(
        MoqResultCode::MoqErrorUnsupported,
        "Stub backend: MoQ transport not enabled",
    )
}

#[no_mangle]
pub unsafe extern "C" fn moq_create_publisher(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
) -> *mut MoqPublisher {
    if client.is_null() || namespace.is_null() || track_name.is_null() {
        return std::ptr::null_mut();
    }

    std::ptr::null_mut() // Stub: can't create publisher
}

#[no_mangle]
pub unsafe extern "C" fn moq_publisher_destroy(publisher: *mut MoqPublisher) {
    if !publisher.is_null() {
        let _ = Box::from_raw(publisher);
    }
}

#[no_mangle]
pub unsafe extern "C" fn moq_publish_data(
    publisher: *mut MoqPublisher,
    data: *const u8,
    _data_len: usize,
    _delivery_mode: MoqDeliveryMode,
) -> MoqResult {
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
}

/* ───────────────────────────────────────────────
 * Subscribing
 * ─────────────────────────────────────────────── */

#[no_mangle]
pub unsafe extern "C" fn moq_subscribe(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
    _data_callback: MoqDataCallback,
    _user_data: *mut std::ffi::c_void,
) -> *mut MoqSubscriber {
    if client.is_null() || namespace.is_null() || track_name.is_null() {
        return std::ptr::null_mut();
    }

    std::ptr::null_mut() // Stub: can't create subscriber
}

#[no_mangle]
pub unsafe extern "C" fn moq_subscriber_destroy(subscriber: *mut MoqSubscriber) {
    if !subscriber.is_null() {
        let _ = Box::from_raw(subscriber);
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
    const VERSION: &[u8] = b"moq_ffi 0.1.0 (stub)\0";
    VERSION.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn moq_last_error() -> *const c_char {
    std::ptr::null() // Stub: no thread-local error tracking
}
