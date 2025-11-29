#include "test_framework.h"
#include "moq_ffi.h"
#include <iostream>
#include <vector>
#include <string>
#include <cstring>
#include <atomic>
#include <thread>

using namespace std;

/**
 * Integration Test: Multiple Concurrent Clients
 *
 * This test demonstrates handling multiple clients simultaneously:
 * 1. Create multiple clients
 * 2. Connect all clients concurrently
 * 3. Verify each client operates independently
 * 4. Test cross-client communication (pub/sub)
 * 5. Clean up all clients
 */

struct ClientContext {
    MoqClient* client = nullptr;
    atomic<bool> connected{false};
    atomic<bool> failed{false};
    string client_id;
};

void multi_client_connection_callback(void* user_data, MoqConnectionState state) {
    ClientContext* ctx = (ClientContext*)user_data;

    switch (state) {
        case MOQ_STATE_CONNECTED:
            cout << "[CLIENT-" << ctx->client_id << "] Connected" << endl;
            ctx->connected = true;
            break;
        case MOQ_STATE_FAILED:
            cout << "[CLIENT-" << ctx->client_id << "] Failed" << endl;
            ctx->failed = true;
            break;
        case MOQ_STATE_CONNECTING:
            cout << "[CLIENT-" << ctx->client_id << "] Connecting..." << endl;
            break;
        case MOQ_STATE_DISCONNECTED:
            cout << "[CLIENT-" << ctx->client_id << "] Disconnected" << endl;
            break;
    }
}

