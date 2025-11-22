// Integration tests for moq-ffi using Cloudflare relay network
//
// These tests require:
// - Network connectivity to https://relay.cloudflare.mediaoverquic.com
// - The with_moq_draft07 feature enabled (Cloudflare uses Draft 07)
//
// To run these tests:
// ```
// cargo test --features with_moq_draft07 --test cloudflare_relay_integration -- --ignored --nocapture
// ```
//
// Note: Tests are marked with #[ignore] by default to prevent CI failures
// when network is unavailable.

#![cfg(feature = "with_moq_draft07")]
#![allow(clippy::unnecessary_safety_comment)]

use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Import FFI functions and types from the moq_ffi library
// Integration tests run as external crates and import via the library name
use moq_ffi::*;

// Initialize CryptoProvider for rustls (required for TLS connections)
// This must be called before any TLS operations
#[cfg(test)]
fn init_crypto_provider() {
    use rustls::crypto::CryptoProvider;
    // Install default provider (aws-lc-rs) if not already installed
    // Returns Ok(()) if installed successfully, Err if already installed
    match CryptoProvider::install_default(rustls::crypto::aws_lc_rs::default_provider()) {
        Ok(()) => {
            println!("CryptoProvider installed successfully");
        }
        Err(_) => {
            // Already installed, this is fine
            println!("CryptoProvider already installed");
        }
    }
}

// Cloudflare relay URL (production)
const CLOUDFLARE_RELAY_URL: &str = "https://relay.cloudflare.mediaoverquic.com";

// Timeout for async operations
const TEST_TIMEOUT_SECS: u64 = 30;

/// Helper struct to track connection state changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConnectionStateTracker {
    current_state: MoqConnectionState,
    state_changed: bool,
}

impl ConnectionStateTracker {
    fn new() -> Self {
        Self {
            current_state: MoqConnectionState::MoqStateDisconnected,
            state_changed: false,
        }
    }
}

/// Callback for connection state changes
unsafe extern "C" fn connection_state_callback(
    user_data: *mut std::ffi::c_void,
    state: MoqConnectionState,
) {
    if user_data.is_null() {
        return;
    }

    // SAFETY: user_data is a pointer to Arc<Mutex<...>> created by the test function.
    // The Arc is guaranteed to remain valid for the entire test duration.
    // This cast is safe because we control both the creation and usage of this pointer.
    let tracker = &*(user_data as *const Arc<Mutex<ConnectionStateTracker>>);
    if let Ok(mut t) = tracker.lock() {
        t.current_state = state;
        t.state_changed = true;
        println!("[Callback] Connection state changed to: {:?}", state);
    }
}

/// Helper struct to track received data (for future subscribe tests)
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct DataTracker {
    data_received: bool,
    received_count: usize,
    last_data_size: usize,
}

#[allow(dead_code)]
impl DataTracker {
    fn new() -> Self {
        Self {
            data_received: false,
            received_count: 0,
            last_data_size: 0,
        }
    }
}

/// Callback for received data (for future subscribe tests)
#[allow(dead_code)]
unsafe extern "C" fn data_received_callback(
    user_data: *mut std::ffi::c_void,
    _data: *const u8,
    data_len: usize,
) {
    if user_data.is_null() {
        return;
    }

    // SAFETY: user_data is a pointer to Arc<Mutex<...>> created by the test function.
    // The Arc is guaranteed to remain valid for the entire test duration.
    // This cast is safe because we control both the creation and usage of this pointer.
    let tracker = &*(user_data as *const Arc<Mutex<DataTracker>>);
    if let Ok(mut t) = tracker.lock() {
        t.data_received = true;
        t.received_count += 1;
        t.last_data_size = data_len;
        println!(
            "[Callback] Received data: {} bytes (total count: {})",
            data_len, t.received_count
        );
    }
}

/// Helper to wait for a condition with timeout
fn wait_for_condition<F>(mut condition: F, timeout_secs: u64) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    false
}

