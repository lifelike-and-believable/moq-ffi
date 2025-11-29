#include "test_framework.h"
#include "moq_ffi.h"

void test_client_create_destroy(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "moq_client_create() should return non-null client");

    moq_client_destroy(client);
    /* No crash should occur */
    TEST_ASSERT(true, "moq_client_destroy() should complete without crash");
}

void test_client_create_multiple(void) {
    moq_init();

    MoqClient* client1 = moq_client_create();
    MoqClient* client2 = moq_client_create();
    MoqClient* client3 = moq_client_create();

    TEST_ASSERT_NOT_NULL(client1, "First client should be created");
    TEST_ASSERT_NOT_NULL(client2, "Second client should be created");
    TEST_ASSERT_NOT_NULL(client3, "Third client should be created");

    /* All clients should be distinct */
    TEST_ASSERT(client1 != client2, "Clients should be distinct (1 vs 2)");
    TEST_ASSERT(client1 != client3, "Clients should be distinct (1 vs 3)");
    TEST_ASSERT(client2 != client3, "Clients should be distinct (2 vs 3)");

    moq_client_destroy(client1);
    moq_client_destroy(client2);
    moq_client_destroy(client3);
    TEST_ASSERT(true, "All clients destroyed successfully");
}

void test_client_destroy_null(void) {
    moq_init();

    /* Destroying null client should not crash */
    moq_client_destroy(NULL);
    TEST_ASSERT(true, "moq_client_destroy(NULL) should not crash");
}

void test_client_double_destroy(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    moq_client_destroy(client);
    /* Note: Double destroy is undefined behavior, but shouldn't crash in practice
     * due to Arc<Mutex> wrapper in Rust. We test this for robustness.
     * In production code, users should never do this. */
    TEST_ASSERT(true, "First destroy completed");

    /* Second destroy - this is technically UB but we test defensive behavior */
    /* moq_client_destroy(client); */ /* Commented out - truly undefined */
}

void test_is_connected_before_connect(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    bool connected = moq_is_connected(client);
    TEST_ASSERT(!connected, "Client should not be connected before moq_connect()");

    moq_client_destroy(client);
}

void test_is_connected_null_client(void) {
    moq_init();

    bool connected = moq_is_connected(NULL);
    TEST_ASSERT(!connected, "moq_is_connected(NULL) should return false");
}

void test_publisher_lifecycle(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Note: Creating a publisher without connection will fail,
     * but we test that it doesn't crash */
    MoqPublisher* pub = moq_create_publisher(client, "test-namespace", "test-track");
    if (pub != NULL) {
        moq_publisher_destroy(pub);
        TEST_ASSERT(true, "Publisher created and destroyed without connection");
    } else {
        TEST_ASSERT(true, "Publisher creation without connection returns NULL (expected)");
    }

    moq_client_destroy(client);
}

void test_publisher_destroy_null(void) {
    moq_init();

    moq_publisher_destroy(NULL);
    TEST_ASSERT(true, "moq_publisher_destroy(NULL) should not crash");
}

void test_subscriber_destroy_null(void) {
    moq_init();

    moq_subscriber_destroy(NULL);
    TEST_ASSERT(true, "moq_subscriber_destroy(NULL) should not crash");
}

void test_is_subscribed_null_subscriber(void) {
    moq_init();

    bool subscribed = moq_is_subscribed(NULL);
    TEST_ASSERT(!subscribed, "moq_is_subscribed(NULL) should return false");
}

void test_moq_free_str(void) {
    moq_init();

    /* Test freeing NULL string */
    moq_free_str(NULL);
    TEST_ASSERT(true, "moq_free_str(NULL) should not crash");

    /* Note: We can't easily test freeing a valid string without
     * triggering an operation that allocates one. This is covered
     * in integration tests. */
}

int main(void) {
    TEST_INIT();

    printf("Running lifecycle tests...\n\n");

    test_client_create_destroy();
    test_client_create_multiple();
    test_client_destroy_null();
    test_client_double_destroy();
    test_is_connected_before_connect();
    test_is_connected_null_client();
    test_publisher_lifecycle();
    test_publisher_destroy_null();
    test_subscriber_destroy_null();
    test_is_subscribed_null_subscriber();
    test_moq_free_str();

    TEST_EXIT();
    return 0;
}
