//! Tests for the panels.

use super::*;

const BODY: Rect = Rect { x: 0, y: 0, width: 100, height: 40 };

#[test]
fn the_last_panel_cannot_be_hidden() {
    let mut panels = Panels::without(&[Panel::Cpu, Panel::Memory, Panel::Network, Panel::Sensors]);
    assert!(panels.is_shown(Panel::Processes));
    assert!(!panels.toggle(Panel::Processes));
    assert!(panels.is_shown(Panel::Processes));
    assert!(panels.toggle(Panel::Cpu));
}

#[test]
fn hiding_every_panel_keeps_the_last_one() {
    let panels = Panels::without(&[
        Panel::Cpu,
        Panel::Memory,
        Panel::Network,
        Panel::Sensors,
        Panel::Processes,
    ]);
    assert!(panels.is_shown(Panel::Processes));
}

#[test]
fn every_panel_gets_a_place_and_they_do_not_overlap() {
    let p = Panels::default().place(BODY);
    let rects = [p.cpu, p.memory, p.network, p.sensors, p.processes].map(|r| r.expect("shown"));
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            assert!(a.intersection(*b).is_empty());
        }
    }
}

#[test]
fn processes_alone_take_the_whole_body() {
    let p =
        Panels::without(&[Panel::Cpu, Panel::Memory, Panel::Network, Panel::Sensors]).place(BODY);
    assert_eq!(p.processes, Some(BODY));
    assert_eq!(p.cpu, None);
}

#[test]
fn without_processes_the_top_takes_the_whole_body() {
    let p = Panels::without(&[Panel::Processes]).place(BODY);
    assert_eq!(p.processes, None);
    assert_eq!(p.cpu.unwrap().height, BODY.height);
}

#[test]
fn a_lone_top_panel_takes_the_whole_top() {
    let p = Panels::without(&[Panel::Cpu, Panel::Network, Panel::Sensors]).place(BODY);
    assert_eq!(p.memory.unwrap().width, BODY.width);
}

#[test]
fn panels_are_parsed_by_name() {
    assert_eq!(Panel::parse("Net"), Some(Panel::Network));
    assert_eq!(Panel::parse("nope"), None);
}

#[test]
fn the_sensors_take_the_right_of_the_top_or_all_of_it_alone() {
    let p = Panels::default().place(BODY);
    let sensors = p.sensors.expect("shown");
    assert_eq!(sensors.right(), BODY.right());
    assert_eq!(sensors.width, 25);
    let alone = Panels::without(&[Panel::Cpu, Panel::Memory, Panel::Network]).place(BODY);
    assert_eq!(alone.sensors.map(|r| r.width), Some(BODY.width));
}
