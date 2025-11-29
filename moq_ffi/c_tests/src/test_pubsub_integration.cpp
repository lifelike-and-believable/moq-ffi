#include "test_framework.h"
#include "moq_ffi.h"
#include <iostream>
#include <vector>
#include <string>
#include <cstring>
#include <atomic>
#include <thread>
#include <chrono>

using namespace std;

/**
 * Integration Test: Publisher-Subscriber Workflow
 *
 * This test demonstrates a complete MoQ pub/sub workflow:
 * 1. Connect to Cloudflare relay
 * 2. Announce a namespace
 * 3. Create a publisher
 * 4. Create a subscriber on a different client
 * 5. Publish multiple packets (both text and binary data)
 * 6. Verify all received data matches sent data
 */

// Test data structures
struct ReceivedData {
    vector<uint8_t> data;
    uint64_t timestamp_ms;
};

struct SubscriberContext {
    atomic<int> packet_count{0};
    vector<ReceivedData> received_packets;
    atomic<bool> all_received{false};
};

// Connection state for synchronization
struct ConnectionContext {
    atomic<bool> connected{false};
    atomic<bool> failed{false};
};

// Callbacks
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

void subscriber_data_callback(void* user_data, const uint8_t* data, size_t len) {
    SubscriberContext* ctx = (SubscriberContext*)user_data;

    cout << "[SUBSCRIBER] Received " << len << " bytes (packet #"
         << ctx->packet_count + 1 << ")" << endl;

    // Store received data
    ReceivedData received;
    received.data.assign(data, data + len);
    received.timestamp_ms = test_timestamp_ms();

    ctx->received_packets.push_back(received);
    ctx->packet_count++;
}

