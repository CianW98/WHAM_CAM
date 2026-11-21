//! Machines: the list on this computer (with the built-in Default), its file, and the
//! project's copy of its machine's settings.

use wham_engine::machine::{DEFAULT_MACHINE, from_text, to_text};
use wham_engine::*;

/// Settings different from Default in every way.
fn shop_settings() -> MachineSettings {
    MachineSettings {
        post: PostProcessor::Generic,
        rapid: RapidRates {
            xy: 10000.0,
            z: 3000.0,
        },
        tool_changer: true,
        travel_x: Some(Travel {
            min: 0.0,
            max: 600.0,
        }),
        travel_y: Some(Travel {
            min: 0.0,
            max: 400.0,
        }),
        travel_z: Some(Travel {
            min: -100.0,
            max: 0.0,
        }),
        spindle_control: SpindleControl::OnOff,
        max_spindle_rpm: Some(24000.0),
        max_feed: Some(6000.0),
    }
}

/// Machines with Default and "Shop" (shop settings).
fn shop() -> Machines {
    let mut m = Machines::default();
    let name = m.add_machine();
    m.rename_machine(&name, "Shop").unwrap();
    m.set_settings("Shop", &shop_settings()).unwrap();
    m
}

#[test]
fn default_is_always_there_and_read_only() {
    let mut m = Machines::default();
    assert_eq!(m.machines.len(), 1);
    let d = m.machine(DEFAULT_MACHINE).unwrap();
    assert_eq!(d.settings, MachineSettings::default());
    assert_eq!(
        d.settings.rapid,
        RapidRates {
            xy: 5000.0,
            z: 5000.0
        }
    );
    assert!(!d.settings.tool_changer);
    assert_eq!(d.settings.travel_x, None);
    assert_eq!(d.settings.spindle_control, SpindleControl::VariableRpm);
    assert_eq!(d.settings.max_spindle_rpm, None);
    assert_eq!(d.settings.max_feed, None);
    assert_eq!(m.for_new().name, DEFAULT_MACHINE);

    assert!(m.delete_machine(DEFAULT_MACHINE).is_err());
    assert!(m.rename_machine(DEFAULT_MACHINE, "Mine").is_err());
    assert!(m.set_settings(DEFAULT_MACHINE, &shop_settings()).is_err());
    assert_eq!(m, Machines::default());
}

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
    for bad in ["Default", "default", " DEFAULT "] {
        assert!(m.rename_machine("Machine 2", bad).is_err(), "{bad}");
    }
    assert!(m.delete_machine("WorkBee").is_ok());
    assert!(m.delete_machine("WorkBee").is_err());
    assert_eq!(m.machines.len(), 2);
}

#[test]
fn new_machines_copy_default_and_new_from_copies_the_machine() {
    let mut m = shop();
    let added = m.add_machine();
    assert_eq!(
        m.machine(&added).unwrap().settings,
        MachineSettings::default()
    );
    assert_eq!(m.new_from("Shop").unwrap(), "Shop copy");
    assert_eq!(m.new_from("Shop").unwrap(), "Shop copy 1");
    assert_eq!(m.machine("Shop copy").unwrap().settings, shop_settings());
    // A copy of Default is editable.
    let copy = m.new_from(DEFAULT_MACHINE).unwrap();
    assert_eq!(copy, "Default copy");
    m.set_settings(&copy, &shop_settings()).unwrap();
    assert!(m.new_from("Nope").is_err());
}

#[test]
fn the_machine_for_new_projects() {
    let mut m = shop();
    m.set_for_new_projects("Shop").unwrap();
    assert_eq!(m.for_new().name, "Shop");
    m.rename_machine("Shop", "Router").unwrap();
    assert_eq!(m.for_new_projects, "Router");
    // Deleting it goes back to Default.
    m.delete_machine("Router").unwrap();
    assert_eq!(m.for_new().name, DEFAULT_MACHINE);
    assert!(m.set_for_new_projects("Nope").is_err());
}

