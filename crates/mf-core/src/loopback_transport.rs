// SPDX-License-Identifier: MIT
use crate::{
    BasicFailure, BasicPlan, BasicState, Device, Error, ErrorKind, FirmwareVersion, Kernel,
    ModelMeters, VID,
};
use std::os::fd::AsRawFd;
fn word(d: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([d[i], d[i + 1]])
}
fn put(d: &mut [u8], i: usize, v: u16) {
    d[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
impl Kernel {
    pub(crate) fn transact_loopback(
        &mut self,
        plan: &BasicPlan,
    ) -> Result<BasicState, BasicFailure> {
        let firmware = plan
            .before
            .firmware()
            .ok_or_else(|| BasicFailure::from(Error::from("Missing loopback firmware metadata")))?;
        if !self
            .module_capabilities()
            .map_err(BasicFailure::from)?
            .basic_loopback
            || !firmware.supports_loopback(plan.device.product_id)
            || plan.changes.is_empty()
            || plan.changes.len() > 32
        {
            return Err(Error::new(
                ErrorKind::UnsupportedCapability,
                "Firmware-gated loopback transactions are unavailable",
            )
            .into());
        }
        let mut data = [0u8; 250];
        put(&mut data, 12, firmware.raw_bcd);
        put(&mut data, 0, 1);
        put(&mut data, 2, plan.device.bus);
        put(&mut data, 4, plan.device.address);
        put(&mut data, 6, plan.device.product_id);
        put(&mut data, 8, plan.before.data.len() as u16);
        put(&mut data, 10, plan.changes.len() as u16);
        data[14..14 + plan.before.data.len()].copy_from_slice(&plan.before.data);
        for (i, c) in plan.changes.iter().enumerate() {
            put(&mut data, 66 + i * 4, c.selector);
            put(&mut data, 68 + i * 4, c.value);
        }
        let request = libc::_IOWR::<[u8; 250]>(b'M' as u32, 0x46);
        let result = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request as libc::c_ulong,
                data.as_mut_ptr(),
            )
        };
        let failure = (result < 0).then(std::io::Error::last_os_error);
        let flags = word(&data, 248);
        let after = if flags & 4 != 0 {
            Some(
                BasicState::new(
                    plan.device.product_id,
                    &data[194..194 + plan.before.data.len()],
                )
                .and_then(|s| s.enable_loopback(firmware))
                .map_err(BasicFailure::from)?,
            )
        } else {
            None
        };
        if let Some(io) = failure {
            let reply_lost = io.raw_os_error() == Some(libc::EFAULT);
            let mut error = Error::io(io, "Firmware-gated loopback transaction failed");
            error.device_write_attempted = if reply_lost {
                None
            } else {
                Some(flags & 8 != 0)
            };
            error.rollback_attempted = flags & 1 != 0;
            error.rollback_verified = flags & 2 != 0;
            return Err(BasicFailure { error, after });
        }
        if word(&data, 246) as usize != plan.changes.len()
            || flags & 8 == 0
            || after.as_ref() != Some(&plan.predicted)
        {
            let mut error = Error::new(
                ErrorKind::ReadbackFailed,
                "Loopback transaction did not return complete verified state",
            );
            error.device_write_attempted = Some(true);
            return Err(BasicFailure { error, after });
        }
        Ok(after.unwrap())
    }
    pub fn model_meters(&self) -> Result<(Device, FirmwareVersion, ModelMeters), Error> {
        if !self.module_capabilities()?.basic_meters {
            return Err(Error::new(
                ErrorKind::UnsupportedCapability,
                "The module lacks model-specific meters",
            ));
        }
        let mut data = [0u8; 44];
        let request = libc::_IOR::<[u8; 44]>(b'M' as u32, 0x47);
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
                "Cannot read model-specific meters",
            ));
        }
        decode_meters(&data)
    }
}

fn decode_meters(data: &[u8]) -> Result<(Device, FirmwareVersion, ModelMeters), Error> {
    if data.len() != 44
        || &data[..4] != b"MFLM"
        || word(data, 4) != 1
        || word(data, 6) != VID
        || word(data, 18) != 0
    {
        return Err(Error::new(
            ErrorKind::ModuleIncompatible,
            "Invalid model meter record",
        ));
    }
    let pid = word(data, 8);
    let firmware = FirmwareVersion::from_bcd(word(data, 16))
        .filter(|v| v.supports_loopback(pid))
        .ok_or_else(|| {
            Error::new(
                ErrorKind::UnsupportedCapability,
                "Incompatible meter firmware",
            )
        })?;
    let length = if pid == 0xaf93 { 24 } else { 16 };
    if word(data, 14) as usize != length || data[20 + length..].iter().any(|v| *v != 0) {
        return Err(Error::new(
            ErrorKind::ReadbackFailed,
            "Invalid meter length or padding",
        ));
    }
    Ok((
        Device {
            vendor_id: VID,
            product_id: pid,
            bus: word(data, 10),
            address: word(data, 12),
        },
        firmware,
        ModelMeters::decode(pid, &data[20..20 + length])?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meter_records_require_identity_firmware_shape_and_zero_padding() {
        for pid in [0xaf80, 0xaf90, 0xaf93] {
            let mut data = [0u8; 44];
            data[..4].copy_from_slice(b"MFLM");
            for (offset, value) in [
                (4, 1),
                (6, VID),
                (8, pid),
                (10, 2),
                (12, 3),
                (14, if pid == 0xaf93 { 24 } else { 16 }),
                (16, if pid == 0xaf93 { 0x100 } else { 0x152 }),
            ] {
                put(&mut data, offset, value);
            }
            let (device, firmware, meters) = decode_meters(&data).unwrap();
            assert_eq!(device.product_id, pid);
            assert!(firmware.supports_loopback(pid));
            assert_eq!(meters.inputs_dbfs.len(), if pid == 0xaf80 { 1 } else { 2 });
            for (offset, value) in [
                (0, 0),
                (4, 2),
                (6, 0),
                (8, 0xaf82),
                (14, 20),
                (16, 0x149),
                (18, 1),
            ] {
                let mut bad = data;
                put(&mut bad, offset, value);
                assert!(decode_meters(&bad).is_err(), "{pid:x}:{offset}");
            }
            if pid != 0xaf93 {
                let mut bad = data;
                bad[43] = 1;
                assert!(decode_meters(&bad).is_err());
            }
            assert!(decode_meters(&data[..43]).is_err());
        }
    }
}
