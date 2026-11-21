//! Machines: the list on this computer, its file, and the project's machine.

use wham_engine::machine::{from_text, to_text};
use wham_engine::*;

#[test]
fn machines_get_unique_names() {
    let mut m = Machines::default();
    assert_eq!(m.add_machine(), "Machine 1");
    assert_eq!(m.add_machine(), "Machine 2");
    assert_eq!(
        m.rename_machine("Machine 1", " WorkBee ").unwrap(),
        "WorkBee"
    );
    assert!(m.rename_machine("Machine 2", "WorkBee").is_err());
    assert!(m.rename_machine("Machine 2", "  ").is_err());
    assert!(m.rename_machine("Nope", "X").is_err());
    assert_eq!(m.machine("WorkBee").unwrap().post, PostProcessor::Generic);
    assert!(m.delete_machine("WorkBee"));
    assert!(!m.delete_machine("WorkBee"));
    assert_eq!(m.machines.len(), 1);
}

#[test]
fn machines_file_round_trip_and_damage() {
    let mut m = Machines::default();
    m.add_machine();
    m.add_machine();
    let text = to_text(&m);
    assert!(text.contains("\"format\": \"WHAM_CAM machines\""));
    assert_eq!(from_text(&text).unwrap(), m);
    assert!(from_text("nope").is_err());
    let twice = text.replace("Machine 2", "Machine 1");
    assert!(from_text(&twice).unwrap_err().contains("two machines"));
    let newer = text.replace("\"format_version\": 1", "\"format_version\": 9");
    assert!(from_text(&newer).unwrap_err().contains("newer"));
}

#[test]
fn the_project_names_its_machine_and_saves_it() {
    let mut p = Project::new();
    assert_eq!(p.document().machine, None);
    p.set_machine(Some("WorkBee".into()));
    assert_eq!(p.document().machine.as_deref(), Some("WorkBee"));
    let text = p.save_text("job");
    let mut other = Project::new();
    other.open(&text, "job").unwrap();
    assert_eq!(other.document().machine.as_deref(), Some("WorkBee"));
    p.undo();
    assert_eq!(p.document().machine, None);
}

#[test]
fn a_machine_cannot_be_called_generic() {
    let mut m = Machines::default();
    let name = m.add_machine();
    for bad in ["Generic", "generic", "  GENERIC "] {
        let err = m.rename_machine(&name, bad).unwrap_err();
        assert!(err.contains("no machine"), "{err}");
    }
    assert_eq!(
        m.rename_machine(&name, "Generic router").unwrap(),
        "Generic router"
    );
}

#[test]
fn a_new_machine_has_the_default_settings() {
    let mut m = Machines::default();
    let name = m.add_machine();
    let machine = m.machine(&name).unwrap();
    assert_eq!(
        machine.rapid,
        RapidRates {
            xy: 5000.0,
            z: 5000.0
        }
    );
    assert!(!machine.tool_changer);
    assert_eq!(machine.travel_x, None);
    assert_eq!(machine.travel_y, None);
    assert_eq!(machine.travel_z, None);
    assert_eq!(machine.spindle_control, SpindleControl::VariableRpm);
    assert_eq!(machine.max_spindle_rpm, None);
    assert_eq!(machine.max_feed, None);
}

#[test]
fn machine_settings_are_checked_and_saved() {
    let mut m = Machines::default();
    m.add_machine();
    let mut s = m.machine("Machine 1").unwrap().clone();
    s.name = "ignored".into();
    s.rapid = RapidRates {
        xy: 10000.0,
        z: 3000.0,
    };
    s.tool_changer = true;
    s.travel_x = Some(Travel {
        min: 0.0,
        max: 600.0,
    });
    s.travel_z = Some(Travel {
        min: -100.0,
        max: 0.0,
    });
    s.spindle_control = SpindleControl::OnOff;
    s.max_spindle_rpm = Some(24000.0);
    s.max_feed = Some(6000.0);
    m.set_settings("Machine 1", &s).unwrap();
    let got = m.machine("Machine 1").unwrap();
    // The name is not changed by the settings.
    assert_eq!(got.name, "Machine 1");
    assert_eq!(got.rapid.z, 3000.0);
    assert_eq!(
        got.travel_z,
        Some(Travel {
            min: -100.0,
            max: 0.0
        })
    );
    // Saved and read back exactly.
    assert_eq!(from_text(&to_text(&m)).unwrap(), m);

    // Invalid settings change nothing.
    let before = m.clone();
    for bad in [
        Machine {
            rapid: RapidRates { xy: 0.0, z: 3000.0 },
            ..s.clone()
        },
        Machine {
            rapid: RapidRates {
                xy: 5000.0,
                z: -1.0,
            },
            ..s.clone()
        },
        Machine {
            travel_y: Some(Travel { min: 5.0, max: 5.0 }),
            ..s.clone()
        },
        Machine {
            travel_x: Some(Travel {
                min: 600.0,
                max: 0.0,
            }),
            ..s.clone()
        },
        Machine {
            max_spindle_rpm: Some(0.0),
            ..s.clone()
        },
        Machine {
            max_feed: Some(f64::NAN),
            ..s.clone()
        },
    ] {
        assert!(m.set_settings("Machine 1", &bad).is_err());
    }
    assert_eq!(m, before);
    assert!(m.set_settings("Nope", &s).is_err());
}

#[test]
fn a_machines_file_with_only_name_and_post_gets_the_defaults() {
    let text = r#"{ "format": "WHAM_CAM machines", "format_version": 1,
        "machines": [ { "name": "Old", "post": "Generic" } ] }"#;
    let m = from_text(text).unwrap();
    assert_eq!(m.machine("Old").unwrap(), &Machine::new("Old"));
}

#[test]
fn a_machines_file_with_bad_settings_is_damaged() {
    let mut m = Machines::default();
    m.add_machine();
    let text = to_text(&m).replace("\"xy\": 5000.0", "\"xy\": 0.0");
    assert!(from_text(&text).unwrap_err().contains("Rapid rates"));
}

#[test]
fn rapid_rates_come_from_the_projects_machine() {
    let mut m = Machines::default();
    m.add_machine();
    let mut s = Machine::new("x");
    s.rapid = RapidRates {
        xy: 8000.0,
        z: 2000.0,
    };
    m.set_settings("Machine 1", &s).unwrap();
    assert_eq!(m.rapid_rates(Some("Machine 1")).z, 2000.0);
    // Generic (none chosen) and a machine not on this computer use the defaults.
    assert_eq!(m.rapid_rates(None), RapidRates::default());
    assert_eq!(m.rapid_rates(Some("Gone")), RapidRates::default());
}
