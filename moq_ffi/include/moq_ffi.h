/*
 * MoQ FFI - C API for Media over QUIC Transport
 * 
 * This header provides a C interface to the Rust moq-transport library.
 * It enables C++ applications (including Unreal Engine plugins) to use
 * the MoQ protocol for low-latency media streaming.
 */

#ifndef MOQ_FFI_H
#define MOQ_FFI_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ───────────────────────────────────────────────
 * Platform-specific exports
 * ─────────────────────────────────────────────── */
#if defined(_WIN32) || defined(_WIN64)
    #ifdef MOQ_FFI_EXPORTS
        #define MOQ_API __declspec(dllexport)
    #else
        #define MOQ_API __declspec(dllimport)
    #endif
#else
    #define MOQ_API __attribute__((visibility("default")))
#endif

/* ───────────────────────────────────────────────
 * Types
 * ─────────────────────────────────────────────── */

/**
 * Opaque handle to a MoQ client session
 */
typedef struct MoqClient MoqClient;

/**
 * Opaque handle to a MoQ publisher
 */
typedef struct MoqPublisher MoqPublisher;

/**
 * Opaque handle to a MoQ subscriber
 */
typedef struct MoqSubscriber MoqSubscriber;

/**
 * Result code for MoQ operations
 */
typedef enum {
    MOQ_OK = 0,
    MOQ_ERROR_INVALID_ARGUMENT = 1,
    MOQ_ERROR_CONNECTION_FAILED = 2,
    MOQ_ERROR_NOT_CONNECTED = 3,
    MOQ_ERROR_TIMEOUT = 4,
    MOQ_ERROR_INTERNAL = 5,
    MOQ_ERROR_UNSUPPORTED = 6,
    MOQ_ERROR_BUFFER_TOO_SMALL = 7,
} MoqResultCode;

/**
 * Result structure
 */
typedef struct {
    MoqResultCode code;
    const char* message;  // Owned by FFI layer, free with moq_free_str()
} MoqResult;

/**
 * Connection state callback
 */
typedef enum {
    MOQ_STATE_DISCONNECTED = 0,
    MOQ_STATE_CONNECTING = 1,
    MOQ_STATE_CONNECTED = 2,
    MOQ_STATE_FAILED = 3,
} MoqConnectionState;

/**
 * Delivery mode for data transmission
 */
typedef enum {
    MOQ_DELIVERY_DATAGRAM = 0,  // Lossy, for high-frequency updates
    MOQ_DELIVERY_STREAM = 1,    // Reliable, for critical data
} MoqDeliveryMode;

/* ───────────────────────────────────────────────
 * Callbacks
 * ─────────────────────────────────────────────── */

/**
 * Connection state change callback
 * @param user_data User-provided context pointer
 * @param state New connection state
 */
typedef void (*MoqConnectionCallback)(void* user_data, MoqConnectionState state);

/**
 * Data received callback
 * @param user_data User-provided context pointer
 * @param data Pointer to received data buffer
 * @param data_len Length of received data
 */
typedef void (*MoqDataCallback)(void* user_data, const uint8_t* data, size_t data_len);

/**
 * Track announcement callback
 * @param user_data User-provided context pointer
 * @param namespace Namespace of the announced track
 * @param track_name Name of the announced track
 */
typedef void (*MoqTrackCallback)(void* user_data, const char* namespace_str, const char* track_name);

/* ───────────────────────────────────────────────
 * Track Discovery (Catalog-Based)
 * ─────────────────────────────────────────────── */

/**
 * Track information from catalog
 * 
 * Contains metadata about a track discovered via catalog subscription.
 * All string pointers are only valid during the callback invocation.
 */
typedef struct {
    const char* name;           /**< Track name (required, never NULL) */
    const char* codec;          /**< Codec string (may be NULL) */
    const char* mime_type;      /**< MIME type (may be NULL) */
    uint32_t width;             /**< Video width in pixels (0 if not applicable) */
    uint32_t height;            /**< Video height in pixels (0 if not applicable) */
    uint32_t bitrate;           /**< Bitrate in bits per second (0 if unknown) */
    uint32_t sample_rate;       /**< Audio sample rate in Hz (0 if not applicable) */
    const char* language;       /**< Language code, e.g., "en" (may be NULL) */
} MoqTrackInfo;

