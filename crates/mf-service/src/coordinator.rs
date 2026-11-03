// SPDX-License-Identifier: MIT
use crate::{
    hardware::{Descriptor, Hardware},
    protocol::{self, ApiError, Result},
    storage::{self, Paths},
};
use mf_core::{Capabilities, Model, ModuleCapabilities, Plan, State};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    time::{Duration, Instant},
};

#[path = "live.rs"]
mod live;

const MAX_PLANS: usize = 128;
const MAX_OPERATIONS: usize = 4096;
const EVENTS: [&str; 9] = [
    "device_added",
    "device_removed",
    "capabilities_changed",
    "state_changed",
    "operation_started",
    "operation_finished",
    "control_updated",
    "meters_updated",
    "control_panel_requested",
];
macro_rules! outcome_value {
    ($value:expr) => {{
        let outcome = $value;
        json!({
            "device_outcome":outcome.device_outcome,
            "device_write_attempted":outcome.device_write_attempted,
            "rollback_attempted":outcome.rollback_attempted,
            "rollback_verified":outcome.rollback_verified,
            "after_state_valid":outcome.after_state.is_some(),
            "state":outcome.after_state.as_ref().map(|s|s.decoded()),
            "journal_status":outcome.journal_status,
            "journal":outcome.journal,"backup":outcome.backup,
            "error":outcome.error.map(ApiError::from).map(|e|e.value()),
            "journal_error":outcome.journal_error
        })
    }};
}
struct DeviceState {
    descriptor: Descriptor,
    id: String,
    generation: String,
    revision: String,
    state: Option<State>,
    basic: Option<mf_core::BasicState>,
    error: Option<ApiError>,
    module: ModuleCapabilities,
    meter_retry: Instant,
    button_observation: Option<(u8, Instant)>,
}
impl DeviceState {
    fn button_counter(&self) -> Option<u8> {
        // Physical evidence covers normal-mode MF4, USB firmware descriptor 1.0.1 only.
        if self.descriptor.device.product_id != mf_core::MF4_PID
            || self.descriptor.model != Model::MiniFuse4
            || self.descriptor.firmware != mf_core::FirmwareVersion::from_bcd(0x0101)
        {
            return None;
        }
        self.state.as_ref()?.control_panel_counter()
    }
    fn capabilities(&self) -> Capabilities {
        let mut capabilities = self.descriptor.model.capabilities();
        capabilities.control_panel_button = self.button_counter().is_some();
        if capabilities.control_panel_button {
            capabilities.limitations = vec![
                "Input writes and signal meters require matching module features.",
                "Short-press logo events are validated only for MiniFuse 4 USB firmware descriptor 1.0.1; desktop activation requires an optional session agent.",
            ];
        }
        if self
            .descriptor
            .model
            .profile(self.descriptor.device.product_id)
            .is_some_and(|p| p.device_mode == mf_core::DeviceMode::Initialization)
        {
            capabilities.mixer = false;
            capabilities.host_snapshots = false;
            capabilities.switch_readback = false;
            capabilities.mixes.clear();
        }
        capabilities.input_switches &= self.state.is_some() && self.module.input_switches;
        capabilities.signal_meters &= self.state.is_some() && self.module.signal_meters;
        if let Some(state) = &self.basic {
            capabilities.switch_readback = true;
            capabilities.input_switches = self.module.basic_switches;
            if state.loopback_enabled() && self.module.basic_loopback {
                capabilities.mixer = true;
                capabilities.host_snapshots = true;
                capabilities.signal_meters = self.module.basic_meters;
                capabilities.mixes = vec![mf_core::MixCapabilities {
                    mix: mf_core::Mix::Loopback,
                    sources: state.loopback_sources(),
                    routes: vec!["mix"],
                    membership: vec![],
                }];
                capabilities.limitations = vec![
                    "Experimental model-specific hardware loopback; physical acceptance pending.",
                    "Windows SLS virtual-device mixing is not provided.",
                    "Presets restore mapped mixer fields only; input modes and opaque words are preserved.",
                ];
            }
        }
        capabilities
    }
    fn profile(&self) -> Option<mf_core::ModelProfile> {
        let mut profile = self
            .descriptor
            .model
            .profile(self.descriptor.device.product_id)?;
        if self.basic.as_ref().is_some_and(|s| s.loopback_enabled()) && self.module.basic_loopback {
            profile.layout = "loopback_mixer";
        }
        Some(profile)
    }
    fn plan(&self, operations: &[mf_core::Operation]) -> Result<DevicePlan> {
        let plan = if let Some(state) = &self.basic {
            DevicePlan::Basic(mf_core::plan_basic(
                &self.descriptor.device,
                state,
                operations,
            )?)
        } else {
            DevicePlan::Mixer(Box::new(mf_core::plan(
                &self.descriptor.device,
                self.state.as_ref().ok_or_else(|| {
                    ApiError::new(
                        "unsupported_capability",
                        "Confirmed mixer state is unavailable",
                    )
                })?,
                operations,
            )?))
        };
        self.validate_plan(&plan)?;
        Ok(plan)
    }
    fn snapshot(&self) -> Result<mf_core::Snapshot> {
        if let Some(state) = &self.basic {
            mf_core::Snapshot::new_basic(self.descriptor.device.clone(), state)
                .map_err(|e| ApiError::new("unsupported_capability", e))
        } else {
            Ok(mf_core::Snapshot::new(
                self.descriptor.device.clone(),
                self.state.as_ref().ok_or_else(|| {
                    ApiError::new("unsupported_capability", "Mixer snapshots are unavailable")
                })?,
            ))
        }
    }
    fn validate_plan(&self, plan: &DevicePlan) -> Result<()> {
        if let DevicePlan::Basic(p) = plan {
            return if self.basic.is_some()
                && (!p.requires_input_switches() || self.capabilities().input_switches)
                && (!p.before().loopback_enabled() || self.capabilities().mixer)
            {
                Ok(())
            } else {
                Err(ApiError::new(
                    "unsupported_capability",
                    "Model-specific verified switch transport is unavailable",
                ))
            };
        }
        let DevicePlan::Mixer(plan) = plan else {
            unreachable!()
        };
        if plan.requires_input_switches() && !self.capabilities().input_switches {
            return Err(ApiError::new(
                "unsupported_capability",
                "The active module does not support verified input-switch transactions",
            ));
        }
        Ok(())
    }
}

