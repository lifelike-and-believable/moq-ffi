#include "test_framework.h"
#include "moq_ffi.h"
#include <stdio.h>
#include <string.h>

/* Track announcement callback tracking */
typedef struct {
    int callback_count;
    char last_namespace[256];
    char last_track[256];
} TrackCallbackData;

void track_callback(void* user_data, const char* namespace_name, const char* track_name) {
    TrackCallbackData* data = (TrackCallbackData*)user_data;
    data->callback_count++;

    if (namespace_name) {
        strncpy(data->last_namespace, namespace_name, sizeof(data->last_namespace) - 1);
        data->last_namespace[sizeof(data->last_namespace) - 1] = '\0';
    }

    if (track_name) {
        strncpy(data->last_track, track_name, sizeof(data->last_track) - 1);
        data->last_track[sizeof(data->last_track) - 1] = '\0';
    }

    printf("  Track announcement: namespace='%s', track='%s'\n",
           namespace_name ? namespace_name : "null",
           track_name ? track_name : "null");
}

/* Catalog callback tracking */
typedef struct {
    int callback_count;
    int last_track_count;
} CatalogCallbackData;

void catalog_callback(void* user_data, const MoqTrackInfo* tracks, size_t track_count) {
    CatalogCallbackData* data = (CatalogCallbackData*)user_data;
    data->callback_count++;
    data->last_track_count = (int)track_count;

    printf("  Catalog callback: %zu tracks\n", track_count);

    for (size_t i = 0; i < track_count; i++) {
        printf("    Track %zu: name=%s, codec=%s, mime=%s\n",
               i,
               tracks[i].name ? tracks[i].name : "null",
               tracks[i].codec ? tracks[i].codec : "null",
               tracks[i].mime_type ? tracks[i].mime_type : "null");
    }
}

void test_subscribe_announces_null_client(void) {
    moq_init();

    MoqResult result = moq_subscribe_announces(NULL, track_callback, NULL);
    TEST_ASSERT_NEQ(result.code, MOQ_OK,
                    "moq_subscribe_announces(NULL client) should fail");
    TEST_ASSERT_EQ(result.code, MOQ_ERROR_INVALID_ARGUMENT,
                   "Should return INVALID_ARGUMENT for NULL client");
}

void test_subscribe_announces_null_callback(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    /* NULL callback is valid - it means "unregister" according to the API */
    MoqResult result = moq_subscribe_announces(client, NULL, NULL);
    TEST_ASSERT_EQ(result.code, MOQ_OK,
                   "moq_subscribe_announces() with NULL callback should succeed (unregister)");

    moq_client_destroy(client);
}

void test_subscribe_announces_not_connected(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    TrackCallbackData cb_data = {0};
    MoqResult result = moq_subscribe_announces(client, track_callback, &cb_data);

    /* According to the API, the callback is stored and will activate on connect.
     * This is expected to succeed even without connection. */
    TEST_ASSERT_EQ(result.code, MOQ_OK,
                   "moq_subscribe_announces() should succeed (stores callback for later)");
    printf("Subscribe announces without connection: code=%d\n", result.code);

    moq_client_destroy(client);
}

void test_subscribe_catalog_null_client(void) {
    moq_init();

    MoqSubscriber* sub = moq_subscribe_catalog(NULL, "namespace", "catalog", catalog_callback, NULL);
    TEST_ASSERT_NULL(sub, "moq_subscribe_catalog(NULL client) should return NULL");
}

void test_subscribe_catalog_null_namespace(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqSubscriber* sub = moq_subscribe_catalog(client, NULL, "catalog", catalog_callback, NULL);
    TEST_ASSERT_NULL(sub, "moq_subscribe_catalog() with NULL namespace should return NULL");

    moq_client_destroy(client);
}

void test_subscribe_catalog_null_callback(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    MoqSubscriber* sub = moq_subscribe_catalog(client, "namespace", "catalog", NULL, NULL);
    TEST_ASSERT_NULL(sub, "moq_subscribe_catalog() with NULL callback should return NULL");

    moq_client_destroy(client);
}

void test_subscribe_catalog_not_connected(void) {
    moq_init();

    MoqClient* client = moq_client_create();
    TEST_ASSERT_NOT_NULL(client, "Client should be created");

    CatalogCallbackData cb_data = {0};
    MoqSubscriber* sub = moq_subscribe_catalog(client, "namespace", "catalog", catalog_callback, &cb_data);

    /* May return NULL or non-NULL depending on implementation */
    printf("Catalog subscribe without connection returned: %p\n", (void*)sub);

    if (sub != NULL) {
        moq_subscriber_destroy(sub);
    }

    moq_client_destroy(client);
}

void test_track_info_structure(void) {
    moq_init();

    /* Verify MoqTrackInfo structure fields are accessible */
    MoqTrackInfo track = {0};

    track.name = "test-track";
    track.codec = "h264";
    track.mime_type = "video/h264";
    track.width = 1920;
    track.height = 1080;
    track.bitrate = 5000000;
    track.sample_rate = 0;
    track.language = "en";

    TEST_ASSERT_STR_EQ(track.name, "test-track", "Track name should be set");
    TEST_ASSERT_STR_EQ(track.codec, "h264", "Codec should be set");
    TEST_ASSERT_STR_EQ(track.mime_type, "video/h264", "MIME type should be set");
    TEST_ASSERT_EQ(track.width, 1920, "Width should be 1920");
    TEST_ASSERT_EQ(track.height, 1080, "Height should be 1080");
    TEST_ASSERT_EQ(track.bitrate, 5000000, "Bitrate should be 5000000");
    TEST_ASSERT_STR_EQ(track.language, "en", "Language should be 'en'");
}

int main(void) {
    TEST_INIT();

    printf("Running track discovery tests...\n\n");

    test_subscribe_announces_null_client();
    test_subscribe_announces_null_callback();
    test_subscribe_announces_not_connected();

    test_subscribe_catalog_null_client();
    test_subscribe_catalog_null_namespace();
    test_subscribe_catalog_null_callback();
    test_subscribe_catalog_not_connected();

    test_track_info_structure();

    TEST_EXIT();
    return 0;
}
