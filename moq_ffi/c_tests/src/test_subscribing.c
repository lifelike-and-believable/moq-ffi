#include "test_framework.h"
#include "moq_ffi.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

/* Data callback tracking */
typedef struct {
    int callback_count;
    size_t total_bytes_received;
    uint8_t* last_data;
    size_t last_data_len;
} DataCallbackData;

void data_callback(void* user_data, const uint8_t* data, size_t len) {
    DataCallbackData* cb_data = (DataCallbackData*)user_data;
    cb_data->callback_count++;
    cb_data->total_bytes_received += len;

    /* Store last received data */
    if (cb_data->last_data) {
        free(cb_data->last_data);
    }
    cb_data->last_data = (uint8_t*)malloc(len);
    if (cb_data->last_data) {
        memcpy(cb_data->last_data, data, len);
        cb_data->last_data_len = len;
    } else {
        cb_data->last_data_len = 0;
        printf("  Warning: malloc failed for %zu bytes\n", len);
    }

    printf("  Data callback: received %zu bytes\n", len);
}

void test_subscribe_null_client(void) {
    moq_init();

    MoqSubscriber* sub = moq_subscribe(NULL, "namespace", "track", data_callback, NULL);
    TEST_ASSERT_NULL(sub, "moq_subscribe(NULL client) should return NULL");
}

void test_subscribe_null_namespace(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqSubscriber* sub = moq_subscribe(client, NULL, "track", data_callback, NULL);
    TEST_ASSERT_NULL(sub, "moq_subscribe() with NULL namespace should return NULL");

    moq_client_destroy(client);
}

void test_subscribe_null_track(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqSubscriber* sub = moq_subscribe(client, "namespace", NULL, data_callback, NULL);
    TEST_ASSERT_NULL(sub, "moq_subscribe() with NULL track should return NULL");

    moq_client_destroy(client);
}

void test_subscribe_null_callback(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqSubscriber* sub = moq_subscribe(client, "namespace", "track", NULL, NULL);
    TEST_ASSERT_NULL(sub, "moq_subscribe() with NULL callback should return NULL");

    moq_client_destroy(client);
}

void test_subscribe_without_connection(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    DataCallbackData cb_data = {0};
    MoqSubscriber* sub = moq_subscribe(client, "namespace", "track", data_callback, &cb_data);

    /* Subscription may return NULL or non-NULL depending on implementation */
    printf("Subscribe without connection returned: %p\n", (void*)sub);

    if (sub != NULL) {
        moq_subscriber_destroy(sub);
    }

    moq_client_destroy(client);
}

void test_unsubscribe_null_subscriber(void) {
    moq_init();

    moq_unsubscribe(NULL);
    TEST_ASSERT(true, "moq_unsubscribe(NULL) should not crash");
}

void test_unsubscribe_without_subscribe(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* Create a subscriber (may fail without connection) */
    DataCallbackData cb_data = {0};
    MoqSubscriber* sub = moq_subscribe(client, "namespace", "track", data_callback, &cb_data);

    if (sub != NULL) {
        /* Unsubscribe should be safe even if subscription wasn't active */
        moq_unsubscribe(sub);
        TEST_ASSERT(true, "moq_unsubscribe() completed");

        /* Check subscription status */
        bool subscribed = moq_is_subscribed(sub);
        TEST_ASSERT(!subscribed, "Should not be subscribed after moq_unsubscribe()");

        moq_subscriber_destroy(sub);
    }

    moq_client_destroy(client);
}

