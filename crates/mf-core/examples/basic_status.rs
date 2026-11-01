// SPDX-License-Identifier: MIT
//! Explicit read-only evidence capture for experimental normal-mode models.
use mf_core::Kernel;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).any(|arg| arg == "--help") {
        println!(
            "Read one model-specific basic status record; never write controls.\nEnable this test explicitly with MF_SERVICE_EXPERIMENTAL_BASIC=1.\nConnect one normal-mode MiniFuse 1, 2 or 2 OTG and install the matching test module."
        );
        return Ok(());
    }
    if std::env::args().len() != 1
        || std::env::var("MF_SERVICE_EXPERIMENTAL_BASIC").as_deref() != Ok("1")
    {
        return Err(
            "Read-only model testing requires MF_SERVICE_EXPERIMENTAL_BASIC=1; see --help".into(),
        );
    }
    let (device, state) = Kernel::open(false)?.read_basic()?;
    let payload = mf_core::hex(state.raw());
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version":1, "evidence_kind":"hardware_readback",
            "hardware_validation":"pending", "device_write_attempted":false,
            "device":device, "state_length":state.raw().len(),
            "payload_hex":payload, "state":state.decoded()
        }))?
    );
    Ok(())
}
