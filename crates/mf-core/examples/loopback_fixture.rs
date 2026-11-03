// SPDX-License-Identifier: MIT
//! Regenerate synthetic frontend fixtures without opening a device or socket.
use mf_core::{
    BasicState, Device, FirmwareVersion, Level, Mix, MixCapabilities, Model, ModelMeters,
    Operation, VID,
};
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut devices = Vec::new();
    for pid in [0xaf80, 0xaf90, 0xaf93] {
        let model = Model::from_pid(pid).unwrap();
        let device = Device {
            vendor_id: VID,
            product_id: pid,
            bus: 1,
            address: 2,
        };
        let firmware =
            FirmwareVersion::from_bcd(if pid == 0xaf93 { 0x100 } else { 0x152 }).unwrap();
        let state = BasicState::new(pid, &vec![0; if pid == 0xaf93 { 52 } else { 46 }])?
            .enable_loopback(firmware)?;
        let names = state.loopback_sources();
        let mut changes: Vec<_> = names
            .iter()
            .map(|source| Operation::Channel {
                mix: Mix::Loopback,
                source: (*source).into(),
                level: Some(Level::Db(-12.0)),
                pan: None,
                mute: None,
                solo: None,
            })
            .collect();
        changes.push(Operation::Master {
            mix: Mix::Loopback,
            level: Level::Db(-6.0),
        });
        changes.push(Operation::LoopbackRouting {
            enabled: Some(true),
            usb12: None,
            monitor: None,
        });
        let plan = mf_core::plan_basic(&device, &state, &changes)?;
        let mut profile = model.profile(pid).unwrap();
        profile.layout = "loopback_mixer";
        let mut caps = model.capabilities();
        caps.mixer = true;
        caps.host_snapshots = true;
        caps.signal_meters = true;
        caps.input_switches = true;
        caps.switch_readback = true;
        caps.mixes = vec![MixCapabilities {
            mix: Mix::Loopback,
            sources: names,
            routes: vec!["mix"],
            membership: vec![],
        }];
        let id = format!("synthetic-{}", profile.id);
        let raw: Vec<u8> = (0..if pid == 0xaf93 { 6 } else { 4 })
            .flat_map(|i| (0x40000000i32 >> (i % 4)).to_le_bytes())
            .collect();
        let meters = ModelMeters::decode(pid, &raw)?;
        devices.push(json!({"device":{"device_id":id,"generation":"synthetic-generation","model":model.name(),"product_id":pid,"connection":"ready","profile":profile,"firmware":mf_core::firmware_status(pid,Some(firmware)),"capabilities":caps},"state":{"device_id":id,"generation":"synthetic-generation","revision":"synthetic-revision","capabilities":caps,"state":plan.predicted().decoded()},"meters":{"device_id":id,"generation":"synthetic-generation","meter_layout":"minifuse-basic-c2-v1","available":true,"inputs_dbfs":meters.inputs_dbfs,"loopback_dbfs":meters.loopback_dbfs,"otg_dbfs":meters.otg_dbfs}}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema_version":1,"evidence_kind":"synthetic","hardware_validation":"pending","devices":devices})
        )?
    );
    Ok(())
}