/**
 * Catalog update callback
 * 
 * Invoked when a catalog track is received with updated track information.
 * 
 * @param user_data User-provided context pointer
 * @param tracks Array of track info structures
 * @param track_count Number of tracks in the array
 * 
 * @note The tracks array and all strings within are only valid during the callback.
 *       Copy any data you need to retain before returning.
 * @note This callback may be invoked from a background thread.
 */
typedef void (*MoqCatalogCallback)(void* user_data, const MoqTrackInfo* tracks, size_t track_count);

/* ───────────────────────────────────────────────
 * Initialization
 * ─────────────────────────────────────────────── */

/**
 * Initialize the MoQ FFI crypto provider
 *
 * Must be called during module initialization before any TLS operations.
 * Safe to call multiple times - subsequent calls are no-ops.
 *
 * This ensures the rustls crypto provider is installed in the process
 * before any TLS connections are attempted.
 *
 * @return true on success (always succeeds)
 * 
 * @note Thread-safe: can be called from any thread
 * @note Idempotent: safe to call multiple times
 * @note Available since: v0.1.0
 * 
 * Example usage:
 * @code
 *   // C++: Call immediately after DLL load / module initialization
 *   moq_init();
 *   
 *   // ... then later ...
 *   // Now safe to create clients and connect
 *   MoqClient* client = moq_client_create();
 * @endcode
 */
MOQ_API bool moq_init(void);

/* ───────────────────────────────────────────────
 * Client Management
 * ─────────────────────────────────────────────── */

/**
 * Create a new MoQ client instance
 * @return Handle to the client or NULL on failure
 */
MOQ_API MoqClient* moq_client_create(void);

/**
 * Destroy a MoQ client and free resources
 * @param client Client handle
 */
MOQ_API void moq_client_destroy(MoqClient* client);

/**
 * Connect to a MoQ relay server
 * 
 * Supported URL schemes:
 * - https:// - WebTransport over QUIC (Draft 07 and Draft 14)
 * 
 * Future enhancements (Draft 14):
 * - quic:// - Raw QUIC connection (planned)
 * 
 * @param client Client handle
 * @param url Connection URL (e.g., "https://relay.example.com:443")
 * @param connection_callback Optional callback for connection state changes
 * @param user_data User context pointer passed to callbacks
 * @return Result of the connection attempt
 * 
 * @note Draft 07 (CloudFlare): WebTransport only
 * @note Draft 14 (Latest): WebTransport (raw QUIC planned for future release)
 */
MOQ_API MoqResult moq_connect(
    MoqClient* client,
    const char* url,
    MoqConnectionCallback connection_callback,
    void* user_data
);

/**
 * Disconnect from the MoQ relay
 * @param client Client handle
 * @return Result of the disconnection
 */
MOQ_API MoqResult moq_disconnect(MoqClient* client);

/**
 * Check if client is currently connected
 * @param client Client handle
 * @return true if connected, false otherwise
 */
MOQ_API bool moq_is_connected(const MoqClient* client);

/* ───────────────────────────────────────────────
 * Publishing
 * ─────────────────────────────────────────────── */

/**
 * Announce a namespace for publishing
 * @param client Client handle
 * @param namespace_str Namespace to announce
 * @return Result of the announcement
 */
MOQ_API MoqResult moq_announce_namespace(MoqClient* client, const char* namespace_str);

/**
 * Create a publisher for a specific track (defaults to stream mode)
 * @param client Client handle
 * @param namespace_str Namespace of the track
 * @param track_name Name of the track
 * @return Handle to the publisher or NULL on failure
 * @deprecated Use moq_create_publisher_ex() to specify delivery mode
 */
MOQ_API MoqPublisher* moq_create_publisher(
    MoqClient* client,
    const char* namespace_str,
    const char* track_name
);

