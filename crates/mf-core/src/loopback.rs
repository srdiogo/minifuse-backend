// SPDX-License-Identifier: MIT
//! Experimental hardware loopback; independent of the Windows SLS mixer.
use crate::basic::BasicChange;
use crate::state::{decode_db, level_pan};
use crate::{
    BasicPlan, BasicState, Device, Error, ErrorKind, FirmwareVersion, Level, Mix, Operation,
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize)]
pub struct LoopbackRouting {
    pub enabled: bool,
    pub usb12: bool,
    pub monitor: bool,
}

pub(crate) fn sources(pid: u16) -> Vec<(&'static str, u16, Option<u16>)> {
    let mut result = vec![("analog1", 0, None)];
    if pid != 0xaf80 {
        result.push(("analog2", 1, None));
    }
    result.push(("usb12", 2, Some(3)));
    result.push((if pid == 0xaf93 { "usb56" } else { "usb34" }, 4, Some(5)));
    result
}

/// Offsets are parameter enum * 2, not selector * 2.
pub(crate) fn target(pid: u16, selector: u16, value: u16) -> Result<(usize, u16), Error> {
    let family = selector >> 8;
    let ch = selector & 255;
    let signed = value as i16;
    let present = ch < 6 && !(pid == 0xaf80 && ch == 1);
    if !matches!(pid, 0xaf80 | 0xaf90 | 0xaf93) {
        return Err(Error::from("Unsupported loopback model"));
    }
    match family {
        0x06 | 0x0c if present && signed <= 12 * 256 => {
            Ok(((family as usize + ch as usize) * 2, 0))
        }
        0x12 if present && value <= 1 => Ok((36, 1 << ch)),
        0x13..=0x15 if ch == 0 && value <= 1 => Ok((family as usize * 2, 1)),
        0x16 if ch == 0 && signed <= 0 && (signed >= -64 * 256 || signed == i16::MIN) => {
            Ok((44, 0))
        }
        0x17 | 0x19 if pid == 0xaf93 && ch == 0 && value <= 1 => Ok((family as usize * 2, 0)),
        0x18 if pid == 0xaf93 && ch == 0 && value <= 3 => Ok((48, 0)),
        _ => crate::basic::target(pid, selector, value),
    }
}

impl BasicState {
    pub fn enable_loopback(mut self, firmware: FirmwareVersion) -> Result<Self, Error> {
        if !firmware.supports_loopback(self.pid) {
            return Err(Error::new(
                ErrorKind::UnsupportedCapability,
                "Firmware is outside this model's experimental loopback family",
            ));
        }
        self.firmware = Some(firmware);
        // Reject malformed mapped gains before displaying or restoring them.
        for (_, left, right) in sources(self.pid) {
            for selector in if let Some(right) = right {
                vec![0x600 | left, 0xc00 | right]
            } else {
                vec![0x600 | left, 0xc00 | left]
            } {
                let offset = ((selector >> 8) + (selector & 255)) as usize * 2;
                target(self.pid, selector, self.word(offset))?;
            }
        }
        target(self.pid, 0x1600, self.word(44))?;
        if self.pid == 0xaf93 {
            for selector in [0x1700, 0x1800, 0x1900] {
                target(self.pid, selector, self.word((selector >> 8) as usize * 2))?;
            }
        }
        Ok(self)
    }
    pub fn loopback_enabled(&self) -> bool {
        self.firmware.is_some_and(|f| f.supports_loopback(self.pid))
    }
    pub fn loopback_sources(&self) -> Vec<&'static str> {
        sources(self.pid)
            .into_iter()
            .map(|(name, _, _)| name)
            .collect()
    }
    pub fn firmware(&self) -> Option<FirmwareVersion> {
        self.firmware
    }
    pub(crate) fn word(&self, i: usize) -> u16 {
        u16::from_le_bytes([self.data[i], self.data[i + 1]])
    }
    pub(crate) fn loopback_view(&self) -> Value {
        let channels: Vec<_> = sources(self.pid)
            .into_iter()
            .map(|(name, left, right)| {
                let left_db = decode_db(self.word((6 + left) as usize * 2));
                let right_db = decode_db(self.word((12 + right.unwrap_or(left)) as usize * 2));
                let (level, pan) = if right.is_some() {
                    (left_db, None)
                } else {
                    level_pan(left_db, right_db)
                };
                let left_muted = self.word(36) & (1 << left) != 0;
                let right_muted = right
                    .map(|i| self.word(36) & (1 << i) != 0)
                    .unwrap_or(left_muted);
                json!({
                    "channel": name, "db_left": left_db, "db_right": right_db,
                    "derived_level_db": level, "derived_pan": pan,
                    "mute": left_muted || right_muted, "mute_mixed": left_muted != right_muted,
                    "level_mixed": right.is_some() && left_db != right_db, "solo": false,
                    "mono_included": true, "stereo_pair_included": right.is_some(),
                    "pan_available": right.is_none()
                })
            })
            .collect();
        let routing = LoopbackRouting {
            enabled: self.word(38) & 1 != 0,
            usb12: self.word(40) & 1 != 0,
            monitor: self.word(42) & 1 != 0,
        };
        json!({"mixes":[{"mix":"loopback","master_db":decode_db(self.word(44)),"route":"mix","channels":channels}],
            "loopback":routing,"firmware":crate::firmware_status(self.pid,self.firmware),
            "otg":if self.pid==0xaf93 {json!({"otg_to_monitor":self.word(46)==1,"meter_source":self.word(48),"main_to_otg":self.word(50)})}else{Value::Null}})
    }
}

