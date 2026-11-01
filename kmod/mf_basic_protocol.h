/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef MF_BASIC_PROTOCOL_H
#define MF_BASIC_PROTOCOL_H
#include <linux/types.h>
#include <linux/ioctl.h>
#include <linux/errno.h>
#define MFB_MAX_STATE 52
#define MFB_MAX_CHANGES 4
#define MF_CAP_BASIC_SWITCHES 4
#define MFB_AFTER_VALID 4
#define MFB_WRITE_ATTEMPTED 8
struct mfb_record {
	__u8 magic[4];
	__le16 version, vendor, product, bus, address, length;
	__u8 data[MFB_MAX_STATE];
} __attribute__((packed));
struct mfb_change { __le16 selector, value; } __attribute__((packed));
struct mfb_transaction {
	__le16 version, bus, address, product, length, count;
	__u8 expected[MFB_MAX_STATE];
	struct mfb_change changes[MFB_MAX_CHANGES];
	__u8 after[MFB_MAX_STATE];
	__le16 completed, flags;
} __attribute__((packed));
#define MFB_IOCTL_STATUS _IOR('M', 0x44, struct mfb_record)
#define MFB_IOCTL_APPLY _IOWR('M', 0x45, struct mfb_transaction)
static inline unsigned int mfb_state_length(__u16 pid)
{
	return pid == 0xaf80 || pid == 0xaf90 ? 46 : pid == 0xaf93 ? 52 : 0;
}
static inline int mfb_target(__u16 pid, __u16 selector, __u16 value, __u16 *offset, __u16 *mask)
{
	if (!mfb_state_length(pid) || value > 1)
		return -EINVAL;
	if (selector == 0 || (selector == 1 && pid != 0xaf80)) {
		*offset = 0;
		*mask = 1U << selector;
	} else if (selector == 0x0400) {
		*offset = 8;
		*mask = 1;
	} else if (selector == 0x0500) {
		*offset = 10;
		*mask = 1;
	} else {
		return -EINVAL;
	}
	return 0;
}
#endif