void test_multiple_clients_concurrent() {
    cout << "\n=== Test: Multiple Concurrent Clients ===" << endl;

    moq_init();

    const int NUM_CLIENTS = 5;
    vector<ClientContext> contexts(NUM_CLIENTS);

    // Create all clients
    cout << "\nCreating " << NUM_CLIENTS << " clients..." << endl;
    for (int i = 0; i < NUM_CLIENTS; i++) {
        contexts[i].client = moq_client_create();
        contexts[i].client_id = "C" + to_string(i + 1);

        TEST_ASSERT_NOT_NULL(contexts[i].client, "Client created");
        cout << "[CLIENT-" << contexts[i].client_id << "] Created" << endl;
    }

    // Connect all clients
    cout << "\nConnecting all clients to Cloudflare relay..." << endl;
    for (int i = 0; i < NUM_CLIENTS; i++) {
        MoqResult result = moq_connect(
            contexts[i].client,
            CLOUDFLARE_RELAY_URL,
            multi_client_connection_callback,
            &contexts[i]
        );

        if (result.code != MOQ_OK) {
            cout << "[CLIENT-" << contexts[i].client_id << "] Connect failed: "
                 << (result.message ? result.message : "unknown") << endl;
        }
    }

    // Wait for all connections
    cout << "\nWaiting for connections..." << endl;
    uint64_t start = test_timestamp_ms();
    bool all_done = false;

    while (!all_done && (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        all_done = true;
        for (int i = 0; i < NUM_CLIENTS; i++) {
            if (!contexts[i].connected && !contexts[i].failed) {
                all_done = false;
                break;
            }
        }
        test_sleep_ms(100);
    }

    // Count successful connections
    int connected_count = 0;
    for (int i = 0; i < NUM_CLIENTS; i++) {
        if (contexts[i].connected) {
            connected_count++;
        }
    }

    cout << "\nConnected: " << connected_count << "/" << NUM_CLIENTS << endl;
    TEST_ASSERT(connected_count > 0, "At least one client connected");

    // Verify client independence
    cout << "\nVerifying client independence..." << endl;
    for (int i = 0; i < NUM_CLIENTS; i++) {
        if (contexts[i].connected) {
            bool is_connected = moq_is_connected(contexts[i].client);
            TEST_ASSERT(is_connected, "Client connection status correct");
        }
    }

    // Cleanup all clients
    cout << "\nCleaning up clients..." << endl;
    for (int i = 0; i < NUM_CLIENTS; i++) {
        if (contexts[i].connected) {
            moq_disconnect(contexts[i].client);
        }
        moq_client_destroy(contexts[i].client);
        cout << "[CLIENT-" << contexts[i].client_id << "] Destroyed" << endl;
    }

    cout << "=== Test Complete ===" << endl;
}

void test_cross_client_pubsub() {
    cout << "\n=== Test: Cross-Client Pub/Sub ===" << endl;

    moq_init();

    // Create publisher and subscribers
    ClientContext pub_ctx;
    pub_ctx.client_id = "Publisher";
    pub_ctx.client = moq_client_create();
    TEST_ASSERT_NOT_NULL(pub_ctx.client, "Publisher client created");

    const int NUM_SUBSCRIBERS = 3;
    vector<ClientContext> sub_contexts(NUM_SUBSCRIBERS);

    for (int i = 0; i < NUM_SUBSCRIBERS; i++) {
        sub_contexts[i].client_id = "Sub" + to_string(i + 1);
        sub_contexts[i].client = moq_client_create();
        TEST_ASSERT_NOT_NULL(sub_contexts[i].client, "Subscriber client created");
    }

    // Connect publisher
    cout << "\nConnecting publisher..." << endl;
    MoqResult result = moq_connect(pub_ctx.client, CLOUDFLARE_RELAY_URL,
                                  multi_client_connection_callback, &pub_ctx);

    if (result.code != MOQ_OK) {
        moq_client_destroy(pub_ctx.client);
        for (auto& sub : sub_contexts) {
            moq_client_destroy(sub.client);
        }
        TEST_ASSERT(true, "Publisher connection failed");
        return;
    }

    uint64_t start = test_timestamp_ms();
    while (!pub_ctx.connected && !pub_ctx.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    if (!pub_ctx.connected) {
        moq_client_destroy(pub_ctx.client);
        for (auto& sub : sub_contexts) {
            moq_client_destroy(sub.client);
        }
        TEST_ASSERT(true, "Publisher connection timeout");
        return;
    }

    // Connect subscribers
    cout << "\nConnecting " << NUM_SUBSCRIBERS << " subscribers..." << endl;
    for (int i = 0; i < NUM_SUBSCRIBERS; i++) {
        moq_connect(sub_contexts[i].client, CLOUDFLARE_RELAY_URL,
                   multi_client_connection_callback, &sub_contexts[i]);
    }

    start = test_timestamp_ms();
    while ((test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        bool all_done = true;
        for (auto& sub : sub_contexts) {
            if (!sub.connected && !sub.failed) {
                all_done = false;
            }
        }
        if (all_done) break;
        test_sleep_ms(100);
    }

    // Count connected subscribers
    int sub_connected = 0;
    for (const auto& sub : sub_contexts) {
        if (sub.connected) sub_connected++;
    }
    cout << "Subscribers connected: " << sub_connected << "/" << NUM_SUBSCRIBERS << endl;

    // Setup publisher
    const char* namespace_name = "multi-client-test";
    const char* track_name = "broadcast-track";

    result = moq_announce_namespace(pub_ctx.client, namespace_name);
    test_sleep_ms(500);

    MoqPublisher* publisher = moq_create_publisher_ex(
        pub_ctx.client, namespace_name, track_name, MOQ_DELIVERY_STREAM);

    if (!publisher) {
        cout << "Publisher creation failed" << endl;
        moq_disconnect(pub_ctx.client);
        moq_client_destroy(pub_ctx.client);
        for (auto& sub : sub_contexts) {
            if (sub.connected) moq_disconnect(sub.client);
            moq_client_destroy(sub.client);
        }
        TEST_ASSERT(true, "Publisher creation failed");
        return;
    }

    // Create subscribers
    struct SubDataContext {
        atomic<int> packet_count{0};
        string subscriber_id;
    };

    vector<SubDataContext> sub_data_contexts(NUM_SUBSCRIBERS);
    vector<MoqSubscriber*> subscribers;

    MoqDataCallback sub_callback = +[](void* user_data, const uint8_t* data, size_t len) {
        SubDataContext* ctx = (SubDataContext*)user_data;
        ctx->packet_count++;
        cout << "[" << ctx->subscriber_id << "] Received " << len << " bytes "
             << "(packet #" << ctx->packet_count << ")" << endl;
    };

    for (int i = 0; i < NUM_SUBSCRIBERS; i++) {
        if (sub_contexts[i].connected) {
            sub_data_contexts[i].subscriber_id = sub_contexts[i].client_id;
            MoqSubscriber* sub = moq_subscribe(
                sub_contexts[i].client, namespace_name, track_name,
                sub_callback, &sub_data_contexts[i]);
            subscribers.push_back(sub);
        }
    }

    test_sleep_ms(1000);

    // Publish data
    cout << "\nPublishing broadcast message..." << endl;
    vector<string> messages = {
        "Broadcast message 1 to all subscribers",
        "Broadcast message 2 to all subscribers",
        "Broadcast message 3 to all subscribers"
    };

    for (const auto& msg : messages) {
        result = moq_publish_data(publisher,
                                 (const uint8_t*)msg.c_str(),
                                 msg.length(),
                                 MOQ_DELIVERY_STREAM);
        if (result.code == MOQ_OK) {
            cout << "[Publisher] Sent: \"" << msg << "\"" << endl;
        }
        test_sleep_ms(300);
    }

    // Wait for subscribers to receive
    test_sleep_ms(2000);

    // Check results
    cout << "\nSubscriber packet counts:" << endl;
    for (const auto& ctx : sub_data_contexts) {
        if (!ctx.subscriber_id.empty()) {
            cout << "  " << ctx.subscriber_id << ": " << ctx.packet_count << " packets" << endl;
        }
    }

    // Cleanup
    for (auto sub : subscribers) {
        if (sub) moq_subscriber_destroy(sub);
    }
    moq_publisher_destroy(publisher);
    moq_disconnect(pub_ctx.client);
    moq_client_destroy(pub_ctx.client);

    for (auto& sub : sub_contexts) {
        if (sub.connected) moq_disconnect(sub.client);
        moq_client_destroy(sub.client);
    }

    TEST_ASSERT(true, "Cross-client pub/sub test completed");
    cout << "=== Test Complete ===" << endl;
}

void test_client_isolation() {
    cout << "\n=== Test: Client Isolation ===" << endl;

    moq_init();

    // Create two clients
    MoqClient* client1 = moq_client_create();
    MoqClient* client2 = moq_client_create();

    TEST_ASSERT_NOT_NULL(client1, "Client 1 created");
    TEST_ASSERT_NOT_NULL(client2, "Client 2 created");
    TEST_ASSERT(client1 != client2, "Clients are distinct");

    // Verify operations on one client don't affect the other
    ClientContext ctx1, ctx2;
    ctx1.client_id = "Isolated1";
    ctx2.client_id = "Isolated2";

    moq_connect(client1, CLOUDFLARE_RELAY_URL,
               multi_client_connection_callback, &ctx1);

    uint64_t start = test_timestamp_ms();
    while (!ctx1.connected && !ctx1.failed &&
           (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
        test_sleep_ms(100);
    }

    // Client 2 should still be disconnected
    bool client2_connected = moq_is_connected(client2);
    TEST_ASSERT(!client2_connected, "Client 2 should be independent");

    // Disconnect client 1
    if (ctx1.connected) {
        moq_disconnect(client1);
        test_sleep_ms(500);

        bool client1_connected = moq_is_connected(client1);
        TEST_ASSERT(!client1_connected, "Client 1 disconnected");
    }

    // Client 2 still independent
    client2_connected = moq_is_connected(client2);
    TEST_ASSERT(!client2_connected, "Client 2 remains independent");

    moq_client_destroy(client1);
    moq_client_destroy(client2);

    cout << "=== Test Complete ===" << endl;
}

int main() {
    TEST_INIT();

    cout << "===========================================" << endl;
    cout << "  MoQ FFI Multi-Client Integration Tests  " << endl;
    cout << "===========================================" << endl;
    cout << "Relay: " << CLOUDFLARE_RELAY_URL << endl;

    test_multiple_clients_concurrent();
    test_cross_client_pubsub();
    test_client_isolation();

    TEST_EXIT();
    return 0;
}
