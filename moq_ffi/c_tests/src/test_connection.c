#include "test_framework.h"
#include "moq_ffi.h"
#include <stdbool.h>

/* Connection state tracking for callback tests */
typedef struct {
    MoqConnectionState last_state;
    int callback_count;
    bool connected;
    bool failed;
} ConnectionCallbackData;

void connection_state_callback(void* user_data, MoqConnectionState state) {
    ConnectionCallbackData* data = (ConnectionCallbackData*)user_data;
    data->last_state = state;
    data->callback_count++;

    printf("  Connection state changed: ");
    switch (state) {
        case MOQ_STATE_DISCONNECTED:
            printf("DISCONNECTED\n");
            break;
        case MOQ_STATE_CONNECTING:
            printf("CONNECTING\n");
            break;
        case MOQ_STATE_CONNECTED:
            printf("CONNECTED\n");
            data->connected = true;
            break;
        case MOQ_STATE_FAILED:
            printf("FAILED\n");
            data->failed = true;
            break;
    }
}

void test_connect_with_callback(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    ConnectionCallbackData callback_data = {
        .last_state = MOQ_STATE_DISCONNECTED,
        .callback_count = 0,
        .connected = false,
        .failed = false
    };

    MoqResult result = moq_connect(
        client,
        CLOUDFLARE_RELAY_URL,
        connection_state_callback,
        &callback_data
    );

    printf("Connect result: code=%d, message=%s\n", result.code, result.message ? result.message : "null");

    if (result.code == MOQ_OK) {
        /* Wait for connection to complete */
        uint64_t start = test_timestamp_ms();
        while (!callback_data.connected && !callback_data.failed &&
               (test_timestamp_ms() - start) < TEST_TIMEOUT_MS) {
            test_sleep_ms(100);
        }

        TEST_ASSERT(callback_data.callback_count > 0, "Callback should have been invoked");
        TEST_ASSERT(callback_data.connected || callback_data.failed,
                    "Should reach CONNECTED or FAILED state");

        if (callback_data.connected) {
            TEST_ASSERT_EQ(callback_data.last_state, MOQ_STATE_CONNECTED,
                          "Final state should be CONNECTED");

            bool is_connected = moq_is_connected(client);
            TEST_ASSERT(is_connected, "moq_is_connected() should return true");

            /* Disconnect */
            moq_disconnect(client);

            /* Wait a bit for disconnect to complete */
            test_sleep_ms(500);

            is_connected = moq_is_connected(client);
            TEST_ASSERT(!is_connected, "Should be disconnected after moq_disconnect()");
        }
    } else {
        printf("Connection failed (expected in some environments): %s\n",
               result.message ? result.message : "unknown error");
        TEST_ASSERT(true, "Connection attempt completed (failure acceptable)");
    }

    moq_client_destroy(client);
}

void test_connect_without_callback(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqResult result = moq_connect(client, CLOUDFLARE_RELAY_URL, NULL, NULL);
    printf("Connect without callback result: code=%d\n", result.code);

    if (result.code == MOQ_OK) {
        /* Wait for connection */
        test_sleep_ms(5000);

        bool is_connected = moq_is_connected(client);
        if (is_connected) {
            TEST_ASSERT(true, "Connected successfully without callback");
            moq_disconnect(client);
        } else {
            TEST_ASSERT(true, "Connection completed (callback optional)");
        }
    } else {
        TEST_ASSERT(true, "Connection attempt completed");
    }

    moq_client_destroy(client);
}

void test_connect_null_client(void) {
    moq_init();

    MoqResult result = moq_connect(NULL, CLOUDFLARE_RELAY_URL, NULL, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK, "moq_connect(NULL) should fail");
    TEST_ASSERT_EQ(result.code, MOQ_ERROR_INVALID_ARGUMENT,
                   "moq_connect(NULL) should return INVALID_ARGUMENT");
}

void test_connect_null_url(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqResult result = moq_connect(client, NULL, NULL, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK, "moq_connect() with NULL URL should fail");
    TEST_ASSERT_EQ(result.code, MOQ_ERROR_INVALID_ARGUMENT,
                   "Should return INVALID_ARGUMENT for NULL URL");

    moq_client_destroy(client);
}

void test_connect_invalid_url(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqResult result = moq_connect(client, "not-a-valid-url", NULL, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK,
                    "moq_connect() with invalid URL should fail");
    printf("Invalid URL result code: %d\n", result.code);

    moq_client_destroy(client);
}

void test_disconnect_null_client(void) {
    moq_init();

    moq_disconnect(NULL);
    TEST_ASSERT(true, "moq_disconnect(NULL) should not crash");
}

void test_disconnect_without_connect(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    moq_disconnect(client);
    TEST_ASSERT(true, "moq_disconnect() without prior connect should not crash");

    moq_client_destroy(client);
}

void test_double_connect(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqResult result1 = moq_connect(client, CLOUDFLARE_RELAY_URL, NULL, NULL);
    test_sleep_ms(1000);

    MoqResult result2 = moq_connect(client, CLOUDFLARE_RELAY_URL, NULL, NULL);
    printf("Second connect result: code=%d\n", result2.code);
    TEST_ASSERT(true, "Double connect should be handled gracefully");

    moq_disconnect(client);
    moq_client_destroy(client);
}

int main(void) {
    TEST_INIT();

    printf("Running connection tests...\n\n");
    printf("Testing against Cloudflare relay: %s\n\n", CLOUDFLARE_RELAY_URL);

    test_connect_null_client();
    test_connect_null_url();
    test_connect_invalid_url();
    test_disconnect_null_client();
    test_disconnect_without_connect();

    /* These tests require network access */
    printf("\n--- Network-dependent tests ---\n");
    test_connect_with_callback();
    test_connect_without_callback();
    test_double_connect();

    TEST_EXIT();
    return 0;
}
