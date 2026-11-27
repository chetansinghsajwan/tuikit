//! Tests for reading the graphics card.

use super::*;

#[test]
fn nvidia_smi_gives_the_name_the_load_the_memory_and_the_temperature() {
    let gpu = parse_nvidia("NVIDIA GeForce RTX 4060, 37, 1024, 8192, 51\n").expect("a card");
    assert_eq!(gpu.name, "NVIDIA GeForce RTX 4060");
    assert_eq!((gpu.percent, gpu.celsius), (37.0, Some(51.0)));
    assert_eq!((gpu.memory_used, gpu.memory_total), (1024 << 20, 8192 << 20));
    assert_eq!(parse_nvidia(""), None);
    assert_eq!(parse_nvidia("NVIDIA, [N/A], 1, 2, 3"), None, "no load, no card to show");
}

#[cfg(target_os = "linux")]
#[test]
fn an_amd_card_is_read_from_its_sysfs_folder() {
    let root = std::env::temp_dir().join(format!("sm-gpu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("card0/device");
    let write = |path: &str, value: &str| {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, value).unwrap();
    };
    write("gpu_busy_percent", "42\n");
    write("mem_info_vram_used", "1000\n");
    write("mem_info_vram_total", "4000\n");
    write("hwmon/hwmon3/temp1_input", "61000\n");
    assert_eq!(amd_cards(&root), std::slice::from_ref(&dir));
    let gpu = read_amd(&dir).expect("a card");
    assert_eq!((gpu.percent, gpu.memory_used, gpu.memory_total), (42.0, 1000, 4000));
    assert_eq!(gpu.celsius, Some(61.0));
    std::fs::remove_dir_all(&root).unwrap();
    assert!(amd_cards(&root).is_empty());
}
