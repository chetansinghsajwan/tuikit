use super::*;

#[test]
fn set_get_remove_and_reload() {
    let f = std::env::temp_dir().join(format!("files-bm-{}/bookmarks", std::process::id()));
    let mut b = Bookmarks::load(Some(f.clone()));
    assert!(b.get('a').is_none());
    b.set('a', PathBuf::from("/tmp/a b")).unwrap();
    b.set('z', PathBuf::from("/x")).unwrap();
    let again = Bookmarks::load(Some(f.clone()));
    assert_eq!(again.get('a'), Some(Path::new("/tmp/a b")));
    assert_eq!(again.iter().map(|(k, _)| k).collect::<String>(), "az");
    assert!(b.remove('a').unwrap() && !b.remove('a').unwrap());
    assert!(Bookmarks::load(Some(f)).get('a').is_none());
}

#[test]
fn without_a_file_they_stay_in_memory() {
    let mut b = Bookmarks::load(None);
    b.set('q', PathBuf::from("/q")).unwrap();
    assert_eq!(b.get('q'), Some(Path::new("/q")));
}
