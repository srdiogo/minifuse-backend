// SPDX-License-Identifier: MIT
//! Explicit read-only evidence capture for experimental normal-mode models.
use mf_core::{Device, FirmwareVersion, Kernel};
use serde_json::json;
use std::{fs, path::Path};

fn firmware(device: &Device) -> Option<FirmwareVersion> {
    for entry in fs::read_dir("/sys/bus/usb/devices").ok()?.flatten() {
        let read = |name: &str, radix| {
            fs::read_to_string(entry.path().join(Path::new(name)))
                .ok()
                .and_then(|s| u16::from_str_radix(s.trim(), radix).ok())
        };
        if read("idVendor", 16) == Some(device.vendor_id)
            && read("idProduct", 16) == Some(device.product_id)
            && read("busnum", 10) == Some(device.bus)
            && read("devnum", 10) == Some(device.address)
        {
            return read("bcdDevice", 16).and_then(FirmwareVersion::from_bcd);
        }
    }
    None
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!(
            "Read one model-specific B2 status record; never write controls.\nEnable with MF_SERVICE_EXPERIMENTAL_BASIC=1 or MF_SERVICE_EXPERIMENTAL_LOOPBACK=1.\nThe loopback opt-in decodes the native mixer only for a compatible confirmed firmware family.\nOptional --meters reads one model-specific C2 peak record, with the loopback opt-in.\nConnect one normal-mode MiniFuse 1, 2 or 2 OTG and install the matching test module."
        );
        return Ok(());
    }
    let native = std::env::var("MF_SERVICE_EXPERIMENTAL_LOOPBACK").as_deref() == Ok("1");
    let want_meters = args == ["--meters"];
    if (!args.is_empty() && !want_meters)
        || (!native
            && (want_meters
                || std::env::var("MF_SERVICE_EXPERIMENTAL_BASIC").as_deref() != Ok("1")))
    {
        return Err("Read-only model testing requires an explicit opt-in; see --help".into());
    }
    let kernel = Kernel::open(false)?;
    let (device, mut state) = kernel.read_basic()?;
    let version = firmware(&device);
    if native && kernel.module_capabilities()?.basic_loopback {
        if let Some(version) = version.filter(|v| v.supports_loopback(device.product_id)) {
            state = state.enable_loopback(version)?;
        }
    }
    let meters = if want_meters {
        if !state.loopback_enabled() {
            return Err(
                "Meters require compatible confirmed firmware and the matching loopback module"
                    .into(),
            );
        }
        let (identity, version_read, meters) = kernel.model_meters()?;
        if identity != device || Some(version_read) != version {
            return Err("Meter identity or firmware changed during capture".into());
        }
        Some(meters)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version":1,"evidence_kind":"hardware_readback","hardware_validation":"pending",
            "device_write_attempted":false,"device":device,"firmware":mf_core::firmware_status(device.product_id,version),
            "state_length":state.raw().len(),"payload_hex":mf_core::hex(state.raw()),"state":state.decoded(),"meters":meters
        }))?
    );
    Ok(())
}