/**
 * Create a publisher for a specific track with explicit delivery mode
 * @param client Client handle
 * @param namespace_str Namespace of the track
 * @param track_name Name of the track
 * @param delivery_mode Delivery mode (datagram or stream)
 * @return Handle to the publisher or NULL on failure
 */
MOQ_API MoqPublisher* moq_create_publisher_ex(
    MoqClient* client,
    const char* namespace_str,
    const char* track_name,
    MoqDeliveryMode delivery_mode
);

/**
 * Destroy a publisher
 * @param publisher Publisher handle
 */
MOQ_API void moq_publisher_destroy(MoqPublisher* publisher);

/**
 * Publish data on a track
 * @param publisher Publisher handle
 * @param data Data buffer to publish
 * @param data_len Length of data
 * @param delivery_mode Delivery mode (datagram or stream)
 * @return Result of the publish operation
 */
MOQ_API MoqResult moq_publish_data(
    MoqPublisher* publisher,
    const uint8_t* data,
    size_t data_len,
    MoqDeliveryMode delivery_mode
);

/* ───────────────────────────────────────────────
 * Subscribing
 * ─────────────────────────────────────────────── */

/**
 * Subscribe to a track
 * @param client Client handle
 * @param namespace_str Namespace of the track
 * @param track_name Name of the track
 * @param data_callback Callback for received data
 * @param user_data User context pointer passed to callbacks
 * @return Handle to the subscriber or NULL on failure
 */
MOQ_API MoqSubscriber* moq_subscribe(
    MoqClient* client,
    const char* namespace_str,
    const char* track_name,
    MoqDataCallback data_callback,
    void* user_data
);

/**
 * Unsubscribe and destroy a subscriber
 * @param subscriber Subscriber handle
 */
MOQ_API void moq_subscriber_destroy(MoqSubscriber* subscriber);

/**
 * Unsubscribe from a track without destroying the subscriber handle
 * 
 * This function stops receiving data from the track by:
 * - Aborting the reader task (stops processing incoming data)
 * - Dropping the track reader (signals to the relay)
 * - Marking the subscriber as unsubscribed
 * 
 * After calling this function:
 * - No more data callbacks will be invoked
 * - moq_is_subscribed() will return false
 * - The subscriber handle remains valid but inactive
 * - Call moq_subscriber_destroy() to free the handle
 * 
 * @param subscriber Subscriber handle
 * @return MOQ_OK on success or if already unsubscribed,
 *         MOQ_ERROR_INVALID_ARGUMENT if subscriber is null
 * 
 * @note Thread-safe
 * @note Idempotent: safe to call multiple times
 * @note Available since: v0.2.0
 * 
 * Example usage:
 * @code
 *   MoqSubscriber* sub = moq_subscribe(client, "ns", "track", callback, NULL);
 *   // ... receive data ...
 *   
 *   // Stop receiving without destroying handle
 *   MoqResult result = moq_unsubscribe(sub);
 *   if (result.code == MOQ_OK) {
 *       // No longer receiving data
 *       // Can still query moq_is_subscribed(sub) == false
 *   }
 *   
 *   // When done, destroy the handle
 *   moq_subscriber_destroy(sub);
 * @endcode
 */
MOQ_API MoqResult moq_unsubscribe(MoqSubscriber* subscriber);

/**
 * Check if the subscriber is currently subscribed to a track
 * 
 * @param subscriber Subscriber handle
 * @return true if actively subscribed, false if null/unsubscribed/error
 * 
 * @note Thread-safe
 * @note Available since: v0.2.0
 */
MOQ_API bool moq_is_subscribed(const MoqSubscriber* subscriber);

/* ───────────────────────────────────────────────
 * Namespace Announcement Discovery
 * ─────────────────────────────────────────────── */

