/* SPDX-License-Identifier: GPL-2.0-only */
/* Execute the production transaction header with in-memory kernel/USB hooks.
 * No device node, USB stack, firmware or kernel module is opened or loaded.
 * This checks guard/rollback bookkeeping, not real-device protocol behavior.
 */
#define _DEFAULT_SOURCE
#include <assert.h>
#include <endian.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../../kmod/mf_loopback_protocol.h"

#define MF4_VID 0x1c75
#define FMODE_READ 1
#define FMODE_WRITE 2
#define GFP_KERNEL 0
#define ERESTARTSYS 512
#define USB_DIR_IN 0x80
#define USB_TYPE_CLASS 0x20
#define USB_RECIP_INTERFACE 1
#define __user
#define cpu_to_le16 htole16
#define le16_to_cpu le16toh

typedef __u8 u8;
typedef __u16 u16;
struct file {
	unsigned int f_mode;
};
struct usb_bus {
	unsigned int busnum;
};
struct usb_device {
	struct {
		__le16 idVendor, idProduct, bcdDevice;
	} descriptor;
	struct usb_bus *bus;
	unsigned int devnum;
};
struct fixture {
	struct usb_device devices[2];
	struct usb_bus bus;
	u8 state[MFB_MAX_STATE], peaks[24];
	unsigned int devices_present, references, allocations, reads, meter_reads, writes, sleeps;
	unsigned int read_failure, write_failure, rollback_failure, external_change;
	u16 selectors[80], values[80];
	bool locked, interrupted, allocation_failure, input_fault, output_fault;
	bool short_read, ignore_writes, apply_before_error;
} f;
static int mf4_query_mutex;
static unsigned int cases;

static u16 mf4_get_word(const u8 *bytes) { return (u16)bytes[0] | (u16)bytes[1] << 8; }
static void mf4_put_word(u16 value, u8 *bytes)
{
	bytes[0] = value & 255;
	bytes[1] = value >> 8;
}
static void *kzalloc(size_t size, unsigned int flags)
{
	assert(flags == GFP_KERNEL);
	if (f.allocation_failure)
		return NULL;
	void *value = calloc(1, size);
	assert(value);
	f.allocations++;
	return value;
}
static void kfree(void *value)
{
	if (value) {
		assert(f.allocations);
		f.allocations--;
		free(value);
	}
}
static int copy_from_user(void *to, const void *from, size_t size)
{
	if (f.input_fault)
		return 1;
	memcpy(to, from, size);
	return 0;
}
static int copy_to_user(void *to, const void *from, size_t size)
{
	if (f.output_fault)
		return 1;
	memcpy(to, from, size);
	return 0;
}
static int mutex_lock_interruptible(int *lock)
{
	assert(lock == &mf4_query_mutex);
	if (f.interrupted)
		return 1;
	assert(!f.locked);
	f.locked = true;
	return 0;
}
static void mutex_unlock(int *lock)
{
	assert(lock == &mf4_query_mutex && f.locked);
	f.locked = false;
}
static struct usb_device *usb_get_dev(struct usb_device *device)
{
	f.references++;
	return device;
}
static void usb_put_dev(struct usb_device *device)
{
	assert(device && f.references);
	f.references--;
}
static void usb_for_each_dev(void *context, int (*visit)(struct usb_device *, void *))
{
	assert(f.locked);
	for (unsigned int i = 0; i < f.devices_present; i++)
		visit(&f.devices[i], context);
}
static unsigned int usb_rcvctrlpipe(struct usb_device *device, unsigned int endpoint)
{
	assert(device && endpoint == 0);
	return 0;
}
static int usb_control_msg(struct usb_device *device, unsigned int pipe, u8 request, u8 type,
			   u16 value, u16 index, void *data, u16 length, int timeout)
{
	assert(f.locked && device == &f.devices[0] && pipe == 0);
	assert(type == 0xa1 && value == 0 && index == 0 && timeout == 750);
	if (request == 0xb2) {
		assert(length == mfb_state_length(le16_to_cpu(device->descriptor.idProduct)));
		f.reads++;
		if (f.reads == f.read_failure)
			return -ENODEV;
		if (f.reads == f.external_change)
			f.state[2] ^= 1; /* opaque word */
		memcpy(data, f.state, length);
	} else {
		assert(request == 0xc2);
		assert(length == (le16_to_cpu(device->descriptor.idProduct) == 0xaf93 ? 24 : 16));
		f.meter_reads++;
		memcpy(data, f.peaks, length);
	}
	return f.short_read ? length - 1 : length;
}
static int mf4_set(struct usb_device *device, u16 selector, const __le16 *encoded)
{
	u16 offset, mask, raw;
	memcpy(&raw, encoded, sizeof(raw));
	u16 value = le16_to_cpu(raw);
	assert(f.locked && device == &f.devices[0] && f.writes < 80);
	f.selectors[f.writes] = selector;
	f.values[f.writes++] = value;
	bool failed = f.writes == f.write_failure || f.writes == f.rollback_failure;
	if (failed && !f.apply_before_error)
		return -ETIMEDOUT;
	assert(mfl_target(le16_to_cpu(device->descriptor.idProduct), selector, value, &offset,
			  &mask) == 0);
	if (!f.ignore_writes) {
		u16 old = mf4_get_word(f.state + offset);
		mf4_put_word(mask ? (value ? old | mask : old & ~mask) : value, f.state + offset);
	}
	return failed ? -ETIMEDOUT : 0;
}
static void msleep(unsigned int delay)
{
	assert(delay == 20);
	f.sleeps++;
}
static const void *memchr_inv(const void *data, int value, size_t length)
{
	const u8 *bytes = data;
	for (size_t i = 0; i < length; i++)
		if (bytes[i] != value)
			return bytes + i;
	return NULL;
}
#include "../../../kmod/mf_basic_transport.h"

