#ifndef TEST_FRAMEWORK_H
#define TEST_FRAMEWORK_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Test statistics */
typedef struct {
    int total_tests;
    int passed_tests;
    int failed_tests;
} TestStats;

/* Global test statistics */
extern TestStats g_test_stats;

/* Test macros */
#define TEST_INIT() \
    do { \
        g_test_stats.total_tests = 0; \
        g_test_stats.passed_tests = 0; \
        g_test_stats.failed_tests = 0; \
    } while(0)

#define TEST_ASSERT(condition, message) \
    do { \
        g_test_stats.total_tests++; \
        if (!(condition)) { \
            fprintf(stderr, "[FAIL] %s:%d: %s\n", __FILE__, __LINE__, message); \
            g_test_stats.failed_tests++; \
        } else { \
            fprintf(stdout, "[PASS] %s\n", message); \
            g_test_stats.passed_tests++; \
        } \
    } while(0)

#define TEST_ASSERT_EQ(actual, expected, message) \
    do { \
        g_test_stats.total_tests++; \
        if ((actual) != (expected)) { \
            fprintf(stderr, "[FAIL] %s:%d: %s (expected %d, got %d)\n", \
                    __FILE__, __LINE__, message, (int)(expected), (int)(actual)); \
            g_test_stats.failed_tests++; \
        } else { \
            fprintf(stdout, "[PASS] %s\n", message); \
            g_test_stats.passed_tests++; \
        } \
    } while(0)

#define TEST_ASSERT_NEQ(actual, not_expected, message) \
    do { \
        g_test_stats.total_tests++; \
        if ((actual) == (not_expected)) { \
            fprintf(stderr, "[FAIL] %s:%d: %s (got %d)\n", \
                    __FILE__, __LINE__, message, (int)(actual)); \
            g_test_stats.failed_tests++; \
        } else { \
            fprintf(stdout, "[PASS] %s\n", message); \
            g_test_stats.passed_tests++; \
        } \
    } while(0)

#define TEST_ASSERT_STR_EQ(actual, expected, message) \
    do { \
        g_test_stats.total_tests++; \
        if (strcmp((actual), (expected)) != 0) { \
            fprintf(stderr, "[FAIL] %s:%d: %s (expected '%s', got '%s')\n", \
                    __FILE__, __LINE__, message, (expected), (actual)); \
            g_test_stats.failed_tests++; \
        } else { \
            fprintf(stdout, "[PASS] %s\n", message); \
            g_test_stats.passed_tests++; \
        } \
    } while(0)

#define TEST_ASSERT_NULL(ptr, message) \
    do { \
        g_test_stats.total_tests++; \
        if ((ptr) != NULL) { \
            fprintf(stderr, "[FAIL] %s:%d: %s (expected NULL, got %p)\n", \
                    __FILE__, __LINE__, message, (void*)(ptr)); \
            g_test_stats.failed_tests++; \
        } else { \
            fprintf(stdout, "[PASS] %s\n", message); \
            g_test_stats.passed_tests++; \
        } \
    } while(0)

#define TEST_ASSERT_NOT_NULL(ptr, message) \
    do { \
        g_test_stats.total_tests++; \
        if ((ptr) == NULL) { \
            fprintf(stderr, "[FAIL] %s:%d: %s (got NULL)\n", \
                    __FILE__, __LINE__, message); \
            g_test_stats.failed_tests++; \
        } else { \
            fprintf(stdout, "[PASS] %s\n", message); \
            g_test_stats.passed_tests++; \
        } \
    } while(0)

#define TEST_ASSERT_MEM_EQ(actual, expected, size, message) \
    do { \
        g_test_stats.total_tests++; \
        if (memcmp((actual), (expected), (size)) != 0) { \
            fprintf(stderr, "[FAIL] %s:%d: %s (memory mismatch)\n", \
                    __FILE__, __LINE__, message); \
            g_test_stats.failed_tests++; \
        } else { \
            fprintf(stdout, "[PASS] %s\n", message); \
            g_test_stats.passed_tests++; \
        } \
    } while(0)

#define TEST_SUMMARY() \
    do { \
        fprintf(stdout, "\n========== TEST SUMMARY ==========\n"); \
        fprintf(stdout, "Total:  %d\n", g_test_stats.total_tests); \
        fprintf(stdout, "Passed: %d\n", g_test_stats.passed_tests); \
        fprintf(stdout, "Failed: %d\n", g_test_stats.failed_tests); \
        fprintf(stdout, "==================================\n"); \
    } while(0)

#define TEST_EXIT() \
    do { \
        TEST_SUMMARY(); \
        exit(g_test_stats.failed_tests > 0 ? 1 : 0); \
    } while(0)

/* Utility functions */
void test_sleep_ms(int milliseconds);
uint64_t test_timestamp_ms(void);

/* Test configuration */
#define CLOUDFLARE_RELAY_URL "https://relay.cloudflare.mediaoverquic.com"
#define TEST_TIMEOUT_MS 30000
#define SHORT_TIMEOUT_MS 5000

#ifdef __cplusplus
}
#endif

#endif /* TEST_FRAMEWORK_H */
