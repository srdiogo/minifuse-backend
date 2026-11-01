/* SPDX-License-Identifier: GPL-2.0-only */
/* Experimental model-specific controls. Never query initialization-mode IDs. */
struct mfb_search { struct usb_device *device; unsigned int count; };
static int mfb_find(struct usb_device *device, void *context)
{
	struct mfb_search *s = context;
	if (le16_to_cpu(device->descriptor.idVendor) != MF4_VID ||
	    !mfb_state_length(le16_to_cpu(device->descriptor.idProduct)))
		return 0;
	s->count++;
	if (!s->device)
		s->device = usb_get_dev(device);
	return 0;
}
static int mfb_status(struct usb_device *device, u8 *data, unsigned int length)
{
	int result = usb_control_msg(device, usb_rcvctrlpipe(device, 0), 0xb2,
		USB_DIR_IN | USB_TYPE_CLASS | USB_RECIP_INTERFACE, 0, 0, data, length, 750);
	return result < 0 ? result : result == length ? 0 : -EPROTO;
}
static long mfb_read(struct file *file, unsigned long arg)
{
	struct mfb_search s = {};
	struct mfb_record *r;
	int result;
	unsigned int length;
	if (!(file->f_mode & FMODE_READ))
		return -EACCES;
	r = kzalloc(sizeof(*r), GFP_KERNEL);
	if (!r)
		return -ENOMEM;
	if (mutex_lock_interruptible(&mf4_query_mutex)) { kfree(r); return -ERESTARTSYS; }
	usb_for_each_dev(&s, mfb_find);
	if (s.count != 1) { result = s.device ? -EBUSY : -ENODEV; goto done; }
	length = mfb_state_length(le16_to_cpu(s.device->descriptor.idProduct));
	memcpy(r->magic, "MFBS", 4);
	r->version = cpu_to_le16(1);
	r->vendor = cpu_to_le16(MF4_VID);
	r->product = s.device->descriptor.idProduct;
	r->bus = cpu_to_le16(s.device->bus->busnum);
	r->address = cpu_to_le16(s.device->devnum);
	r->length = cpu_to_le16(length);
	result = mfb_status(s.device, r->data, length);
	if (!result && copy_to_user((void __user *)arg, r, sizeof(*r)))
		result = -EFAULT;
done:
	if (s.device) usb_put_dev(s.device);
	mutex_unlock(&mf4_query_mutex);
	kfree(r);
	return result;
}
static int mfb_wait(struct usb_device *device, struct mfb_transaction *tx, const u8 *expected, unsigned int length)
{
	int result, attempt;
	u16 flags = le16_to_cpu(tx->flags) & ~MFB_AFTER_VALID;
	tx->flags = cpu_to_le16(flags);
	for (attempt = 0; attempt < 5; attempt++) {
		msleep(20);
		tx->flags = cpu_to_le16(flags);
		result = mfb_status(device, tx->after, length);
		if (result) return result;
		tx->flags = cpu_to_le16(flags | MFB_AFTER_VALID);
		if (!memcmp(tx->after, expected, length)) return 0;
	}
	return -EIO;
}
struct mfb_work {
	struct mfb_transaction tx;
	u8 before[MFB_MAX_STATE], predicted[MFB_MAX_STATE];
	__le16 originals[MFB_MAX_CHANGES];
};
static long mfb_apply(struct file *file, unsigned long arg)
{
	struct mfb_search s = {};
	struct mfb_work *w;
	struct mfb_transaction *tx;
	unsigned int count, length, i, j, attempted = 0;
	u16 pid, selector, offset, mask, value, old, flags;
	int result, failure, undo = 0;
	if (!(file->f_mode & FMODE_WRITE)) return -EACCES;
	w = kzalloc(sizeof(*w), GFP_KERNEL);
	if (!w) return -ENOMEM;
	tx = &w->tx;
	if (copy_from_user(tx, (void __user *)arg, sizeof(*tx))) { result = -EFAULT; goto free; }
	tx->flags = 0; tx->completed = 0; memset(tx->after, 0, sizeof(tx->after));
	pid = le16_to_cpu(tx->product); length = mfb_state_length(pid); count = le16_to_cpu(tx->count);
	if (le16_to_cpu(tx->version) != 1 || !length || le16_to_cpu(tx->length) != length ||
	    !count || count > MFB_MAX_CHANGES || memchr_inv(tx->expected + length, 0, MFB_MAX_STATE - length)) {
		result = -EINVAL; goto reply;
	}
	for (i = 0; i < count; i++) {
		selector = le16_to_cpu(tx->changes[i].selector);
		result = mfb_target(pid, selector, le16_to_cpu(tx->changes[i].value), &offset, &mask);
		if (result) goto reply;
		for (j = 0; j < i; j++) if (tx->changes[j].selector == tx->changes[i].selector) {
			result = -EINVAL; goto reply;
		}
	}
	if (mutex_lock_interruptible(&mf4_query_mutex)) { result = -ERESTARTSYS; goto reply; }
	usb_for_each_dev(&s, mfb_find);
	if (s.count != 1) { result = s.device ? -EBUSY : -ENODEV; goto unlock; }
	if (le16_to_cpu(s.device->descriptor.idProduct) != pid || s.device->bus->busnum != le16_to_cpu(tx->bus) || s.device->devnum != le16_to_cpu(tx->address)) {
		result = -ESTALE; goto unlock;
	}
	result = mfb_status(s.device, w->before, length);
	if (result) goto unlock;
	memcpy(tx->after, w->before, length); tx->flags = cpu_to_le16(MFB_AFTER_VALID);
	if (memcmp(tx->expected, w->before, length)) { result = -EAGAIN; goto unlock; }
	memcpy(w->predicted, w->before, length);
	for (i = 0; i < count; i++) {
		selector = le16_to_cpu(tx->changes[i].selector); value = le16_to_cpu(tx->changes[i].value);
		result = mfb_target(pid, selector, value, &offset, &mask); if (result) goto unlock;
		old = mf4_get_word(w->before + offset); w->originals[i] = cpu_to_le16(!!(old & mask));
		old = mf4_get_word(w->predicted + offset);
		mf4_put_word(value ? old | mask : old & ~mask, w->predicted + offset);
	}
	for (i = 0; i < count; i++) {
		attempted = i + 1;
		tx->flags = cpu_to_le16((le16_to_cpu(tx->flags) | MFB_WRITE_ATTEMPTED) & ~MFB_AFTER_VALID);
		result = mf4_set(s.device, le16_to_cpu(tx->changes[i].selector), &tx->changes[i].value);
		if (result) goto rollback;
		tx->completed = cpu_to_le16(i + 1);
	}
	result = mfb_wait(s.device, tx, w->predicted, length);
	if (!result) goto unlock;
rollback:
	failure = result;
	tx->flags = cpu_to_le16(le16_to_cpu(tx->flags) | 1);
	for (i = attempted; i > 0; i--) {
		result = mf4_set(s.device, le16_to_cpu(tx->changes[i-1].selector), &w->originals[i-1]);
		if (result) undo = result;
	}
	result = mfb_wait(s.device, tx, w->before, length);
	flags = le16_to_cpu(tx->flags);
	if (!result && !undo) flags |= 2;
	tx->flags = cpu_to_le16(flags);
	result = failure;
unlock:
	if (s.device) usb_put_dev(s.device);
	mutex_unlock(&mf4_query_mutex);
reply:
	if (copy_to_user((void __user *)arg, tx, sizeof(*tx))) result = -EFAULT;
free:
	kfree(w);
	return result;
}
