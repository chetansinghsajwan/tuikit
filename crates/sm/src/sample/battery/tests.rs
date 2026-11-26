//! Tests for reading the battery.

use super::*;

#[test]
fn pmset_gives_the_percent_the_state_and_the_time_left() {
    let text = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=4653155)\t85%; discharging; 3:45 remaining present: true\n";
    assert_eq!(
        parse_pmset(text),
        Some(Battery {
            percent: 85.0,
            charge: Charge::Discharging,
            time_left: Some(3 * 3600 + 45 * 60)
        })
    );
    let charged = " -InternalBattery-0 (id=1)\t100%; charged; 0:00 remaining present: true";
    assert_eq!(parse_pmset(charged).map(|b| b.charge), Some(Charge::Full));
    let calculating = " -InternalBattery-0 (id=1)\t40%; charging; (no estimate) present: true";
    assert_eq!(parse_pmset(calculating).and_then(|b| b.time_left), None);
    assert_eq!(parse_pmset("Now drawing from 'AC Power'\n"), None);
}

#[cfg(target_os = "linux")]
#[test]
fn sysfs_gives_the_first_battery_and_how_long_it_lasts() {
    let root = std::env::temp_dir().join(format!("sm-battery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let write = |dir: &str, name: &str, value: &str| {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(name), value).unwrap();
    };
    write("AC", "type", "Mains\n");
    write("BAT0", "type", "Battery\n");
    write("BAT0", "capacity", "50\n");
    write("BAT0", "status", "Discharging\n");
    write("BAT0", "energy_now", "20000000\n");
    write("BAT0", "power_now", "10000000\n");
    let b = linux::read(&root).expect("a battery");
    assert_eq!((b.percent, b.charge, b.time_left), (50.0, Charge::Discharging, Some(7200)));
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(linux::read(&root), None);
}