fn level(value: Level, low: f64, high: f64) -> Result<u16, Error> {
    match value {
        Level::Silence => Ok(0x8000),
        Level::Db(db) => crate::mixer::db_word(&db.to_string(), low, high).map_err(Error::from),
    }
}
pub(crate) fn operation(state: &BasicState, op: &Operation) -> Result<Vec<BasicChange>, Error> {
    if !state.loopback_enabled() {
        return Err(Error::new(
            ErrorKind::UnsupportedCapability,
            "Loopback requires compatible confirmed firmware and an explicitly enabled test session",
        ));
    }
    let mut out = Vec::new();
    let mut add = |selector, value| out.push(BasicChange { selector, value });
    match op {
        Operation::Channel {
            mix: Mix::Loopback,
            source,
            level: db,
            pan,
            mute,
            solo,
        } => {
            if solo.is_some() {
                return Err(Error::from("Hardware loopback has no solo control"));
            }
            let (_, left, right) = sources(state.pid)
                .into_iter()
                .find(|(n, _, _)| n == source)
                .ok_or_else(|| Error::from("Unsupported loopback source for this model"))?;
            if db.is_some() || pan.is_some() {
                if let Some(right) = right {
                    if pan.is_some() {
                        return Err(Error::from(
                            "Stereo playback pairs do not expose a pan control",
                        ));
                    }
                    let value = level(db.unwrap(), -60.0, 12.0)?;
                    add(0x600 | left, value);
                    add(0xc00 | right, value);
                } else {
                    let (old, p) = level_pan(
                        decode_db(state.word((6 + left) as usize * 2)),
                        decode_db(state.word((12 + left) as usize * 2)),
                    );
                    let p = pan.or(p).unwrap_or(0.5);
                    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
                        return Err(Error::from("Pan must be 0..1"));
                    }
                    let gain = match db {
                        Some(v) => decode_db(level(*v, -60.0, 12.0)?),
                        None => old.map(|v| v.clamp(-60.0, 12.0)),
                    };
                    let (l, r) = crate::mixer::encode_level_pan(gain, p);
                    add(0x600 | left, l);
                    add(0xc00 | left, r);
                }
            }
            if let Some(value) = mute {
                add(0x1200 | left, u16::from(*value));
                if let Some(right) = right {
                    add(0x1200 | right, u16::from(*value));
                }
            }
        }
        Operation::Master {
            mix: Mix::Loopback,
            level: db,
        } => add(0x1600, level(*db, -64.0, 0.0)?),
        Operation::LoopbackRouting {
            enabled,
            usb12,
            monitor,
        } => {
            for (selector, value) in [(0x1300, enabled), (0x1400, usb12), (0x1500, monitor)] {
                if let Some(value) = value {
                    add(selector, u16::from(*value));
                }
            }
        }
        Operation::OtgRouting {
            main_to_otg,
            otg_to_monitor,
            meter_source,
        } if state.pid == 0xaf93 => {
            if let Some(v) = main_to_otg {
                add(0x1900, *v);
            }
            if let Some(v) = otg_to_monitor {
                add(0x1700, u16::from(*v));
            }
            if let Some(v) = meter_source {
                add(0x1800, *v);
            }
        }
        _ => {
            return Err(Error::new(
                ErrorKind::UnsupportedCapability,
                "Operation is unavailable for this hardware loopback mixer",
            ));
        }
    }
    if out.is_empty() {
        return Err(Error::from("Specify at least one supported loopback field"));
    }
    Ok(out)
}

