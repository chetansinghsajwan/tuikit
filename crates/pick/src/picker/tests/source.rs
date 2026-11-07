use super::super::*;

#[test]
fn files_source_marks_dirs_only_when_asked() {
    let d = std::env::temp_dir().join(format!("pick-src-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("sub/f.txt"), "x").unwrap();
    for (dirs, want) in [(false, 1u32), (true, 2u32)] {
        let opts = PickerOptions {
            mode: Mode::Fuzzy,
            paths: true,
            root: d.clone(),
            preview: false,
            git: false,
            query: String::new(),
        };
        let mut p = Picker::new(opts, Arc::new(|| {}));
        crate::source::spawn_files(p.injector(), d.clone(), dirs, None);
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            p.nucleo.tick(10);
            if p.matched() >= want {
                break;
            }
        }
        assert_eq!(p.matched(), want, "dirs={dirs}");
        assert_eq!(p.nucleo.snapshot().matched_items(..).any(|i| i.data.display == "sub/"), dirs);
    }
}
