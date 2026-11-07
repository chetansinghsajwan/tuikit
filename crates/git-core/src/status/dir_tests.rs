use super::*;

#[test]
fn a_folder_lists_every_state_beneath_it_in_a_fixed_order() {
    // The map iterates in a different order from one run to the next: the result must not.
    for _ in 0..20 {
        let mut s = Status::default();
        for (name, st) in [
            ("d/a", FileStatus { staged: None, unstaged: Some(Kind::Untracked) }),
            ("d/b", FileStatus { staged: Some(Kind::Added), unstaged: None }),
            ("d/c", FileStatus { staged: None, unstaged: Some(Kind::Modified) }),
            ("d/e", FileStatus { staged: None, unstaged: Some(Kind::Typechange) }),
            ("d/f", FileStatus { staged: None, unstaged: Some(Kind::Deleted) }),
            ("other/x", FileStatus { staged: None, unstaged: Some(Kind::Conflict) }),
        ] {
            s.files.insert(PathBuf::from(name), st);
        }
        assert_eq!(
            s.dir_kinds(Path::new("d")),
            [Kind::Deleted, Kind::Modified, Kind::Added, Kind::Untracked]
        );
        assert_eq!(s.dir(Path::new("d")), Some(Kind::Deleted));
        assert_eq!(s.dir(Path::new("other")), Some(Kind::Conflict));
        assert_eq!(s.dir(Path::new("clean")), None);
    }
}
