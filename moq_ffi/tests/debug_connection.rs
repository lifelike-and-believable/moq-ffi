// Debug test to understand connection hanging issue
//
// This test helps diagnose why connections hang in "Connecting" state

#![cfg(feature = "with_moq_draft07")]

use std::ffi::CString;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;

use moq_ffi::*;

// Initialize CryptoProvider for rustls (required for TLS connections)
fn init_crypto_provider() {
    use rustls::crypto::CryptoProvider;
    match CryptoProvider::install_default(rustls::crypto::aws_lc_rs::default_provider()) {
        Ok(()) => {
            println!("✓ CryptoProvider installed successfully");
        }
        Err(_) => {
            println!("✓ CryptoProvider already installed");
        }
    }
}

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
    
    let tracker = &*(user_data as *const Arc<Mutex<ConnectionStateTracker>>);
    if let Ok(mut t) = tracker.lock() {
        t.current_state = state;
        t.state_changed = true;
        println!("🔔 [Callback] Connection state changed to: {:?}", state);
    }
}

#[test]
#[ignore] // Manual test for debugging
fn test_debug_cloudflare_connection() {
    println!("\n═══════════════════════════════════════");
    println!("DEBUG: Testing connection to Cloudflare");
    println!("═══════════════════════════════════════\n");
    
    // Step 1: Initialize crypto provider
    println!("Step 1: Initialize CryptoProvider");
    init_crypto_provider();
    
    // Step 2: Create client
    println!("\nStep 2: Create MoQ client");
    let client = moq_client_create();
    assert!(!client.is_null(), "Failed to create client");
    println!("✓ Client created: {:?}", client);
    
    // Step 3: Setup connection state tracking
    println!("\nStep 3: Setup connection state tracking");
    let state_tracker = Arc::new(Mutex::new(ConnectionStateTracker::new()));
    let tracker_ptr = &state_tracker as *const _ as *mut std::ffi::c_void;
    println!("✓ State tracker setup complete");
    
    // Step 4: Initiate connection
    println!("\nStep 4: Connect to Cloudflare relay");
    let url = CString::new("https://relay.cloudflare.mediaoverquic.com").unwrap();
    println!("  URL: https://relay.cloudflare.mediaoverquic.com");
    
    println!("  Calling moq_connect...");
    let start_time = std::time::Instant::now();
    
    let result = unsafe {
        moq_connect(
            client,
            url.as_ptr(),
            Some(connection_state_callback),
            tracker_ptr,
        )
    };
    
    let connect_duration = start_time.elapsed();
    println!("  moq_connect returned after {:?}", connect_duration);
    println!("  Result code: {:?}", result.code);
    
    if !result.message.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(result.message) };
        println!("  Error message: {}", msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }
    
    // Step 5: Check state
    println!("\nStep 5: Check connection state");
    if let Ok(tracker) = state_tracker.lock() {
        println!("  Current state: {:?}", tracker.current_state);
        println!("  State changed: {}", tracker.state_changed);
    }
    
    // Step 6: Wait for a few seconds to see if state changes
    println!("\nStep 6: Wait 5 seconds for state changes...");
    for i in 1..=5 {
        thread::sleep(Duration::from_secs(1));
        if let Ok(tracker) = state_tracker.lock() {
            println!("  [{} sec] State: {:?}", i, tracker.current_state);
        }
    }
    
    // Step 7: Try to disconnect
    println!("\nStep 7: Disconnect");
    let disconnect_start = std::time::Instant::now();
    let disconnect_result = unsafe { moq_disconnect(client) };
    let disconnect_duration = disconnect_start.elapsed();
    println!("  moq_disconnect returned after {:?}", disconnect_duration);
    println!("  Disconnect result: {:?}", disconnect_result.code);
    
    // Step 8: Cleanup
    println!("\nStep 8: Cleanup");
    unsafe { moq_client_destroy(client) };
    println!("✓ Client destroyed");
    
    println!("\n═══════════════════════════════════════");
    println!("DEBUG: Test Complete");
    println!("═══════════════════════════════════════\n");
}

#[test]
#[ignore] // Manual test for debugging
fn test_debug_invalid_url() {
    println!("\n═══════════════════════════════════════");
    println!("DEBUG: Testing with invalid URL");
    println!("═══════════════════════════════════════\n");
    
    init_crypto_provider();
    
    let client = moq_client_create();
    assert!(!client.is_null());
    
    let url = CString::new("invalid-url-format").unwrap();
    
    println!("Connecting to invalid URL...");
    let start_time = std::time::Instant::now();
    
    let result = unsafe {
        moq_connect(
            client,
            url.as_ptr(),
            None,
            std::ptr::null_mut(),
        )
    };
    
    let duration = start_time.elapsed();
    println!("moq_connect returned after {:?}", duration);
    println!("Result code: {:?}", result.code);
    
    if !result.message.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(result.message) };
        println!("Error message: {}", msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }
    
    // Should fail immediately, not hang
    assert_ne!(result.code, MoqResultCode::MoqOk);
    assert!(duration.as_secs() < 5, "Should fail quickly, not hang");
    
    unsafe { moq_client_destroy(client) };
    
    println!("\n✓ Test passed - invalid URL fails quickly\n");
}

#[test]
#[ignore] // Manual test for debugging
fn test_debug_unreachable_host() {
    println!("\n═══════════════════════════════════════");
    println!("DEBUG: Testing with unreachable host");
    println!("═══════════════════════════════════════\n");
    
    init_crypto_provider();
    
    let client = moq_client_create();
    assert!(!client.is_null());
    
    // Use TEST-NET-1 (192.0.2.1) - reserved for documentation, not routable
    let url = CString::new("https://192.0.2.1:443").unwrap();
    
    println!("Connecting to unreachable host (should timeout)...");
    let start_time = std::time::Instant::now();
    
    let result = unsafe {
        moq_connect(
            client,
            url.as_ptr(),
            None,
            std::ptr::null_mut(),
        )
    };
    
    let duration = start_time.elapsed();
    println!("moq_connect returned after {:?}", duration);
    println!("Result code: {:?}", result.code);
    
    if !result.message.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(result.message) };
        println!("Error message: {}", msg.to_string_lossy());
        unsafe { moq_free_str(result.message) };
    }
    
    // Should timeout after ~30 seconds
    println!("\nTimeout behavior:");
    println!("  Expected: ~30 seconds");
    println!("  Actual: {:?}", duration);
    println!("  Result: {}", if duration.as_secs() >= 29 && duration.as_secs() <= 35 {
        "✓ Timeout working correctly"
    } else {
        "✗ Timeout not working as expected"
    });
    
    unsafe { moq_client_destroy(client) };
    
    println!("\n═══════════════════════════════════════\n");
}
