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

use std::collections::BTreeSet;
use std::ffi::{CStr, CString};
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Import FFI functions and types from the moq_ffi library
// Integration tests run as external crates and import via the library name
use moq_ffi::*;

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
    payloads: Vec<Vec<u8>>,
}

#[allow(dead_code)]
impl DataTracker {
    fn new() -> Self {
        Self {
            data_received: false,
            received_count: 0,
            last_data_size: 0,
            payloads: Vec::new(),
        }
    }
}

/// Callback for received data (for future subscribe tests)
#[allow(dead_code)]
unsafe extern "C" fn data_received_callback(
    user_data: *mut std::ffi::c_void,
    data: *const u8,
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
        if !data.is_null() && data_len > 0 {
            let payload = slice::from_raw_parts(data, data_len).to_vec();
            t.payloads.push(payload);
        } else {
            t.payloads.push(Vec::new());
        }
        println!("[Callback] Received data: {} bytes (total count: {})", data_len, t.received_count);
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

fn log_result(label: &str, result: &MoqResult) {
    println!("{} result: {:?}", label, result.code);
    if !result.message.is_null() {
        let msg = unsafe { CStr::from_ptr(result.message) };
        println!("{} message: {}", label, msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }
}

fn last_error_message() -> Option<String> {
    unsafe {
        let err_ptr = moq_last_error();
        if err_ptr.is_null() {
            None
        } else {
            CStr::from_ptr(err_ptr).to_str().ok().map(|s| s.to_string())
        }
    }
}

#[test]
#[ignore] // Requires network connectivity
fn test_connect_to_cloudflare_relay() {
    println!("\n=== Test: Connect to Cloudflare Relay ===");
    
    // Test the recommended usage pattern: initialize FFI before creating clients
    assert!(moq_init(), "moq_init() should succeed");
    println!("moq_init() succeeded");
    
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
        result.code == MoqResultCode::MoqOk || 
        result.code == MoqResultCode::MoqErrorTimeout,
        "Connection should either succeed or timeout"
    );
    
    // Wait for state change (if connection was initiated)
    if result.code == MoqResultCode::MoqOk {
        let connected = wait_for_condition(
            || {
                if let Ok(tracker) = state_tracker.lock() {
                    tracker.state_changed && 
                    (tracker.current_state == MoqConnectionState::MoqStateConnected ||
                     tracker.current_state == MoqConnectionState::MoqStateFailed)
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
        assert!(connected || result.code == MoqResultCode::MoqErrorTimeout, 
                "Connection should complete (success or failure) within timeout");
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
    
    // Test the recommended usage pattern: initialize FFI before creating clients
    assert!(moq_init(), "moq_init() should succeed");
    
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
        moq_connect(client, url.as_ptr(), Some(connection_state_callback), tracker_ptr)
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
    
    // Test the recommended usage pattern: initialize FFI before creating clients
    assert!(moq_init(), "moq_init() should succeed");
    
    let client = moq_client_create();
    assert!(!client.is_null());
    
    // Setup state tracking
    let state_tracker = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let tracker_ptr = &state_tracker as *const _ as *mut std::ffi::c_void;
    
    // Connect
    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();
    let result = unsafe {
        moq_connect(client, url.as_ptr(), Some(connection_state_callback), tracker_ptr)
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
#[ignore] // Requires network connectivity and relay pub/sub support
fn test_publish_subscribe_roundtrip() {
    println!("\n=== Test: Publish/Subscribe Roundtrip ===");
    assert!(moq_init(), "moq_init() should succeed");

    let publisher_client = moq_client_create();
    let subscriber_client = moq_client_create();
    assert!(!publisher_client.is_null(), "Failed to create publisher client");
    assert!(!subscriber_client.is_null(), "Failed to create subscriber client");

    let publisher_state = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let subscriber_state = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let pub_state_ptr = &publisher_state as *const _ as *mut std::ffi::c_void;
    let sub_state_ptr = &subscriber_state as *const _ as *mut std::ffi::c_void;

    let data_tracker = Arc::new(Mutex::new(DataTracker::new()));
    let data_tracker_ptr = &data_tracker as *const _ as *mut std::ffi::c_void;

    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();
    let publisher_connect = unsafe {
        moq_connect(
            publisher_client,
            url.as_ptr(),
            Some(connection_state_callback),
            pub_state_ptr,
        )
    };
    log_result("Publisher connect", &publisher_connect);
    assert!(
        publisher_connect.code == MoqResultCode::MoqOk
            || publisher_connect.code == MoqResultCode::MoqErrorTimeout,
        "Publisher connect should succeed or report timeout"
    );

    let subscriber_connect = unsafe {
        moq_connect(
            subscriber_client,
            url.as_ptr(),
            Some(connection_state_callback),
            sub_state_ptr,
        )
    };
    log_result("Subscriber connect", &subscriber_connect);
    assert!(
        subscriber_connect.code == MoqResultCode::MoqOk
            || subscriber_connect.code == MoqResultCode::MoqErrorTimeout,
        "Subscriber connect should succeed or report timeout"
    );

    let publisher_connected = wait_for_condition(
        || {
            if let Ok(state) = publisher_state.lock() {
                state.current_state == MoqConnectionState::MoqStateConnected
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(
        publisher_connected,
        "Publisher client failed to reach connected state within timeout"
    );

    let subscriber_connected = wait_for_condition(
        || {
            if let Ok(state) = subscriber_state.lock() {
                state.current_state == MoqConnectionState::MoqStateConnected
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(
        subscriber_connected,
        "Subscriber client failed to reach connected state within timeout"
    );

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    let namespace = format!("moq-ffi-test/roundtrip-{}", timestamp);
    let track_name = format!("track-{}", timestamp);

    let namespace_c = CString::new(namespace.clone()).unwrap();
    let announce_result = unsafe { moq_announce_namespace(publisher_client, namespace_c.as_ptr()) };
    log_result("Announce namespace", &announce_result);
    assert_eq!(
        announce_result.code,
        MoqResultCode::MoqOk,
        "Announce namespace should succeed"
    );

    let track_c = CString::new(track_name.clone()).unwrap();
    let publisher_handle = unsafe {
        moq_create_publisher(publisher_client, namespace_c.as_ptr(), track_c.as_ptr())
    };
    assert!(
        !publisher_handle.is_null(),
        "Publisher handle should not be null"
    );

    // Publish initial data to make the track "live" before subscribing
    let initial_payload = b"ffi-roundtrip-init".to_vec();
    let init_publish = unsafe {
        moq_publish_data(
            publisher_handle,
            initial_payload.as_ptr(),
            initial_payload.len(),
            MoqDeliveryMode::MoqDeliveryStream,
        )
    };
    log_result("Initial publish to make track live", &init_publish);
    
    // Give the relay time to propagate the track
    std::thread::sleep(Duration::from_millis(500));

    let namespace_c_sub = CString::new(namespace.clone()).unwrap();
    let track_c_sub = CString::new(track_name.clone()).unwrap();
    let subscriber_handle = unsafe {
        moq_subscribe(
            subscriber_client,
            namespace_c_sub.as_ptr(),
            track_c_sub.as_ptr(),
            Some(data_received_callback),
            data_tracker_ptr,
        )
    };
    assert!(
        !subscriber_handle.is_null(),
        "Subscriber handle should not be null: {:?}",
        last_error_message()
    );

    let payloads: Vec<Vec<u8>> = vec![
        b"ffi-roundtrip-object-1".to_vec(),
        b"ffi-roundtrip-object-2".to_vec(),
    ];

    for (index, payload) in payloads.iter().enumerate() {
        let publish_result = unsafe {
            moq_publish_data(
                publisher_handle,
                payload.as_ptr(),
                payload.len(),
                MoqDeliveryMode::MoqDeliveryStream,
            )
        };
        log_result(&format!("Publish payload {}", index), &publish_result);
        assert_eq!(
            publish_result.code,
            MoqResultCode::MoqOk,
            "Publish {} should succeed",
            index
        );
        std::thread::sleep(Duration::from_millis(200));
    }

    let expected_count = payloads.len();
    let received = wait_for_condition(
        || {
            if let Ok(tracker) = data_tracker.lock() {
                tracker.received_count >= expected_count
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(
        received,
        "Expected subscriber to receive {} payloads",
        expected_count
    );

    if let Ok(tracker) = data_tracker.lock() {
        assert!(
            tracker.payloads.len() >= expected_count,
            "Subscriber did not store all payloads"
        );
        for (index, expected_payload) in payloads.iter().enumerate() {
            let received_payload = tracker
                .payloads
                .get(index)
                .expect("Missing payload entry");
            assert_eq!(
                received_payload,
                expected_payload,
                "Payload {} data mismatch",
                index
            );
        }
    }

    unsafe {
        moq_subscriber_destroy(subscriber_handle);
        moq_publisher_destroy(publisher_handle);
        moq_disconnect(publisher_client);
        moq_disconnect(subscriber_client);
        moq_client_destroy(publisher_client);
        moq_client_destroy(subscriber_client);
    }

    println!("=== Publish/Subscribe Roundtrip Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity and relay pub/sub support
fn test_publish_subscribe_single_object_group() {
    println!("\n=== Test: Publish/Subscribe Single Object per Group ===");
    assert!(moq_init(), "moq_init() should succeed");

    let publisher_client = moq_client_create();
    let subscriber_client = moq_client_create();
    assert!(!publisher_client.is_null(), "Failed to create publisher client");
    assert!(!subscriber_client.is_null(), "Failed to create subscriber client");

    let publisher_state = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let subscriber_state = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let pub_state_ptr = &publisher_state as *const _ as *mut std::ffi::c_void;
    let sub_state_ptr = &subscriber_state as *const _ as *mut std::ffi::c_void;

    let data_tracker = Arc::new(Mutex::new(DataTracker::new()));
    let data_tracker_ptr = &data_tracker as *const _ as *mut std::ffi::c_void;

    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();
    let publisher_connect = unsafe {
        moq_connect(
            publisher_client,
            url.as_ptr(),
            Some(connection_state_callback),
            pub_state_ptr,
        )
    };
    log_result("Publisher connect", &publisher_connect);
    assert!(
        publisher_connect.code == MoqResultCode::MoqOk
            || publisher_connect.code == MoqResultCode::MoqErrorTimeout,
        "Publisher connect should succeed or timeout"
    );

    let subscriber_connect = unsafe {
        moq_connect(
            subscriber_client,
            url.as_ptr(),
            Some(connection_state_callback),
            sub_state_ptr,
        )
    };
    log_result("Subscriber connect", &subscriber_connect);
    assert!(
        subscriber_connect.code == MoqResultCode::MoqOk
            || subscriber_connect.code == MoqResultCode::MoqErrorTimeout,
        "Subscriber connect should succeed or timeout"
    );

    let publisher_connected = wait_for_condition(
        || {
            if let Ok(state) = publisher_state.lock() {
                state.current_state == MoqConnectionState::MoqStateConnected
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(publisher_connected, "Publisher did not enter connected state");

    let subscriber_connected = wait_for_condition(
        || {
            if let Ok(state) = subscriber_state.lock() {
                state.current_state == MoqConnectionState::MoqStateConnected
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(subscriber_connected, "Subscriber did not enter connected state");

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_micros();
    let namespace = format!("moq-ffi-test/single-object-{}", timestamp);
    let track_name = format!("track-{}", timestamp);

    let namespace_c = CString::new(namespace.clone()).unwrap();
    let track_c = CString::new(track_name.clone()).unwrap();

    let announce_result = unsafe { moq_announce_namespace(publisher_client, namespace_c.as_ptr()) };
    log_result("Announce namespace", &announce_result);
    assert_eq!(announce_result.code, MoqResultCode::MoqOk);

    let publisher_handle = unsafe {
        moq_create_publisher(publisher_client, namespace_c.as_ptr(), track_c.as_ptr())
    };
    assert!(!publisher_handle.is_null(), "Publisher handle should not be null");

    // Publish initial data to make the track "live" before subscribing
    let init_payload = b"ffi-single-object-init".to_vec();
    let init_publish = unsafe {
        moq_publish_data(
            publisher_handle,
            init_payload.as_ptr(),
            init_payload.len(),
            MoqDeliveryMode::MoqDeliveryStream,
        )
    };
    log_result("Initial publish to make track live", &init_publish);
    
    // Give the relay time to propagate the track
    std::thread::sleep(Duration::from_millis(500));

    let namespace_c_sub = CString::new(namespace.clone()).unwrap();
    let track_c_sub = CString::new(track_name.clone()).unwrap();
    let subscriber_handle = unsafe {
        moq_subscribe(
            subscriber_client,
            namespace_c_sub.as_ptr(),
            track_c_sub.as_ptr(),
            Some(data_received_callback),
            data_tracker_ptr,
        )
    };
    assert!(
        !subscriber_handle.is_null(),
        "Subscriber handle should not be null: {:?}",
        last_error_message()
    );

    // Publish a single payload to match one-object-per-group semantics
    let payload = b"ffi-single-object-test".to_vec();
    let publish_result = unsafe {
        moq_publish_data(
            publisher_handle,
            payload.as_ptr(),
            payload.len(),
            MoqDeliveryMode::MoqDeliveryStream,
        )
    };
    log_result("Publish single payload", &publish_result);
    assert_eq!(publish_result.code, MoqResultCode::MoqOk, "Publish should succeed");

    let received = wait_for_condition(
        || {
            if let Ok(tracker) = data_tracker.lock() {
                tracker.received_count >= 1
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(received, "Subscriber did not receive payload");

    if let Ok(tracker) = data_tracker.lock() {
        let first_payload = tracker.payloads.get(0).expect("Missing payload entry");
        assert_eq!(first_payload, &payload, "Single payload mismatch");
    }

    unsafe {
        moq_subscriber_destroy(subscriber_handle);
        moq_publisher_destroy(publisher_handle);
        moq_disconnect(publisher_client);
        moq_disconnect(subscriber_client);
        moq_client_destroy(publisher_client);
        moq_client_destroy(subscriber_client);
    }

    println!("=== Single Object per Group Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity and relay datagram support
fn test_publish_subscribe_datagram_delivery() {
    println!("\n=== Test: Publish/Subscribe via Datagram Delivery ===");
    assert!(moq_init(), "moq_init() should succeed");

    let publisher_client = moq_client_create();
    let subscriber_client = moq_client_create();
    assert!(!publisher_client.is_null(), "Failed to create publisher client");
    assert!(!subscriber_client.is_null(), "Failed to create subscriber client");

    let publisher_state = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let subscriber_state = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let pub_state_ptr = &publisher_state as *const _ as *mut std::ffi::c_void;
    let sub_state_ptr = &subscriber_state as *const _ as *mut std::ffi::c_void;

    let data_tracker = Arc::new(Mutex::new(DataTracker::new()));
    let data_tracker_ptr = &data_tracker as *const _ as *mut std::ffi::c_void;

    let url = CString::new(CLOUDFLARE_RELAY_URL).unwrap();
    let publisher_connect = unsafe {
        moq_connect(
            publisher_client,
            url.as_ptr(),
            Some(connection_state_callback),
            pub_state_ptr,
        )
    };
    log_result("Publisher connect", &publisher_connect);
    assert!(
        publisher_connect.code == MoqResultCode::MoqOk
            || publisher_connect.code == MoqResultCode::MoqErrorTimeout,
        "Publisher connect should succeed or timeout"
    );

    let subscriber_connect = unsafe {
        moq_connect(
            subscriber_client,
            url.as_ptr(),
            Some(connection_state_callback),
            sub_state_ptr,
        )
    };
    log_result("Subscriber connect", &subscriber_connect);
    assert!(
        subscriber_connect.code == MoqResultCode::MoqOk
            || subscriber_connect.code == MoqResultCode::MoqErrorTimeout,
        "Subscriber connect should succeed or timeout"
    );

    let publisher_connected = wait_for_condition(
        || {
            if let Ok(state) = publisher_state.lock() {
                state.current_state == MoqConnectionState::MoqStateConnected
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(publisher_connected, "Publisher did not enter connected state");

    let subscriber_connected = wait_for_condition(
        || {
            if let Ok(state) = subscriber_state.lock() {
                state.current_state == MoqConnectionState::MoqStateConnected
            } else {
                false
            }
        },
        TEST_TIMEOUT_SECS,
    );
    assert!(subscriber_connected, "Subscriber did not enter connected state");

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_micros();
    let namespace = format!("moq-ffi-test/datagram-{}", timestamp);
    let track_name = format!("track-{}", timestamp);

    let namespace_c = CString::new(namespace.clone()).unwrap();
    let track_c = CString::new(track_name.clone()).unwrap();

    let announce_result = unsafe { moq_announce_namespace(publisher_client, namespace_c.as_ptr()) };
    log_result("Announce namespace", &announce_result);
    assert_eq!(announce_result.code, MoqResultCode::MoqOk);

    let datagram_publisher = unsafe {
        moq_create_publisher_ex(
            publisher_client,
            namespace_c.as_ptr(),
            track_c.as_ptr(),
            MoqDeliveryMode::MoqDeliveryDatagram,
        )
    };
    assert!(
        !datagram_publisher.is_null(),
        "Datagram publisher handle should not be null"
    );

    // Publish initial datagram to make the track "live" before subscribing
    let init_payload = b"ffi-dgram-init".to_vec();
    let init_publish = unsafe {
        moq_publish_data(
            datagram_publisher,
            init_payload.as_ptr(),
            init_payload.len(),
            MoqDeliveryMode::MoqDeliveryDatagram,
        )
    };
    log_result("Initial datagram publish to make track live", &init_publish);
    
    // Give the relay time to propagate the track
    std::thread::sleep(Duration::from_millis(500));

    let namespace_c_sub = CString::new(namespace.clone()).unwrap();
    let track_c_sub = CString::new(track_name.clone()).unwrap();
    let subscriber_handle = unsafe {
        moq_subscribe(
            subscriber_client,
            namespace_c_sub.as_ptr(),
            track_c_sub.as_ptr(),
            Some(data_received_callback),
            data_tracker_ptr,
        )
    };
    assert!(
        !subscriber_handle.is_null(),
        "Subscriber handle should not be null: {:?}",
        last_error_message()
    );

    let payloads: Vec<Vec<u8>> = vec![
        b"ffi-dgram-object-1".to_vec(),
        b"ffi-dgram-object-2".to_vec(),
        b"ffi-dgram-object-3".to_vec(),
    ];

    for (index, payload) in payloads.iter().enumerate() {
        let publish_result = unsafe {
            moq_publish_data(
                datagram_publisher,
                payload.as_ptr(),
                payload.len(),
                MoqDeliveryMode::MoqDeliveryDatagram,
            )
        };
        log_result(&format!("Publish datagram {}", index), &publish_result);
        assert_eq!(
            publish_result.code,
            MoqResultCode::MoqOk,
            "Datagram publish {} should succeed",
            index
        );
        std::thread::sleep(Duration::from_millis(150));
    }

    let expected_count = payloads.len();
    // Datagrams are unreliable - wait to see if any arrive, but don't fail if none do
    let _received = wait_for_condition(
        || {
            if let Ok(tracker) = data_tracker.lock() {
                // For datagrams, we accept at least 1 received (unreliable delivery)
                tracker.received_count >= 1
            } else {
                false
            }
        },
        5, // Short timeout since datagrams may not arrive at all
    );
    
    if let Ok(tracker) = data_tracker.lock() {
        println!("Received {}/{} datagram payloads (datagrams are unreliable, 0 is acceptable)", 
                 tracker.received_count, expected_count);
        
        // Verify that any received payloads match expected ones
        if !tracker.payloads.is_empty() {
            let expected_set: BTreeSet<Vec<u8>> = payloads.clone().into_iter().collect();
            for received_payload in &tracker.payloads {
                assert!(
                    expected_set.contains(received_payload),
                    "Received unexpected datagram payload"
                );
            }
            println!("All received datagrams matched expected payloads");
        }
    }
    
    // The test passes if publishes succeeded - datagram delivery is best-effort
    println!("Datagram test completed - publishes succeeded, delivery is best-effort");

    unsafe {
        moq_subscriber_destroy(subscriber_handle);
        moq_publisher_destroy(datagram_publisher);
        moq_disconnect(publisher_client);
        moq_disconnect(subscriber_client);
        moq_client_destroy(publisher_client);
        moq_client_destroy(subscriber_client);
    }

    println!("=== Datagram Publish/Subscribe Test Complete ===\n");
}

#[test]
#[ignore] // Requires network connectivity
fn test_multiple_clients() {
    println!("\n=== Test: Multiple Clients ===");
    
    // Test the recommended usage pattern: initialize FFI before creating clients
    assert!(moq_init(), "moq_init() should succeed");
    
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
        moq_connect(client1, url.as_ptr(), Some(connection_state_callback), tracker_ptr1)
    };
    
    let result2 = unsafe {
        moq_connect(client2, url.as_ptr(), Some(connection_state_callback), tracker_ptr2)
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
    let result = unsafe {
        moq_connect(client, invalid_url.as_ptr(), None, ptr::null_mut())
    };
    
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
    assert!(!version_str.to_bytes().is_empty(), "Version should not be empty");
    
    // Test last_error (should be null initially)
    let last_error = moq_last_error();
    println!("Initial last_error: {:?}", last_error);
    
    println!("=== Test Complete ===\n");
}
