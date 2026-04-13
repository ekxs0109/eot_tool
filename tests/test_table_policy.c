#include <stdio.h>

#include "../src/table_policy.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);

#define ASSERT_EQ(actual, expected) do { \
  if ((actual) != (expected)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual: %d, expected: %d)", \
             #actual, #expected, (int)(actual), (int)(expected)); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define TAG_cvt  0x63767420u
#define TAG_hdmx 0x68646d78u
#define TAG_VDMX 0x56444d58u
#define TAG_name 0x6e616d65u

static void test_table_policy_classifies_extra_tables(void) {
  ASSERT_EQ(table_policy_for_tag(TAG_cvt), TABLE_POLICY_REENCODE);
  ASSERT_EQ(table_policy_for_tag(TAG_hdmx), TABLE_POLICY_REENCODE);
  ASSERT_EQ(table_policy_for_tag(TAG_VDMX), TABLE_POLICY_DROP_WITH_WARNING);
}

static void test_table_policy_keeps_other_tables(void) {
  ASSERT_EQ(table_policy_for_tag(TAG_name), TABLE_POLICY_KEEP);
}

static void test_subset_table_policy_classifies_extra_tables(void) {
  ASSERT_EQ(subset_table_policy_for_tag(TAG_cvt), TABLE_POLICY_KEEP);
  ASSERT_EQ(subset_table_policy_for_tag(TAG_hdmx), TABLE_POLICY_DROP_WITH_WARNING);
  ASSERT_EQ(subset_table_policy_for_tag(TAG_VDMX), TABLE_POLICY_DROP_WITH_WARNING);
}

void register_table_policy_tests(void) {
  test_register("test_table_policy_classifies_extra_tables",
                test_table_policy_classifies_extra_tables);
  test_register("test_table_policy_keeps_other_tables",
                test_table_policy_keeps_other_tables);
  test_register("test_subset_table_policy_classifies_extra_tables",
                test_subset_table_policy_classifies_extra_tables);
}