/**
 * Subscribe to namespace announcements from other publishers
 * 
 * Registers a callback to be invoked when the relay forwards ANNOUNCE messages
 * from other publishers. This enables dynamic discovery of available namespaces
 * without knowing them in advance.
 * 
 * **Current Limitations:**
 * - Most relays (including Cloudflare) do not currently forward announcements
 *   to subscribers. This function prepares your application for future relay
 *   support of SUBSCRIBE_NAMESPACE or similar discovery mechanisms.
 * - Until relay support is available, the callback will not be invoked.
 * 
 * **Future Behavior (when relay support is available):**
 * - Callback will be invoked for each namespace announced by other publishers
 * - namespace_str: The announced namespace path (e.g., "mocap/performer1")
 * - track_name: Will be NULL for namespace-level announcements
 * 
 * @param client Client handle (must be connected)
 * @param callback Callback for namespace announcements (may be NULL to unregister)
 * @param user_data User context pointer passed to callback
 * @return MOQ_OK on success, error code on failure
 * 
 * @note Thread-safe
 * @note Callback may be invoked from a background thread
 * @note Call with callback=NULL to stop receiving announcements
 * @note Available since: v0.2.0
 * 
 * Example usage:
 * @code
 *   void on_namespace_announced(void* ctx, const char* ns, const char* track) {
 *       printf("Publisher available: %s\n", ns);
 *       // Now you can subscribe to tracks in this namespace
 *   }
 *   
 *   // Register for announcements (future-proofing)
 *   moq_subscribe_announces(client, on_namespace_announced, user_data);
 *   
 *   // For now, also use manual namespace specification
 *   // since relays don't forward announcements yet
 * @endcode
 */
MOQ_API MoqResult moq_subscribe_announces(
    MoqClient* client,
    MoqTrackCallback callback,
    void* user_data
);

/* ───────────────────────────────────────────────
 * Catalog Subscription (Track Discovery)
 * ─────────────────────────────────────────────── */

/**
 * Subscribe to a catalog track for automatic track discovery
 * 
 * The catalog track should publish JSON data conforming to the MoQ catalog format
 * (draft-ietf-moq-catalogformat). When catalog updates are received, the callback
 * is invoked with the parsed track list.
 * 
 * This enables applications to discover dynamically created tracks within a
 * known namespace without needing to know track names in advance.
 * 
 * Common catalog track names:
 * - "catalog" or "catalog.json" (convention for JSON catalogs)
 * - The init segment track name for CMAF workflows
 * 
 * @param client Client handle (must be connected)
 * @param namespace_str Namespace containing the catalog track
 * @param catalog_track_name Name of the catalog track (e.g., "catalog")
 * @param callback Callback for track list updates (may be NULL)
 * @param user_data User context pointer passed to callback
 * @return Subscriber handle for the catalog, or NULL on failure
 * 
 * @note Thread-safe
 * @note Callback may be invoked from a background thread; ensure thread-safe access
 *       to any shared data within the callback
 * @note If callback is NULL, the catalog is subscribed but no notifications are sent
 * 
 * Example usage:
 * @code
 *   void on_tracks_updated(void* ctx, const MoqTrackInfo* tracks, size_t count) {
 *       for (size_t i = 0; i < count; i++) {
 *           printf("Track: %s (codec: %s)\n", 
 *                  tracks[i].name, 
 *                  tracks[i].codec ? tracks[i].codec : "unknown");
 *       }
 *   }
 *   
 *   MoqSubscriber* catalog = moq_subscribe_catalog(
 *       client, "my-broadcast", "catalog", on_tracks_updated, NULL);
 *   if (catalog) {
 *       // Catalog subscription active, callback will fire on updates
 *   }
 * @endcode
 */
MOQ_API MoqSubscriber* moq_subscribe_catalog(
    MoqClient* client,
    const char* namespace_str,
    const char* catalog_track_name,
    MoqCatalogCallback callback,
    void* user_data
);

/* ───────────────────────────────────────────────
 * Utilities
 * ─────────────────────────────────────────────── */

/**
 * Free a string allocated by the FFI layer
 * @param str String to free
 */
MOQ_API void moq_free_str(const char* str);

/**
 * Get the version string of the MoQ FFI library
 * @return Version string (do not free)
 */
MOQ_API const char* moq_version(void);

/**
 * Get the last error message for the current thread
 * @return Error message string (do not free) or NULL if no error
 */
MOQ_API const char* moq_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* MOQ_FFI_H */