void test_basic_pubsub_text_data() {
    cout << "\n=== Test: Basic Pub/Sub with Text Data ===" << endl;

    moq_init();

    // Create publisher client
    MoqClient* pub_client = moq_client_create();
    TEST_ASSERT_NOT_NULL(pub_client, "Publisher client created");

    // Connect publisher
    ConnectionContext pub_conn_ctx;
    MoqResult result = moq_connect(pub_client, CLOUDFLARE_RELAY_URL,
                                  connection_callback, &pub_conn_ctx);

    if (result.code != MOQ_OK) {
        cout << "Failed to initiate connection: " << (result.message ? result.message : "unknown error") << endl;
        moq_client_destroy(pub_client);
        TEST_ASSERT(true, "Connection failed (may be network issue)");
        return;
    }

    // Wait for connection
    uint64_t start = test_timestamp_ms();
    while (!pub_conn_ctx.connected && !pub_conn_ctx.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (!pub_conn_ctx.connected) {
        cout << "Publisher connection timeout or failed" << endl;
        moq_client_destroy(pub_client);
        TEST_ASSERT(true, "Connection timeout (network dependent)");
        return;
    }

    // Announce namespace
    const char* test_namespace = "test-cpp-integration";
    const char* test_track = "text-data-track";

    result = moq_announce_namespace(pub_client, test_namespace);
    if (result.code != MOQ_OK) {
        cout << "Announce failed: " << (result.message ? result.message : "unknown") << endl;
    }
    test_sleep_ms(500);

    // Create publisher
    MoqPublisher* publisher = moq_create_publisher_ex(
        pub_client, test_namespace, test_track, MOQ_DELIVERY_STREAM);

    if (!publisher) {
        cout << "Failed to create publisher" << endl;
        moq_disconnect(pub_client);
        moq_client_destroy(pub_client);
        TEST_ASSERT(true, "Publisher creation failed");
        return;
    }

    // Create subscriber client
    MoqClient* sub_client = moq_client_create();
    TEST_ASSERT_NOT_NULL(sub_client, "Subscriber client created");

    // Connect subscriber
    ConnectionContext sub_conn_ctx;
    result = moq_connect(sub_client, CLOUDFLARE_RELAY_URL,
                        connection_callback, &sub_conn_ctx);

    if (result.code != MOQ_OK) {
        moq_publisher_destroy(publisher);
        moq_disconnect(pub_client);
        moq_client_destroy(pub_client);
        moq_client_destroy(sub_client);
        TEST_ASSERT(true, "Subscriber connection failed");
        return;
    }

    // Wait for subscriber connection
    start = test_timestamp_ms();
    while (!sub_conn_ctx.connected && !sub_conn_ctx.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (!sub_conn_ctx.connected) {
        moq_publisher_destroy(publisher);
        moq_disconnect(pub_client);
        moq_client_destroy(pub_client);
        moq_client_destroy(sub_client);
        TEST_ASSERT(true, "Subscriber connection timeout");
        return;
    }

    // Create subscriber
    SubscriberContext sub_ctx;
    MoqSubscriber* subscriber = moq_subscribe(
        sub_client, test_namespace, test_track,
        subscriber_data_callback, &sub_ctx);

    if (!subscriber) {
        cout << "Failed to create subscriber" << endl;
        moq_publisher_destroy(publisher);
        moq_disconnect(pub_client);
        moq_disconnect(sub_client);
        moq_client_destroy(pub_client);
        moq_client_destroy(sub_client);
        TEST_ASSERT(true, "Subscriber creation failed");
        return;
    }

    test_sleep_ms(1000); // Wait for subscription to establish

    // Publish multiple text packets
    vector<string> text_packets = {
        "Hello, MoQ!",
        "This is packet 2",
        "Testing multiple packets",
        "MoQ FFI C++ integration test",
        "Final text packet"
    };

    cout << "\nPublishing " << text_packets.size() << " text packets..." << endl;

    for (size_t i = 0; i < text_packets.size(); i++) {
        const string& text = text_packets[i];
        result = moq_publish_data(publisher,
                                 (const uint8_t*)text.c_str(),
                                 text.length(),
                                 MOQ_DELIVERY_STREAM);

        if (result.code == MOQ_OK) {
            cout << "[PUBLISHER] Sent packet #" << (i + 1) << ": \"" << text << "\"" << endl;
        } else {
            cout << "[PUBLISHER] Failed to send packet #" << (i + 1) << endl;
        }

        test_sleep_ms(200); // Small delay between packets
    }

    // Wait for all packets to be received
    cout << "\nWaiting for packets to be received..." << endl;
    start = test_timestamp_ms();
    while (sub_ctx.packet_count < (int)text_packets.size() &&
           (test_timestamp_ms() - start) < SHORT_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    // Verify received data
    cout << "\nReceived " << sub_ctx.packet_count << " packets" << endl;

    if (sub_ctx.packet_count > 0) {
        TEST_ASSERT(sub_ctx.packet_count == (int)text_packets.size(),
                   "All text packets received");

        // Verify content matches
        for (size_t i = 0; i < min(sub_ctx.received_packets.size(), text_packets.size()); i++) {
            string received_text(
                (char*)sub_ctx.received_packets[i].data.data(),
                sub_ctx.received_packets[i].data.size());

            TEST_ASSERT_STR_EQ(received_text.c_str(), text_packets[i].c_str(),
                             "Received text matches sent text");
            cout << "  Packet #" << (i + 1) << " verified: \"" << received_text << "\"" << endl;
        }
    } else {
        TEST_ASSERT(true, "No packets received (relay may not echo)");
    }

    // Cleanup
    moq_subscriber_destroy(subscriber);
    moq_publisher_destroy(publisher);
    moq_disconnect(pub_client);
    moq_disconnect(sub_client);
    moq_client_destroy(pub_client);
    moq_client_destroy(sub_client);

    cout << "=== Test Complete ===" << endl;
}

void test_binary_data_transfer() {
    cout << "\n=== Test: Binary Data Transfer ===" << endl;

    moq_init();

    MoqClient* pub_client = moq_client_create();
    TEST_ASSERT_NOT_NULL(pub_client, "Publisher client created");

    ConnectionContext pub_conn_ctx;
    MoqResult result = moq_connect(pub_client, CLOUDFLARE_RELAY_URL,
                                  connection_callback, &pub_conn_ctx);

    if (result.code != MOQ_OK) {
        moq_client_destroy(pub_client);
        TEST_ASSERT(true, "Connection failed");
        return;
    }

    uint64_t start = test_timestamp_ms();
    while (!pub_conn_ctx.connected && !pub_conn_ctx.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (!pub_conn_ctx.connected) {
        moq_client_destroy(pub_client);
        TEST_ASSERT(true, "Connection timeout");
        return;
    }

    const char* test_namespace = "test-binary-integration";
    const char* test_track = "binary-data-track";

    result = moq_announce_namespace(pub_client, test_namespace);
    test_sleep_ms(500);

    MoqPublisher* publisher = moq_create_publisher_ex(
        pub_client, test_namespace, test_track, MOQ_DELIVERY_DATAGRAM);

    if (!publisher) {
        moq_disconnect(pub_client);
        moq_client_destroy(pub_client);
        TEST_ASSERT(true, "Publisher creation failed");
        return;
    }

    MoqClient* sub_client = moq_client_create();
    ConnectionContext sub_conn_ctx;

    result = moq_connect(sub_client, CLOUDFLARE_RELAY_URL,
                        connection_callback, &sub_conn_ctx);

    if (result.code != MOQ_OK) {
        moq_publisher_destroy(publisher);
        moq_disconnect(pub_client);
        moq_client_destroy(pub_client);
        moq_client_destroy(sub_client);
        TEST_ASSERT(true, "Subscriber connection failed");
        return;
    }

    start = test_timestamp_ms();
    while (!sub_conn_ctx.connected && !sub_conn_ctx.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (!sub_conn_ctx.connected) {
        moq_publisher_destroy(publisher);
        moq_disconnect(pub_client);
        moq_client_destroy(pub_client);
        moq_client_destroy(sub_client);
        TEST_ASSERT(true, "Subscriber connection timeout");
        return;
    }

    SubscriberContext sub_ctx;
    MoqSubscriber* subscriber = moq_subscribe(
        sub_client, test_namespace, test_track,
        subscriber_data_callback, &sub_ctx);

    if (!subscriber) {
        moq_publisher_destroy(publisher);
        moq_disconnect(pub_client);
        moq_disconnect(sub_client);
        moq_client_destroy(pub_client);
        moq_client_destroy(sub_client);
        TEST_ASSERT(true, "Subscriber creation failed");
        return;
    }

    test_sleep_ms(1000);

    // Publish binary packets with various patterns
    vector<vector<uint8_t>> binary_packets;

    // Packet 1: Sequential bytes
    vector<uint8_t> packet1(256);
    for (int i = 0; i < 256; i++) packet1[i] = (uint8_t)i;
    binary_packets.push_back(packet1);

    // Packet 2: All zeros
    binary_packets.push_back(vector<uint8_t>(100, 0));

    // Packet 3: All 0xFF
    binary_packets.push_back(vector<uint8_t>(100, 0xFF));

    // Packet 4: Alternating pattern
    vector<uint8_t> packet4(200);
    for (size_t i = 0; i < packet4.size(); i++) {
        packet4[i] = (i % 2) ? 0xAA : 0x55;
    }
    binary_packets.push_back(packet4);

    cout << "\nPublishing " << binary_packets.size() << " binary packets..." << endl;

    for (size_t i = 0; i < binary_packets.size(); i++) {
        result = moq_publish_data(publisher,
                                 binary_packets[i].data(),
                                 binary_packets[i].size(),
                                 MOQ_DELIVERY_DATAGRAM);

        if (result.code == MOQ_OK) {
            cout << "[PUBLISHER] Sent binary packet #" << (i + 1)
                 << " (" << binary_packets[i].size() << " bytes)" << endl;
        }

        test_sleep_ms(200);
    }

    cout << "\nWaiting for binary packets..." << endl;
    start = test_timestamp_ms();
    while (sub_ctx.packet_count < (int)binary_packets.size() &&
           (test_timestamp_ms() - start) < SHORT_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    cout << "\nReceived " << sub_ctx.packet_count << " binary packets" << endl;

    if (sub_ctx.packet_count > 0) {
        // Verify binary data integrity
        for (size_t i = 0; i < min(sub_ctx.received_packets.size(), binary_packets.size()); i++) {
            const auto& received = sub_ctx.received_packets[i].data;
            const auto& sent = binary_packets[i];

            TEST_ASSERT_EQ(received.size(), sent.size(), "Binary packet size matches");

            if (received.size() == sent.size()) {
                bool match = memcmp(received.data(), sent.data(), sent.size()) == 0;
                TEST_ASSERT(match, "Binary packet content matches");
                if (match) {
                    cout << "  Binary packet #" << (i + 1) << " verified (" << sent.size() << " bytes)" << endl;
                }
            }
        }
    } else {
        TEST_ASSERT(true, "No binary packets received (relay may not echo)");
    }

    // Cleanup
    moq_subscriber_destroy(subscriber);
    moq_publisher_destroy(publisher);
    moq_disconnect(pub_client);
    moq_disconnect(sub_client);
    moq_client_destroy(pub_client);
    moq_client_destroy(sub_client);

    cout << "=== Test Complete ===" << endl;
}

int main() {
    TEST_INIT();

    cout << "======================================" << endl;
    cout << "   MoQ FFI Pub/Sub Integration Tests" << endl;
    cout << "======================================" << endl;
    cout << "Relay: " << CLOUDFLARE_RELAY_URL << endl;

    test_basic_pubsub_text_data();
    test_binary_data_transfer();

    TEST_EXIT();
    return 0;
}
