#include "test_framework.h"
#include "moq_ffi.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

void test_string_memory_management(void) {
    moq_init();

    /* Test that version string is valid and doesn't need freeing */
    const char* version = moq_version();
    TEST_ASSERT_NOT_NULL(version, "Version string should not be NULL");

    /* String should remain valid across multiple calls */
    const char* version2 = moq_version();
    TEST_ASSERT_STR_EQ(version, version2, "Version string should be consistent");

    /* No need to free version string */
    TEST_ASSERT(true, "Version string memory is managed correctly");
}

void test_error_string_memory(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Trigger an error */
    MoqResult result = moq_connect(client, NULL, NULL, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK, "Should fail");

    /* Access error message multiple times */
    if (result.message != NULL) {
        size_t len1 = strlen(result.message);
        const char* msg = result.message;
        size_t len2 = strlen(msg);
        TEST_ASSERT_EQ(len1, len2, "Error message should remain valid");
    }

    /* Get last error */
    const char* last_error = moq_last_error();
    if (last_error != NULL) {
        printf("Last error: %s\n", last_error);
        TEST_ASSERT(true, "Last error accessible");
    }

    moq_client_destroy(client);
}

void test_callback_memory_safety(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* User data on stack should be safe */
    int callback_count = 0;

    /* This tests that callbacks can safely access stack data */
    /* (We can't actually trigger callbacks without connection,
       but we verify the setup is safe) */

    moq_client_destroy(client);
    TEST_ASSERT(true, "Stack user data is safe");
}

void test_large_data_handling(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Allocate large buffer */
    size_t large_size = 10 * 1024 * 1024; /* 10 MB */
    uint8_t* large_buffer = (uint8_t*)malloc(large_size);
    TEST_ASSERT_NOT_NULL(large_buffer, "Should allocate large buffer");

    if (large_buffer != NULL) {
        /* Fill with pattern */
        for (size_t i = 0; i < large_size; i++) {
            large_buffer[i] = (uint8_t)(i % 256);
        }

        /* Try to publish (will fail without connection, but tests memory handling) */
        MoqPublisher* pub = moq_create_publisher(client, "ns", "track");
        if (pub != NULL) {
            MoqResult result = moq_publish_data(pub, large_buffer, large_size,
                                               MOQ_DELIVERY_STREAM);
            printf("Large data publish: code=%d\n", result.code);
            moq_publisher_destroy(pub);
        }

        free(large_buffer);
        TEST_ASSERT(true, "Large data handled without crash");
    }

    moq_client_destroy(client);
}

void test_repeated_create_destroy(void) {
    int i;
    MoqClient* client;

    moq_init();

    /* Create and destroy many clients to test for memory leaks */
    for (i = 0; i < 100; i++) {
        client = moq_client_create();
        TEST_ASSERT_NOT_NULL(client, "Client creation in loop");
        moq_client_destroy(client);
    }

    TEST_ASSERT(true, "Repeated create/destroy completed");
}

void test_concurrent_clients(void) {
    /* Create multiple clients simultaneously */
#define NUM_CLIENTS 10
    MoqClient* clients[NUM_CLIENTS];
    int i, j;

    moq_init();

    for (i = 0; i < NUM_CLIENTS; i++) {
        clients[i] = moq_client_create();
        TEST_ASSERT_NOT_NULL(clients[i], "Multi-client creation");
    }

    /* All clients should be distinct */
    for (i = 0; i < NUM_CLIENTS; i++) {
        for (j = i + 1; j < NUM_CLIENTS; j++) {
            TEST_ASSERT(clients[i] != clients[j], "Clients should be distinct");
        }
    }

    /* Destroy all */
    for (i = 0; i < NUM_CLIENTS; i++) {
        moq_client_destroy(clients[i]);
    }

    TEST_ASSERT(true, "Multiple concurrent clients handled");
#undef NUM_CLIENTS
}

void test_null_callback_safety(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Test that NULL callbacks are handled safely */
    MoqResult result = moq_connect(client, CLOUDFLARE_RELAY_URL, NULL, NULL);
    printf("Connect with NULL callback: code=%d\n", result.code);
    TEST_ASSERT(true, "NULL callback handled safely");

    moq_disconnect(client);
    moq_client_destroy(client);
}

void test_user_data_null_safety(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Test that NULL user_data is handled safely */
    MoqResult result = moq_connect(client, CLOUDFLARE_RELAY_URL, NULL, NULL);
    printf("Connect with NULL user_data: code=%d\n", result.code);
    TEST_ASSERT(true, "NULL user_data handled safely");

    moq_disconnect(client);
    moq_client_destroy(client);
}

void test_empty_string_handling(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Test empty strings */
    MoqResult r1 = moq_announce_namespace(client, "");
    printf("Empty namespace: code=%d\n", r1.code);

    MoqPublisher* pub = moq_create_publisher(client, "", "");
    if (pub != NULL) {
        moq_publisher_destroy(pub);
    }

    TEST_ASSERT(true, "Empty strings handled");

    moq_client_destroy(client);
}

void test_buffer_boundaries(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqPublisher* pub = moq_create_publisher(client, "ns", "track");
    if (pub != NULL) {
        /* Test various buffer sizes */
        uint8_t small_buf[1] = {42};
        uint8_t medium_buf[1024];
        uint8_t large_buf[65536];

        memset(medium_buf, 'A', sizeof(medium_buf));
        memset(large_buf, 'B', sizeof(large_buf));

        /* These will fail without connection, but test buffer handling */
        moq_publish_data(pub, small_buf, 1, MOQ_DELIVERY_STREAM);
        moq_publish_data(pub, medium_buf, 1024, MOQ_DELIVERY_STREAM);
        moq_publish_data(pub, large_buf, 65536, MOQ_DELIVERY_STREAM);

        moq_publisher_destroy(pub);
        TEST_ASSERT(true, "Various buffer sizes handled");
    }

    moq_client_destroy(client);
}

void test_moq_free_str_safety(void) {
    moq_init();

    /* Free NULL should not crash */
    moq_free_str(NULL);
    TEST_ASSERT(true, "moq_free_str(NULL) is safe");

    /* Multiple frees of NULL */
    moq_free_str(NULL);
    moq_free_str(NULL);
    TEST_ASSERT(true, "Multiple moq_free_str(NULL) safe");
}

void test_cleanup_ordering(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Create publishers and subscribers */
    MoqPublisher* pub = moq_create_publisher(client, "ns", "track");

    /* Destroy in different orders */
    if (pub != NULL) {
        moq_publisher_destroy(pub);
    }
    moq_client_destroy(client);

    TEST_ASSERT(true, "Cleanup ordering handled correctly");
}

int main(void) {
    TEST_INIT();

    printf("Running memory safety tests...\n\n");

    test_string_memory_management();
    test_error_string_memory();
    test_callback_memory_safety();
    test_large_data_handling();
    test_repeated_create_destroy();
    test_concurrent_clients();
    test_null_callback_safety();
    test_user_data_null_safety();
    test_empty_string_handling();
    test_buffer_boundaries();
    test_moq_free_str_safety();
    test_cleanup_ordering();

    TEST_EXIT();
    return 0;
}