static void clean(void) { assert(!f.locked && f.references == 0 && f.allocations == 0); }
static void reset(u16 pid)
{
	clean();
	memset(&f, 0, sizeof(f));
	f.bus.busnum = 1;
	f.devices[0] = (struct usb_device){
	    .descriptor = {cpu_to_le16(MF4_VID), cpu_to_le16(pid),
			   cpu_to_le16(pid == 0xaf93 ? 0x100 : 0x152)},
	    .bus = &f.bus,
	    .devnum = 2,
	};
	f.devices[1] = f.devices[0];
	f.devices_present = 1;
	f.state[1] = 0xa0;
	f.state[2] = 0x55;
	f.state[3] = 0xa4;
	mf4_put_word((u16)(-6 * 256), f.state + 44);
}
static struct mfl_transaction transaction(void)
{
	struct mfl_transaction tx = {
	    .version = cpu_to_le16(1),
	    .bus = cpu_to_le16(1),
	    .address = cpu_to_le16(2),
	    .product = f.devices[0].descriptor.idProduct,
	    .length = cpu_to_le16(mfb_state_length(le16_to_cpu(f.devices[0].descriptor.idProduct))),
	    .count = cpu_to_le16(3),
	    .firmware = f.devices[0].descriptor.bcdDevice,
	    .changes = {{cpu_to_le16(0x1600), cpu_to_le16((u16)(-18 * 256))},
			{cpu_to_le16(0x0600), cpu_to_le16((u16)(-24 * 256))},
			{cpu_to_le16(0x1200), cpu_to_le16(1)}},
	};
	memcpy(tx.expected, f.state, le16_to_cpu(tx.length));
	return tx;
}
static long apply(struct mfl_transaction *tx, unsigned int mode)
{
	struct file file = {.f_mode = mode};
	long result = mfl_apply(&file, (unsigned long)(uintptr_t)tx);
	clean();
	cases++;
	return result;
}
static long meters(struct mfl_meter_record *record, unsigned int mode)
{
	struct file file = {.f_mode = mode};
	long result = mfl_meters(&file, (unsigned long)(uintptr_t)record);
	clean();
	cases++;
	return result;
}
static void native_cases(u16 pid)
{
	struct mfl_transaction tx;
	u8 original[MFB_MAX_STATE], predicted[MFB_MAX_STATE];
	reset(pid);
	tx = transaction();
	memcpy(original, f.state, sizeof(original));
	memcpy(predicted, original, sizeof(predicted));
	mf4_put_word((u16)(-18 * 256), predicted + 44);
	mf4_put_word((u16)(-24 * 256), predicted + 12);
	predicted[36] |= 1;
	assert(apply(&tx, FMODE_WRITE) == 0);
	assert(le16_to_cpu(tx.flags) == 12 && le16_to_cpu(tx.completed) == 3);
	assert(f.writes == 3 && f.reads == 2 && !memcmp(f.state, predicted, sizeof(predicted)));
	assert(!memcmp(tx.after, predicted, le16_to_cpu(tx.length)));

	/* Header, selector, copy, access, allocation and lock rejection precede USB. */
	for (unsigned int scenario = 0; scenario < 13; scenario++) {
		reset(pid);
		tx = transaction();
		long expected = -EINVAL;
		unsigned int mode = FMODE_WRITE;
		switch (scenario) {
		case 0:
			tx.version = cpu_to_le16(2);
			break;
		case 1:
			tx.firmware = cpu_to_le16(0x160);
			break;
		case 2:
			tx.firmware = 0;
			break;
		case 3:
			tx.count = cpu_to_le16(33);
			break;
		case 4:
			tx.count = 0;
			break;
		case 5:
			tx.length = cpu_to_le16(45);
			break;
		case 6:
			tx.changes[0].selector = cpu_to_le16(0xffff);
			break;
		case 7:
			tx.changes[1].selector = tx.changes[0].selector;
			break;
		case 8:
			tx.changes[0].value = cpu_to_le16(1);
			break; /* positive master */
		case 9:
			mode = FMODE_READ;
			expected = -EACCES;
			break;
		case 10:
			f.input_fault = true;
			expected = -EFAULT;
			break;
		case 11:
			f.allocation_failure = true;
			expected = -ENOMEM;
			break;
		case 12:
			f.interrupted = true;
			expected = -ERESTARTSYS;
			break;
		}
		assert(apply(&tx, mode) == expected && f.reads == 0 && f.writes == 0);
	}
	if (pid != 0xaf93) {
		reset(pid);
		tx = transaction();
		tx.expected[51] = 1;
		assert(apply(&tx, FMODE_WRITE) == -EINVAL && f.reads == 0 && f.writes == 0);
	}
	for (unsigned int scenario = 0; scenario < 6; scenario++) {
		reset(pid);
		tx = transaction();
		long expected;
		switch (scenario) {
		case 0:
			f.devices_present = 0;
			expected = -ENODEV;
			break;
		case 1:
			f.devices_present = 2;
			expected = -EBUSY;
			break;
		case 2:
			tx.address = cpu_to_le16(3);
			expected = -ESTALE;
			break;
		case 3:
			tx.bus = cpu_to_le16(3);
			expected = -ESTALE;
			break;
		case 4:
			f.devices[0].descriptor.bcdDevice =
			    cpu_to_le16(pid == 0xaf93 ? 0x101 : 0x151);
			expected = -EPROTO;
			break;
		default:
			f.devices[0].descriptor.idProduct = cpu_to_le16(0xaf82);
			expected = -ENODEV;
		}
		assert(apply(&tx, FMODE_WRITE) == expected && f.reads == 0 && f.writes == 0);
	}
	reset(pid);
	tx = transaction();
	f.state[2] ^= 1;
	assert(apply(&tx, FMODE_WRITE) == -EAGAIN && f.writes == 0);
	assert(le16_to_cpu(tx.flags) == MFB_AFTER_VALID &&
	       !memcmp(tx.after, f.state, le16_to_cpu(tx.length)));
	reset(pid);
	mf4_put_word(1, f.state + 44);
	tx = transaction();
	assert(apply(&tx, FMODE_WRITE) == -EINVAL && f.writes == 0);
	reset(pid);
	tx = transaction();
	f.short_read = true;
	assert(apply(&tx, FMODE_WRITE) == -EPROTO && f.writes == 0 && tx.flags == 0);

	/* A failed write may already have taken effect: reverse every attempted write. */
	for (unsigned int failed = 1; failed <= 3; failed++) {
		for (unsigned int acknowledged = 0; acknowledged < 2; acknowledged++) {
			reset(pid);
			tx = transaction();
			f.write_failure = failed;
			f.apply_before_error = acknowledged;
			assert(apply(&tx, FMODE_WRITE) == -ETIMEDOUT);
			assert(le16_to_cpu(tx.flags) == 15 &&
			       le16_to_cpu(tx.completed) == failed - 1);
			assert(f.writes == failed * 2 &&
			       !memcmp(f.state, original, sizeof(original)));
			for (unsigned int i = 0; i < failed; i++)
				assert(f.selectors[failed + i] == f.selectors[failed - i - 1]);
		}
	}
	reset(pid);
	tx = transaction();
	f.ignore_writes = true;
	assert(apply(&tx, FMODE_WRITE) == -EIO && le16_to_cpu(tx.flags) == 15);
	assert(f.reads == 7 && f.sleeps == 6 && f.writes == 6);
	reset(pid);
	tx = transaction();
	f.external_change = 2;
	assert(apply(&tx, FMODE_WRITE) == -EIO && le16_to_cpu(tx.flags) == 13);
	assert(f.state[2] != original[2] && !memcmp(tx.after, f.state, le16_to_cpu(tx.length)));
	reset(pid);
	tx = transaction();
	f.write_failure = 2;
	f.rollback_failure = 3;
	assert(apply(&tx, FMODE_WRITE) == -ETIMEDOUT && le16_to_cpu(tx.flags) == 13);
	reset(pid);
	tx = transaction();
	f.write_failure = 2;
	f.read_failure = 2;
	assert(apply(&tx, FMODE_WRITE) == -ETIMEDOUT && le16_to_cpu(tx.flags) == 9);
	reset(pid);
	tx = transaction();
	f.output_fault = true;
	assert(apply(&tx, FMODE_WRITE) == -EFAULT && f.writes == 3);
	assert(!memcmp(f.state, predicted, sizeof(predicted))); /* reply lost, write happened */
}
static void meter_cases(u16 pid)
{
	struct mfl_meter_record record;
	reset(pid);
	memset(&record, 0xff, sizeof(record));
	f.peaks[3] = 0x40;
	assert(meters(&record, FMODE_READ) == 0 && f.meter_reads == 1 && f.writes == 0);
	assert(!memcmp(record.magic, "MFLM", 4) && le16_to_cpu(record.product) == pid);
	assert(record.firmware == f.devices[0].descriptor.bcdDevice && record.reserved == 0);
	assert(le16_to_cpu(record.length) == (pid == 0xaf93 ? 24 : 16));
	assert(record.data[3] == 0x40);
	for (unsigned int i = le16_to_cpu(record.length); i < 24; i++)
		assert(record.data[i] == 0);
	reset(pid);
	f.devices[0].descriptor.bcdDevice = 0;
	assert(meters(&record, FMODE_READ) == -EPROTO && f.meter_reads == 0);
	reset(pid);
	f.short_read = true;
	assert(meters(&record, FMODE_READ) == -EPROTO && f.meter_reads == 1);
	reset(pid);
	assert(meters(&record, FMODE_WRITE) == -EACCES && f.meter_reads == 0);
}
static void basic_case(u16 pid)
{
	struct file file = {.f_mode = FMODE_READ | FMODE_WRITE};
	struct mfb_record record;
	reset(pid);
	assert(mfb_read(&file, (unsigned long)(uintptr_t)&record) == 0);
	clean();
	cases++;
	assert(!memcmp(record.magic, "MFBS", 4) &&
	       le16_to_cpu(record.length) == mfb_state_length(pid));
	struct mfb_transaction tx = {.version = cpu_to_le16(1),
				     .bus = cpu_to_le16(1),
				     .address = cpu_to_le16(2),
				     .product = cpu_to_le16(pid),
				     .length = record.length,
				     .count = cpu_to_le16(1),
				     .changes = {{0, cpu_to_le16(1)}}};
	memcpy(tx.expected, record.data, le16_to_cpu(record.length));
	assert(mfb_apply(&file, (unsigned long)(uintptr_t)&tx) == 0);
	clean();
	cases++;
	assert(f.state[0] == 1 && f.state[1] == 0xa0 && le16_to_cpu(tx.flags) == 12);
}
int main(void)
{
	static const u16 models[] = {0xaf80, 0xaf90, 0xaf93};
	for (unsigned int i = 0; i < 3; i++) {
		native_cases(models[i]);
		meter_cases(models[i]);
		basic_case(models[i]);
	}
	printf("%u in-memory driver scenarios passed; no hardware access\n", cases);
	return 0;
}
