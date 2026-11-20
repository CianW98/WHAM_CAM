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