/// Restore mapped mixer fields only. Input modes and opaque bytes are never replayed.
pub fn plan_basic_restore(
    device: &Device,
    before: &BasicState,
    snapshot: &crate::Snapshot,
    mix: Option<Mix>,
) -> Result<BasicPlan, Error> {
    let saved = snapshot.basic_state()?;
    if !before.loopback_enabled()
        || saved.pid != before.pid
        || mix.is_some_and(|m| m != Mix::Loopback)
    {
        return Err(Error::new(
            ErrorKind::UnsupportedCapability,
            "Snapshot model/layout is incompatible with this loopback mixer",
        ));
    }
    let mut changes = Vec::new();
    for (_, left, right) in sources(before.pid) {
        for selector in [0x600 | left, 0xc00 | right.unwrap_or(left)] {
            let offset = ((selector >> 8) + (selector & 255)) as usize * 2;
            changes.push(BasicChange {
                selector,
                value: saved.word(offset),
            });
        }
        for ch in [Some(left), right].into_iter().flatten() {
            changes.push(BasicChange {
                selector: 0x1200 | ch,
                value: u16::from(saved.word(36) & (1 << ch) != 0),
            });
        }
    }
    for selector in [0x1300, 0x1400, 0x1500, 0x1600] {
        let word = saved.word((selector >> 8) as usize * 2);
        changes.push(BasicChange {
            selector,
            value: if selector == 0x1600 { word } else { word & 1 },
        });
    }
    if before.pid == 0xaf93 {
        for selector in [0x1700, 0x1800, 0x1900] {
            changes.push(BasicChange {
                selector,
                value: saved.word((selector >> 8) as usize * 2),
            });
        }
    }
    crate::basic::from_changes(device, before, changes)
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelMeters {
    pub inputs_dbfs: Vec<Option<f64>>,
    pub loopback_dbfs: [Option<f64>; 2],
    pub otg_dbfs: Option<[Option<f64>; 2]>,
}
impl ModelMeters {
    pub fn decode(pid: u16, bytes: &[u8]) -> Result<Self, Error> {
        let length = match pid {
            0xaf80 | 0xaf90 => 16,
            0xaf93 => 24,
            _ => return Err(Error::from("Unsupported model meters")),
        };
        if bytes.len() != length {
            return Err(Error::new(
                ErrorKind::ReadbackFailed,
                "Unexpected model meter length",
            ));
        }
        let sample = |index: usize| -> Result<Option<f64>, Error> {
            let value = i32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap());
            if value < 0 {
                return Err(Error::new(
                    ErrorKind::ReadbackFailed,
                    "Negative device peak magnitude",
                ));
            }
            let magnitude = value >> 8;
            Ok((magnitude != 0).then(|| 20.0 * (f64::from(magnitude) / 8388608.0).log10()))
        };
        Ok(Self {
            inputs_dbfs: if pid == 0xaf80 {
                vec![sample(0)?]
            } else {
                vec![sample(0)?, sample(1)?]
            },
            loopback_dbfs: [sample(2)?, sample(3)?],
            otg_dbfs: if pid == 0xaf93 {
                Some([sample(4)?, sample(5)?])
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(pid: u16) -> (Device, BasicState) {
        let device = Device {
            vendor_id: crate::VID,
            product_id: pid,
            bus: 1,
            address: 2,
        };
        let mut bytes = vec![0; crate::basic::state_length(pid).unwrap()];
        bytes[1] = 0xa4;
        bytes[2] = 0x67;
        let firmware =
            FirmwareVersion::from_bcd(if pid == 0xaf93 { 0x100 } else { 0x152 }).unwrap();
        (
            device,
            BasicState::new(pid, &bytes)
                .unwrap()
                .enable_loopback(firmware)
                .unwrap(),
        )
    }
    #[test]
    fn loopback_plans_preserve_unknown_words_and_use_each_model_channel_map() {
        for pid in [0xaf80, 0xaf90, 0xaf93] {
            let (device, before) = fixture(pid);
            let plan = crate::plan_basic(
                &device,
                &before,
                &[
                    Operation::Channel {
                        mix: Mix::Loopback,
                        source: "analog1".into(),
                        level: Some(Level::Db(-6.0)),
                        pan: Some(0.5),
                        mute: Some(true),
                        solo: None,
                    },
                    Operation::Channel {
                        mix: Mix::Loopback,
                        source: if pid == 0xaf93 { "usb56" } else { "usb34" }.into(),
                        level: Some(Level::Db(-12.0)),
                        pan: None,
                        mute: Some(true),
                        solo: None,
                    },
                    Operation::Master {
                        mix: Mix::Loopback,
                        level: Level::Db(-18.0),
                    },
                    Operation::LoopbackRouting {
                        enabled: Some(true),
                        usb12: Some(false),
                        monitor: Some(true),
                    },
                ],
            )
            .unwrap();
            assert_eq!(&plan.predicted().raw()[..12], &before.raw()[..12]);
            assert_eq!(plan.predicted().word(14), before.word(14)); // Unselected input-2 gain is preserved.
            assert_eq!(plan.predicted().word(44) as i16, -18 * 256);
            assert_eq!(plan.predicted().word(38) & 1, 1);
            assert_eq!(plan.predicted().word(42) & 1, 1);
            assert_eq!(plan.predicted().word(36) & 0x31, 0x31);
            assert!(
                crate::plan_basic(
                    &device,
                    &before,
                    &[Operation::Channel {
                        mix: Mix::Loopback,
                        source: "analog1".into(),
                        level: None,
                        pan: None,
                        mute: None,
                        solo: Some(true)
                    }]
                )
                .is_err()
            );
            assert!(
                crate::plan_basic(
                    &device,
                    &before,
                    &[Operation::OtgRouting {
                        main_to_otg: Some(2),
                        otg_to_monitor: None,
                        meter_source: None
                    }]
                )
                .is_err()
            );
        }
    }
    #[test]
    fn firmware_and_snapshot_gates_do_not_restore_input_modes() {
        let (device, before) = fixture(0xaf90);
        let changed = crate::plan_basic(
            &device,
            &before,
            &[Operation::Master {
                mix: Mix::Loopback,
                level: Level::Db(-24.0),
            }],
        )
        .unwrap();
        let snapshot = crate::Snapshot::new_basic(device.clone(), changed.predicted()).unwrap();
        let mut bytes = before.raw().to_vec();
        bytes[0] = 3;
        bytes[8] = 1;
        bytes[2] = 0xb7;
        let current = BasicState::new(0xaf90, &bytes)
            .unwrap()
            .enable_loopback(before.firmware().unwrap())
            .unwrap();
        let restore = plan_basic_restore(&device, &current, &snapshot, None).unwrap();
        assert_eq!(&restore.predicted().raw()[..12], &current.raw()[..12]);
        assert_eq!(restore.predicted().word(44) as i16, -24 * 256);
        let mut bad = snapshot.clone();
        bad.firmware_bcd = Some(0x149);
        assert!(bad.basic_state().is_err());
        bad = snapshot.clone();
        bad.device.product_id = 0xaf80;
        assert!(plan_basic_restore(&device, &current, &bad, None).is_err());
        bad = snapshot;
        bad.payload_hex = "é".into();
        assert!(bad.basic_state().is_err());
        assert!(
            BasicState::new(0xaf90, &[0; 46])
                .unwrap()
                .enable_loopback(FirmwareVersion::from_bcd(0x149).unwrap())
                .is_err()
        );
    }
    #[test]
    fn meter_shape_order_scale_and_unused_mf1_slot_are_explicit() {
        for pid in [0xaf80, 0xaf90, 0xaf93] {
            let mut bytes = vec![0; if pid == 0xaf93 { 24 } else { 16 }];
            bytes[..4].copy_from_slice(&0x40000000i32.to_le_bytes());
            bytes[8..12].copy_from_slice(&0x20000000i32.to_le_bytes());
            let meters = ModelMeters::decode(pid, &bytes).unwrap();
            assert!((meters.inputs_dbfs[0].unwrap() + 6.020599913).abs() < 1e-8);
            assert!((meters.loopback_dbfs[0].unwrap() + 12.041199826).abs() < 1e-8);
            assert_eq!(meters.inputs_dbfs.len(), if pid == 0xaf80 { 1 } else { 2 });
            assert_eq!(meters.otg_dbfs.is_some(), pid == 0xaf93);
            assert!(ModelMeters::decode(pid, &bytes[..bytes.len() - 1]).is_err());
            bytes[4..8].copy_from_slice(&(-1i32).to_le_bytes());
            assert_eq!(ModelMeters::decode(pid, &bytes).is_ok(), pid == 0xaf80);
        }
    }
}