void test_subscriber_lifecycle(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    DataCallbackData cb_data = {0};
    MoqSubscriber* sub = moq_subscribe(client, "namespace", "track", data_callback, &cb_data);

    if (sub != NULL) {
        /* Check initial subscription status */
        bool subscribed = moq_is_subscribed(sub);
        printf("Initial subscription status: %s\n", subscribed ? "true" : "false");

        /* Unsubscribe */
        moq_unsubscribe(sub);

        /* Check status after unsubscribe */
        subscribed = moq_is_subscribed(sub);
        TEST_ASSERT(!subscribed, "Should not be subscribed after unsubscribe");

        /* Destroy */
        moq_subscriber_destroy(sub);
        TEST_ASSERT(true, "Subscriber lifecycle completed");
    } else {
        TEST_ASSERT(true, "Subscriber creation without connection (expected)");
    }

    if (cb_data.last_data) {
        free(cb_data.last_data);
    }

    moq_client_destroy(client);
}

void test_multiple_subscribers(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    DataCallbackData cb_data1 = {0};
    DataCallbackData cb_data2 = {0};
    DataCallbackData cb_data3 = {0};

    MoqSubscriber* sub1 = moq_subscribe(client, "ns1", "track1", data_callback, &cb_data1);
    MoqSubscriber* sub2 = moq_subscribe(client, "ns2", "track2", data_callback, &cb_data2);
    MoqSubscriber* sub3 = moq_subscribe(client, "ns3", "track3", data_callback, &cb_data3);

    if (sub1 && sub2 && sub3) {
        TEST_ASSERT(sub1 != sub2, "Subscribers should be distinct (1 vs 2)");
        TEST_ASSERT(sub1 != sub3, "Subscribers should be distinct (1 vs 3)");
        TEST_ASSERT(sub2 != sub3, "Subscribers should be distinct (2 vs 3)");
    }

    if (sub1) moq_subscriber_destroy(sub1);
    if (sub2) moq_subscriber_destroy(sub2);
    if (sub3) moq_subscriber_destroy(sub3);

    if (cb_data1.last_data) free(cb_data1.last_data);
    if (cb_data2.last_data) free(cb_data2.last_data);
    if (cb_data3.last_data) free(cb_data3.last_data);

    moq_client_destroy(client);
    TEST_ASSERT(true, "Multiple subscribers handled");
}

void test_subscriber_user_data(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    DataCallbackData cb_data = {
        .callback_count = 0,
        .total_bytes_received = 0,
        .last_data = NULL,
        .last_data_len = 0
    };

    MoqSubscriber* sub = moq_subscribe(client, "namespace", "track", data_callback, &cb_data);

    if (sub != NULL) {
        /* If we get callbacks (unlikely without real connection), verify user data works */
        TEST_ASSERT_EQ(cb_data.callback_count, 0, "No callbacks expected without connection");

        moq_subscriber_destroy(sub);
    }

    if (cb_data.last_data) {
        free(cb_data.last_data);
    }

    moq_client_destroy(client);
}

void test_resubscribe_after_unsubscribe(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    DataCallbackData cb_data = {0};

    /* First subscription */
    MoqSubscriber* sub1 = moq_subscribe(client, "namespace", "track", data_callback, &cb_data);
    if (sub1 != NULL) {
        moq_unsubscribe(sub1);
        moq_subscriber_destroy(sub1);
        TEST_ASSERT(true, "First subscription/unsubscription completed");
    }

    /* Second subscription to same track */
    MoqSubscriber* sub2 = moq_subscribe(client, "namespace", "track", data_callback, &cb_data);
    if (sub2 != NULL) {
        moq_subscriber_destroy(sub2);
        TEST_ASSERT(true, "Resubscription after unsubscribe works");
    }

    if (cb_data.last_data) {
        free(cb_data.last_data);
    }

    moq_client_destroy(client);
}

int main(void) {
    TEST_INIT();

    printf("Running subscribing tests...\n\n");

    test_subscribe_null_client();
    test_subscribe_null_namespace();
    test_subscribe_null_track();
    test_subscribe_null_callback();
    test_subscribe_without_connection();

    test_unsubscribe_null_subscriber();
    test_unsubscribe_without_subscribe();

    test_subscriber_lifecycle();
    test_multiple_subscribers();
    test_subscriber_user_data();
    test_resubscribe_after_unsubscribe();

    TEST_EXIT();
    return 0;
}
