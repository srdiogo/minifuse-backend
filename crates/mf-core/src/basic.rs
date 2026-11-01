// SPDX-License-Identifier: MIT
//! Experimental MF1/MF2/OTG switches, isolated from the validated MF4 layout.
use crate::{
    Device, DeviceOutcome, Error, ErrorKind, InputSwitch, JournalStatus, Model, Operation, config,
    hex,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const BASIC_MAX_STATE: usize = 52;
pub const BASIC_MAX_CHANGES: usize = 4;

pub fn state_length(pid: u16) -> Option<usize> {
    match pid {
        0xaf80 | 0xaf90 => Some(46),
        0xaf93 => Some(52),
        _ => None,
    }
}

/// Fixed selectors established by legacy work and model-specific host analysis.
pub fn target(pid: u16, selector: u16, value: u16) -> Result<(usize, u16), Error> {
    let inputs = match pid {
        0xaf80 => 1,
        0xaf90 | 0xaf93 => 2,
        _ => 0,
    };
    if inputs == 0 || value > 1 {
        return Err(Error::from("Unsupported basic model or switch value"));
    }
    match selector {
        0 if inputs >= 1 => Ok((0, 1)),
        1 if inputs == 2 => Ok((0, 2)),
        0x0400 => Ok((8, 1)),
        0x0500 => Ok((10, 1)),
        _ => Err(Error::from("Unsupported model-specific switch selector")),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasicState {
    pub(crate) pid: u16,
    pub(crate) data: Vec<u8>,
}
impl BasicState {
    pub fn new(pid: u16, bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > BASIC_MAX_STATE || state_length(pid) != Some(bytes.len()) {
            return Err(Error::new(
                ErrorKind::ModuleIncompatible,
                "Unexpected model-specific B2 length",
            ));
        }
        Ok(Self {
            pid,
            data: bytes.to_vec(),
        })
    }
    pub fn raw(&self) -> &[u8] {
        &self.data
    }
    pub fn product_id(&self) -> u16 {
        self.pid
    }
    pub fn decoded(&self) -> Value {
        let word = |i: usize| u16::from_le_bytes([self.data[i], self.data[i + 1]]);
        let mut inputs = json!({"inst1":word(0)&1!=0,"phantom_power":word(8)&1!=0,"direct_monitor":word(10)&1!=0});
        if self.pid != 0xaf80 {
            inputs["inst2"] = json!(word(0) & 2 != 0);
        }
        json!({"model":Model::from_pid(self.pid).unwrap().name(),"layout":"minifuse-basic-b2-v1","inputs":inputs,"mixes":[]})
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BasicChange {
    pub selector: u16,
    pub value: u16,
}
#[derive(Clone, Debug)]
pub struct BasicPlan {
    pub(crate) device: Device,
    pub(crate) before: BasicState,
    pub(crate) predicted: BasicState,
    pub(crate) changes: Vec<BasicChange>,
}
impl BasicPlan {
    pub fn device(&self) -> &Device {
        &self.device
    }
    pub fn before(&self) -> &BasicState {
        &self.before
    }
    pub fn predicted(&self) -> &BasicState {
        &self.predicted
    }
    pub fn summary(&self) -> Value {
        json!({"layout":"minifuse-basic-b2-v1","usb_write_count":self.changes.len(),"changes":self.changes,"predicted_after_hex":hex(&self.predicted.data),"hardware_validation":"pending"})
    }
}

pub fn plan_basic(
    device: &Device,
    before: &BasicState,
    operations: &[Operation],
) -> Result<BasicPlan, Error> {
    if device.vendor_id != crate::VID
        || device.product_id != before.pid
        || state_length(device.product_id).is_none()
    {
        return Err(Error::new(
            ErrorKind::UnsupportedCapability,
            "Basic state does not match the normal-mode device",
        ));
    }
    if operations.is_empty() || operations.len() > BASIC_MAX_CHANGES {
        return Err(Error::from("Expected one to four switch operations"));
    }
    let mut predicted = before.clone();
    let mut changes = Vec::new();
    let mut seen = Vec::new();
    for operation in operations {
        let Operation::InputSwitch {
            target: control,
            enabled,
        } = operation
        else {
            return Err(Error::new(
                ErrorKind::UnsupportedCapability,
                "Only model-specific input/monitor switches are available in this experimental path",
            ));
        };
        let selector = match control {
            InputSwitch::Inst1 => 0,
            InputSwitch::Inst2 => 1,
            InputSwitch::PhantomPower => 0x0400,
            InputSwitch::DirectMonitor => 0x0500,
        };
        let (offset, mask) = target(before.pid, selector, u16::from(*enabled))?;
        if seen.contains(&selector) {
            return Err(Error::from("Duplicate switch target"));
        }
        seen.push(selector);
        let old = u16::from_le_bytes([predicted.data[offset], predicted.data[offset + 1]]);
        let new = if *enabled { old | mask } else { old & !mask };
        predicted.data[offset..offset + 2].copy_from_slice(&new.to_le_bytes());
        if new != old {
            changes.push(BasicChange {
                selector,
                value: u16::from(*enabled),
            });
        }
    }
    Ok(BasicPlan {
        device: device.clone(),
        before: before.clone(),
        predicted,
        changes,
    })
}

#[derive(Clone, Debug)]
pub struct BasicFailure {
    pub error: Error,
    pub after: Option<BasicState>,
}
impl From<Error> for BasicFailure {
    fn from(error: Error) -> Self {
        Self { error, after: None }
    }
}
pub trait BasicBackend {
    fn transact_basic(&mut self, plan: &BasicPlan) -> Result<BasicState, BasicFailure>;
}

#[derive(Clone, Debug)]
pub struct BasicOutcome {
    pub device_outcome: DeviceOutcome,
    pub device_write_attempted: Option<bool>,
    pub rollback_attempted: bool,
    pub rollback_verified: bool,
    pub after_state: Option<BasicState>,
    pub journal_status: JournalStatus,
    pub journal: Option<PathBuf>,
    pub backup: Option<PathBuf>,
    pub error: Option<Error>,
    pub journal_error: Option<String>,
}

pub fn execute_basic<B: BasicBackend>(
    backend: &mut B,
    plan: &BasicPlan,
    directory: &Path,
) -> Result<BasicOutcome, Error> {
    if plan.changes.is_empty() {
        return Ok(BasicOutcome {
            device_outcome: DeviceOutcome::Verified,
            device_write_attempted: Some(false),
            rollback_attempted: false,
            rollback_verified: false,
            after_state: Some(plan.before.clone()),
            journal_status: JournalStatus::NotNeeded,
            journal: None,
            backup: None,
            error: None,
            journal_error: None,
        });
    }
    let prepare = || -> Result<_, String> {
        config::create_backup_dir(directory)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let name = format!("basic-{stamp}-{}", std::process::id());
        let backup = directory.join(format!("{name}.json"));
        config::save_new(&backup,&serde_json::to_vec_pretty(&json!({"schema_version":1,"layout":"minifuse-basic-b2-v1","device":plan.device,"payload_hex":hex(&plan.before.data)})).map_err(|e|e.to_string())?)?;
        let path = directory.join(format!("{name}.operation.json"));
        let journal = json!({"schema_version":1,"layout":"minifuse-basic-b2-v1","status":"prepared","device":plan.device,"backup":backup,"changes":plan.changes,"expected_after_hex":hex(&plan.predicted.data)});
        config::save_new(
            &path,
            &serde_json::to_vec_pretty(&journal).map_err(|e| e.to_string())?,
        )?;
        Ok((backup, path, journal, name))
    };
    let (backup, path, mut journal, name) =
        prepare().map_err(|e| Error::new(ErrorKind::BackupFailed, e))?;
    let result = backend.transact_basic(plan).and_then(|after| {
        if after == plan.predicted {
            Ok(after)
        } else {
            let mut error = Error::new(
                ErrorKind::ReadbackFailed,
                "Basic switch readback did not match the complete predicted state",
            );
            error.device_write_attempted = Some(true);
            Err(BasicFailure {
                error,
                after: Some(after),
            })
        }
    });
    let (outcome, attempted, rollback_attempted, rollback_verified, after) = match &result {
        Ok(after) => (
            DeviceOutcome::Verified,
            Some(true),
            false,
            false,
            Some(after.clone()),
        ),
        Err(f) => {
            let restored = f.error.rollback_verified && f.after.as_ref() == Some(&plan.before);
            let outcome = if restored {
                DeviceOutcome::FailedRestored
            } else if f.error.rollback_attempted {
                DeviceOutcome::FailedUnrestored
            } else if f.error.device_write_attempted == Some(false) {
                DeviceOutcome::Rejected
            } else {
                DeviceOutcome::Unknown
            };
            (
                outcome,
                f.error.device_write_attempted,
                f.error.rollback_attempted,
                restored,
                f.after.clone(),
            )
        }
    };
    journal["status"] = if result.is_ok() { "verified" } else { "failed" }.into();
    journal["device_outcome"] = serde_json::to_value(outcome).unwrap();
    journal["rollback_attempted"] = json!(rollback_attempted);
    journal["rollback_verified"] = json!(rollback_verified);
    if let Some(after) = &after {
        journal["after_hex"] = hex(&after.data).into();
    }
    if let Err(f) = &result {
        journal["error"] = f.error.to_string().into();
    }
    let temporary = directory.join(format!("{name}.operation.pending"));
    let persist = serde_json::to_vec_pretty(&journal)
        .map_err(|e| e.to_string())
        .and_then(|bytes| config::save_new(&temporary, &bytes))
        .and_then(|_| fs::rename(&temporary, &path).map_err(|e| e.to_string()))
        .and_then(|_| config::sync_parent(&path));
    Ok(BasicOutcome {
        device_outcome: outcome,
        device_write_attempted: attempted,
        rollback_attempted,
        rollback_verified,
        after_state: after,
        journal_status: if persist.is_ok() {
            JournalStatus::Finalized
        } else {
            JournalStatus::Failed
        },
        journal: Some(path),
        backup: Some(backup),
        error: result.err().map(|f| f.error),
        journal_error: persist.err(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_lengths_modes_and_switch_masks_are_separate() {
        for pid in [0xaf80, 0xaf90, 0xaf93] {
            let bytes = vec![0xa4; state_length(pid).unwrap()];
            let state = BasicState::new(pid, &bytes).unwrap();
            let device = Device {
                vendor_id: crate::VID,
                product_id: pid,
                bus: 1,
                address: 2,
            };
            let plan = plan_basic(
                &device,
                &state,
                &[Operation::InputSwitch {
                    target: InputSwitch::PhantomPower,
                    enabled: true,
                }],
            )
            .unwrap();
            for (i, (&a, &b)) in state.data.iter().zip(&plan.predicted.data).enumerate() {
                assert_eq!(a ^ b, if i == 8 { 1 } else { 0 });
            }
            assert!(BasicState::new(pid, &[0; 190]).is_err());
            let inst2 = plan_basic(
                &device,
                &state,
                &[Operation::InputSwitch {
                    target: InputSwitch::Inst2,
                    enabled: true,
                }],
            );
            assert_eq!(inst2.is_ok(), pid != 0xaf80);
        }
        for pid in [0xaf70, 0xaf72, 0xaf82, 0xaf92, 0xaf91] {
            assert!(state_length(pid).is_none());
            assert!(target(pid, 0, 1).is_err());
        }
    }
    #[test]
    fn bounds_duplicate_and_mixer_operations_are_rejected() {
        let d = Device {
            vendor_id: crate::VID,
            product_id: 0xaf90,
            bus: 1,
            address: 2,
        };
        let s = BasicState::new(0xaf90, &[0; 46]).unwrap();
        let op = Operation::InputSwitch {
            target: InputSwitch::Inst1,
            enabled: true,
        };
        assert!(plan_basic(&d, &s, &[op.clone(), op]).is_err());
        assert!(target(0xaf90, 0x0300, 1).is_err());
        assert!(target(0xaf90, 0x0401, 1).is_err());
        assert!(target(0xaf90, 0x0400, 2).is_err());
    }
    #[test]
    fn durable_baseline_precedes_dispatch_and_readback_is_not_assumed() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "mf-basic-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        struct Fake {
            directory: PathBuf,
            calls: usize,
            wrong: bool,
        }
        impl BasicBackend for Fake {
            fn transact_basic(&mut self, plan: &BasicPlan) -> Result<BasicState, BasicFailure> {
                self.calls += 1;
                let entries: Vec<_> = fs::read_dir(&self.directory)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .collect();
                assert!(
                    entries
                        .iter()
                        .any(|p| p.to_string_lossy().ends_with(".operation.json"))
                );
                assert_eq!(
                    fs::metadata(&self.directory).unwrap().permissions().mode() & 0o777,
                    0o700
                );
                if self.wrong {
                    Ok(plan.before.clone())
                } else {
                    Ok(plan.predicted.clone())
                }
            }
        }
        use std::os::unix::fs::PermissionsExt;
        let device = Device {
            vendor_id: crate::VID,
            product_id: 0xaf90,
            bus: 1,
            address: 2,
        };
        let before = BasicState::new(0xaf90, &[0; 46]).unwrap();
        let plan = plan_basic(
            &device,
            &before,
            &[Operation::InputSwitch {
                target: InputSwitch::Inst2,
                enabled: true,
            }],
        )
        .unwrap();
        let mut backend = Fake {
            directory: root.clone(),
            calls: 0,
            wrong: true,
        };
        let outcome = execute_basic(&mut backend, &plan, &root).unwrap();
        assert_eq!(outcome.device_outcome, DeviceOutcome::Unknown);
        assert_eq!(outcome.device_write_attempted, Some(true));
        assert_eq!(outcome.journal_status, JournalStatus::Finalized);
        assert!(!outcome.rollback_verified);
        assert!(outcome.error.is_some());
        let blocked = root.join("not-a-directory");
        fs::write(&blocked, b"blocked").unwrap();
        assert!(execute_basic(&mut backend, &plan, &blocked).is_err());
        assert_eq!(backend.calls, 1);
        let noop = plan_basic(
            &device,
            &before,
            &[Operation::InputSwitch {
                target: InputSwitch::Inst1,
                enabled: false,
            }],
        )
        .unwrap();
        let outcome = execute_basic(&mut backend, &noop, &blocked).unwrap();
        assert_eq!(outcome.device_write_attempted, Some(false));
        assert_eq!(backend.calls, 1);
        fs::remove_dir_all(root).unwrap();
    }
}
