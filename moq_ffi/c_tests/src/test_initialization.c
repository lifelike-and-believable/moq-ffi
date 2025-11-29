#include "test_framework.h"
#include "moq_ffi.h"

void test_moq_init_basic(void) {
    bool result = moq_init();
    TEST_ASSERT(result, "moq_init() should succeed");
}

void test_moq_init_idempotent(void) {
    bool result1 = moq_init();
    TEST_ASSERT(result1, "First moq_init() should succeed");

    bool result2 = moq_init();
    TEST_ASSERT(result2, "Second moq_init() should succeed (idempotent)");

    bool result3 = moq_init();
    TEST_ASSERT(result3, "Third moq_init() should succeed (idempotent)");
}

void test_moq_version(void) {
    const char* version = moq_version();
    TEST_ASSERT_NOT_NULL(version, "moq_version() should return non-null string");
    TEST_ASSERT(strlen(version) > 0, "Version string should not be empty");
    printf("MoQ FFI version: %s\n", version);
}

void test_moq_last_error_initial(void) {
    /* Initially there should be no error */
    const char* error = moq_last_error();
    /* Error could be NULL or empty string */
    if (error != NULL) {
        printf("Initial error message: '%s'\n", error);
    }
}

void test_result_codes(void) {
    /* Verify all result codes are defined */
    TEST_ASSERT_EQ(MOQ_OK, 0, "MOQ_OK should be 0");
    TEST_ASSERT_NEQ(MOQ_ERROR_INVALID_ARGUMENT, 0, "MOQ_ERROR_INVALID_ARGUMENT should not be 0");
    TEST_ASSERT_NEQ(MOQ_ERROR_CONNECTION_FAILED, 0, "MOQ_ERROR_CONNECTION_FAILED should not be 0");
    TEST_ASSERT_NEQ(MOQ_ERROR_NOT_CONNECTED, 0, "MOQ_ERROR_NOT_CONNECTED should not be 0");
    TEST_ASSERT_NEQ(MOQ_ERROR_TIMEOUT, 0, "MOQ_ERROR_TIMEOUT should not be 0");
    TEST_ASSERT_NEQ(MOQ_ERROR_INTERNAL, 0, "MOQ_ERROR_INTERNAL should not be 0");
    TEST_ASSERT_NEQ(MOQ_ERROR_UNSUPPORTED, 0, "MOQ_ERROR_UNSUPPORTED should not be 0");
    TEST_ASSERT_NEQ(MOQ_ERROR_BUFFER_TOO_SMALL, 0, "MOQ_ERROR_BUFFER_TOO_SMALL should not be 0");
}

void test_connection_state_enum(void) {
    /* Verify connection state enum values */
    TEST_ASSERT_EQ(MOQ_STATE_DISCONNECTED, 0, "MOQ_STATE_DISCONNECTED should be 0");
    TEST_ASSERT_NEQ(MOQ_STATE_CONNECTING, MOQ_STATE_DISCONNECTED,
                    "MOQ_STATE_CONNECTING should differ from DISCONNECTED");
    TEST_ASSERT_NEQ(MOQ_STATE_CONNECTED, MOQ_STATE_DISCONNECTED,
                    "MOQ_STATE_CONNECTED should differ from DISCONNECTED");
    TEST_ASSERT_NEQ(MOQ_STATE_FAILED, MOQ_STATE_DISCONNECTED,
                    "MOQ_STATE_FAILED should differ from DISCONNECTED");
}

void test_delivery_mode_enum(void) {
    /* Verify delivery mode enum values */
    TEST_ASSERT_EQ(MOQ_DELIVERY_DATAGRAM, 0, "MOQ_DELIVERY_DATAGRAM should be 0");
    TEST_ASSERT_NEQ(MOQ_DELIVERY_STREAM, MOQ_DELIVERY_DATAGRAM,
                    "MOQ_DELIVERY_STREAM should differ from DATAGRAM");
}

int main(void) {
    TEST_INIT();

    printf("Running initialization tests...\n\n");

    test_moq_init_basic();
    test_moq_init_idempotent();
    test_moq_version();
    test_moq_last_error_initial();
    test_result_codes();
    test_connection_state_enum();
    test_delivery_mode_enum();

    TEST_EXIT();
    return 0;
}
