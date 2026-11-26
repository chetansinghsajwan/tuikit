//! Tests for the disk health.

use super::*;

#[test]
fn a_partition_names_its_whole_disk() {
    assert_eq!(whole_disk("/dev/nvme0n1p2"), "/dev/nvme0n1");
    assert_eq!(whole_disk("/dev/nvme0n1"), "/dev/nvme0n1");
    assert_eq!(whole_disk("/dev/mmcblk0p1"), "/dev/mmcblk0");
    assert_eq!(whole_disk("/dev/sda1"), "/dev/sda");
    assert_eq!(whole_disk("disk3s1s1"), "disk3");
    assert_eq!(whole_disk("/dev/mapper/root"), "/dev/mapper/root");
}

#[test]
fn smartctl_and_diskutil_say_good_or_failing() {
    assert_eq!(parse_smartctl(r#"{"smart_status":{"passed":true}}"#), Some(Health::Good));
    assert_eq!(parse_smartctl(r#"{"smart_status":{"passed":false}}"#), Some(Health::Failing));
    assert_eq!(parse_smartctl(r#"{"smartctl":{"exit_status":2}}"#), None);
    assert_eq!(parse_diskutil("   SMART Status:              Verified\n"), Some(Health::Good));
    assert_eq!(parse_diskutil("   SMART Status:              Not Supported\n"), None);
}
