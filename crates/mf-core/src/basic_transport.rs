// SPDX-License-Identifier: MIT
use crate::{
    BasicBackend, BasicFailure, BasicPlan, BasicState, Device, Error, ErrorKind, Kernel, VID,
    basic::state_length,
};
use std::os::fd::AsRawFd;
const STATUS: usize = 68;
const TX: usize = 136;
fn word(data: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([data[i], data[i + 1]])
}
fn put(data: &mut [u8], i: usize, v: u16) {
    data[i..i + 2].copy_from_slice(&v.to_le_bytes());
}

pub(crate) fn decode(data: &[u8]) -> Result<(Device, BasicState), Error> {
    if data.len() != STATUS || &data[..4] != b"MFBS" || word(data, 4) != 1 || word(data, 6) != VID {
        return Err(Error::new(
            ErrorKind::ModuleIncompatible,
            "Invalid basic control status header",
        ));
    }
    let pid = word(data, 8);
    let length = usize::from(word(data, 14));
    if state_length(pid) != Some(length) || data[16 + length..].iter().any(|b| *b != 0) {
        return Err(Error::new(
            ErrorKind::ModuleIncompatible,
            "Invalid basic model, length or padding",
        ));
    }
    let device = Device {
        vendor_id: VID,
        product_id: pid,
        bus: word(data, 10),
        address: word(data, 12),
    };
    Ok((device, BasicState::new(pid, &data[16..16 + length])?))
}

impl Kernel {
    pub fn read_basic(&self) -> Result<(Device, BasicState), Error> {
        if !self.module_capabilities()?.basic_switches {
            return Err(Error::new(
                ErrorKind::UnsupportedCapability,
                "The module lacks the experimental model-specific switch transport",
            ));
        }
        let mut data = [0u8; STATUS];
        let request = libc::_IOR::<[u8; STATUS]>(b'M' as u32, 0x44);
        let result = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request as libc::c_ulong,
                data.as_mut_ptr(),
            )
        };
        if result < 0 {
            return Err(Error::io(
                std::io::Error::last_os_error(),
                "Cannot read model-specific basic state",
            ));
        }
        decode(&data)
    }
}
impl BasicBackend for Kernel {
    fn transact_basic(&mut self, plan: &BasicPlan) -> Result<BasicState, BasicFailure> {
        if plan.before.loopback_enabled() {
            return self.transact_loopback(plan);
        }
        if !self
            .module_capabilities()
            .map_err(BasicFailure::from)?
            .basic_switches
        {
            return Err(Error::new(
                ErrorKind::UnsupportedCapability,
                "The module lacks model-specific switch transactions",
            )
            .into());
        }
        if plan.changes.is_empty() || plan.changes.len() > 4 {
            return Err(Error::from("Invalid basic transaction count").into());
        }
        let mut data = [0u8; TX];
        put(&mut data, 0, 1);
        put(&mut data, 2, plan.device.bus);
        put(&mut data, 4, plan.device.address);
        put(&mut data, 6, plan.device.product_id);
        put(&mut data, 8, plan.before.data.len() as u16);
        put(&mut data, 10, plan.changes.len() as u16);
        data[12..12 + plan.before.data.len()].copy_from_slice(&plan.before.data);
        for (i, c) in plan.changes.iter().enumerate() {
            put(&mut data, 64 + i * 4, c.selector);
            put(&mut data, 66 + i * 4, c.value);
        }
        let request = libc::_IOWR::<[u8; TX]>(b'M' as u32, 0x45);
        let result = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request as libc::c_ulong,
                data.as_mut_ptr(),
            )
        };
        let flags = word(&data, 134);
        let after = if flags & 4 != 0 {
            Some(
                BasicState::new(
                    plan.device.product_id,
                    &data[80..80 + plan.before.data.len()],
                )
                .map_err(BasicFailure::from)?,
            )
        } else {
            None
        };
        if result < 0 {
            let io = std::io::Error::last_os_error();
            let reply_lost = io.raw_os_error() == Some(libc::EFAULT);
            let mut error = Error::io(io, "Model-specific switch transaction failed");
            error.device_write_attempted = if reply_lost {
                None
            } else {
                Some(flags & 8 != 0)
            };
            error.rollback_attempted = flags & 1 != 0;
            error.rollback_verified = flags & 2 != 0;
            return Err(BasicFailure { error, after });
        }
        if word(&data, 132) as usize != plan.changes.len()
            || flags & 8 == 0
            || after.as_ref() != Some(&plan.predicted)
        {
            let mut error = Error::new(
                ErrorKind::ReadbackFailed,
                "Basic transaction did not return complete verified state",
            );
            error.device_write_attempted = Some(true);
            return Err(BasicFailure { error, after });
        }
        Ok(after.unwrap())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_never_accepts_mf4_storage_modes_wrong_size_or_padding() {
        for pid in [0xaf80, 0xaf90, 0xaf93] {
            let mut data = [0u8; STATUS];
            data[..4].copy_from_slice(b"MFBS");
            put(&mut data, 4, 1);
            put(&mut data, 6, VID);
            put(&mut data, 8, pid);
            put(&mut data, 14, state_length(pid).unwrap() as u16);
            assert!(decode(&data).is_ok());
            if pid != 0xaf93 {
                data[67] = 1;
                assert!(decode(&data).is_err());
                data[67] = 0;
            }
            put(&mut data, 14, 65535);
            assert!(decode(&data).is_err());
            put(&mut data, 14, state_length(pid).unwrap() as u16);
            put(&mut data, 8, 0xaf82);
            assert!(decode(&data).is_err());
        }
        assert!(decode(&[0; 206]).is_err());
    }
    #[test]
    fn basic_ioctl_sizes_are_independent_of_mf4() {
        assert_eq!(
            (libc::_IOR::<[u8; STATUS]>(b'M' as u32, 0x44) >> 16) & 0x3fff,
            68
        );
        assert_eq!(
            (libc::_IOWR::<[u8; TX]>(b'M' as u32, 0x45) >> 16) & 0x3fff,
            136
        );
    }
}
