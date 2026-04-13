#include "table_policy.h"

#define TAG_cvt  0x63767420u
#define TAG_DSIG 0x44534947u
#define TAG_hdmx 0x68646d78u
#define TAG_VDMX 0x56444d58u

table_policy_t table_policy_for_tag(uint32_t tag) {
  switch (tag) {
    case TAG_DSIG:
      return TABLE_POLICY_DROP_WITH_WARNING;
    case TAG_cvt:
    case TAG_hdmx:
      return TABLE_POLICY_REENCODE;
    case TAG_VDMX:
      return TABLE_POLICY_DROP_WITH_WARNING;
    default:
      return TABLE_POLICY_KEEP;
  }
}

table_policy_t subset_table_policy_for_tag(uint32_t tag) {
  switch (tag) {
    case TAG_DSIG:
    case TAG_hdmx:
    case TAG_VDMX:
      return TABLE_POLICY_DROP_WITH_WARNING;
    case TAG_cvt:
    default:
      return TABLE_POLICY_KEEP;
  }
}