enum DevicePlan {
    Mixer(Box<Plan>),
    Basic(mf_core::BasicPlan),
}
impl DevicePlan {
    fn predicted(&self) -> Value {
        match self {
            Self::Mixer(p) => p.predicted().decoded(),
            Self::Basic(p) => p.predicted().decoded(),
        }
    }
    fn matches(&self, device: &DeviceState) -> bool {
        match self {
            Self::Mixer(p) => device.state.as_ref() == Some(p.predicted()),
            Self::Basic(p) => device.basic.as_ref() == Some(p.predicted()),
        }
    }
    fn summary(&self) -> Value {
        match self {
            Self::Mixer(p) => p.summary(),
            Self::Basic(p) => p.summary(),
        }
    }
}

struct Proposal {
    owner: String,
    device_id: String,
    generation: String,
    revision: String,
    plan: DevicePlan,
    expires: Instant,
}
struct Client {
    token: Option<String>,
    events: HashSet<String>,
    device: Option<String>,
    seq: u64,
    subscribed: bool,
}
struct Work {
    id: String,
    proposal: Proposal,
    record: Value,
    path: std::path::PathBuf,
}

/// All methods run on the one blocking coordinator worker. The acceptor and
/// bounded per-client writers never perform hardware calls.
pub struct Coordinator<H: Hardware> {
    hardware: H,
    paths: Paths,
    pub epoch: String,
    pub seq: u64,
    devices: Vec<DeviceState>,
    plans: HashMap<String, Proposal>,
    operations: HashMap<String, Value>,
    clients: HashMap<u64, Client>,
    pub pending_events: Vec<(u64, Value)>,
    work: VecDeque<Work>,
    live: Option<live::Control>,
    live_work: Option<live::Sample>,
}
fn public_operation(record: &Value) -> Value {
    let mut result = record.clone();
    for key in [
        "client_token",
        "idempotency_key",
        "plan_digest",
        "plan_id",
        "begin",
    ] {
        result
            .as_object_mut()
            .expect("Operation object")
            .remove(key);
    }
    result
}
impl<H: Hardware> Coordinator<H> {
    pub fn new(hardware: H, paths: Paths) -> std::io::Result<Self> {
        paths.prepare()?;
        let mut operations = HashMap::new();
        let mut keys = HashSet::new();
        for entry in fs::read_dir(paths.metadata.join("operations"))? {
            let path = entry?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            if operations.len() >= MAX_OPERATIONS {
                return Err(std::io::Error::other(
                    "Operation store is full; archive resolved records explicitly",
                ));
            }
            let mut record = storage::read(&path)?;
            let id = record["operation_id"]
                .as_str()
                .ok_or_else(|| std::io::Error::other("Invalid operation record"))?
                .to_owned();
            storage::safe_id(&id).map_err(|e| std::io::Error::other(e.message))?;
            let owner = record["client_token"]
                .as_str()
                .ok_or_else(|| std::io::Error::other("Missing operation owner"))?;
            let key = record["idempotency_key"]
                .as_str()
                .ok_or_else(|| std::io::Error::other("Missing idempotency key"))?;
            storage::safe_id(owner).map_err(|e| std::io::Error::other(e.message))?;
            if record["schema_version"] != 1
                || record["plan_id"].as_str().is_none()
                || !keys.insert((owner.to_owned(), key.to_owned()))
            {
                return Err(std::io::Error::other(
                    "Invalid or conflicting retained operation records require reconciliation",
                ));
            }
            if !matches!(
                record["status"].as_str(),
                Some("pending" | "completed" | "indeterminate")
            ) {
                return Err(std::io::Error::other("Unsupported operation status"));
            }
            if record["kind"] == "live_control"
                && record["samples"].as_array().is_none_or(|s| {
                    s.len() > 512
                        || s.iter().enumerate().any(|(i, s)| {
                            s["sequence"].as_u64() != Some(i as u64 + 1)
                                || s.get("value").is_none()
                                || !s["finish"].is_boolean()
                        })
                })
            {
                return Err(std::io::Error::other(
                    "Invalid retained continuous-control samples",
                ));
            }
            if record["status"] == "pending" {
                record["status"] = "indeterminate".into();
                if record["kind"] == "live_control" {
                    record["gesture_end"] = "interrupted".into();
                }
                record["device_outcome"] = "unknown".into();
                record["device_write_attempted"] = Value::Null;
                record["rollback_attempted"] = Value::Null;
                record["rollback_verified"] = false.into();
                record["after_state_valid"] = false.into();
                record["journal_status"] = "unknown".into();
            }
            operations.insert(id, record);
        }
        Ok(Self {
            hardware,
            paths,
            epoch: storage::token(),
            seq: 0,
            devices: Vec::new(),
            plans: HashMap::new(),
            operations,
            clients: HashMap::new(),
            pending_events: Vec::new(),
            work: VecDeque::new(),
            live: None,
            live_work: None,
        })
    }
    pub fn connected(&mut self, id: u64) {
        self.clients.insert(
            id,
            Client {
                token: None,
                events: HashSet::new(),
                device: None,
                seq: 0,
                subscribed: false,
            },
        );
    }
    pub fn disconnected(&mut self, id: u64) {
        if self.live.as_ref().is_some_and(|c| c.connection == id) {
            self.end_control("disconnected");
        }
        if let Some(owner) = self.clients.remove(&id).and_then(|c| c.token) {
            self.plans.retain(|_, p| p.owner != owner);
        }
    }
    pub fn has_subscribers(&self) -> bool {
        self.clients.values().any(|c| !c.events.is_empty())
    }
    pub fn has_control_panel_subscribers(&self) -> bool {
        self.devices.iter().any(|d| {
            d.button_counter().is_some()
                && self.clients.values().any(|c| {
                    c.events.contains("control_panel_requested")
                        && c.device.as_ref().is_none_or(|id| id == &d.id)
                })
        })
    }
    pub fn has_meter_subscribers(&self) -> bool {
        self.clients
            .values()
            .any(|c| c.events.contains("meters_updated"))
    }
    /// Ephemeral samples never alter revisions, snapshots, plans or journals.
    pub fn poll_meters(&mut self) {
        for i in 0..self.devices.len() {
            let d = &self.devices[i];
            if Instant::now() < d.meter_retry
                || !d.capabilities().signal_meters
                || !self.clients.values().any(|c| {
                    c.events.contains("meters_updated")
                        && c.device.as_ref().is_none_or(|id| id == &d.id)
                })
            {
                continue;
            }
            let id = d.id.clone();
            let generation = d.generation.clone();
            if d.basic.is_some() {
                let descriptor = d.descriptor.clone();
                let data = match self.hardware.model_meters(&descriptor) {
                    Ok(meters) => {
                        json!({"device_id":id,"generation":generation,"available":true,"meter_layout":"minifuse-basic-c2-v1","inputs_dbfs":meters.inputs_dbfs,"loopback_dbfs":meters.loopback_dbfs,"otg_dbfs":meters.otg_dbfs})
                    }
                    Err(error) => {
                        self.devices[i].meter_retry = Instant::now() + Duration::from_secs(1);
                        json!({"device_id":id,"generation":generation,"available":false,"error":error.value()})
                    }
                };
                self.emit("meters_updated", Some(&id), data);
                continue;
            }
            let result = self.hardware.meters();
            let data = match result {
                Ok((device, meters)) if device == self.devices[i].descriptor.device => {
                    json!({"device_id":id,"generation":generation,"available":true,
                        "sources_dbfs":meters.sources_dbfs,"mixes_dbfs":meters.mixes_dbfs})
                }
                result => {
                    self.devices[i].meter_retry = Instant::now() + Duration::from_secs(1);
                    let error = result.err().unwrap_or_else(|| {
                        ApiError::new("device_changed", "Meter device identity changed")
                    });
                    json!({"device_id":id,"generation":generation,"available":false,"error":error.value()})
                }
            };
            self.emit("meters_updated", Some(&id), data);
        }
    }
    fn emit(&mut self, event: &str, device: Option<&str>, data: Value) {
        self.seq += 1;
        for (&id, client) in &mut self.clients {
            if client.events.contains(event)
                && client.device.as_deref().is_none_or(|d| Some(d) == device)
            {
                client.seq += 1;
                let frame = json!({"event":event,"epoch":self.epoch,"seq":client.seq,"data":data});
                self.pending_events.push((id, frame));
            }
        }
    }
    fn inventory(&self) -> Value {
        Value::Array(self.devices.iter().map(|d|json!({"device_id":d.id,"generation":d.generation,"model":d.descriptor.model.name(),"product_id":d.descriptor.device.product_id,"profile":d.profile(),"firmware":mf_core::firmware_status(d.descriptor.device.product_id,d.descriptor.firmware),"connection":if d.state.is_some()||d.basic.is_some(){"ready"}else{"unavailable"},"capabilities":d.capabilities(),"error":d.error.as_ref().map(ApiError::value)})).collect())
    }
    fn state_value(&self, index: usize) -> Result<Value> {
        let d = &self.devices[index];
        if let Some(state) = &d.basic {
            return Ok(
                json!({"device_id":d.id,"generation":d.generation,"revision":d.revision,"state":state.decoded(),"capabilities":d.capabilities()}),
            );
        }
        let state = d.state.as_ref().ok_or_else(|| {
            d.error
                .clone()
                .unwrap_or_else(|| ApiError::new("device_unavailable", "Refresh device status"))
        })?;
        Ok(
            json!({"device_id":d.id,"generation":d.generation,"revision":d.revision,"state":state.decoded(),"capabilities":d.capabilities()}),
        )
    }
    pub fn refresh(&mut self) -> Result<()> {
        if self
            .live
            .as_ref()
            .is_some_and(|c| c.touched.elapsed() >= Duration::from_secs(30))
        {
            self.end_control("expired");
        }
        let inventory = match self.hardware.inventory() {
            Ok(inventory) => inventory,
            Err(error) => {
                self.plans.clear();
                for i in 0..self.devices.len() {
                    let mixer_changed = self.devices[i].state.take().is_some();
                    let basic_changed = self.devices[i].basic.take().is_some();
                    let changed = mixer_changed || basic_changed;
                    self.devices[i].error = Some(error.clone());
                    self.devices[i].button_observation = None;
                    if changed {
                        let id = self.devices[i].id.clone();
                        self.emit(
                            "capabilities_changed",
                            Some(&id),
                            json!({"device_id":id,"available":false,"error":error.value()}),
                        );
                    }
                }
                return Err(error);
            }
        };
        if inventory.len() > 64 {
            return Err(ApiError::new(
                "ambiguous_device",
                "Too many device descriptors",
            ));
        }
        let mut previous = std::mem::take(&mut self.devices);
        for descriptor in inventory {
            let entry = if let Some(i) = previous.iter().position(|d| d.descriptor == descriptor) {
                previous.remove(i)
            } else {
                let id = storage::token();
                let generation = storage::token();
                self.emit(
                    "device_added",
                    Some(&id),
                    json!({"device_id":id,"generation":generation,"model":descriptor.model.name()}),
                );
                DeviceState {
                    descriptor,
                    id,
                    generation,
                    revision: storage::token(),
                    state: None,
                    basic: None,
                    error: None,
                    module: ModuleCapabilities::default(),
                    meter_retry: Instant::now(),
                    button_observation: None,
                }
            };
            self.devices.push(entry);
        }
        for d in previous {
            self.plans.retain(|_, p| p.device_id != d.id);
            self.emit(
                "device_removed",
                Some(&d.id),
                json!({"device_id":d.id,"generation":d.generation}),
            );
        }
        let mf4 = self
            .devices
            .iter()
            .filter(|d| d.descriptor.device.product_id == mf_core::MF4_PID)
            .count();
        for i in 0..self.devices.len() {
            let model = self.devices[i].descriptor.model;
            if matches!(
                self.devices[i].descriptor.device.product_id,
                0xaf80 | 0xaf90 | 0xaf93
            ) {
                let descriptor = self.devices[i].descriptor.clone();
                let result = if descriptor.firmware.is_some_and(|firmware| {
                    firmware.supports_loopback(descriptor.device.product_id)
                }) {
                    self.hardware
                        .read_basic(&descriptor)
                        .and_then(|(device, state)| {
                            self.hardware
                                .module_capabilities()
                                .map(|module| (device, state, module))
                        })
                } else {
                    Err(ApiError::new(
                        "unsupported_capability",
                        "Model controls require a compatible USB firmware descriptor",
                    ))
                };
                match result {
                    Ok((device, state, module))
                        if device == descriptor.device
                            && state.product_id() == device.product_id
                            && module.basic_switches
                            && (!state.loopback_enabled()
                                || (module.basic_loopback
                                    && state.firmware() == descriptor.firmware)) =>
                    {
                        let changed = self.devices[i].basic.as_ref() != Some(&state);
                        let recovered = self.devices[i].error.is_some();
                        if recovered {
                            let id = self.devices[i].id.clone();
                            self.devices[i].generation = storage::token();
                            self.plans.retain(|_, p| p.device_id != id);
                        }
                        let capabilities_changed =
                            self.devices[i].module != module || self.devices[i].basic.is_none();
                        let d = &mut self.devices[i];
                        d.basic = Some(state);
                        d.module = module;
                        d.error = None;
                        if changed || recovered || capabilities_changed {
                            d.revision = storage::token();
                            let id = d.id.clone();
                            if capabilities_changed {
                                self.emit("capabilities_changed",Some(&id),json!({"device_id":id,"available":true,"capabilities":self.devices[i].capabilities()}));
                            }
                            self.emit("state_changed", Some(&id), self.state_value(i)?);
                        }
                    }
                    other => {
                        let error = other.err().unwrap_or_else(|| {
                            ApiError::new(
                                "device_changed",
                                "Basic state or transport does not match the selected model",
                            )
                        });
                        let changed = self.devices[i].basic.take().is_some()
                            || self.devices[i].error.as_ref().map(|e| e.code) != Some(error.code);
                        self.devices[i].error = Some(error);
                        let id = self.devices[i].id.clone();
                        self.plans.retain(|_, p| p.device_id != id);
                        if changed {
                            self.emit("capabilities_changed",Some(&id),json!({"device_id":id,"generation":self.devices[i].generation,"available":false,"error":self.devices[i].error.as_ref().map(ApiError::value)}));
                        }
                    }
                }
                continue;
            }
            let result = if matches!(
                self.devices[i].descriptor.device.product_id,
                0xaf82 | 0xaf92 | 0xaf72
            ) {
                Err(ApiError::new(
                    "unsupported_capability",
                    "The interface is in initialization/storage USB mode; audio controls are unavailable. No mode change was attempted.",
                ))
            } else if model != Model::MiniFuse4 {
                Err(ApiError::new(
                    "unsupported_capability",
                    "Mixer state is available only for the validated MiniFuse 4 layout",
                ))
            } else if mf4 != 1 {
                Err(ApiError::new(
                    "ambiguous_device",
                    "Connect one MiniFuse 4; no interface was chosen",
                ))
            } else {
                self.hardware.read().and_then(|(device, state)| {
                    self.hardware
                        .module_capabilities()
                        .map(|module| (device, state, module))
                })
            };
            match result {
                Ok((device, state, module)) if device == self.devices[i].descriptor.device => {
                    let changed = self.devices[i].state.as_ref() != Some(&state);
                    let previous_button = self.devices[i].button_counter().is_some();
                    let module_changed = self.devices[i].module != module;
                    let recovered = self.devices[i].error.is_some();
                    if recovered {
                        let id = self.devices[i].id.clone();
                        self.devices[i].generation = storage::token();
                        self.plans.retain(|_, p| p.device_id != id);
                    }
                    let d = &mut self.devices[i];
                    d.error = None;
                    d.state = Some(state);
                    d.module = module;
                    let now = Instant::now();
                    let counter = d.button_counter();
                    let requested = counter.is_some_and(|current| {
                        !recovered
                            && d.button_observation.is_some_and(|(previous, seen)| {
                                previous != current
                                    && now.duration_since(seen) <= Duration::from_secs(2)
                            })
                    });
                    d.button_observation = counter.map(|counter| (counter, now));
                    let capabilities_changed =
                        module_changed || previous_button != counter.is_some();
                    if changed || recovered || capabilities_changed {
                        d.revision = storage::token();
                        let id = d.id.clone();
                        if capabilities_changed {
                            self.plans.retain(|_, p| p.device_id != id);
                            self.emit("capabilities_changed",Some(&id),json!({"device_id":id,"available":true,"capabilities":self.devices[i].capabilities()}));
                        }
                        self.emit("state_changed", Some(&id), self.state_value(i)?);
                    }
                    if requested {
                        let d = &self.devices[i];
                        let id = d.id.clone();
                        let data = json!({"device_id":id,"generation":d.generation,
                            "event_id":storage::token(),"source":"arturia_logo","gesture":"short_press"});
                        self.emit("control_panel_requested", Some(&id), data);
                    }
                }
                result => {
                    let error = result.err().unwrap_or_else(|| {
                        ApiError::new(
                            "device_changed",
                            "Descriptor identity changed during the read",
                        )
                    });
                    let changed = self.devices[i].state.take().is_some()
                        || self.devices[i].error.as_ref().map(|e| e.code) != Some(error.code);
                    self.devices[i].error = Some(error);
                    self.devices[i].button_observation = None;
                    let id = self.devices[i].id.clone();
                    self.plans.retain(|_, p| p.device_id != id);
                    if changed {
                        self.emit("capabilities_changed",Some(&id),json!({"device_id":id,"generation":self.devices[i].generation,"available":false,"error":self.devices[i].error.as_ref().map(ApiError::value)}));
                    }
                }
            }
        }
        Ok(())
    }
    fn identity(&self, id: &str, generation: &str) -> Result<usize> {
        let index = self
            .devices
            .iter()
            .position(|d| d.id == id)
            .ok_or_else(|| {
                ApiError::new(
                    "device_changed",
                    "Device is no longer connected; refresh inventory",
                )
            })?;
        if self.devices[index].generation != generation {
            return Err(ApiError::new(
                "device_changed",
                "Device generation changed; refresh state",
            ));
        }
        self.state_value(index)?;
        Ok(index)
    }
    fn revision(&self, index: usize, revision: &str) -> Result<()> {
        if self.devices[index].revision != revision {
            return Err(ApiError::new(
                "stale_state",
                "Device state changed; create a new plan",
            ));
        }
        Ok(())
    }
    fn propose(&mut self, owner: &str, index: usize, plan: DevicePlan) -> Result<Value> {
        self.devices[index].validate_plan(&plan)?;
        self.plans.retain(|_, p| p.expires > Instant::now());
        if self.plans.len() >= MAX_PLANS
            || self.plans.values().filter(|p| p.owner == owner).count() >= 8
        {
            return Err(ApiError::new(
                "queue_full",
                "Too many outstanding plans; wait for expiry",
            ));
        }
        let id = storage::token();
        let d = &self.devices[index];
        let result = json!({"plan_id":id,"epoch":self.epoch,"device_id":d.id,"generation":d.generation,"revision":d.revision,"expires_in_ms":30000,"predicted_state":plan.predicted(),"summary":plan.summary()});
        self.plans.insert(
            id,
            Proposal {
                owner: owner.into(),
                device_id: d.id.clone(),
                generation: d.generation.clone(),
                revision: d.revision.clone(),
                plan,
                expires: Instant::now() + Duration::from_secs(30),
            },
        );
        Ok(result)
    }
    pub fn request(&mut self, connection: u64, request: protocol::Request) -> Value {
        let id = request.id.clone();
        let result = self.dispatch(connection, &request.method, request.params);
        match result {
            Ok(result) => json!({"api":1,"id":id,"result":result}),
            Err(e) => json!({"api":1,"id":id,"error":e.value()}),
        }
    }
    fn dispatch(&mut self, connection: u64, method: &str, value: Value) -> Result<Value> {
        if method == "hello" {
            let p: protocol::Hello = protocol::params(value)?;
            protocol::identifier(&p.client)?;
            if !p.supported_api.contains(&1) || p.supported_api.len() > 8 {
                return Err(ApiError::new("unsupported_api", "No compatible API major"));
            }
            let client = self
                .clients
                .get(&connection)
                .ok_or_else(|| ApiError::invalid("Connection is closed"))?;
            if client.token.is_some() {
                return Err(ApiError::invalid("hello is allowed once per connection"));
            }
            let token = if let Some(token) = p.client_token {
                storage::safe_id(&token)?;
                let path = self
                    .paths
                    .metadata
                    .join("clients")
                    .join(format!("{token}.json"));
                storage::read(&path)
                    .map_err(|_| ApiError::new("access_denied", "Unknown client token"))?;
                token
            } else {
                if fs::read_dir(self.paths.metadata.join("clients"))
                    .map_err(|e| ApiError::invalid(e.to_string()))?
                    .count()
                    >= 4096
                {
                    return Err(ApiError::new("queue_full", "Client identity store is full"));
                }
                let token = storage::token();
                storage::write_new(
                    &self
                        .paths
                        .metadata
                        .join("clients")
                        .join(format!("{token}.json")),
                    &json!({"client":p.client}),
                )
                .map_err(|e| ApiError::new("backup_failed", e.to_string()))?;
                token
            };
            self.clients.get_mut(&connection).unwrap().token = Some(token.clone());
            return Ok(
                json!({"selected_api":1,"service_version":env!("CARGO_PKG_VERSION"),"epoch":self.epoch,"client_token":token,"features":["plans","subscriptions","presets","durable_operations","live_controls","signal_meters"]}),
            );
        }
        let owner = self
            .clients
            .get(&connection)
            .and_then(|c| c.token.clone())
            .ok_or_else(|| ApiError::invalid("Negotiate hello first"))?;
        match method {
            "get_model_profiles" => {
                let _: protocol::Empty = protocol::params(value)?;
                let profiles: Vec<_> = [0xaf80, 0xaf82, 0xaf90, 0xaf92, 0xaf93, 0xaf70, 0xaf72]
                    .into_iter()
                    .filter_map(|pid| {
                        Model::from_pid(pid)
                            .and_then(|m| m.profile(pid))
                            .map(|profile| json!({"product_id":pid,"profile":profile}))
                    })
                    .collect();
                Ok(json!({"schema_version":1,"profiles":profiles}))
            }
            "begin_control" => self.begin_control(connection, &owner, value),
            "update_control" => self.update_control(connection, &owner, value),
            "cancel_control" => self.cancel_control(&owner, value),
            "list_devices" => {
                let _: protocol::Empty = protocol::params(value)?;
                self.refresh()?;
                Ok(json!({"devices":self.inventory()}))
            }
            "get_state" => {
                let p: protocol::Identity = protocol::params(value)?;
                self.identity(&p.device_id, &p.generation)?;
                self.refresh()?;
                let i = self.identity(&p.device_id, &p.generation)?;
                self.state_value(i)
            }
            "plan_changes" => {
                let p: protocol::Changes = protocol::params(value)?;
                let i = self.identity(&p.device_id, &p.generation)?;
                self.revision(i, &p.expected_revision)?;
                let operations: Vec<_> = p.changes.into_iter().map(Into::into).collect();
                let d = &self.devices[i];
                let plan = d.plan(&operations)?;
                self.refresh()?;
                let i = self.identity(&p.device_id, &p.generation)?;
                self.revision(i, &p.expected_revision)?;
                self.propose(&owner, i, plan)
            }
            "plan_restore" => {
                let p: protocol::Restore = protocol::params(value)?;
                let snapshot = protocol::snapshot(&p.snapshot)?;
                let i = self.identity(&p.device_id, &p.generation)?;
                self.revision(i, &p.revision)?;
                let d = &self.devices[i];
                let plan = if let Some(state) = &d.basic {
                    DevicePlan::Basic(mf_core::plan_basic_restore(
                        &d.descriptor.device,
                        state,
                        &snapshot,
                        p.mix,
                    )?)
                } else {
                    DevicePlan::Mixer(Box::new(mf_core::plan_restore(
                        &d.descriptor.device,
                        d.state.as_ref().ok_or_else(|| {
                            ApiError::new("unsupported_capability", "Mixer restore is unavailable")
                        })?,
                        &snapshot,
                        p.mix,
                    )?))
                };
                self.refresh()?;
                let i = self.identity(&p.device_id, &p.generation)?;
                self.revision(i, &p.revision)?;
                self.propose(&owner, i, plan)
            }
            "apply_plan" => {
                let p: protocol::Apply = protocol::params(value)?;
                protocol::identifier(&p.idempotency_key)?;
                self.apply(&owner, p)
            }
            "get_operation" => {
                let p: protocol::Query = protocol::params(value)?;
                if p.operation_id.is_some() == p.idempotency_key.is_some() {
                    return Err(ApiError::invalid("Choose operation_id or idempotency_key"));
                }
                self.operations
                    .values()
                    .find(|v| {
                        v["client_token"] == owner
                            && (p.operation_id.as_deref() == v["operation_id"].as_str()
                                || p.idempotency_key.as_deref() == v["idempotency_key"].as_str())
                    })
                    .map(public_operation)
                    .ok_or_else(|| {
                        ApiError::new(
                            "outcome_unknown",
                            "No retained operation matches this client identity",
                        )
                    })
            }
            "capture_snapshot" => {
                let p: protocol::Identity = protocol::params(value)?;
                self.identity(&p.device_id, &p.generation)?;
                self.refresh()?;
                let i = self.identity(&p.device_id, &p.generation)?;
                let d = &self.devices[i];
                Ok(
                    json!({"snapshot":serde_json::to_string_pretty(&d.snapshot()?).map_err(|e|ApiError::invalid(e.to_string()))?}),
                )
            }
            "inspect_snapshot" => {
                let p: protocol::Content = protocol::params(value)?;
                let s = protocol::snapshot(&p.snapshot)?;
                Ok(
                    json!({"schema_version":s.schema_version,"layout":s.layout,"model":Model::from_pid(s.device.product_id).map(|m|m.name()),"state":s.decoded().map_err(ApiError::invalid)?,"capabilities":Model::from_pid(s.device.product_id).map(|m|m.capabilities())}),
                )
            }
            "list_presets" => {
                let _: protocol::Empty = protocol::params(value)?;
                let mut presets = Vec::new();
                for entry in fs::read_dir(&self.paths.presets)
                    .map_err(|e| ApiError::invalid(e.to_string()))?
                    .take(257)
                {
                    if presets.len() >= 256 {
                        return Err(ApiError::new(
                            "queue_full",
                            "Preset store exceeds its limit",
                        ));
                    }
                    let path = entry.map_err(|e| ApiError::invalid(e.to_string()))?.path();
                    if path.extension().is_none_or(|s| s != "json") {
                        continue;
                    }
                    let v = storage::read(&path).map_err(|e| ApiError::invalid(e.to_string()))?;
                    let model = v["snapshot"]
                        .as_str()
                        .and_then(|s| protocol::snapshot(s).ok())
                        .and_then(|s| Model::from_pid(s.device.product_id))
                        .map(|m| m.name())
                        .unwrap_or("Unknown");
                    presets.push(json!({"preset_id":v["preset_id"],"name":v["name"],"model":model,"schema_version":1}));
                }
                Ok(json!({"presets":presets}))
            }
            "save_preset" => {
                let p: protocol::Save = protocol::params(value)?;
                protocol::identifier(&p.name)?;
                let snapshot = protocol::snapshot(&p.snapshot)?;
                if fs::read_dir(&self.paths.presets)
                    .map_err(|e| ApiError::invalid(e.to_string()))?
                    .count()
                    >= 256
                {
                    return Err(ApiError::new("queue_full", "Preset store is full"));
                }
                let id = storage::token();
                let record = json!({"preset_id":id,"name":p.name,"snapshot":serde_json::to_string_pretty(&snapshot).map_err(|e|ApiError::invalid(e.to_string()))?});
                storage::write_new(&self.paths.presets.join(format!("{id}.json")), &record)
                    .map_err(|e| ApiError::new("backup_failed", e.to_string()))?;
                Ok(json!({"preset_id":id,"name":record["name"]}))
            }
            "load_preset" => {
                let p: protocol::Load = protocol::params(value)?;
                storage::safe_id(&p.preset_id)?;
                storage::read(&self.paths.presets.join(format!("{}.json", p.preset_id)))
                    .map_err(|e| ApiError::invalid(e.to_string()))
            }
            "subscribe" => {
                let p: protocol::Subscribe = protocol::params(value)?;
                if p.events.len() > EVENTS.len()
                    || p.events.iter().any(|e| !EVENTS.contains(&e.as_str()))
                {
                    return Err(ApiError::new(
                        "unsupported_capability",
                        "Subscription includes an unavailable event",
                    ));
                }
                if p.device_id.is_some() != p.generation.is_some() {
                    return Err(ApiError::invalid(
                        "Device-scoped subscription needs both identity fields",
                    ));
                }
                self.refresh()?;
                if let (Some(id), Some(generation)) = (&p.device_id, &p.generation) {
                    self.identity(id, generation)?;
                }
                let c = self.clients.get_mut(&connection).unwrap();
                if c.subscribed {
                    return Err(ApiError::invalid(
                        "One subscription per connection; reconnect to change filters",
                    ));
                }
                c.events = p.events.into_iter().collect();
                c.device = p.device_id;
                c.subscribed = true;
                let states: Vec<_> = (0..self.devices.len())
                    .filter_map(|i| self.state_value(i).ok())
                    .collect();
                Ok(
                    json!({"subscription_id":storage::token(),"epoch":self.epoch,"seq":0,"devices":self.inventory(),"states":states}),
                )
            }
            _ => Err(ApiError::invalid("Unknown method")),
        }
    }
    fn apply(&mut self, owner: &str, request: protocol::Apply) -> Result<Value> {
        if let Some(existing) = self
            .operations
            .values()
            .find(|v| v["client_token"] == owner && v["idempotency_key"] == request.idempotency_key)
        {
            if existing["plan_id"] != request.plan_id {
                return Err(ApiError::new(
                    "idempotency_conflict",
                    "Key already belongs to a different immutable plan",
                ));
            }
            return Ok(public_operation(existing));
        }
        if self.live.is_some() {
            return Err(ApiError::new(
                "control_busy",
                "A continuous control owns the interface",
            ));
        }
        let proposal = self
            .plans
            .get(&request.plan_id)
            .ok_or_else(|| ApiError::new("plan_unknown", "Create a fresh plan"))?;
        if proposal.owner != owner {
            return Err(ApiError::new(
                "plan_unknown",
                "Plan does not belong to this client",
            ));
        }
        if proposal.expires <= Instant::now() {
            return Err(ApiError::new("plan_expired", "Create a fresh plan"));
        }
        self.refresh()?;
        let proposal = self.plans.get(&request.plan_id).ok_or_else(|| {
            ApiError::new(
                "device_changed",
                "Device was reconnected; create a fresh plan",
            )
        })?;
        let i = self.identity(&proposal.device_id, &proposal.generation)?;
        self.revision(i, &proposal.revision)?;
        self.devices[i].validate_plan(&proposal.plan)?;
        if self.operations.len() >= MAX_OPERATIONS || self.work.len() >= 32 {
            return Err(ApiError::new(
                "queue_full",
                "Operation store is full; explicitly archive resolved records",
            ));
        }
        let id = storage::token();
        let record = json!({"schema_version":1,"operation_id":id,"client_token":owner,"idempotency_key":request.idempotency_key,"plan_id":request.plan_id,"plan_digest":proposal.plan.summary(),"epoch":self.epoch,"device_id":proposal.device_id,"generation":proposal.generation,"status":"pending"});
        let path = self
            .paths
            .metadata
            .join("operations")
            .join(format!("{id}.json"));
        storage::write_new(&path, &record)
            .map_err(|e| ApiError::new("backup_failed", e.to_string()))?;
        let proposal = self.plans.remove(&request.plan_id).unwrap();
        self.operations.insert(id.clone(), record.clone());
        self.work.push_back(Work {
            id,
            proposal,
            record: record.clone(),
            path,
        });
        Ok(public_operation(&record))
    }
    fn execute_plan(&mut self, plan: &DevicePlan, directory: &std::path::Path) -> Result<Value> {
        match plan {
            DevicePlan::Mixer(p) => self
                .hardware
                .execute(p, directory)
                .map(|o| outcome_value!(o)),
            DevicePlan::Basic(p) => self
                .hardware
                .execute_basic(p, directory)
                .map(|o| outcome_value!(o)),
        }
        .map_err(ApiError::from)
    }
    pub fn execute_next(&mut self) {
        if self.live_work.is_some() {
            self.execute_control();
            return;
        }
        let Some(Work {
            id,
            proposal,
            mut record,
            path,
        }) = self.work.pop_front()
        else {
            return;
        };
        self.emit(
            "operation_started",
            Some(&proposal.device_id),
            json!({"operation_id":id,"device_id":proposal.device_id}),
        );
        let ready = self.refresh().and_then(|()| {
            let index = self.identity(&proposal.device_id, &proposal.generation)?;
            self.revision(index, &proposal.revision)?;
            self.devices[index].validate_plan(&proposal.plan)
        });
        let directory = self.paths.backups.clone();
        let outcome = ready.and_then(|()| self.execute_plan(&proposal.plan, &directory));
        record["status"] = "completed".into();
        match outcome {
            Ok(outcome) => {
                for (key, value) in outcome.as_object().expect("Typed outcome object") {
                    record[key] = value.clone();
                }
            }
            Err(error) => {
                record["device_outcome"] = "rejected".into();
                record["device_write_attempted"] = false.into();
                record["rollback_attempted"] = false.into();
                record["rollback_verified"] = false.into();
                record["after_state_valid"] = false.into();
                record["journal_status"] = "not_needed".into();
                record["error"] = error.value();
            }
        }
        if let Err(error) = storage::update(&path, &record) {
            record["status"] = "indeterminate".into();
            record["metadata_error"] = error.to_string().into();
        }
        self.operations.insert(id, record.clone());
        // Invalidate the cached state even after a verified transaction; refresh
        // before the next plan, preserving the core's full-state final guard.
        let _ = self.refresh();
        self.emit(
            "operation_finished",
            Some(&proposal.device_id),
            public_operation(&record),
        );
    }
}

