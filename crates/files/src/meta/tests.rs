use super::*;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("files-meta-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn text(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
}

#[test]
fn relative_times() {
    let ago_of =
        |s: u64| time_relative(Some(SystemTime::now() - std::time::Duration::from_secs(s)));
    assert_eq!(ago_of(5), "now");
    assert_eq!(ago_of(300), "5m ago");
    assert_eq!(ago_of(4 * 3600), "4h ago");
    assert_eq!(ago_of(3 * 86_400), "3d ago");
    assert_eq!(time_relative(None), "");
}

#[test]
fn human_sizes() {
    assert_eq!(size_short(0), "0");
    assert_eq!(size_short(1536), "1.5K");
    assert_eq!(size_short(15 << 20), "15M");
    assert_eq!(human(0), "0 B");
    assert_eq!(human(1023), "1023 B");
    assert_eq!(human(1536), "1.5 KiB");
    assert_eq!(human(5 << 30), "5.0 GiB");
}

#[test]
fn file_rows_and_csv_extra() {
    let d = dir("rows");
    let f = d.join("t.csv");
    fs::write(&f, "a,b,c\n1,2,3\n4,5,6\n").unwrap();
    let m = Meta::read(&f).unwrap();
    let rows = text(&m.lines(&Theme::default(), None));
    assert_eq!(rows[0], "t.csv");
    assert!(rows.iter().any(|r| r.starts_with("Size") && r.ends_with("18 B")), "{rows:?}");
    assert!(rows.iter().any(|r| r.starts_with("Type") && r.contains("csv")), "{rows:?}");
    assert!(
        rows.iter().any(|r| r.starts_with("CSV") && r.ends_with("3 rows × 3 cols")),
        "{rows:?}"
    );
    #[cfg(unix)]
    assert!(rows.iter().any(|r| r.starts_with("Perm") && r.contains("rw")), "{rows:?}");
}

#[test]
fn symlink_shows_target_and_orphans() {
    let d = dir("link");
    fs::write(d.join("real"), "x").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("real", d.join("ok")).unwrap();
        std::os::unix::fs::symlink("gone", d.join("bad")).unwrap();
        assert_eq!(Meta::read(&d.join("ok")).unwrap().link, Some(("real".into(), false)));
        assert_eq!(Meta::read(&d.join("bad")).unwrap().link, Some(("gone".into(), true)));
    }
}

#[test]
fn folder_size_is_counted_in_the_background() {
    let d = dir("size");
    fs::create_dir_all(d.join("a/b")).unwrap();
    fs::write(d.join("a/x"), vec![0u8; 100]).unwrap();
    fs::write(d.join("a/b/y"), vec![0u8; 50]).unwrap();
    let s = DirSize::spawn(d.clone());
    for _ in 0..200 {
        if s.state().1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(s.state(), (150, true));
    assert_eq!(Meta::read(&d).unwrap().mime, "inode/directory");
}
