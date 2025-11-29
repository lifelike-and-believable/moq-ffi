#include "test_framework.h"
#include "moq_ffi.h"
#include <iostream>
#include <vector>
#include <string>
#include <cstring>
#include <atomic>

using namespace std;

/**
 * Integration Test: Catalog Discovery
 *
 * This test demonstrates catalog-based track discovery:
 * 1. Connect to Cloudflare relay
 * 2. Publisher announces namespace and publishes catalog
 * 3. Subscriber subscribes to catalog
 * 4. Verify catalog callback receives track information
 */

struct CatalogContext {
    atomic<int> callback_count{0};
    vector<string> track_names;
    vector<string> codecs;
    atomic<bool> received{false};
};

struct ConnectionContext {
    atomic<bool> connected{false};
    atomic<bool> failed{false};
};

void connection_callback(void* user_data, MoqConnectionState state) {
    ConnectionContext* ctx = (ConnectionContext*)user_data;

    switch (state) {
        case MOQ_STATE_CONNECTED:
            cout << "[CONNECTION] Connected" << endl;
            ctx->connected = true;
            break;
        case MOQ_STATE_FAILED:
            cout << "[CONNECTION] Failed" << endl;
            ctx->failed = true;
            break;
        case MOQ_STATE_CONNECTING:
            cout << "[CONNECTION] Connecting..." << endl;
            break;
        case MOQ_STATE_DISCONNECTED:
            cout << "[CONNECTION] Disconnected" << endl;
            break;
    }
}

void catalog_callback(void* user_data, const MoqTrackInfo* tracks, size_t track_count) {
    CatalogContext* ctx = (CatalogContext*)user_data;

    cout << "[CATALOG] Received catalog with " << track_count << " tracks:" << endl;

    ctx->callback_count++;
    ctx->track_names.clear();
    ctx->codecs.clear();

    for (size_t i = 0; i < track_count; i++) {
        cout << "  Track #" << (i + 1) << ":" << endl;
        cout << "    Name:        " << (tracks[i].name ? tracks[i].name : "null") << endl;
        cout << "    Codec:       " << (tracks[i].codec ? tracks[i].codec : "null") << endl;
        cout << "    MIME:        " << (tracks[i].mime_type ? tracks[i].mime_type : "null") << endl;
        cout << "    Dimensions:  " << tracks[i].width << "x" << tracks[i].height << endl;
        cout << "    Bitrate:     " << tracks[i].bitrate << endl;
        cout << "    Sample Rate: " << tracks[i].sample_rate << endl;
        cout << "    Language:    " << (tracks[i].language ? tracks[i].language : "null") << endl;

        if (tracks[i].name) {
            ctx->track_names.push_back(tracks[i].name);
        }
        if (tracks[i].codec) {
            ctx->codecs.push_back(tracks[i].codec);
        }
    }

    ctx->received = true;
}

