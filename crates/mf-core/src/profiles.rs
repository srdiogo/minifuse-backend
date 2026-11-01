// SPDX-License-Identifier: MIT
//! Model-specific presentation facts; a profile never grants transport access.
use crate::Model;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceMode {
    Control,
    Initialization,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputControl {
    pub target: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModelProfile {
    pub schema_version: u8,
    pub id: &'static str,
    pub model: &'static str,
    pub device_mode: DeviceMode,
    pub layout: &'static str,
    pub analog_inputs: u8,
    pub analog_outputs: u8,
    pub input_controls: Vec<InputControl>,
    pub hardware_validation: &'static str,
    pub notes: Vec<&'static str>,
}

impl Model {
    pub fn profile(self, pid: u16) -> Option<ModelProfile> {
        if Model::from_pid(pid) != Some(self) {
            return None;
        }
        let mode = if matches!(pid, 0xaf82 | 0xaf92 | 0xaf72) {
            DeviceMode::Initialization
        } else {
            DeviceMode::Control
        };
        let (id, inputs, outputs) = match self {
            Self::MiniFuse1 => ("mf1", 1, 2),
            Self::MiniFuse2 => ("mf2", 2, 2),
            Self::MiniFuse2Otg => ("mf2_otg", 2, 2),
            Self::MiniFuse4 => ("mf4", 4, 4),
        };
        let mut controls = vec![InputControl {
            target: "inst1",
            label: "INST 1",
            description: "Instrument mode for input 1",
        }];
        if self != Self::MiniFuse1 {
            controls.push(InputControl {
                target: "inst2",
                label: "INST 2",
                description: "Instrument mode for input 2",
            });
        }
        controls.push(InputControl {
            target: "phantom_power",
            label: if self == Self::MiniFuse1 {
                "+48 V"
            } else {
                "+48 V · 1–2"
            },
            description: if self == Self::MiniFuse1 {
                "Phantom power for input 1"
            } else {
                "Shared phantom power for inputs 1 and 2"
            },
        });
        if self != Self::MiniFuse4 {
            controls.push(InputControl {
                target: "direct_monitor",
                label: if self == Self::MiniFuse1 {
                    "Direct monitor"
                } else {
                    "Direct mono"
                },
                description: if self == Self::MiniFuse1 {
                    "Direct monitoring of the physical input"
                } else {
                    "Mono summing for direct monitoring"
                },
            });
        }
        let notes = if mode == DeviceMode::Initialization {
            controls.clear();
            vec![
                "Initialization/storage USB mode: audio controls are unavailable. This application does not change device mode or firmware.",
            ]
        } else if self == Self::MiniFuse4 {
            vec!["Mixer, input controls and meters have bounded validation on one MiniFuse 4."]
        } else {
            vec![
                "Model-specific hardware validation is pending. A presentation profile does not enable unverified USB commands.",
                "Physical gain knobs and host software-loopback features are not remote hardware mixer controls.",
            ]
        };
        Some(ModelProfile {
            schema_version: 1,
            id,
            model: self.name(),
            device_mode: mode,
            layout: if mode == DeviceMode::Initialization {
                "initialization"
            } else if self == Self::MiniFuse4 {
                "mf4_mixer"
            } else {
                "inputs_and_monitoring"
            },
            analog_inputs: inputs,
            analog_outputs: outputs,
            input_controls: controls,
            hardware_validation: if self == Self::MiniFuse4 && mode == DeviceMode::Control {
                "bounded_reference"
            } else {
                "pending"
            },
            notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initialization_ids_never_offer_controls() {
        for pid in [0xaf82, 0xaf92, 0xaf72] {
            let profile = Model::from_pid(pid).unwrap().profile(pid).unwrap();
            assert_eq!(profile.device_mode, DeviceMode::Initialization);
            assert!(profile.input_controls.is_empty());
            assert_eq!(profile.layout, "initialization");
        }
    }
    #[test]
    fn each_model_has_its_own_controls_without_mf4_mixer_guesses() {
        for (pid, id, count) in [
            (0xaf80, "mf1", 3),
            (0xaf90, "mf2", 4),
            (0xaf93, "mf2_otg", 4),
            (0xaf70, "mf4", 3),
        ] {
            let profile = Model::from_pid(pid).unwrap().profile(pid).unwrap();
            assert_eq!(profile.id, id);
            assert_eq!(profile.input_controls.len(), count);
            assert_eq!(profile.layout == "mf4_mixer", pid == 0xaf70);
        }
        assert!(Model::MiniFuse1.profile(0xaf90).is_none());
        assert!(Model::from_pid(0xaf91).is_none());
    }
    #[test]
    fn public_presentation_fixture_matches_the_model_profiles() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/models/profiles-v1.json")).unwrap();
        for device in fixture["devices"].as_array().unwrap() {
            let pid = device["product_id"].as_u64().unwrap() as u16;
            let actual =
                serde_json::to_value(Model::from_pid(pid).unwrap().profile(pid).unwrap()).unwrap();
            for (key, value) in device["profile"].as_object().unwrap() {
                assert_eq!(&actual[key], value, "PID {pid:04x}, field {key}");
            }
        }
    }
}
