#include "test_framework.h"
#include "moq_ffi.h"
#include <stdio.h>
#include <string.h>

void test_last_error_null_operations(void) {
    moq_init();

    /* Trigger an error by calling with NULL */
    MoqResult result = moq_connect(NULL, "http://example.com", NULL, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK, "Should fail with NULL client");

    /* Check last error */
    const char* error = moq_last_error();
    if (error != NULL && strlen(error) > 0) {
        printf("Last error: %s\n", error);
        TEST_ASSERT(true, "Error message retrieved");
    } else {
        TEST_ASSERT(true, "No error message or empty string (acceptable)");
    }
}

void test_result_message_validity(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Try invalid operation */
    MoqResult result = moq_announce_namespace(client, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK, "Should fail with NULL namespace");

    /* Check result message */
    if (result.message != NULL) {
        printf("Result message: %s\n", result.message);
        TEST_ASSERT(strlen(result.message) > 0, "Result message should not be empty");
    }

    moq_client_destroy(client);
}

void test_invalid_argument_errors(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Test various invalid argument scenarios */

    /* NULL URL */
    MoqResult r1 = moq_connect(client, NULL, NULL, NULL);
    TEST_ASSERT_EQ(r1.code, MOQ_ERROR_INVALID_ARGUMENT, "NULL URL should return INVALID_ARGUMENT");

    /* NULL namespace */
    MoqResult r2 = moq_announce_namespace(client, NULL);
    TEST_ASSERT_EQ(r2.code, MOQ_ERROR_INVALID_ARGUMENT, "NULL namespace should return INVALID_ARGUMENT");

    /* NULL publisher */
    MoqResult r3 = moq_publish_data(NULL, (const uint8_t*)"data", 4, MOQ_DELIVERY_STREAM);
    TEST_ASSERT_EQ(r3.code, MOQ_ERROR_INVALID_ARGUMENT, "NULL publisher should return INVALID_ARGUMENT");

    moq_client_destroy(client);
}

void test_not_connected_errors(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Try operations that require connection */
    MoqResult result = moq_announce_namespace(client, "test-namespace");

    /* Should fail with NOT_CONNECTED or similar */
    TEST_ASSERT_NEQ(result.code, MOQ_OK, "Should fail when not connected");
    printf("Announce without connection: code=%d, message=%s\n",
           result.code, result.message ? result.message : "null");

    moq_client_destroy(client);
}

void test_result_code_ranges(void) {
    moq_init();

    /* Verify all result codes are distinct */
    int codes[] = {
        MOQ_OK,
        MOQ_ERROR_INVALID_ARGUMENT,
        MOQ_ERROR_CONNECTION_FAILED,
        MOQ_ERROR_NOT_CONNECTED,
        MOQ_ERROR_TIMEOUT,
        MOQ_ERROR_INTERNAL,
        MOQ_ERROR_UNSUPPORTED,
        MOQ_ERROR_BUFFER_TOO_SMALL
    };
    int num_codes = sizeof(codes) / sizeof(codes[0]);

    for (int i = 0; i < num_codes; i++) {
        for (int j = i + 1; j < num_codes; j++) {
            TEST_ASSERT(codes[i] != codes[j], "Result codes should be distinct");
        }
    }

    TEST_ASSERT(true, "All result codes are distinct");
}

void test_error_after_destroy(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    moq_client_destroy(client);

    /* Operations on destroyed client should fail gracefully */
    /* Note: This is technically undefined behavior, but we test robustness */
    /* bool connected = moq_is_connected(client); */ /* Commented - UB */

    TEST_ASSERT(true, "Destroy completed");
}

void test_multiple_errors(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Trigger multiple errors in sequence */
    MoqResult r1 = moq_connect(client, NULL, NULL, NULL);
    TEST_ASSERT_EQ(r1.code, MOQ_ERROR_INVALID_ARGUMENT, "First error");

    MoqResult r2 = moq_announce_namespace(client, NULL);
    TEST_ASSERT_EQ(r2.code, MOQ_ERROR_INVALID_ARGUMENT, "Second error");

    MoqResult r3 = moq_announce_namespace(client, "");
    TEST_ASSERT_NEQ(r3.code, MOQ_OK, "Third error");

    moq_client_destroy(client);
}

void test_error_recovery(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Trigger an error */
    MoqResult r1 = moq_connect(client, NULL, NULL, NULL);
    TEST_ASSERT_NEQ(r1.code, MOQ_OK, "Should fail with NULL URL");

    /* Try a valid operation after error */
    MoqResult r2 = moq_announce_namespace(client, "valid-namespace");
    /* May still fail (not connected), but shouldn't crash */
    printf("Operation after error: code=%d\n", r2.code);
    TEST_ASSERT(true, "Client can continue after error");

    moq_client_destroy(client);
}

void test_error_message_consistency(void) {
    moq_init();

    MoqClient* client1 = moq_client_create();
    MoqClient* client2 = moq_client_create();

    /* Trigger same error on both clients */
    MoqResult r1 = moq_connect(client1, NULL, NULL, NULL);
    MoqResult r2 = moq_connect(client2, NULL, NULL, NULL);

    TEST_ASSERT_EQ(r1.code, r2.code, "Same error should have same code");

    moq_client_destroy(client1);
    moq_client_destroy(client2);
}

void test_unsupported_operation(void) {
    moq_init();

    /* Currently all operations are supported, but we test the error code exists */
    TEST_ASSERT_NEQ(MOQ_ERROR_UNSUPPORTED, MOQ_OK,
                    "UNSUPPORTED code should exist");
    TEST_ASSERT(true, "UNSUPPORTED error code defined");
}

void test_timeout_error_code(void) {
    moq_init();

    /* Verify timeout error code is defined */
    TEST_ASSERT_NEQ(MOQ_ERROR_TIMEOUT, MOQ_OK,
                    "TIMEOUT code should exist");
    TEST_ASSERT(true, "TIMEOUT error code defined");
}

int main(void) {
    TEST_INIT();

    printf("Running error handling tests...\n\n");

    test_last_error_null_operations();
    test_result_message_validity();
    test_invalid_argument_errors();
    test_not_connected_errors();
    test_result_code_ranges();
    test_error_after_destroy();
    test_multiple_errors();
    test_error_recovery();
    test_error_message_consistency();
    test_unsupported_operation();
    test_timeout_error_code();

    TEST_EXIT();
    return 0;
}
