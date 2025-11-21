/**
 * MoQ FFI Example - Basic Client Usage
 * 
 * This example demonstrates how to use the moq_ffi library to:
 * - Create a MoQ client
 * - Connect to a relay server
 * - Announce namespaces
 * - Create publishers and publish data
 * - Create subscribers and receive data
 * 
 * Compile with:
 *   gcc -o test_client test_client.c \
 *       -I../moq_ffi/include \
 *       -L../moq_ffi/target/release \
 *       -lmoq_ffi -lpthread -ldl -lm
 * 
 * Run with:
 *   LD_LIBRARY_PATH=../moq_ffi/target/release ./test_client
 */

#include "moq_ffi.h"
#include <stdio.h>
#include <string.h>

void connection_callback(void* user_data, MoqConnectionState state) {
    const char* state_str = "UNKNOWN";
    switch (state) {
        case MOQ_STATE_DISCONNECTED: state_str = "DISCONNECTED"; break;
        case MOQ_STATE_CONNECTING: state_str = "CONNECTING"; break;
        case MOQ_STATE_CONNECTED: state_str = "CONNECTED"; break;
        case MOQ_STATE_FAILED: state_str = "FAILED"; break;
    }
    printf("Connection state changed: %s\n", state_str);
}

void data_callback(void* user_data, const uint8_t* data, size_t data_len) {
    printf("Received %zu bytes of data\n", data_len);
    // In a real application, you would process the data here
}

int main(int argc, char* argv[]) {
    printf("MoQ FFI Example Client\n");
    printf("======================\n");
    printf("Version: %s\n\n", moq_version());

    // Get server URL from command line or use default
    const char* server_url = (argc > 1) ? argv[1] : "https://relay.example.com:443";

    // Create client
    printf("Creating MoQ client...\n");
    MoqClient* client = moq_client_create();
    if (!client) {
        fprintf(stderr, "Failed to create client\n");
        return 1;
    }
    printf("✓ Client created\n\n");

    // Connect to server
    printf("Connecting to %s...\n", server_url);
    MoqResult result = moq_connect(
        client,
        server_url,
        connection_callback,
        NULL
    );

    if (result.code != MOQ_OK) {
        fprintf(stderr, "✗ Connection failed: %s\n", result.message);
        if (result.message) {
            moq_free_str(result.message);
        }
        moq_client_destroy(client);
        return 1;
    }

    printf("✓ Connected successfully\n\n");

    // Announce a namespace for publishing
    const char* namespace = "my-app";
    printf("Announcing namespace '%s'...\n", namespace);
    result = moq_announce_namespace(client, namespace);
    if (result.code != MOQ_OK) {
        fprintf(stderr, "✗ Failed to announce namespace: %s\n", result.message);
        if (result.message) {
            moq_free_str(result.message);
        }
    } else {
        printf("✓ Namespace announced\n\n");
    }

    // Create a publisher
    const char* track_name = "test-track";
    printf("Creating publisher for '%s/%s'...\n", namespace, track_name);
    MoqPublisher* pub = moq_create_publisher(client, namespace, track_name);
    if (!pub) {
        fprintf(stderr, "✗ Failed to create publisher\n");
    } else {
        printf("✓ Publisher created\n\n");

        // Publish some test data
        printf("Publishing test data...\n");
        const char* message = "Hello, MoQ!";
        result = moq_publish_data(
            pub,
            (const uint8_t*)message,
            strlen(message),
            MOQ_DELIVERY_STREAM  // Use reliable delivery
        );

        if (result.code == MOQ_OK) {
            printf("✓ Published %zu bytes\n\n", strlen(message));
        } else {
            fprintf(stderr, "✗ Failed to publish: %s\n\n", result.message);
            if (result.message) {
                moq_free_str(result.message);
            }
        }

        // Publish more data with datagram delivery (lossy)
        printf("Publishing data via datagram (lossy)...\n");
        uint8_t binary_data[256];
        memset(binary_data, 0x42, sizeof(binary_data));
        result = moq_publish_data(
            pub,
            binary_data,
            sizeof(binary_data),
            MOQ_DELIVERY_DATAGRAM  // Use lossy delivery for high-frequency updates
        );

        if (result.code == MOQ_OK) {
            printf("✓ Published %zu bytes via datagram\n\n", sizeof(binary_data));
        }

        // Clean up publisher
        moq_publisher_destroy(pub);
        printf("✓ Publisher destroyed\n\n");
    }

    // Create a subscriber
    const char* remote_namespace = "remote-app";
    const char* remote_track = "remote-track";
    printf("Subscribing to '%s/%s'...\n", remote_namespace, remote_track);
    MoqSubscriber* sub = moq_subscribe(
        client,
        remote_namespace,
        remote_track,
        data_callback,
        NULL  // user_data pointer (can be used to pass context to callback)
    );

    if (!sub) {
        fprintf(stderr, "✗ Failed to subscribe\n");
    } else {
        printf("✓ Subscribed successfully\n");
        printf("  (Data will be received via callback)\n\n");

        // In a real application, you would keep the program running
        // to receive data. For this example, we'll just clean up.
        
        moq_subscriber_destroy(sub);
        printf("✓ Subscriber destroyed\n\n");
    }

    // Disconnect
    printf("Disconnecting...\n");
    result = moq_disconnect(client);
    if (result.code == MOQ_OK) {
        printf("✓ Disconnected\n");
    }

    // Clean up client
    moq_client_destroy(client);
    printf("✓ Client destroyed\n\n");

    printf("Example completed successfully!\n");
    printf("\nFor real-world usage:\n");
    printf("1. Connect to an actual MoQ relay server\n");
    printf("2. Keep the program running to receive data\n");
    printf("3. Handle callbacks in appropriate threads for your application\n");
    printf("4. Use proper error handling and reconnection logic\n");

    return 0;
}
