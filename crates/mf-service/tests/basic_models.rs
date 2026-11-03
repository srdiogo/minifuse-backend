// SPDX-License-Identifier: MIT
//! Synthetic protocol tests: no captured MF1/MF2 state and no hardware access.
use mf_core::{
    BasicBackend, BasicFailure, BasicPlan, BasicState, Device, Error, Model, ModuleCapabilities,
    Outcome, Plan, State, VID,
};
use mf_service::{
    coordinator::Coordinator,
    hardware::{Descriptor, Hardware},
    protocol::{ApiError, Request, Result},
    storage::Paths,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone)]
struct Fake(Arc<Mutex<Data>>);
struct Data {
    devices: Vec<Descriptor>,
    state: BasicState,
    enabled: bool,
    failed: bool,
    writes: usize,
    mf4_reads: usize,
    loopback: bool,
}
impl BasicBackend for Fake {
    fn transact_basic(
        &mut self,
        plan: &BasicPlan,
    ) -> std::result::Result<BasicState, BasicFailure> {
        let mut d = self.0.lock().unwrap();
        assert_eq!(&d.state, plan.before());
        d.writes += 1;
        d.state = plan.predicted().clone();
        Ok(d.state.clone())
    }
}
impl Hardware for Fake {
    fn inventory(&mut self) -> Result<Vec<Descriptor>> {
        let d = self.0.lock().unwrap();
        if d.failed {
            Err(ApiError::new(
                "device_unavailable",
                "Injected inventory failure",
            ))
        } else {
            Ok(d.devices.clone())
        }
    }
    fn read(&mut self) -> Result<(Device, State)> {
        self.0.lock().unwrap().mf4_reads += 1;
        Err(ApiError::new(
            "unsupported_capability",
            "MF4 must not be queried",
        ))
    }
    fn read_basic(&mut self, descriptor: &Descriptor) -> Result<(Device, BasicState)> {
        let d = self.0.lock().unwrap();
        if !d.enabled || d.devices.len() != 1 {
            return Err(ApiError::new(
                "unsupported_capability",
                "Experimental test transport unavailable",
            ));
        }
        Ok((descriptor.device.clone(), d.state.clone()))
    }
    fn module_capabilities(&mut self) -> Result<ModuleCapabilities> {
        let data = self.0.lock().unwrap();
        Ok(ModuleCapabilities {
            basic_loopback: data.loopback,
            basic_meters: data.loopback,
            basic_switches: true,
            ..Default::default()
        })
    }
    fn model_meters(&mut self, descriptor: &Descriptor) -> Result<mf_core::ModelMeters> {
        Ok(mf_core::ModelMeters::decode(
            descriptor.device.product_id,
            &vec![
                0;
                if descriptor.device.product_id == 0xaf93 {
                    24
                } else {
                    16
                }
            ],
        )
        .unwrap())
    }
    fn execute(&mut self, _: &Plan, _: &Path) -> std::result::Result<Outcome, Error> {
        panic!("Basic model dispatched to MF4")
    }
    fn execute_basic(
        &mut self,
        p: &BasicPlan,
        path: &Path,
    ) -> std::result::Result<mf_core::BasicOutcome, Error> {
        mf_core::execute_basic(self, p, path)
    }
}
struct Test {
    root: PathBuf,
    hardware: Fake,
    service: Coordinator<Fake>,
}
impl Test {
    fn new(pid: u16) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "mf-basic-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let hardware = Fake(Arc::new(Mutex::new(Data {
            devices: vec![Descriptor {
                device: Device {
                    vendor_id: VID,
                    product_id: pid,
                    bus: 1,
                    address: 2,
                },
                model: Model::from_pid(pid).unwrap(),
                connection_marker: 1,
                firmware: mf_core::FirmwareVersion::from_bcd(if pid == 0xaf93 {
                    0x100
                } else {
                    0x152
                }),
            }],
            state: BasicState::new(
                if pid == 0xaf93 { pid } else { 0xaf80 },
                &vec![0; if pid == 0xaf93 { 52 } else { 46 }],
            )
            .unwrap(),
            enabled: true,
            failed: false,
            writes: 0,
            mf4_reads: 0,
            loopback: false,
        })));
        if pid == 0xaf90 {
            hardware.0.lock().unwrap().state = BasicState::new(pid, &[0; 46]).unwrap();
        }
        let service = Coordinator::new(
            hardware.clone(),
            Paths {
                metadata: root.join("metadata"),
                presets: root.join("presets"),
                backups: root.join("backups"),
            },
        )
        .unwrap();
        let mut test = Self {
            root,
            hardware,
            service,
        };
        test.service.connected(1);
        assert!(
            test.call("hello", json!({"client":"basic-test","supported_api":[1]}))
                .get("error")
                .is_none()
        );
        test
    }
    fn enable_loopback(&mut self) {
        let mut d = self.hardware.0.lock().unwrap();
        let pid = d.devices[0].device.product_id;
        let firmware =
            mf_core::FirmwareVersion::from_bcd(if pid == 0xaf93 { 0x100 } else { 0x152 }).unwrap();
        d.devices[0].firmware = Some(firmware);
        d.loopback = true;
        d.state = d.state.clone().enable_loopback(firmware).unwrap();
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.service.request(
            1,
            Request {
                api: 1,
                id: "test".into(),
                method: method.into(),
                params,
            },
        )
    }
    fn device(&mut self) -> Value {
        self.call("list_devices", json!({}))["result"]["devices"][0].clone()
    }
    fn state(&mut self) -> Value {
        let d = self.device();
        let r = self.call(
            "get_state",
            json!({"device_id":d["device_id"],"generation":d["generation"]}),
        );
        assert!(r.get("error").is_none(), "{r}");
        r["result"].clone()
    }
}
impl Drop for Test {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn each_model_uses_its_profile_and_verified_switch_transaction() {
    for (pid, controls) in [(0xaf80, 3), (0xaf90, 4), (0xaf93, 4)] {
        let mut t = Test::new(pid);
        let d = t.device();
        assert_eq!(
            d["profile"]["input_controls"].as_array().unwrap().len(),
            controls
        );
        assert_eq!(d["profile"]["layout"], "inputs_and_monitoring");
        let s = t.state();
        assert_eq!(s["capabilities"]["mixer"], false);
        assert_eq!(s["capabilities"]["input_switches"], true);
        let result=t.call("plan_changes",json!({"device_id":s["device_id"],"generation":s["generation"],"expected_revision":s["revision"],"changes":[{"kind":"input_switch","target":"direct_monitor","enabled":true}]}));
        assert!(result.get("error").is_none(), "{result}");
        let result = t.call(
            "apply_plan",
            json!({"plan_id":result["result"]["plan_id"],"idempotency_key":"switch-test"}),
        );
        assert!(result.get("error").is_none(), "{result}");
        t.service.execute_next();
        let result = t.call(
            "get_operation",
            json!({"operation_id":result["result"]["operation_id"]}),
        );
        assert_eq!(result["result"]["device_outcome"], "verified", "{result}");
        assert_eq!(t.state()["state"]["inputs"]["direct_monitor"], true);
        assert_eq!(t.hardware.0.lock().unwrap().writes, 1);
        assert_eq!(t.hardware.0.lock().unwrap().mf4_reads, 0);
        let identity = t.state();
        let result = t.call(
            "capture_snapshot",
            json!({"device_id":identity["device_id"],"generation":identity["generation"]}),
        );
        assert_eq!(
            result["error"]["code"], "unsupported_capability",
            "{result}"
        );
    }
}

#[test]
fn default_disabled_and_initialization_profiles_never_read_the_mf4_layout() {
    for pid in [0xaf80, 0xaf90, 0xaf93, 0xaf82, 0xaf92, 0xaf72] {
        let mut t = Test::new(pid);
        t.hardware.0.lock().unwrap().enabled = false;
        let d = t.device();
        assert_eq!(d["connection"], "unavailable");
        assert_eq!(d["capabilities"]["input_switches"], false);
        if [0xaf82, 0xaf92, 0xaf72].contains(&pid) {
            assert_eq!(d["profile"]["layout"], "initialization");
            assert!(
                d["profile"]["input_controls"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(t.hardware.0.lock().unwrap().mf4_reads, 0);
        assert_eq!(t.hardware.0.lock().unwrap().writes, 0);
    }
}

#[test]
fn inventory_failure_and_reconnection_invalidate_basic_state_and_plans() {
    let mut t = Test::new(0xaf90);
    let s = t.state();
    let p=t.call("plan_changes",json!({"device_id":s["device_id"],"generation":s["generation"],"expected_revision":s["revision"],"changes":[{"kind":"input_switch","target":"inst2","enabled":true}]}));
    t.hardware.0.lock().unwrap().failed = true;
    assert!(t.call("list_devices", json!({})).get("error").is_some());
    let r = t.call(
        "get_state",
        json!({"device_id":s["device_id"],"generation":s["generation"]}),
    );
    assert!(r.get("error").is_some());
    t.hardware.0.lock().unwrap().failed = false;
    let new = t.state();
    assert_ne!(new["generation"], s["generation"]);
    let r = t.call(
        "apply_plan",
        json!({"plan_id":p["result"]["plan_id"],"idempotency_key":"stale-plan"}),
    );
    assert!(r.get("error").is_some());
    assert_eq!(t.hardware.0.lock().unwrap().writes, 0);
}

#[test]
fn loopback_support_includes_firmware_scoped_live_controls_presets_and_meters() {
    for pid in [0xaf80, 0xaf90, 0xaf93] {
        let mut t = Test::new(pid);
        t.enable_loopback();
        let d = t.device();
        assert_eq!(d["profile"]["layout"], "loopback_mixer");
        assert_eq!(d["firmware"]["loopback_compatible"], true);
        let s = t.state();
        assert_eq!(s["capabilities"]["mixer"], true);
        assert_eq!(s["capabilities"]["host_snapshots"], true);
        let snapshot = t.call(
            "capture_snapshot",
            json!({"device_id":s["device_id"],"generation":s["generation"]}),
        );
        assert!(snapshot.get("error").is_none(), "{snapshot}");
        let begun=t.call("begin_control",json!({"device_id":s["device_id"],"generation":s["generation"],"expected_revision":s["revision"],"target":{"kind":"master_level","mix":"loopback"},"idempotency_key":"loopback-gesture"}));
        assert!(begun.get("error").is_none(), "{begun}");
        let operation = begun["result"]["operation_id"].clone();
        let update = t.call(
            "update_control",
            json!({"operation_id":operation,"sequence":1,"value":-12.0,"finish":true}),
        );
        assert!(update.get("error").is_none(), "{update}");
        t.service.execute_next();
        let result = t.call("get_operation", json!({"operation_id":operation}));
        assert_eq!(result["result"]["device_outcome"], "verified", "{result}");
        let s = t.state();
        assert_eq!(s["state"]["mixes"][0]["master_db"], -12.0);
        let restore=t.call("plan_restore",json!({"device_id":s["device_id"],"generation":s["generation"],"revision":s["revision"],"snapshot":snapshot["result"]["snapshot"]}));
        assert!(restore.get("error").is_none(), "{restore}");
        let sub = t.call("subscribe", json!({"events":["meters_updated"]}));
        assert!(sub.get("error").is_none(), "{sub}");
        t.service.poll_meters();
        assert!(
            t.service
                .pending_events
                .iter()
                .any(|(_, e)| e["event"] == "meters_updated"
                    && e["data"]["meter_layout"] == "minifuse-basic-c2-v1")
        );
        assert_eq!(t.hardware.0.lock().unwrap().mf4_reads, 0);
    }
}

#[test]
fn firmware_or_module_changes_revoke_loopback_and_invalidate_prepared_writes() {
    for raw in [None, Some(0x149), Some(0x160), Some(0x200)] {
        let mut t = Test::new(0xaf90);
        t.enable_loopback();
        let s = t.state();
        let p=t.call("plan_changes",json!({"device_id":s["device_id"],"generation":s["generation"],"expected_revision":s["revision"],"changes":[{"kind":"master","mix":"loopback","level_db":-12}]}));
        assert!(p.get("error").is_none(), "{p}");
        {
            let mut d = t.hardware.0.lock().unwrap();
            d.devices[0].firmware = raw.and_then(mf_core::FirmwareVersion::from_bcd);
            d.state = BasicState::new(0xaf90, d.state.raw()).unwrap();
        }
        let changed = t.device();
        assert_ne!(changed["generation"], s["generation"]);
        assert_eq!(changed["capabilities"]["mixer"], false);
        assert_eq!(changed["capabilities"]["host_snapshots"], false);
        assert_eq!(changed["capabilities"]["signal_meters"], false);
        assert_eq!(changed["capabilities"]["input_switches"], false);
        let result = t.call(
            "apply_plan",
            json!({"plan_id":p["result"]["plan_id"],"idempotency_key":"firmware-changed"}),
        );
        assert!(result.get("error").is_some(), "{result}");
        assert_eq!(t.hardware.0.lock().unwrap().writes, 0);
    }
    // A mismatched transport must not grant advanced state even if its reply claims it.
    for missing_module in [false, true] {
        let mut t = Test::new(0xaf93);
        t.enable_loopback();
        {
            let mut d = t.hardware.0.lock().unwrap();
            if missing_module {
                d.loopback = false;
            } else {
                d.devices[0].firmware = None;
            }
        }
        let d = t.device();
        assert_eq!(d["capabilities"]["mixer"], false);
        assert_eq!(d["connection"], "unavailable");
        assert_eq!(d["profile"]["layout"], "inputs_and_monitoring");
        assert_eq!(t.hardware.0.lock().unwrap().writes, 0);
    }
}
