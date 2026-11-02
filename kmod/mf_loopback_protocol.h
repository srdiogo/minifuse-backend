/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef MF_LOOPBACK_PROTOCOL_H
#define MF_LOOPBACK_PROTOCOL_H
#include "mf_basic_protocol.h"
#define MF_CAP_BASIC_LOOPBACK 8
#define MF_CAP_BASIC_METERS 16
#define MFL_MAX_CHANGES 32
struct mfl_transaction {
	__le16 version, bus, address, product, length, count, firmware;
	__u8 expected[MFB_MAX_STATE];
	struct mfb_change changes[MFL_MAX_CHANGES];
	__u8 after[MFB_MAX_STATE];
	__le16 completed, flags;
} __attribute__((packed));
struct mfl_meter_record {
	__u8 magic[4];
	__le16 version, vendor, product, bus, address, length, firmware, reserved;
	__u8 data[24];
} __attribute__((packed));
#define MFL_IOCTL_APPLY _IOWR('M', 0x46, struct mfl_transaction)
#define MFL_IOCTL_METERS _IOR('M', 0x47, struct mfl_meter_record)
static inline int mfl_supported(__u16 pid, __u16 firmware)
{
	unsigned int major = firmware >> 8, minor = (firmware >> 4) & 15;
	if (major != 1 || (firmware & 15) > 9)
		return 0;
	return ((pid == 0xaf80 || pid == 0xaf90) && minor == 5) || (pid == 0xaf93 && minor == 0);
}
static inline int mfl_target(__u16 pid, __u16 selector, __u16 value, __u16 *offset, __u16 *mask)
{
	unsigned int family = selector >> 8, channel = selector & 255;
	__s16 level = (__s16)value;
	if (!mfb_state_length(pid))
		return -EINVAL;
	*mask = 0;
	if ((family == 6 || family == 12) && channel < 6 && !(pid == 0xaf80 && channel == 1) &&
	    level <= 3072) {
		*offset = 2 * (family + channel);
	} else if (family == 18 && channel < 6 && !(pid == 0xaf80 && channel == 1) && value <= 1) {
		*offset = 36;
		*mask = 1U << channel;
	} else if (family >= 19 && family <= 21 && !channel && value <= 1) {
		*offset = family * 2;
		*mask = 1;
	} else if (family == 22 && !channel && level <= 0 && (level >= -16384 || level == -32768)) {
		*offset = 44;
	} else if (pid == 0xaf93 && !channel &&
		   ((family == 23 || family == 25) ? value <= 1 : family == 24 && value <= 3)) {
		*offset = family * 2;
	} else {
		return mfb_target(pid, selector, value, offset, mask);
	}
	return 0;
}
#endif
