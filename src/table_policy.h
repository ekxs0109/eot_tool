#ifndef EOT_TOOL_TABLE_POLICY_H
#define EOT_TOOL_TABLE_POLICY_H

#include <stdint.h>

typedef enum {
  TABLE_POLICY_KEEP = 0,
  TABLE_POLICY_REENCODE = 1,
  TABLE_POLICY_DROP_WITH_WARNING = 2
} table_policy_t;

table_policy_t table_policy_for_tag(uint32_t tag);
table_policy_t subset_table_policy_for_tag(uint32_t tag);

#endif