#[test]
fn machine_settings_are_checked() {
    let mut m = shop();
    let before = m.clone();
    let s = shop_settings();
    for bad in [
        MachineSettings {
            rapid: RapidRates { xy: 0.0, z: 3000.0 },
            ..s.clone()
        },
        MachineSettings {
            rapid: RapidRates {
                xy: 5000.0,
                z: -1.0,
            },
            ..s.clone()
        },
        MachineSettings {
            travel_y: Some(Travel { min: 5.0, max: 5.0 }),
            ..s.clone()
        },
        MachineSettings {
            travel_x: Some(Travel {
                min: 600.0,
                max: 0.0,
            }),
            ..s.clone()
        },
        MachineSettings {
            max_spindle_rpm: Some(0.0),
            ..s.clone()
        },
        MachineSettings {
            max_feed: Some(f64::NAN),
            ..s.clone()
        },
    ] {
        assert!(m.set_settings("Shop", &bad).is_err());
    }
    assert_eq!(m, before);
    assert!(m.set_settings("Nope", &s).is_err());
}

#[test]
fn machines_file_round_trip_and_damage() {
    let mut m = shop();
    m.set_for_new_projects("Shop").unwrap();
    let text = to_text(&m);
    assert!(text.contains("\"format\": \"WHAM_CAM machines\""));
    assert_eq!(from_text(&text).unwrap(), m);
    assert!(from_text("nope").is_err());
    let newer = text.replace("\"format_version\": 1", "\"format_version\": 9");
    assert!(from_text(&newer).unwrap_err().contains("newer"));
    let mut two = shop();
    two.add_machine();
    let twice = to_text(&two).replace("Machine 1", "Shop");
    assert!(from_text(&twice).unwrap_err().contains("two machines"));
    let bad = text.replace("\"xy\": 10000.0", "\"xy\": 0.0");
    assert!(from_text(&bad).unwrap_err().contains("Rapid rates"));
    let missing = text.replace(
        "\"for_new_projects\": \"Shop\"",
        "\"for_new_projects\": \"Gone\"",
    );
    assert!(from_text(&missing).unwrap_err().contains("new projects"));
}

#[test]
fn the_file_never_changes_default() {
    // A file whose Default was edited, or that has no Default, still gets the built-in one.
    let text = r#"{ "format": "WHAM_CAM machines", "format_version": 1,
        "machines": [ { "name": "Default", "rapid": { "xy": 1.0, "z": 1.0 } },
                      { "name": "Old" } ] }"#;
    let m = from_text(text).unwrap();
    assert_eq!(m.machines[0], Machine::built_in());
    // Settings missing from the file get the Default machine's values.
    assert_eq!(
        m.machine("Old").unwrap().settings,
        MachineSettings::default()
    );
    assert_eq!(m.for_new_projects, DEFAULT_MACHINE);
    let none = r#"{ "format": "WHAM_CAM machines", "format_version": 1, "machines": [] }"#;
    assert_eq!(from_text(none).unwrap(), Machines::default());
}

// --- The project's machine ----------------------------------------------------------------------

#[test]
fn a_project_copies_its_machines_settings_and_saves_them() {
    let m = shop();
    let mut p = Project::new();
    assert_eq!(p.document().machine, DEFAULT_MACHINE);
    assert_eq!(p.document().machine_settings, MachineSettings::default());
    p.apply_machine(m.machine("Shop").unwrap());
    assert_eq!(p.document().machine, "Shop");
    assert_eq!(p.document().machine_settings, shop_settings());
    // Not an undo step, but unsaved.
    assert!(!p.can_undo());
    assert!(p.is_modified());
    let text = p.save_text("job");
    let mut other = Project::new();
    other.open(&text, "job").unwrap();
    assert_eq!(other.document().machine, "Shop");
    assert_eq!(other.document().machine_settings, shop_settings());
}

