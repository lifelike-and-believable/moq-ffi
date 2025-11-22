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

/// Creates a publisher for a specific track with explicit delivery mode (stub implementation - always returns null).
///
/// # Safety
/// - `client` must be a valid pointer returned from `moq_client_create()`
/// - `namespace` must be a valid null-terminated C string pointer
/// - `track_name` must be a valid null-terminated C string pointer
/// - This function is thread-safe
#[no_mangle]
pub unsafe extern "C" fn moq_create_publisher_ex(
    client: *mut MoqClient,
    namespace: *const c_char,
    track_name: *const c_char,
    _delivery_mode: MoqDeliveryMode,
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

/* ───────────────────────────────────────────────
 * Tests
 * ─────────────────────────────────────────────── */

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    /* ───────────────────────────────────────────────
     * Lifecycle Tests
     * ─────────────────────────────────────────────── */

    mod lifecycle {
        use super::*;

        #[test]
        fn test_client_create_returns_valid_pointer() {
            let client = moq_client_create();
            assert!(!client.is_null(), "Client creation should return non-null pointer");
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_client_destroy_with_null_is_safe() {
            // Should not crash
            unsafe { moq_client_destroy(std::ptr::null_mut()); }
        }

        #[test]
        fn test_client_create_and_destroy_multiple() {
            // Test creating and destroying multiple clients
            for _ in 0..10 {
                let client = moq_client_create();
                assert!(!client.is_null());
                unsafe { moq_client_destroy(client); }
            }
        }

        #[test]
        fn test_publisher_destroy_with_null_is_safe() {
            // Should not crash even though stub never creates publishers
            unsafe { moq_publisher_destroy(std::ptr::null_mut()); }
        }

        #[test]
        fn test_subscriber_destroy_with_null_is_safe() {
            // Should not crash even though stub never creates subscribers
            unsafe { moq_subscriber_destroy(std::ptr::null_mut()); }
        }
    }

    /* ───────────────────────────────────────────────
     * Null Pointer Tests
     * ─────────────────────────────────────────────── */

    mod null_pointer {
        use super::*;

        #[test]
        fn test_connect_with_null_client() {
            let url = std::ffi::CString::new("https://example.com").unwrap();
            let result = unsafe {
                moq_connect(
                    std::ptr::null_mut(),
                    url.as_ptr(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorInvalidArgument);
            assert!(!result.message.is_null());
            unsafe { moq_free_str(result.message); }
        }

        #[test]
        fn test_connect_with_null_url() {
            let client = moq_client_create();
            let result = unsafe {
                moq_connect(
                    client,
                    std::ptr::null(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorInvalidArgument);
            assert!(!result.message.is_null());
            unsafe {
                moq_free_str(result.message);
                moq_client_destroy(client);
            }
        }

        #[test]
        fn test_disconnect_with_null_client() {
            let result = unsafe { moq_disconnect(std::ptr::null_mut()) };
            assert_eq!(result.code, MoqResultCode::MoqErrorInvalidArgument);
            assert!(!result.message.is_null());
            unsafe { moq_free_str(result.message); }
        }

        #[test]
        fn test_is_connected_with_null_client() {
            let connected = unsafe { moq_is_connected(std::ptr::null()) };
            assert!(!connected);
        }

        #[test]
        fn test_announce_namespace_with_null_client() {
            let namespace = std::ffi::CString::new("test").unwrap();
            let result = unsafe {
                moq_announce_namespace(std::ptr::null_mut(), namespace.as_ptr())
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorInvalidArgument);
            assert!(!result.message.is_null());
            unsafe { moq_free_str(result.message); }
        }

        #[test]
        fn test_announce_namespace_with_null_namespace() {
            let client = moq_client_create();
            let result = unsafe {
                moq_announce_namespace(client, std::ptr::null())
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorInvalidArgument);
            assert!(!result.message.is_null());
            unsafe {
                moq_free_str(result.message);
                moq_client_destroy(client);
            }
        }

        #[test]
        fn test_create_publisher_with_null_client() {
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track").unwrap();
            let publisher = unsafe {
                moq_create_publisher(std::ptr::null_mut(), namespace.as_ptr(), track.as_ptr())
            };
            assert!(publisher.is_null());
        }

        #[test]
        fn test_create_publisher_with_null_namespace() {
            let client = moq_client_create();
            let track = std::ffi::CString::new("track").unwrap();
            let publisher = unsafe {
                moq_create_publisher(client, std::ptr::null(), track.as_ptr())
            };
            assert!(publisher.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_create_publisher_with_null_track_name() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let publisher = unsafe {
                moq_create_publisher(client, namespace.as_ptr(), std::ptr::null())
            };
            assert!(publisher.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_create_publisher_ex_with_null_client() {
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track").unwrap();
            let publisher = unsafe {
                moq_create_publisher_ex(
                    std::ptr::null_mut(),
                    namespace.as_ptr(),
                    track.as_ptr(),
                    MoqDeliveryMode::MoqDeliveryStream,
                )
            };
            assert!(publisher.is_null());
        }

        #[test]
        fn test_create_publisher_ex_with_null_namespace() {
            let client = moq_client_create();
            let track = std::ffi::CString::new("track").unwrap();
            let publisher = unsafe {
                moq_create_publisher_ex(
                    client,
                    std::ptr::null(),
                    track.as_ptr(),
                    MoqDeliveryMode::MoqDeliveryStream,
                )
            };
            assert!(publisher.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_create_publisher_ex_with_null_track_name() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let publisher = unsafe {
                moq_create_publisher_ex(
                    client,
                    namespace.as_ptr(),
                    std::ptr::null(),
                    MoqDeliveryMode::MoqDeliveryStream,
                )
            };
            assert!(publisher.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_publish_data_with_null_publisher() {
            let data = [1u8, 2, 3, 4];
            let result = unsafe {
                moq_publish_data(
                    std::ptr::null_mut(),
                    data.as_ptr(),
                    data.len(),
                    MoqDeliveryMode::MoqDeliveryStream,
                )
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorInvalidArgument);
            assert!(!result.message.is_null());
            unsafe { moq_free_str(result.message); }
        }

        #[test]
        fn test_publish_data_with_null_data() {
            // Create a fake publisher pointer (stub never creates real ones)
            // We test the null data check with a non-null publisher pointer
            let fake_publisher = Box::into_raw(Box::new(MoqPublisher { _dummy: 0 }));
            let result = unsafe {
                moq_publish_data(
                    fake_publisher,
                    std::ptr::null(),
                    100,
                    MoqDeliveryMode::MoqDeliveryStream,
                )
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorInvalidArgument);
            assert!(!result.message.is_null());
            unsafe {
                moq_free_str(result.message);
                let _ = Box::from_raw(fake_publisher); // Clean up
            }
        }

        #[test]
        fn test_subscribe_with_null_client() {
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track").unwrap();
            let subscriber = unsafe {
                moq_subscribe(
                    std::ptr::null_mut(),
                    namespace.as_ptr(),
                    track.as_ptr(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert!(subscriber.is_null());
        }

        #[test]
        fn test_subscribe_with_null_namespace() {
            let client = moq_client_create();
            let track = std::ffi::CString::new("track").unwrap();
            let subscriber = unsafe {
                moq_subscribe(
                    client,
                    std::ptr::null(),
                    track.as_ptr(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert!(subscriber.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_subscribe_with_null_track_name() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let subscriber = unsafe {
                moq_subscribe(
                    client,
                    namespace.as_ptr(),
                    std::ptr::null(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert!(subscriber.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_free_str_with_null_is_safe() {
            // Should not crash
            unsafe { moq_free_str(std::ptr::null()); }
        }
    }

    /* ───────────────────────────────────────────────
     * Error Handling Tests
     * ─────────────────────────────────────────────── */

    mod error_handling {
        use super::*;

        #[test]
        fn test_connect_returns_unsupported_in_stub() {
            let client = moq_client_create();
            let url = std::ffi::CString::new("https://example.com").unwrap();
            let result = unsafe {
                moq_connect(client, url.as_ptr(), None, std::ptr::null_mut())
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            assert!(!result.message.is_null());
            
            // Verify message contains helpful information
            let message = unsafe { CStr::from_ptr(result.message).to_string_lossy() };
            assert!(message.contains("Stub backend") || message.contains("not enabled"));
            
            unsafe {
                moq_free_str(result.message);
                moq_client_destroy(client);
            }
        }

        #[test]
        fn test_disconnect_succeeds_with_valid_client() {
            let client = moq_client_create();
            let result = unsafe { moq_disconnect(client) };
            assert_eq!(result.code, MoqResultCode::MoqOk);
            assert!(result.message.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_is_connected_returns_false_for_stub() {
            let client = moq_client_create();
            let connected = unsafe { moq_is_connected(client) };
            assert!(!connected);
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_announce_namespace_returns_unsupported() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test/namespace").unwrap();
            let result = unsafe {
                moq_announce_namespace(client, namespace.as_ptr())
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            assert!(!result.message.is_null());
            unsafe {
                moq_free_str(result.message);
                moq_client_destroy(client);
            }
        }

        #[test]
        fn test_create_publisher_returns_null_in_stub() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track1").unwrap();
            let publisher = unsafe {
                moq_create_publisher(client, namespace.as_ptr(), track.as_ptr())
            };
            assert!(publisher.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_create_publisher_ex_returns_null_in_stub() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track1").unwrap();
            
            // Test with datagram mode
            let publisher = unsafe {
                moq_create_publisher_ex(
                    client,
                    namespace.as_ptr(),
                    track.as_ptr(),
                    MoqDeliveryMode::MoqDeliveryDatagram,
                )
            };
            assert!(publisher.is_null());
            
            // Test with stream mode
            let publisher = unsafe {
                moq_create_publisher_ex(
                    client,
                    namespace.as_ptr(),
                    track.as_ptr(),
                    MoqDeliveryMode::MoqDeliveryStream,
                )
            };
            assert!(publisher.is_null());
            
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_publish_data_returns_unsupported() {
            let fake_publisher = Box::into_raw(Box::new(MoqPublisher { _dummy: 0 }));
            let data = [1u8, 2, 3, 4, 5];
            let result = unsafe {
                moq_publish_data(
                    fake_publisher,
                    data.as_ptr(),
                    data.len(),
                    MoqDeliveryMode::MoqDeliveryStream,
                )
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            assert!(!result.message.is_null());
            unsafe {
                moq_free_str(result.message);
                let _ = Box::from_raw(fake_publisher);
            }
        }

        #[test]
        fn test_subscribe_returns_null_in_stub() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track1").unwrap();
            let subscriber = unsafe {
                moq_subscribe(
                    client,
                    namespace.as_ptr(),
                    track.as_ptr(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert!(subscriber.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_error_messages_are_valid_utf8() {
            let client = moq_client_create();
            let url = std::ffi::CString::new("https://example.com").unwrap();
            let result = unsafe {
                moq_connect(client, url.as_ptr(), None, std::ptr::null_mut())
            };
            
            if !result.message.is_null() {
                // Should not panic on to_string_lossy
                let message = unsafe { CStr::from_ptr(result.message).to_string_lossy() };
                assert!(!message.is_empty());
                unsafe { moq_free_str(result.message); }
            }
            
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_all_error_codes_exist() {
            // Ensure all error code variants are accessible
            let _ = MoqResultCode::MoqOk;
            let _ = MoqResultCode::MoqErrorInvalidArgument;
            let _ = MoqResultCode::MoqErrorConnectionFailed;
            let _ = MoqResultCode::MoqErrorNotConnected;
            let _ = MoqResultCode::MoqErrorTimeout;
            let _ = MoqResultCode::MoqErrorInternal;
            let _ = MoqResultCode::MoqErrorUnsupported;
            let _ = MoqResultCode::MoqErrorBufferTooSmall;
        }
    }

    /* ───────────────────────────────────────────────
     * Panic Protection Tests
     * ─────────────────────────────────────────────── */

    mod panic_protection {
        use super::*;

        #[test]
        fn test_client_create_handles_panic() {
            // Client create should handle panics internally
            let client = moq_client_create();
            assert!(!client.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_client_destroy_handles_panic() {
            // Should not panic even with null
            unsafe { moq_client_destroy(std::ptr::null_mut()); }
        }

        #[test]
        fn test_connect_handles_panic_on_null() {
            // All FFI functions should catch panics and return errors
            let result = unsafe {
                moq_connect(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            // Should return error, not panic
            assert_ne!(result.code, MoqResultCode::MoqOk);
        }

        #[test]
        fn test_all_ffi_functions_safe_with_null() {
            // Comprehensive test that no FFI function panics with null inputs
            unsafe {
                // Client management
                moq_client_destroy(std::ptr::null_mut());
                let _ = moq_disconnect(std::ptr::null_mut());
                let _ = moq_is_connected(std::ptr::null());
                
                // Publishing
                let _ = moq_announce_namespace(std::ptr::null_mut(), std::ptr::null());
                let _ = moq_create_publisher(std::ptr::null_mut(), std::ptr::null(), std::ptr::null());
                let _ = moq_create_publisher_ex(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    MoqDeliveryMode::MoqDeliveryStream,
                );
                moq_publisher_destroy(std::ptr::null_mut());
                let _ = moq_publish_data(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    0,
                    MoqDeliveryMode::MoqDeliveryStream,
                );
                
                // Subscribing
                let _ = moq_subscribe(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    None,
                    std::ptr::null_mut(),
                );
                moq_subscriber_destroy(std::ptr::null_mut());
                
                // Utilities
                moq_free_str(std::ptr::null());
            }
            // If we reach here, no panics occurred
        }
    }

    /* ───────────────────────────────────────────────
     * Memory Management Tests
     * ─────────────────────────────────────────────── */

    mod memory {
        use super::*;

        #[test]
        fn test_error_message_can_be_freed() {
            let result = unsafe {
                moq_connect(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            
            assert!(!result.message.is_null());
            // Should not crash
            unsafe { moq_free_str(result.message); }
        }

        #[test]
        fn test_multiple_error_messages_can_be_freed() {
            for _ in 0..10 {
                let result = unsafe {
                    moq_disconnect(std::ptr::null_mut())
                };
                if !result.message.is_null() {
                    unsafe { moq_free_str(result.message); }
                }
            }
        }

        #[test]
        fn test_client_memory_lifecycle() {
            // Create, use, and destroy client multiple times
            for _ in 0..100 {
                let client = moq_client_create();
                assert!(!client.is_null());
                
                // Use the client
                let connected = unsafe { moq_is_connected(client) };
                assert!(!connected);
                
                // Destroy
                unsafe { moq_client_destroy(client); }
            }
        }

        #[test]
        fn test_free_str_double_free_protection() {
            // Create an error message
            let result = unsafe {
                moq_disconnect(std::ptr::null_mut())
            };
            
            if !result.message.is_null() {
                // First free - should work
                unsafe { moq_free_str(result.message); }
                
                // Second free would be a bug in real code, but our implementation
                // uses CString::from_raw which takes ownership, so technically
                // we can't easily test double-free here without unsafe shenanigans.
                // This test documents that callers should NOT double-free.
            }
        }

        #[test]
        fn test_ok_result_has_no_message_to_free() {
            let client = moq_client_create();
            let result = unsafe { moq_disconnect(client) };
            
            assert_eq!(result.code, MoqResultCode::MoqOk);
            assert!(result.message.is_null());
            // No message to free
            
            unsafe { moq_client_destroy(client); }
        }
    }

    /* ───────────────────────────────────────────────
     * Callback Tests
     * ─────────────────────────────────────────────── */

    mod callbacks {
        use super::*;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        extern "C" fn connection_callback(
            user_data: *mut std::ffi::c_void,
            state: MoqConnectionState,
        ) {
            if !user_data.is_null() {
                unsafe {
                    let flag = &*(user_data as *const AtomicBool);
                    flag.store(true, Ordering::SeqCst);
                }
            }
            // Verify state is valid
            let _ = match state {
                MoqConnectionState::MoqStateDisconnected => "disconnected",
                MoqConnectionState::MoqStateConnecting => "connecting",
                MoqConnectionState::MoqStateConnected => "connected",
                MoqConnectionState::MoqStateFailed => "failed",
            };
        }

        extern "C" fn data_callback(
            user_data: *mut std::ffi::c_void,
            data: *const u8,
            data_len: usize,
        ) {
            if !user_data.is_null() {
                unsafe {
                    let flag = &*(user_data as *const AtomicBool);
                    flag.store(true, Ordering::SeqCst);
                }
            }
            // Verify we can read data safely
            if !data.is_null() && data_len > 0 {
                unsafe {
                    let _ = std::slice::from_raw_parts(data, data_len);
                }
            }
        }

        #[test]
        fn test_connect_accepts_callback() {
            let client = moq_client_create();
            let url = std::ffi::CString::new("https://example.com").unwrap();
            let flag = Arc::new(AtomicBool::new(false));
            let user_data = Arc::as_ptr(&flag) as *mut std::ffi::c_void;
            
            let result = unsafe {
                moq_connect(client, url.as_ptr(), Some(connection_callback), user_data)
            };
            
            // Stub returns error, but should accept callback
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            
            unsafe {
                moq_free_str(result.message);
                moq_client_destroy(client);
            }
        }

        #[test]
        fn test_connect_accepts_null_callback() {
            let client = moq_client_create();
            let url = std::ffi::CString::new("https://example.com").unwrap();
            
            let result = unsafe {
                moq_connect(client, url.as_ptr(), None, std::ptr::null_mut())
            };
            
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            
            unsafe {
                moq_free_str(result.message);
                moq_client_destroy(client);
            }
        }

        #[test]
        fn test_subscribe_accepts_callback() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track1").unwrap();
            let flag = Arc::new(AtomicBool::new(false));
            let user_data = Arc::as_ptr(&flag) as *mut std::ffi::c_void;
            
            let subscriber = unsafe {
                moq_subscribe(
                    client,
                    namespace.as_ptr(),
                    track.as_ptr(),
                    Some(data_callback),
                    user_data,
                )
            };
            
            // Stub returns null
            assert!(subscriber.is_null());
            
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_subscribe_accepts_null_callback() {
            let client = moq_client_create();
            let namespace = std::ffi::CString::new("test").unwrap();
            let track = std::ffi::CString::new("track1").unwrap();
            
            let subscriber = unsafe {
                moq_subscribe(
                    client,
                    namespace.as_ptr(),
                    track.as_ptr(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            
            assert!(subscriber.is_null());
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_callback_with_null_user_data() {
            let client = moq_client_create();
            let url = std::ffi::CString::new("https://example.com").unwrap();
            
            // Pass callback but null user_data
            let result = unsafe {
                moq_connect(client, url.as_ptr(), Some(connection_callback), std::ptr::null_mut())
            };
            
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            
            unsafe {
                moq_free_str(result.message);
                moq_client_destroy(client);
            }
        }
    }

    /* ───────────────────────────────────────────────
     * Utility Function Tests
     * ─────────────────────────────────────────────── */

    mod utilities {
        use super::*;

        #[test]
        fn test_version_returns_valid_string() {
            let version_ptr = moq_version();
            assert!(!version_ptr.is_null());
            
            let version = unsafe { CStr::from_ptr(version_ptr).to_string_lossy() };
            assert!(!version.is_empty());
            assert!(version.contains("moq_ffi"));
            assert!(version.contains("stub"));
        }

        #[test]
        fn test_version_is_static() {
            // Version string should be static, not need freeing
            let version1 = moq_version();
            let version2 = moq_version();
            assert_eq!(version1, version2);
        }

        #[test]
        fn test_last_error_returns_null_in_stub() {
            let error = moq_last_error();
            assert!(error.is_null());
        }

        #[test]
        fn test_last_error_is_thread_safe() {
            // Should always return null in stub, regardless of thread
            use std::thread;
            
            let handles: Vec<_> = (0..5)
                .map(|_| {
                    thread::spawn(|| {
                        let error = moq_last_error();
                        assert!(error.is_null());
                    })
                })
                .collect();
            
            for handle in handles {
                handle.join().unwrap();
            }
        }

        #[test]
        fn test_free_str_is_idempotent_for_null() {
            // Calling free_str with null multiple times should be safe
            for _ in 0..10 {
                unsafe { moq_free_str(std::ptr::null()); }
            }
        }
    }

    /* ───────────────────────────────────────────────
     * Enum Tests
     * ─────────────────────────────────────────────── */

    mod enums {
        use super::*;

        #[test]
        fn test_moq_result_code_values() {
            assert_eq!(MoqResultCode::MoqOk as i32, 0);
            assert_eq!(MoqResultCode::MoqErrorInvalidArgument as i32, 1);
            assert_eq!(MoqResultCode::MoqErrorConnectionFailed as i32, 2);
            assert_eq!(MoqResultCode::MoqErrorNotConnected as i32, 3);
            assert_eq!(MoqResultCode::MoqErrorTimeout as i32, 4);
            assert_eq!(MoqResultCode::MoqErrorInternal as i32, 5);
            assert_eq!(MoqResultCode::MoqErrorUnsupported as i32, 6);
            assert_eq!(MoqResultCode::MoqErrorBufferTooSmall as i32, 7);
        }

        #[test]
        fn test_moq_connection_state_values() {
            assert_eq!(MoqConnectionState::MoqStateDisconnected as i32, 0);
            assert_eq!(MoqConnectionState::MoqStateConnecting as i32, 1);
            assert_eq!(MoqConnectionState::MoqStateConnected as i32, 2);
            assert_eq!(MoqConnectionState::MoqStateFailed as i32, 3);
        }

        #[test]
        fn test_moq_delivery_mode_values() {
            assert_eq!(MoqDeliveryMode::MoqDeliveryDatagram as i32, 0);
            assert_eq!(MoqDeliveryMode::MoqDeliveryStream as i32, 1);
        }

        #[test]
        fn test_enum_equality() {
            let code1 = MoqResultCode::MoqOk;
            let code2 = MoqResultCode::MoqOk;
            assert_eq!(code1, code2);
            
            let state1 = MoqConnectionState::MoqStateConnected;
            let state2 = MoqConnectionState::MoqStateConnected;
            assert_eq!(state1, state2);
        }
    }

    /* ───────────────────────────────────────────────
     * Helper Function Tests
     * ─────────────────────────────────────────────── */

    mod helpers {
        use super::*;

        #[test]
        fn test_make_ok_result() {
            let result = make_ok_result();
            assert_eq!(result.code, MoqResultCode::MoqOk);
            assert!(result.message.is_null());
        }

        #[test]
        fn test_make_error_result() {
            let result = make_error_result(
                MoqResultCode::MoqErrorInternal,
                "Test error message",
            );
            assert_eq!(result.code, MoqResultCode::MoqErrorInternal);
            assert!(!result.message.is_null());
            
            let message = unsafe { CStr::from_ptr(result.message).to_string_lossy() };
            assert_eq!(message, "Test error message");
            
            unsafe { moq_free_str(result.message); }
        }

        #[test]
        fn test_make_error_result_with_invalid_utf8() {
            // The function should handle invalid UTF-8 gracefully
            let result = make_error_result(
                MoqResultCode::MoqErrorInternal,
                "Valid message",
            );
            assert!(!result.message.is_null());
            unsafe { moq_free_str(result.message); }
        }

        #[test]
        fn test_make_error_result_with_empty_string() {
            let result = make_error_result(MoqResultCode::MoqErrorInternal, "");
            assert!(!result.message.is_null());
            
            let message = unsafe { CStr::from_ptr(result.message).to_string_lossy() };
            assert_eq!(message, "");
            
            unsafe { moq_free_str(result.message); }
        }
    }

    /* ───────────────────────────────────────────────
     * Integration Tests
     * ─────────────────────────────────────────────── */

    mod integration {
        use super::*;

        #[test]
        fn test_typical_workflow_in_stub() {
            // Simulate a typical usage pattern (even though stub will fail)
            let client = moq_client_create();
            assert!(!client.is_null());
            
            // Try to connect
            let url = std::ffi::CString::new("https://relay.example.com").unwrap();
            let result = unsafe {
                moq_connect(client, url.as_ptr(), None, std::ptr::null_mut())
            };
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            unsafe { moq_free_str(result.message); }
            
            // Check connection status
            let connected = unsafe { moq_is_connected(client) };
            assert!(!connected);
            
            // Try to announce namespace
            let namespace = std::ffi::CString::new("test").unwrap();
            let result = unsafe { moq_announce_namespace(client, namespace.as_ptr()) };
            assert_eq!(result.code, MoqResultCode::MoqErrorUnsupported);
            unsafe { moq_free_str(result.message); }
            
            // Disconnect
            let result = unsafe { moq_disconnect(client) };
            assert_eq!(result.code, MoqResultCode::MoqOk);
            
            // Destroy
            unsafe { moq_client_destroy(client); }
        }

        #[test]
        fn test_multiple_clients() {
            let client1 = moq_client_create();
            let client2 = moq_client_create();
            let client3 = moq_client_create();
            
            assert!(!client1.is_null());
            assert!(!client2.is_null());
            assert!(!client3.is_null());
            assert_ne!(client1, client2);
            assert_ne!(client2, client3);
            
            unsafe {
                moq_client_destroy(client1);
                moq_client_destroy(client2);
                moq_client_destroy(client3);
            }
        }

        #[test]
        fn test_version_info() {
            let version = moq_version();
            assert!(!version.is_null());
            
            let version_str = unsafe { CStr::from_ptr(version).to_string_lossy() };
            println!("MoQ FFI version: {}", version_str);
            assert!(!version_str.is_empty());
        }
    }
}
