/* SPDX-License-Identifier: GPL-2.0-only */
#include "../../../kmod/mf4_protocol.h"
#include "../../../kmod/mf_basic_protocol.h"
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

static uint64_t fold(uint64_t hash, unsigned int value)
{
	return (hash ^ value) * UINT64_C(1099511628211);
}

int main(void)
{
	static const __u16 values[] = {0, 1,	2,    3,      4,      5,      6,
				       7, 3072, 3073, 0x8000, 0xc000, 0xbfff, 0xffff};
	uint64_t hash = UINT64_C(14695981039346656037);
	unsigned int selector, i, model;
	static const __u16 pids[] = {0xaf80, 0xaf90, 0xaf93, 0xaf82, 0xaf92, 0xaf70, 0xaf72};
	uint64_t basic_hash = UINT64_C(14695981039346656037);
	__u16 offset, mask;

	_Static_assert(sizeof(struct mf4_transaction) == 1032, "transaction ABI size");
	_Static_assert(offsetof(struct mf4_transaction, after) == 838, "state offset");
	_Static_assert(offsetof(struct mf4_transaction, flags) == 1030, "flags offset");
	_Static_assert(sizeof(struct mf4_capabilities) == 16, "capabilities ABI size");
	_Static_assert(sizeof(struct mf4_meter_record) == 80, "meter record ABI size");
	_Static_assert(offsetof(struct mf4_meter_record, data) == 16, "meter data offset");
	_Static_assert(offsetof(struct mf4_capabilities, flags) == 8, "capability flags offset");
	_Static_assert(sizeof(struct mfb_record) == 68, "basic status ABI size");
	_Static_assert(sizeof(struct mfb_transaction) == 136, "basic transaction ABI size");
	_Static_assert(offsetof(struct mfb_transaction, expected) == 12, "basic expected offset");
	_Static_assert(offsetof(struct mfb_transaction, changes) == 64, "basic changes offset");
	_Static_assert(offsetof(struct mfb_transaction, after) == 80, "basic after offset");
	_Static_assert(offsetof(struct mfb_transaction, flags) == 134, "basic flags offset");
	assert(mf_legacy_allowed(0xaf80, 0x0400));
	assert(!mf_legacy_allowed(0xaf82, 0x0500));
	assert(mf_legacy_allowed(0xaf90, 0x0400));
	assert(!mf_legacy_allowed(0xaf90, 0x0300));
	assert(!mf_legacy_allowed(0xaf90, 0x0001));
	assert(mf_legacy_allowed(0xaf70, 0x0300));
	assert(mf_legacy_allowed(0xaf70, 0x0001));
	assert(!mf_legacy_allowed(0xaf70, 0x0400));
	assert(!mf_legacy_allowed(0xaf70, 0x0500));
	assert(!mf_legacy_allowed(0x1234, 0));
	for (selector = 0; selector <= 0xffff; selector++) {
		for (i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
			int accepted = mf4_target(selector, values[i], &offset, &mask) == 0;
			hash = fold(hash, accepted);
			if (accepted) {
				hash = fold(hash, offset);
				hash = fold(hash, mask);
			}
		}
	}
	for (model = 0; model < sizeof(pids) / sizeof(pids[0]); model++) {
		basic_hash = fold(basic_hash, mfb_state_length(pids[model]));
		for (selector = 0; selector <= 0xffff; selector++) {
			for (i = 0; i < 3; i++) {
				int accepted = mfb_target(pids[model], selector, i, &offset, &mask) == 0;
				basic_hash = fold(basic_hash, accepted);
				if (accepted) { basic_hash = fold(basic_hash, offset); basic_hash = fold(basic_hash, mask); }
			}
		}
	}
	printf("%lu %llu %lu %lu %lu %lu %llu\n", (unsigned long)MF4_IOCTL_APPLY,
	       (unsigned long long)hash, (unsigned long)MF4_IOCTL_CAPABILITIES,
	       (unsigned long)MF4_IOCTL_METERS, (unsigned long)MFB_IOCTL_STATUS,
           (unsigned long)MFB_IOCTL_APPLY, (unsigned long long)basic_hash);
	return 0;
}
