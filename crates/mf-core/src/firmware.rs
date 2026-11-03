// SPDX-License-Identifier: MIT
//! Read-only compatibility decisions from the USB device descriptor.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub raw_bcd: u16,
}
impl FirmwareVersion {
    pub fn from_bcd(raw: u16) -> Option<Self> {
        let major = (raw >> 8) as u8;
        let minor = ((raw >> 4) & 15) as u8;
        let patch = (raw & 15) as u8;
        if major == 0 || major > 9 || minor > 9 || patch > 9 {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
            raw_bcd: raw,
        })
    }
    pub fn display(self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
    /// Candidate families supported by the inspected host layout, pending hardware acceptance.
    pub fn supports_loopback(self, pid: u16) -> bool {
        Self::from_bcd(self.raw_bcd) == Some(self)
            && matches!(
                (pid, self.major, self.minor),
                (0xaf80 | 0xaf90, 1, 5) | (0xaf93, 1, 0)
            )
    }
}
pub fn firmware_status(pid: u16, version: Option<FirmwareVersion>) -> serde_json::Value {
    serde_json::json!({
        "version":version.map(FirmwareVersion::display), "raw_bcd":version.map(|v|v.raw_bcd),
        "source":"usb_device_descriptor", "loopback_compatible":version.is_some_and(|v|v.supports_loopback(pid)),
        "required_family":match pid {0xaf80|0xaf90=>Some("1.5.x"),0xaf93=>Some("1.0.x"),_=>None},
        "hardware_validation":if matches!(pid,0xaf80|0xaf90|0xaf93){Some("pending")}else{None}
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatibility_is_model_specific_and_unknown_versions_fail_closed() {
        for raw in [0, 0xffff, 0x15a, 0x1a0, 0xa00] {
            assert!(FirmwareVersion::from_bcd(raw).is_none());
        }
        for pid in [0xaf80, 0xaf90] {
            for raw in [0x150, 0x151, 0x152, 0x159] {
                assert!(
                    FirmwareVersion::from_bcd(raw)
                        .unwrap()
                        .supports_loopback(pid)
                );
            }
            for raw in [0x100, 0x120, 0x149, 0x160, 0x200] {
                assert!(
                    !FirmwareVersion::from_bcd(raw)
                        .unwrap()
                        .supports_loopback(pid)
                );
            }
        }
        assert!(
            FirmwareVersion::from_bcd(0x100)
                .unwrap()
                .supports_loopback(0xaf93)
        );
        assert!(
            !FirmwareVersion::from_bcd(0x150)
                .unwrap()
                .supports_loopback(0xaf93)
        );
        assert!(
            !FirmwareVersion::from_bcd(0x150)
                .unwrap()
                .supports_loopback(0xaf82)
        );
    }
}