#[cfg(test)]
mod expiry_tests {
    use super::*;
    struct NoHardware;
    impl Hardware for NoHardware {
        fn inventory(&mut self) -> Result<Vec<Descriptor>> {
            panic!("Expired plans must not call hardware")
        }
        fn read(&mut self) -> Result<(mf_core::Device, State)> {
            panic!("Expired plans must not read")
        }
        fn execute(
            &mut self,
            _: &Plan,
            _: &std::path::Path,
        ) -> std::result::Result<mf_core::Outcome, mf_core::Error> {
            panic!("Expired plans must not dispatch")
        }
    }
    #[test]
    fn expiry_is_checked_before_discovery_or_dispatch() {
        let root = std::env::temp_dir().join(format!(
            "mf-expiry-{}-{}",
            std::process::id(),
            storage::token()
        ));
        let paths = Paths {
            metadata: root.join("metadata"),
            presets: root.join("presets"),
            backups: root.join("backups"),
        };
        let mut c = Coordinator::new(NoHardware, paths).unwrap();
        let device = mf_core::Device {
            vendor_id: mf_core::VID,
            product_id: mf_core::MF4_PID,
            bus: 0,
            address: 0,
        };
        let plan = mf_core::plan(
            &device,
            &State([0; 190]),
            &[mf_core::Operation::Master {
                mix: mf_core::Mix::Out12,
                level: mf_core::Level::Silence,
            }],
        )
        .unwrap();
        c.plans.insert(
            "expired".into(),
            Proposal {
                owner: "owner".into(),
                device_id: "device".into(),
                generation: "generation".into(),
                revision: "revision".into(),
                plan: DevicePlan::Mixer(Box::new(plan)),
                expires: Instant::now() - Duration::from_secs(1),
            },
        );
        let error = c
            .apply(
                "owner",
                protocol::Apply {
                    plan_id: "expired".into(),
                    idempotency_key: "key".into(),
                },
            )
            .unwrap_err();
        assert_eq!(error.code, "plan_expired");
        fs::remove_dir_all(root).unwrap();
    }
}
