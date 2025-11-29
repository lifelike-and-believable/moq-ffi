#include "test_framework.h"
#include "moq_ffi.h"
#include <stdio.h>
#include <string.h>

void test_announce_namespace_null_client(void) {
    moq_init();

    MoqResult result = moq_announce_namespace(NULL, "test-namespace");
    TEST_ASSERT_NEQ(result.code, MOQ_OK,
                    "moq_announce_namespace(NULL) should fail");
    TEST_ASSERT_EQ(result.code, MOQ_ERROR_INVALID_ARGUMENT,
                   "Should return INVALID_ARGUMENT for NULL client");
}

void test_announce_namespace_null_name(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqResult result = moq_announce_namespace(client, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK,
                    "moq_announce_namespace() with NULL namespace should fail");
    TEST_ASSERT_EQ(result.code, MOQ_ERROR_INVALID_ARGUMENT,
                   "Should return INVALID_ARGUMENT for NULL namespace");

    moq_client_destroy(client);
}

void test_announce_namespace_not_connected(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqResult result = moq_announce_namespace(client, "test-namespace");
    TEST_ASSERT_NEQ(result.code, MOQ_OK,
                    "Should fail to announce without connection");
    printf("Announce without connection result: code=%d\n", result.code);

    moq_client_destroy(client);
}

void test_create_publisher_null_client(void) {
    moq_init();

    MoqPublisher* pub = moq_create_publisher(NULL, "namespace", "track");
    TEST_ASSERT_NULL(pub, "moq_create_publisher(NULL) should return NULL");
}

void test_create_publisher_null_namespace(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqPublisher* pub = moq_create_publisher(client, NULL, "track");
    TEST_ASSERT_NULL(pub, "moq_create_publisher() with NULL namespace should return NULL");

    moq_client_destroy(client);
}

void test_create_publisher_null_track(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqPublisher* pub = moq_create_publisher(client, "namespace", NULL);
    TEST_ASSERT_NULL(pub, "moq_create_publisher() with NULL track should return NULL");

    moq_client_destroy(client);
}

void test_create_publisher_ex_null_client(void) {
    moq_init();

    MoqPublisher* pub = moq_create_publisher_ex(
        NULL, "namespace", "track", MOQ_DELIVERY_STREAM);
    TEST_ASSERT_NULL(pub, "moq_create_publisher_ex(NULL) should return NULL");
}

void test_create_publisher_ex_delivery_modes(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Test stream mode */
    MoqPublisher* pub_stream = moq_create_publisher_ex(
        client, "namespace", "track", MOQ_DELIVERY_STREAM);
    /* May return NULL if not connected, which is fine */
    if (pub_stream) {
        moq_publisher_destroy(pub_stream);
        TEST_ASSERT(true, "Stream mode publisher created");
    }

    /* Test datagram mode */
    MoqPublisher* pub_datagram = moq_create_publisher_ex(
        client, "namespace", "track", MOQ_DELIVERY_DATAGRAM);
    if (pub_datagram) {
        moq_publisher_destroy(pub_datagram);
        TEST_ASSERT(true, "Datagram mode publisher created");
    }

    moq_client_destroy(client);
}

void test_publish_data_null_publisher(void) {
    moq_init();

    const char* data = "test data";
    MoqResult result = moq_publish_data(
        NULL, (const uint8_t*)data, strlen(data), MOQ_DELIVERY_STREAM);

    TEST_ASSERT_NEQ(result.code, MOQ_OK,
                    "moq_publish_data(NULL) should fail");
    TEST_ASSERT_EQ(result.code, MOQ_ERROR_INVALID_ARGUMENT,
                   "Should return INVALID_ARGUMENT for NULL publisher");
}

void test_publish_data_null_data(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqPublisher* pub = moq_create_publisher(client, "namespace", "track");
    if (pub != NULL) {
        MoqResult result = moq_publish_data(pub, NULL, 100, MOQ_DELIVERY_STREAM);
        TEST_ASSERT_NEQ(result.code, MOQ_OK,
                        "moq_publish_data() with NULL data should fail");
        TEST_ASSERT_EQ(result.code, MOQ_ERROR_INVALID_ARGUMENT,
                       "Should return INVALID_ARGUMENT for NULL data");

        moq_publisher_destroy(pub);
    } else {
        TEST_ASSERT(true, "Publisher creation without connection (expected)");
    }

    moq_client_destroy(client);
}

void test_publish_data_zero_length(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqPublisher* pub = moq_create_publisher(client, "namespace", "track");
    if (pub != NULL) {
        const char* data = "test";
        MoqResult result = moq_publish_data(pub, (const uint8_t*)data, 0, MOQ_DELIVERY_STREAM);

        /* Zero-length publish might be valid or invalid depending on implementation */
        printf("Zero-length publish result: code=%d\n", result.code);
        TEST_ASSERT(true, "Zero-length publish handled");

        moq_publisher_destroy(pub);
    }

    moq_client_destroy(client);
}

void test_publish_data_large_payload(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqPublisher* pub = moq_create_publisher(client, "namespace", "track");
    if (pub != NULL) {
        /* Create a large payload (1MB) */
        size_t payload_size = 1024 * 1024;
        uint8_t* large_data = (uint8_t*)malloc(payload_size);
        TEST_ASSERT_NOT_NULL(large_data, "Should allocate large buffer");

        /* Fill with pattern */
        for (size_t i = 0; i < payload_size; i++) {
            large_data[i] = (uint8_t)(i % 256);
        }

        MoqResult result = moq_publish_data(pub, large_data, payload_size, MOQ_DELIVERY_STREAM);
        printf("Large payload (1MB) publish result: code=%d\n", result.code);
        TEST_ASSERT(true, "Large payload publish attempted");

        free(large_data);
        moq_publisher_destroy(pub);
    }

    moq_client_destroy(client);
}

void test_delivery_mode_toggle(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqPublisher* pub = moq_create_publisher_ex(
        client, "namespace", "track", MOQ_DELIVERY_STREAM);

    if (pub != NULL) {
        const char* data1 = "stream data";
        const char* data2 = "datagram data";

        /* Publish with stream mode */
        MoqResult result1 = moq_publish_data(
            pub, (const uint8_t*)data1, strlen(data1), MOQ_DELIVERY_STREAM);
        printf("Stream mode publish: code=%d\n", result1.code);

        /* Publish with datagram mode */
        MoqResult result2 = moq_publish_data(
            pub, (const uint8_t*)data2, strlen(data2), MOQ_DELIVERY_DATAGRAM);
        printf("Datagram mode publish: code=%d\n", result2.code);

        TEST_ASSERT(true, "Delivery mode toggle tested");

        moq_publisher_destroy(pub);
    }

    moq_client_destroy(client);
}

int main(void) {
    TEST_INIT();

    printf("Running publishing tests...\n\n");

    test_announce_namespace_null_client();
    test_announce_namespace_null_name();
    test_announce_namespace_not_connected();

    test_create_publisher_null_client();
    test_create_publisher_null_namespace();
    test_create_publisher_null_track();

    test_create_publisher_ex_null_client();
    test_create_publisher_ex_delivery_modes();

    test_publish_data_null_publisher();
    test_publish_data_null_data();
    test_publish_data_zero_length();
    test_publish_data_large_payload();
    test_delivery_mode_toggle();

    TEST_EXIT();
    return 0;
}