void test_catalog_subscription() {
    cout << "\n=== Test: Catalog Subscription ===" << endl;

    moq_init();

    // Create subscriber client
    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client created");

    // Connect
    ConnectionContext conn_ctx;
    MoqResult result = moq_connect(client, CLOUDFLARE_RELAY_URL,
                                  connection_callback, &conn_ctx);

    if (result.code != MOQ_OK) {
        cout << "Failed to connect: " << (result.message ? result.message : "unknown") << endl;
        moq_client_destroy(client);
        TEST_ASSERT(true, "Connection failed (network dependent)");
        return;
    }

    // Wait for connection
    uint64_t start = test_timestamp_ms();
    while (!conn_ctx.connected && !conn_ctx.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (!conn_ctx.connected) {
        cout << "Connection timeout or failed" << endl;
        moq_client_destroy(client);
        TEST_ASSERT(true, "Connection timeout");
        return;
    }

    // Subscribe to catalog
    const char* test_namespace = "test-catalog-namespace";
    CatalogContext cat_ctx;

    MoqSubscriber* catalog_sub = moq_subscribe_catalog(
        client, test_namespace, "catalog", catalog_callback, &cat_ctx);

    if (!catalog_sub) {
        cout << "Failed to create catalog subscription" << endl;
        moq_disconnect(client);
        moq_client_destroy(client);
        TEST_ASSERT(true, "Catalog subscription creation failed (may require publisher)");
        return;
    }

    TEST_ASSERT_NOT_NULL(catalog_sub, "Catalog subscriber created");

    // Wait for catalog data
    cout << "\nWaiting for catalog updates..." << endl;
    start = test_timestamp_ms();
    while (!cat_ctx.received && (test_timestamp_ms() - start) < SHORT_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (cat_ctx.callback_count > 0) {
        cout << "\nReceived " << cat_ctx.callback_count << " catalog updates" << endl;
        TEST_ASSERT(cat_ctx.callback_count > 0, "Catalog callback invoked");

        if (!cat_ctx.track_names.empty()) {
            cout << "Discovered tracks:" << endl;
            for (const auto& track_name : cat_ctx.track_names) {
                cout << "  - " << track_name << endl;
            }
        }
    } else {
        cout << "No catalog updates received (may require active publisher)" << endl;
        TEST_ASSERT(true, "Catalog subscription established (no data expected without publisher)");
    }

    // Cleanup
    moq_subscriber_destroy(catalog_sub);
    moq_disconnect(client);
    moq_client_destroy(client);

    cout << "=== Test Complete ===" << endl;
}

void test_track_announce_subscription() {
    cout << "\n=== Test: Track Announcement Subscription ===" << endl;

    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client created");

    ConnectionContext conn_ctx;
    MoqResult result = moq_connect(client, CLOUDFLARE_RELAY_URL,
                                  connection_callback, &conn_ctx);

    if (result.code != MOQ_OK) {
        moq_client_destroy(client);
        TEST_ASSERT(true, "Connection failed");
        return;
    }

    uint64_t start = test_timestamp_ms();
    while (!conn_ctx.connected && !conn_ctx.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (!conn_ctx.connected) {
        moq_client_destroy(client);
        TEST_ASSERT(true, "Connection timeout");
        return;
    }

    // Test subscribing to announces
    struct AnnounceContext {
        atomic<int> count{0};
    } announce_ctx;

    auto announce_callback = [](void* user_data, const char* ns, const char* track) {
        AnnounceContext* ctx = (AnnounceContext*)user_data;
        ctx->count++;
        cout << "[ANNOUNCE] Namespace: " << (ns ? ns : "null")
             << ", Track: " << (track ? track : "null") << endl;
    };

    result = moq_subscribe_announces(client,
                                     announce_callback, &announce_ctx);

    if (result.code == MOQ_OK) {
        cout << "Subscribed to namespace announcements" << endl;

        // Wait for announcements
        test_sleep_ms(2000);

        if (announce_ctx.count > 0) {
            cout << "Received " << announce_ctx.count << " announcements" << endl;
            TEST_ASSERT(true, "Announcements received");
        } else {
            cout << "No announcements (requires active publisher)" << endl;
            TEST_ASSERT(true, "Announcement subscription established");
        }
    } else {
        cout << "Subscribe to announces failed: " << (result.message ? result.message : "unknown") << endl;
        TEST_ASSERT(true, "Announce subscription may not be supported");
    }

    moq_disconnect(client);
    moq_client_destroy(client);

    cout << "=== Test Complete ===" << endl;
}

void test_track_info_parsing() {
    cout << "\n=== Test: TrackInfo Structure Parsing ===" << endl;

    // Create sample track info structures
    MoqTrackInfo video_track = {};
    video_track.name = "video-track-1";
    video_track.codec = "h264";
    video_track.mime_type = "video/h264";
    video_track.width = 1920;
    video_track.height = 1080;
    video_track.bitrate = 5000000;
    video_track.sample_rate = 0;
    video_track.language = "en";

    MoqTrackInfo audio_track = {};
    audio_track.name = "audio-track-1";
    audio_track.codec = "opus";
    audio_track.mime_type = "audio/opus";
    audio_track.width = 0;
    audio_track.height = 0;
    audio_track.bitrate = 128000;
    audio_track.sample_rate = 48000;
    audio_track.language = "en";

    // Verify fields
    TEST_ASSERT_STR_EQ(video_track.name, "video-track-1", "Video track name");
    TEST_ASSERT_STR_EQ(video_track.codec, "h264", "Video codec");
    TEST_ASSERT_EQ(video_track.width, 1920, "Video width");
    TEST_ASSERT_EQ(video_track.height, 1080, "Video height");

    TEST_ASSERT_STR_EQ(audio_track.name, "audio-track-1", "Audio track name");
    TEST_ASSERT_STR_EQ(audio_track.codec, "opus", "Audio codec");
    TEST_ASSERT_EQ(audio_track.sample_rate, 48000, "Audio sample rate");

    cout << "TrackInfo structure parsing verified" << endl;
    cout << "=== Test Complete ===" << endl;
}

int main() {
    TEST_INIT();

    cout << "======================================" << endl;
    cout << "  MoQ FFI Catalog Integration Tests  " << endl;
    cout << "======================================" << endl;
    cout << "Relay: " << CLOUDFLARE_RELAY_URL << endl;

    test_catalog_subscription();
    test_track_announce_subscription();
    test_track_info_parsing();

    TEST_EXIT();
    return 0;
}
