// SPDX-License-Identifier: MIT
use crate::protocol::{ApiError, Result};
use mf_core::{Device, Kernel, Meters, Model, ModuleCapabilities, Outcome, Plan, State, VID};
use std::{fs, os::unix::fs::MetadataExt, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub device: Device,
    pub model: Model,
    pub connection_marker: u64,
    pub firmware: Option<mf_core::FirmwareVersion>,
}
pub trait Hardware: Send + 'static {
    fn inventory(&mut self) -> Result<Vec<Descriptor>>;
    fn read(&mut self) -> Result<(Device, State)>;
    fn read_basic(&mut self, _descriptor: &Descriptor) -> Result<(Device, mf_core::BasicState)> {
        Err(ApiError::new(
            "unsupported_capability",
            "Model-specific switch validation is pending; experimental basic transport is disabled",
        ))
    }
    fn execute_basic(
        &mut self,
        _plan: &mf_core::BasicPlan,
        _backups: &Path,
    ) -> std::result::Result<mf_core::BasicOutcome, mf_core::Error> {
        Err(mf_core::Error::new(
            mf_core::ErrorKind::UnsupportedCapability,
            "Experimental model-specific switch transport is disabled",
        ))
    }
    fn module_capabilities(&mut self) -> Result<ModuleCapabilities> {
        Ok(ModuleCapabilities::default())
    }
    fn meters(&mut self) -> Result<(Device, Meters)> {
        Err(ApiError::new(
            "unsupported_capability",
            "Signal meters unavailable",
        ))
    }
    fn model_meters(&mut self, _descriptor: &Descriptor) -> Result<mf_core::ModelMeters> {
        Err(ApiError::new(
            "unsupported_capability",
            "Model-specific meters unavailable",
        ))
    }
    fn execute(
        &mut self,
        plan: &Plan,
        backups: &Path,
    ) -> std::result::Result<Outcome, mf_core::Error>;
}
fn experimental_enabled() -> bool {
    std::env::var("MF_SERVICE_EXPERIMENTAL_BASIC").as_deref() == Ok("1")
        || std::env::var("MF_SERVICE_EXPERIMENTAL_LOOPBACK").as_deref() == Ok("1")
}
pub struct Linux;
impl Hardware for Linux {
    fn read_basic(&mut self, descriptor: &Descriptor) -> Result<(Device, mf_core::BasicState)> {
        if !experimental_enabled() {
            return Err(ApiError::new(
                "unsupported_capability",
                "This model's hardware validation is pending. Experimental basic controls require an explicitly enabled test session.",
            ));
        }
        let kernel = Kernel::open(false)?;
        let mut result = kernel.read_basic()?;
        if std::env::var("MF_SERVICE_EXPERIMENTAL_LOOPBACK").as_deref() == Ok("1")
            && kernel.module_capabilities()?.basic_loopback
        {
            if let Some(firmware) = descriptor
                .firmware
                .filter(|f| f.supports_loopback(descriptor.device.product_id))
            {
                result.1 = result.1.enable_loopback(firmware)?;
            }
        }
        if result.0 != descriptor.device {
            return Err(ApiError::new(
                "device_changed",
                "Basic readback belongs to a different interface",
            ));
        }
        Ok(result)
    }
    fn execute_basic(
        &mut self,
        plan: &mf_core::BasicPlan,
        backups: &Path,
    ) -> std::result::Result<mf_core::BasicOutcome, mf_core::Error> {
        if !experimental_enabled()
            || (plan.before().loopback_enabled()
                && std::env::var("MF_SERVICE_EXPERIMENTAL_LOOPBACK").as_deref() != Ok("1"))
        {
            return Err(mf_core::Error::new(
                mf_core::ErrorKind::UnsupportedCapability,
                "Experimental basic controls are disabled",
            ));
        }
        mf_core::execute_basic(&mut Kernel::open(true)?, plan, backups)
    }
    fn model_meters(&mut self, descriptor: &Descriptor) -> Result<mf_core::ModelMeters> {
        if std::env::var("MF_SERVICE_EXPERIMENTAL_LOOPBACK").as_deref() != Ok("1") {
            return Err(ApiError::new(
                "unsupported_capability",
                "Experimental loopback meters are disabled",
            ));
        }
        let (device, firmware, meters) = Kernel::open(false)?.model_meters()?;
        if device != descriptor.device || Some(firmware) != descriptor.firmware {
            return Err(ApiError::new(
                "device_changed",
                "Meter identity or firmware changed",
            ));
        }
        Ok(meters)
    }
    fn inventory(&mut self) -> Result<Vec<Descriptor>> {
        let entries = fs::read_dir("/sys/bus/usb/devices")
            .map_err(|e| ApiError::new("device_unavailable", e.to_string()))?;
        let mut result = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let read = |name: &str, base| {
                fs::read_to_string(path.join(name))
                    .ok()
                    .and_then(|s| u16::from_str_radix(s.trim(), base).ok())
            };
            if read("idVendor", 16) != Some(VID) {
                continue;
            }
            let Some(pid) = read("idProduct", 16) else {
                continue;
            };
            let Some(model) = Model::from_pid(pid) else {
                continue;
            };
            let (Some(bus), Some(address)) = (read("busnum", 10), read("devnum", 10)) else {
                continue;
            };
            result.push(Descriptor {
                device: Device {
                    vendor_id: VID,
                    product_id: pid,
                    bus,
                    address,
                },
                model,
                connection_marker: fs::metadata(&path).map(|m| m.ino()).unwrap_or(0),
                firmware: read("bcdDevice", 16).and_then(mf_core::FirmwareVersion::from_bcd),
            });
        }
        result.sort_by_key(|d| (d.device.bus, d.device.address));
        Ok(result)
    }
    fn read(&mut self) -> Result<(Device, State)> {
        Kernel::open(false)?.read().map_err(Into::into)
    }
    fn module_capabilities(&mut self) -> Result<ModuleCapabilities> {
        Kernel::open(false)?
            .module_capabilities()
            .map_err(Into::into)
    }
    fn meters(&mut self) -> Result<(Device, Meters)> {
        Kernel::open(false)?.meters().map_err(Into::into)
    }
    fn execute(
        &mut self,
        plan: &Plan,
        backups: &Path,
    ) -> std::result::Result<Outcome, mf_core::Error> {
        mf_core::execute(&mut Kernel::open(true)?, plan, backups)
    }
}