#[test]
fn a_new_project_takes_the_machine_for_new_projects() {
    let m = shop();
    let mut p = Project::new();
    p.new_project(
        &ProjectTemplate::built_in("Default"),
        m.machine("Shop").unwrap(),
    );
    assert_eq!(p.document().machine, "Shop");
    assert_eq!(p.document().machine_settings, shop_settings());
    assert!(!p.is_modified());
}

#[test]
fn project_changes_win_over_setup_changes() {
    let mut m = shop();
    let templates = ProjectTemplates::default();
    let mut p = Project::new();
    p.apply_machine(m.machine("Shop").unwrap());
    // Changed in the project: rapid Z only.
    let mut own = p.document().machine_settings.clone();
    own.rapid.z = 2000.0;
    p.set_machine_settings(&own).unwrap();
    assert_eq!(
        p.document()
            .machine_overrides
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![MachineSetting::RapidZ]
    );
    // Setup changes both rapid rates: only XY follows.
    let mut s = shop_settings();
    s.rapid = RapidRates {
        xy: 12000.0,
        z: 4000.0,
    };
    m.set_settings("Shop", &s).unwrap();
    p.follow_setup(&templates, &m);
    assert_eq!(
        p.document().machine_settings.rapid,
        RapidRates {
            xy: 12000.0,
            z: 2000.0
        }
    );
    // Reset puts it back to the machine's value, and it follows again.
    p.reset_machine_setting(m.machine("Shop").unwrap(), MachineSetting::RapidZ);
    assert_eq!(p.document().machine_settings.rapid.z, 4000.0);
    assert!(p.document().machine_overrides.is_empty());
    // Picking a machine again starts fresh: nothing counts as changed in the project.
    p.set_machine_settings(&own).unwrap();
    p.apply_machine(m.machine("Shop").unwrap());
    assert!(p.document().machine_overrides.is_empty());
    // Invalid project settings are refused.
    let mut bad = own.clone();
    bad.rapid.xy = 0.0;
    assert!(p.set_machine_settings(&bad).is_err());
}

#[test]
fn a_project_follows_its_machine_being_renamed_but_keeps_values_if_deleted() {
    let mut m = shop();
    let templates = ProjectTemplates::default();
    let mut p = Project::new();
    p.apply_machine(m.machine("Shop").unwrap());
    m.rename_machine("Shop", "Router").unwrap();
    p.machine_renamed("Shop", "Router");
    assert_eq!(p.document().machine, "Router");
    m.delete_machine("Router").unwrap();
    p.follow_setup(&templates, &m);
    assert_eq!(p.document().machine, "Router");
    assert_eq!(p.document().machine_settings, shop_settings());
    // Another machine's rename doesn't touch it.
    p.machine_renamed("Other", "X");
    assert_eq!(p.document().machine, "Router");
}

#[test]
fn opening_a_project_whose_machine_changed_since() {
    let mut m = shop();
    let templates = ProjectTemplates::default();
    let mut p = Project::new();
    p.apply_machine(m.machine("Shop").unwrap());
    let text = p.save_text("job");
    assert!(p.setup_mismatches(&templates, &m).is_empty());

    let mut s = shop_settings();
    s.max_feed = Some(9000.0);
    m.set_settings("Shop", &s).unwrap();

    // Keep: the project's value stays and counts as changed in the project.
    let mut keep = Project::new();
    keep.open(&text, "job").unwrap();
    assert_eq!(
        keep.setup_mismatches(&templates, &m),
        vec!["machine \"Shop\""]
    );
    keep.keep_project_values(&templates, &m);
    assert_eq!(keep.document().machine_settings.max_feed, Some(6000.0));
    assert!(
        keep.document()
            .machine_overrides
            .contains(&MachineSetting::MaxFeed)
    );
    assert!(keep.setup_mismatches(&templates, &m).is_empty());

    // Use the machine's values.
    let mut follow = Project::new();
    follow.open(&text, "job").unwrap();
    follow.follow_setup(&templates, &m);
    assert_eq!(follow.document().machine_settings.max_feed, Some(9000.0));
    assert!(follow.is_modified());
}