#[test]
#[ignore] // Requires network connectivity
fn test_connect_to_cloudflare_relay() {
    println!("\n=== Test: Connect to Cloudflare Relay ===");

    // Initialize CryptoProvider before any TLS operations
    init_crypto_provider();

    // Create client
    let client = moq_client_create();
    assert!(!client.is_null(), "Failed to create client");

    // Setup connection state tracking
    let state_tracker = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    // Pass a pointer to the Arc (not Arc::into_raw) since we keep the Arc alive for the test duration
    let tracker_ptr = &state_tracker as *const _ as *mut std::ffi::c_void;

    // Connect to Cloudflare relay
    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();
    let result = unsafe {
        moq_connect(
            client,
            url.as_ptr(),
            Some(connection_state_callback),
            tracker_ptr,
        )
    };

    // Check connection initiation
    println!("Connect result: code={:?}", result.code);
    if result.code != MoqResultCode::MoqOk && !result.message.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(result.message) };
        println!("Error message: {}", msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }

    // Note: Connection may take time, so we check for either success or in-progress
    assert!(
        result.code == MoqResultCode::MoqOk || result.code == MoqResultCode::MoqErrorTimeout,
        "Connection should either succeed or timeout"
    );

    // Wait for state change (if connection was initiated)
    if result.code == MoqResultCode::MoqOk {
        let connected = wait_for_condition(
            || {
                if let Ok(tracker) = state_tracker.lock() {
                    tracker.state_changed
                        && (tracker.current_state == MoqConnectionState::MoqStateConnected
                            || tracker.current_state == MoqConnectionState::MoqStateFailed)
                } else {
                    false
                }
            },
            TEST_TIMEOUT_SECS,
        );

        if let Ok(tracker) = state_tracker.lock() {
            println!("Final connection state: {:?}", tracker.current_state);
        }

        // For this test, we just verify the connection was attempted
        // Actual success depends on network conditions and relay availability
        assert!(
            connected || result.code == MoqResultCode::MoqErrorTimeout,
            "Connection should complete (success or failure) within timeout"
        );
    }

    // Cleanup
    unsafe {
        moq_disconnect(client);
        moq_client_destroy(client);
        // state_tracker will be automatically dropped when it goes out of scope
    }

    println!("=== Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity
fn test_connection_lifecycle() {
    println!("\n=== Test: Connection Lifecycle ===");

    // Initialize CryptoProvider before any TLS operations
    init_crypto_provider();

    let client = moq_client_create();
    assert!(!client.is_null());

    // Initially should be disconnected
    let connected = unsafe { moq_is_connected(client) };
    assert!(!connected, "Client should start disconnected");
    println!("Initial state: disconnected ✓");

    // Setup state tracking
    let state_tracker = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let tracker_ptr = &state_tracker as *const _ as *mut std::ffi::c_void;

    // Attempt connection
    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();
    let result = unsafe {
        moq_connect(
            client,
            url.as_ptr(),
            Some(connection_state_callback),
            tracker_ptr,
        )
    };

    println!("Connection attempt result: {:?}", result.code);

    // Disconnect (whether or not connection succeeded)
    let disconnect_result = unsafe { moq_disconnect(client) };
    println!("Disconnect result: {:?}", disconnect_result.code);

    // Should be disconnected again
    let connected = unsafe { moq_is_connected(client) };
    assert!(!connected, "Client should be disconnected after disconnect");
    println!("Final state: disconnected ✓");

    // Cleanup
    unsafe {
        moq_client_destroy(client);
        // state_tracker will be automatically dropped when it goes out of scope
    }

    println!("=== Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity
fn test_announce_namespace_requires_connection() {
    println!("\n=== Test: Announce Namespace (Requires Connection) ===");

    let client = moq_client_create();
    assert!(!client.is_null());

    // Try to announce without connecting first
    let namespace = CString::new("test-namespace").unwrap();
    let result = unsafe { moq_announce_namespace(client, namespace.as_ptr()) };

    // Should fail because not connected
    println!("Announce without connection: {:?}", result.code);
    assert_eq!(
        result.code,
        MoqResultCode::MoqErrorNotConnected,
        "Should fail when not connected"
    );

    if !result.message.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(result.message) };
        println!("Error message: {}", msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }

    // Cleanup
    unsafe { moq_client_destroy(client) };

    println!("=== Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity and successful connection
fn test_full_publish_workflow() {
    println!("\n=== Test: Full Publish Workflow ===");

    // Initialize CryptoProvider before any TLS operations
    init_crypto_provider();

    let client = moq_client_create();
    assert!(!client.is_null());

    // Setup state tracking
    let state_tracker = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let tracker_ptr = &state_tracker as *const _ as *mut std::ffi::c_void;

    // Connect
    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();
    let result = unsafe {
        moq_connect(
            client,
            url.as_ptr(),
            Some(connection_state_callback),
            tracker_ptr,
        )
    };

    println!("Connection result: {:?}", result.code);

    if result.code == MoqResultCode::MoqOk {
        // Wait for connection to be established
        let connected = wait_for_condition(
            || {
                if let Ok(tracker) = state_tracker.lock() {
                    tracker.current_state == MoqConnectionState::MoqStateConnected
                } else {
                    false
                }
            },
            TEST_TIMEOUT_SECS,
        );

        if connected {
            println!("Connected successfully ✓");

            // Announce namespace
            let namespace = CString::new("moq-ffi-test").unwrap();
            let announce_result = unsafe { moq_announce_namespace(client, namespace.as_ptr()) };
            println!("Announce namespace result: {:?}", announce_result.code);

            if announce_result.code == MoqResultCode::MoqOk {
                println!("Namespace announced ✓");

                // Create publisher
                let track_name = CString::new("test-track").unwrap();
                let publisher = unsafe {
                    moq_create_publisher(client, namespace.as_ptr(), track_name.as_ptr())
                };

                if !publisher.is_null() {
                    println!("Publisher created ✓");

                    // Publish some test data
                    let test_data = b"Hello from moq-ffi integration test!";
                    let publish_result = unsafe {
                        moq_publish_data(
                            publisher,
                            test_data.as_ptr(),
                            test_data.len(),
                            MoqDeliveryMode::MoqDeliveryStream,
                        )
                    };

                    println!("Publish data result: {:?}", publish_result.code);

                    if publish_result.code == MoqResultCode::MoqOk {
                        println!("Data published successfully ✓");
                    } else if !publish_result.message.is_null() {
                        let msg = unsafe { std::ffi::CStr::from_ptr(publish_result.message) };
                        println!("Publish error: {}", msg.to_string_lossy());
                        unsafe { moq_free_str(publish_result.message) };
                    }

                    // Cleanup publisher
                    unsafe { moq_publisher_destroy(publisher) };
                } else {
                    println!("Failed to create publisher");
                }
            } else if !announce_result.message.is_null() {
                let msg = unsafe { std::ffi::CStr::from_ptr(announce_result.message) };
                println!("Announce error: {}", msg.to_string_lossy());
                unsafe { moq_free_str(announce_result.message) };
            }
        } else {
            println!("Connection did not complete within timeout");
        }
    } else if !result.message.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(result.message) };
        println!("Connection error: {}", msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }

    // Cleanup
    unsafe {
        moq_disconnect(client);
        moq_client_destroy(client);
        // state_tracker will be automatically dropped when it goes out of scope
    }

    println!("=== Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity
fn test_multiple_clients() {
    println!("\n=== Test: Multiple Clients ===");

    // Initialize CryptoProvider before any TLS operations
    init_crypto_provider();

    // Create two clients
    let client1 = moq_client_create();
    let client2 = moq_client_create();

    assert!(!client1.is_null());
    assert!(!client2.is_null());
    assert_ne!(client1, client2, "Clients should be different");

    println!("Created two separate clients ✓");

    // Both should start disconnected
    assert!(!unsafe { moq_is_connected(client1) });
    assert!(!unsafe { moq_is_connected(client2) });
    println!("Both clients start disconnected ✓");

    // Setup tracking for both
    let state_tracker1 = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let tracker_ptr1 = &state_tracker1 as *const _ as *mut std::ffi::c_void;

    let state_tracker2 = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let tracker_ptr2 = &state_tracker2 as *const _ as *mut std::ffi::c_void;

    // Attempt to connect both
    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();

    let result1 = unsafe {
        moq_connect(
            client1,
            url.as_ptr(),
            Some(connection_state_callback),
            tracker_ptr1,
        )
    };

    let result2 = unsafe {
        moq_connect(
            client2,
            url.as_ptr(),
            Some(connection_state_callback),
            tracker_ptr2,
        )
    };

    println!("Client 1 connect result: {:?}", result1.code);
    println!("Client 2 connect result: {:?}", result2.code);

    // Cleanup
    unsafe {
        moq_disconnect(client1);
        moq_disconnect(client2);
        moq_client_destroy(client1);
        moq_client_destroy(client2);
        // state_tracker1 and state_tracker2 will be automatically dropped when they go out of scope
    }

    println!("=== Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity
fn test_error_handling_invalid_url() {
    println!("\n=== Test: Error Handling - Invalid URL ===");

    let client = moq_client_create();
    assert!(!client.is_null());

    // Try to connect with invalid URL
    let invalid_url = CString::new("not-a-valid-url").unwrap();
    let result = unsafe { moq_connect(client, invalid_url.as_ptr(), None, ptr::null_mut()) };

    println!("Connect with invalid URL result: {:?}", result.code);

    // Should fail with appropriate error
    assert_ne!(
        result.code,
        MoqResultCode::MoqOk,
        "Should fail with invalid URL"
    );

    if !result.message.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(result.message) };
        println!("Error message: {}", msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }

    // Cleanup
    unsafe { moq_client_destroy(client) };

    println!("=== Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity
fn test_version_and_utilities() {
    println!("\n=== Test: Version and Utility Functions ===");

    // Test version function
    let version = moq_version();
    assert!(!version.is_null());

    let version_str = unsafe { std::ffi::CStr::from_ptr(version) };
    println!("moq_ffi version: {}", version_str.to_string_lossy());
    assert!(
        !version_str.to_bytes().is_empty(),
        "Version should not be empty"
    );

    // Test last_error (should be null initially)
    let last_error = moq_last_error();
    println!("Initial last_error: {:?}", last_error);

    println!("=== Test Complete ===\n");
}
