use super::*;
use std::fs;
use std::process::Command;

/// Commits made through `Repo::create_commit` use the ambient git identity,
/// which a clean build sandbox does not have.
fn identity() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for (k, v) in [
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
        ] {
            // SAFETY: set once, before any test spawns git, to constants.
            unsafe { std::env::set_var(k, v) };
        }
    });
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

fn repo(name: &str) -> PathBuf {
    identity();
    let d = std::env::temp_dir().join(format!("git-core-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    fs::write(d.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    fs::write(d.join("b.txt"), "b\n").unwrap();
    fs::create_dir(d.join("sub")).unwrap();
    fs::write(d.join("sub/c.txt"), "c\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "init"]);
    d.canonicalize().unwrap()
}

#[test]
fn not_a_repo() {
    let d = std::env::temp_dir().join(format!("git-core-none-{}", std::process::id()));
    fs::create_dir_all(&d).unwrap();
    assert!(Repo::discover(&d).is_none());
}

#[test]
fn status_staged_unstaged_untracked() {
    let d = repo("status");
    fs::write(d.join("a.txt"), "1\nX\n3\n4\n5\n").unwrap(); // modified, unstaged
    fs::write(d.join("b.txt"), "bb\n").unwrap();
    git(&d, &["add", "b.txt"]); // modified, staged
    fs::write(d.join("new.txt"), "n\n").unwrap(); // untracked
    fs::write(d.join("sub/d.txt"), "d\n").unwrap();
    git(&d, &["add", "sub/d.txt"]); // added, staged
    fs::remove_file(d.join("sub/c.txt")).unwrap(); // deleted, unstaged

    let r = Repo::discover(&d).unwrap();
    let s = r.status().unwrap();
    assert_eq!(s.get(Path::new("a.txt")).unwrap().unstaged, Some(Kind::Modified));
    assert_eq!(s.get(Path::new("b.txt")).unwrap().staged, Some(Kind::Modified));
    assert_eq!(s.get(Path::new("new.txt")).unwrap().unstaged, Some(Kind::Untracked));
    assert_eq!(s.get(Path::new("sub/d.txt")).unwrap().staged, Some(Kind::Added));
    assert_eq!(s.get(Path::new("sub/c.txt")).unwrap().unstaged, Some(Kind::Deleted));
    assert!(s.dir(Path::new("sub")).is_some());
    assert!(s.get(Path::new("nothing")).is_none());
}

#[test]
fn clean_repo_is_empty() {
    let d = repo("clean");
    assert!(Repo::discover(&d).unwrap().status().unwrap().is_empty());
}

#[test]
fn conflict_is_reported() {
    let d = repo("conflict");
    git(&d, &["checkout", "-q", "-b", "other"]);
    fs::write(d.join("b.txt"), "other\n").unwrap();
    git(&d, &["commit", "-qam", "other"]);
    git(&d, &["checkout", "-q", "main"]);
    fs::write(d.join("b.txt"), "main\n").unwrap();
    git(&d, &["commit", "-qam", "main"]);
    let _ = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "merge", "other"])
        .current_dir(&d)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output();
    let s = Repo::discover(&d).unwrap().status().unwrap();
    assert_eq!(s.get(Path::new("b.txt")).unwrap().primary(), Some(Kind::Conflict));
}

#[test]
fn refs_commits_and_changes() {
    let d = repo("history");
    git(&d, &["tag", "v1"]);
    git(&d, &["tag", "-a", "v1-annotated", "-m", "release"]);
    git(&d, &["checkout", "-q", "-b", "feature"]);
    fs::write(d.join("a.txt"), "1\n2\nX\n4\n5\nmore\n").unwrap(); // +2 -1 vs "1..5"
    fs::write(d.join("new.txt"), "n\nn\n").unwrap();
    git(&d, &["rm", "-q", "b.txt"]);
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "second\n\nlonger body"]);
    let r = Repo::discover(&d).unwrap();

    let refs = r.refs().unwrap();
    let names: Vec<(&str, RefKind, bool)> =
        refs.iter().map(|x| (x.name.as_str(), x.kind, x.is_head)).collect();
    assert_eq!(names[0], ("feature", RefKind::Branch, true), "{names:?}");
    assert!(names.contains(&("main", RefKind::Branch, false)));
    assert!(
        names.contains(&("v1", RefKind::Tag, false))
            && names.contains(&("v1-annotated", RefKind::Tag, false))
    );
    // Annotated tags peel to the commit.
    let init = r.commit("main").unwrap();
    assert_eq!(refs.iter().find(|x| x.name == "v1-annotated").unwrap().oid, init.oid);
    assert_eq!(refs[0].subject, "second");

    let log = r.commits(10).unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "second");
    assert_eq!(log[0].message, "second\n\nlonger body");
    assert_eq!((log[0].author.as_str(), log[0].email.as_str()), ("t", "t@t"));
    assert_eq!(log[0].parents, vec![log[1].oid.clone()]);
    assert_eq!(r.commits(1).unwrap().len(), 1);

    let ch = r.changes("HEAD").unwrap();
    let get = |p: &str| ch.iter().find(|c| c.path == p).unwrap_or_else(|| panic!("{p} in {ch:?}"));
    assert_eq!(
        (get("a.txt").kind, get("a.txt").added, get("a.txt").removed),
        (Kind::Modified, 2, 1)
    );
    assert_eq!((get("new.txt").kind, get("new.txt").added), (Kind::Added, 2));
    assert_eq!(get("b.txt").kind, Kind::Deleted);
    // The root commit is diffed against the empty tree.
    assert!(r.changes(&init.oid).unwrap().iter().all(|c| c.kind == Kind::Added));
}

#[test]
fn patch_has_hunks_with_context() {
    let d = repo("patch");
    let base: String = (1..=20).map(|n| format!("{n}\n")).collect();
    fs::write(d.join("long.txt"), &base).unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "base"]);
    // Change line 5 and line 18 (far apart: two hunks), add a file, delete one.
    let changed: String = (1..=20)
        .map(|n| {
            if n == 5 {
                "FIVE\n".into()
            } else if n == 18 {
                "EIGHTEEN\n".into()
            } else {
                format!("{n}\n")
            }
        })
        .collect();
    fs::write(d.join("long.txt"), changed).unwrap();
    fs::write(d.join("new.txt"), "x\ny\n").unwrap();
    git(&d, &["rm", "-q", "b.txt"]);
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "change"]);

    let r = Repo::discover(&d).unwrap();
    let p = r.patch("HEAD").unwrap();
    let file = |n: &str| p.iter().find(|f| f.path == n).unwrap();

    let long = file("long.txt");
    assert_eq!(long.hunks.len(), 2, "{long:?}");
    let h = &long.hunks[0];
    assert_eq!((h.old_start, h.old_len, h.new_start, h.new_len), (2, 7, 2, 7));
    let shape: Vec<(PatchKind, &str)> = h.lines.iter().map(|l| (l.kind, l.text.as_str())).collect();
    assert_eq!(
        shape,
        vec![
            (PatchKind::Context, "2"),
            (PatchKind::Context, "3"),
            (PatchKind::Context, "4"),
            (PatchKind::Removed, "5"),
            (PatchKind::Added, "FIVE"),
            (PatchKind::Context, "6"),
            (PatchKind::Context, "7"),
            (PatchKind::Context, "8"),
        ]
    );
    assert_eq!((long.hunks[1].old_start, long.hunks[1].old_len), (15, 6)); // 15..20, ends at EOF

    let new = file("new.txt");
    assert_eq!(new.hunks.len(), 1);
    assert_eq!(
        (
            new.hunks[0].old_start,
            new.hunks[0].old_len,
            new.hunks[0].new_start,
            new.hunks[0].new_len
        ),
        (0, 0, 1, 2)
    );
    assert!(new.hunks[0].lines.iter().all(|l| l.kind == PatchKind::Added));
    let gone = file("b.txt");
    assert_eq!((gone.kind, gone.hunks[0].new_len), (Kind::Deleted, 0));

    // Counts of every hunk are consistent with its lines.
    for f in &p {
        for h in &f.hunks {
            let old = h.lines.iter().filter(|l| l.kind != PatchKind::Added).count() as u32;
            let new = h.lines.iter().filter(|l| l.kind != PatchKind::Removed).count() as u32;
            assert_eq!((old, new), (h.old_len, h.new_len));
        }
    }
}

#[test]
fn nearby_changes_merge_into_one_hunk_and_binary_has_none() {
    let d = repo("merge");
    let base: String = (1..=12).map(|n| format!("{n}\n")).collect();
    fs::write(d.join("m.txt"), &base).unwrap();
    fs::write(d.join("bin.dat"), [0u8, 1, 2]).unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "base"]);
    let changed: String =
        (1..=12).map(|n| if n == 3 || n == 8 { "Z\n".into() } else { format!("{n}\n") }).collect();
    fs::write(d.join("m.txt"), changed).unwrap();
    fs::write(d.join("bin.dat"), [0u8, 9, 9, 9]).unwrap();
    git(&d, &["commit", "-qam", "edit"]);
    let p = Repo::discover(&d).unwrap().patch("HEAD").unwrap();
    // Lines 3 and 8 are 4 apart (<= 6): one hunk.
    assert_eq!(p.iter().find(|f| f.path == "m.txt").unwrap().hunks.len(), 1);
    let bin = p.iter().find(|f| f.path == "bin.dat").unwrap();
    assert!(bin.binary && bin.hunks.is_empty());
}

#[test]
fn diff_file_shows_staged_and_unstaged_sides() {
    let d = repo("diffside");
    fs::write(d.join("a.txt"), "1\n2\nTHREE\n4\n5\n").unwrap();
    git(&d, &["add", "a.txt"]); // staged: line 3 changed
    fs::write(d.join("a.txt"), "1\n2\nTHREE\n4\n5\nsix\n").unwrap(); // unstaged: line 6 added
    fs::write(d.join("new.txt"), "n1\nn2\n").unwrap(); // untracked
    let r = Repo::discover(&d).unwrap();

    let staged = r.diff_file(Path::new("a.txt"), Side::Staged).unwrap();
    assert_eq!(staged.kind, Kind::Modified);
    let adds: Vec<&str> = staged.hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind == PatchKind::Added)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(adds, vec!["THREE"]);

    let unstaged = r.diff_file(Path::new("a.txt"), Side::Unstaged).unwrap();
    let adds: Vec<&str> = unstaged.hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind == PatchKind::Added)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(adds, vec!["six"]);

    let untracked = r.diff_file(Path::new("new.txt"), Side::Unstaged).unwrap();
    assert_eq!(untracked.kind, Kind::Untracked);
    assert_eq!(untracked.hunks[0].lines.len(), 2);
    // A new file staged shows as added; unborn-HEAD style "no staged change" is empty.
    git(&d, &["add", "new.txt"]);
    assert_eq!(r.diff_file(Path::new("new.txt"), Side::Staged).unwrap().kind, Kind::Added);
    assert!(r.diff_file(Path::new("b.txt"), Side::Staged).unwrap().hunks.is_empty());
    assert_eq!(r.branch().as_deref(), Some("main"));
}

#[test]
fn stage_unstage_discard_and_commit() {
    let d = repo("writes");
    fs::write(d.join("a.txt"), "changed\n").unwrap();
    fs::write(d.join("new.txt"), "n\n").unwrap();
    fs::remove_file(d.join("b.txt")).unwrap();
    let r = Repo::discover(&d).unwrap();
    let kind = |r: &Repo, p: &str| r.status().unwrap().get(Path::new(p)).copied();

    r.stage(&[PathBuf::from("a.txt"), PathBuf::from("new.txt"), PathBuf::from("b.txt")]).unwrap();
    assert_eq!(kind(&r, "a.txt").unwrap().staged, Some(Kind::Modified));
    assert_eq!(kind(&r, "new.txt").unwrap().staged, Some(Kind::Added));
    assert_eq!(kind(&r, "b.txt").unwrap().staged, Some(Kind::Deleted), "deletions stage too");

    r.unstage(&[PathBuf::from("new.txt")]).unwrap();
    assert_eq!(kind(&r, "new.txt").unwrap().unstaged, Some(Kind::Untracked));
    assert!(kind(&r, "new.txt").unwrap().staged.is_none());

    // Discard restores a tracked file; an untracked one is deleted.
    fs::write(d.join("a.txt"), "scribble\n").unwrap();
    r.discard(Path::new("a.txt"), false).unwrap();
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "changed\n"); // back to the staged content
    r.discard(Path::new("new.txt"), true).unwrap();
    assert!(!d.join("new.txt").exists());

    assert!(r.create_commit("  ").is_err(), "empty message");
    let summary = r.create_commit("second\n\nbody").unwrap();
    assert!(summary.ends_with(" second"), "{summary}");
    assert!(r.status().unwrap().is_empty());
    assert_eq!(r.commits(3).unwrap()[0].subject, "second");
    // Nothing staged: git refuses and the reason comes back.
    assert!(r.create_commit("again").is_err());

    r.stage_all().unwrap();
    r.unstage_all().unwrap();
    assert!(r.status().unwrap().is_empty());
}

#[test]
fn unstage_works_before_the_first_commit() {
    let d = std::env::temp_dir().join(format!("git-core-test-{}-unborn2", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    fs::write(d.join("f.txt"), "x\n").unwrap();
    let r = Repo::discover(&d).unwrap();
    r.stage(&[PathBuf::from("f.txt")]).unwrap();
    assert_eq!(r.status().unwrap().get(Path::new("f.txt")).unwrap().staged, Some(Kind::Added));
    r.unstage(&[PathBuf::from("f.txt")]).unwrap();
    assert!(r.status().unwrap().get(Path::new("f.txt")).unwrap().staged.is_none());
    assert!(r.create_commit("first").is_err(), "nothing staged");
}

#[test]
fn stashes_and_unborn_head() {
    let d = repo("stash");
    fs::write(d.join("a.txt"), "changed\n").unwrap();
    git(&d, &["stash", "push", "-q", "-m", "wip one"]);
    fs::write(d.join("a.txt"), "changed again\n").unwrap();
    git(&d, &["stash", "push", "-q", "-m", "wip two"]);
    let r = Repo::discover(&d).unwrap();
    let s = r.stashes().unwrap();
    assert_eq!(s.len(), 2);
    assert_eq!((s[0].index, s[1].index), (0, 1));
    assert!(s[0].message.contains("wip two") && s[1].message.contains("wip one"), "{s:?}");
    assert!(r.commit(&s[0].oid).is_ok());

    let e = std::env::temp_dir().join(format!("git-core-test-{}-unborn", std::process::id()));
    let _ = fs::remove_dir_all(&e);
    fs::create_dir_all(&e).unwrap();
    git(&e, &["init", "-q", "-b", "main"]);
    let r = Repo::discover(&e).unwrap();
    assert!(
        r.commits(5).unwrap().is_empty()
            && r.refs().unwrap().is_empty()
            && r.stashes().unwrap().is_empty()
    );
}

#[test]
fn line_changes_vs_head() {
    let d = repo("lines");
    // modify line 2, add a line at the end, delete line 4
    fs::write(d.join("a.txt"), "1\nX\n3\n5\nnew\n").unwrap();
    let r = Repo::discover(&d).unwrap();
    let c = r.line_changes(&d.join("a.txt")).unwrap();
    assert!(c.contains(&LineChange { kind: LineKind::Modified, start: 2, end: 2 }), "{c:?}");
    assert!(c.iter().any(|x| x.kind == LineKind::Added && x.start == 5), "{c:?}");
    assert!(c.iter().any(|x| x.kind == LineKind::Deleted), "{c:?}");
}

#[test]
fn untracked_file_is_all_added_and_clean_file_has_none() {
    let d = repo("untracked");
    fs::write(d.join("n.txt"), "x\ny\n").unwrap();
    let r = Repo::discover(&d).unwrap();
    assert_eq!(
        r.line_changes(&d.join("n.txt")).unwrap(),
        vec![LineChange { kind: LineKind::Added, start: 1, end: 2 }]
    );
    assert!(r.line_changes(&d.join("b.txt")).unwrap().is_empty());
}

#[test]
fn branches_and_stashes_through_the_git_cli() {
    let d = repo("ops");
    let r = Repo::discover(&d).unwrap();
    r.create_branch("feature").unwrap();
    assert_eq!(Repo::discover(&d).unwrap().branch().as_deref(), Some("feature"));
    assert!(r.create_branch(" ").is_err());
    fs::write(d.join("b.txt"), "feature\n").unwrap();
    git(&d, &["commit", "-qam", "on feature"]);
    r.checkout_ref("main", RefKind::Branch).unwrap();
    r.merge("feature").unwrap(); // fast-forward
    assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "feature\n");
    r.delete_branch("feature", false).unwrap();
    assert!(Repo::discover(&d).unwrap().refs().unwrap().iter().all(|x| x.name != "feature"));
    // An unmerged branch needs force.
    r.create_branch("wip").unwrap();
    fs::write(d.join("b.txt"), "wip\n").unwrap();
    git(&d, &["commit", "-qam", "wip"]);
    r.checkout_ref("main", RefKind::Branch).unwrap();
    assert!(r.delete_branch("wip", false).is_err());
    r.delete_branch("wip", true).unwrap();

    // Stash: push, apply (keeps it), pop and drop.
    fs::write(d.join("a.txt"), "changed\n").unwrap();
    r.stash_push("my stash").unwrap();
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "1\n2\n3\n4\n5\n");
    let r = Repo::discover(&d).unwrap();
    assert_eq!(r.stashes().unwrap().len(), 1);
    r.stash_apply(0).unwrap();
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "changed\n");
    git(&d, &["checkout", "-q", "a.txt"]);
    let r = Repo::discover(&d).unwrap();
    r.stash_pop(0).unwrap();
    assert!(Repo::discover(&d).unwrap().stashes().unwrap().is_empty());
    git(&d, &["checkout", "-q", "a.txt"]);
    fs::write(d.join("a.txt"), "again\n").unwrap();
    r.stash_push("").unwrap();
    r.stash_drop(0).unwrap();
    assert!(Repo::discover(&d).unwrap().stashes().unwrap().is_empty());
}

#[test]
fn commit_operations_tags_and_amend() {
    let d = repo("commitops");
    let r = Repo::discover(&d).unwrap();
    fs::write(d.join("b.txt"), "second\n").unwrap();
    git(&d, &["commit", "-qam", "second"]);
    let second = Repo::discover(&d).unwrap().commits(5).unwrap()[0].oid.clone();
    let first = Repo::discover(&d).unwrap().commits(5).unwrap()[1].oid.clone();
    // A branch and a tag at the first commit.
    r.create_tag("v1", &first).unwrap();
    r.create_branch_at("old", &first).unwrap();
    assert_eq!(Repo::discover(&d).unwrap().branch().as_deref(), Some("old"));
    assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "b\n");
    r.rename_branch("old", "older").unwrap();
    assert!(Repo::discover(&d).unwrap().refs().unwrap().iter().any(|x| x.name == "older"));
    // Cherry-pick the second commit onto it, then undo that with a revert.
    r.cherry_pick(&second).unwrap();
    assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "second\n");
    r.revert("HEAD").unwrap();
    assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "b\n");
    // Reset: soft keeps the changes staged, hard drops them.
    r.reset(&first, ResetMode::Soft).unwrap();
    assert_eq!(Repo::discover(&d).unwrap().commits(5).unwrap().len(), 1);
    r.reset(&first, ResetMode::Hard).unwrap();
    assert!(Repo::discover(&d).unwrap().status().unwrap().is_empty());
    // Amend: reword, then add a staged change without changing the message.
    r.amend(Some("renamed first")).unwrap();
    assert_eq!(Repo::discover(&d).unwrap().commits(1).unwrap()[0].subject, "renamed first");
    assert!(r.amend(Some(" ")).is_err());
    fs::write(d.join("a.txt"), "amended\n").unwrap();
    git(&d, &["add", "a.txt"]);
    r.amend(None).unwrap();
    assert_eq!(Repo::discover(&d).unwrap().commits(1).unwrap()[0].subject, "renamed first");
    assert!(Repo::discover(&d).unwrap().status().unwrap().is_empty());
    // Reword leaves the staged changes staged.
    fs::write(d.join("a.txt"), "staged later\n").unwrap();
    git(&d, &["add", "a.txt"]);
    r.reword("only the message").unwrap();
    assert_eq!(Repo::discover(&d).unwrap().commits(1).unwrap()[0].subject, "only the message");
    assert!(!Repo::discover(&d).unwrap().status().unwrap().is_empty());
    git(&d, &["reset", "-q", "--hard"]);
    // Reword an older commit: the ones after it are rebuilt, the files stay.
    fs::write(d.join("older.txt"), "o\n").unwrap();
    git(&d, &["add", "older.txt"]);
    git(&d, &["commit", "-qm", "older"]);
    fs::write(d.join("newer.txt"), "n\n").unwrap();
    git(&d, &["add", "newer.txt"]);
    git(&d, &["commit", "-qm", "newer"]);
    let before = Repo::discover(&d).unwrap().commits(5).unwrap();
    assert!(before.len() >= 2);
    let tree = |rev: &str| {
        String::from_utf8(
            std::process::Command::new("git")
                .current_dir(&d)
                .args(["rev-parse", &format!("{rev}^{{tree}}")])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
    };
    let head_tree = tree("HEAD");
    r.reword_commit(&before[1].oid, "older, reworded").unwrap();
    let after = Repo::discover(&d).unwrap().commits(5).unwrap();
    assert_eq!(after[1].subject, "older, reworded");
    assert_eq!(after[0].subject, before[0].subject);
    assert_ne!(after[0].oid, before[0].oid);
    assert_eq!(tree("HEAD"), head_tree);
    r.delete_tag("v1").unwrap();
    assert!(Repo::discover(&d).unwrap().refs().unwrap().iter().all(|x| x.name != "v1"));
    // Rebase a feature branch onto main, which has moved on.
    r.checkout_ref("main", RefKind::Branch).unwrap();
    r.create_branch("feat").unwrap();
    fs::write(d.join("f.txt"), "f\n").unwrap();
    git(&d, &["add", "f.txt"]);
    git(&d, &["commit", "-qm", "feature"]);
    r.checkout_ref("main", RefKind::Branch).unwrap();
    fs::write(d.join("c.txt"), "c\n").unwrap();
    git(&d, &["add", "c.txt"]);
    git(&d, &["commit", "-qm", "main moves"]);
    r.checkout_ref("feat", RefKind::Branch).unwrap();
    r.rebase("main").unwrap();
    assert!(d.join("c.txt").exists() && d.join("f.txt").exists());
    assert_eq!(Repo::discover(&d).unwrap().commits(1).unwrap()[0].subject, "feature");
}

#[test]
fn stopped_operations_are_seen_continued_and_aborted() {
    let d = repo("stopped");
    let r = Repo::discover(&d).unwrap();
    git(&d, &["checkout", "-q", "-b", "other"]);
    fs::write(d.join("b.txt"), "other\n").unwrap();
    git(&d, &["commit", "-qam", "other"]);
    let other = Repo::discover(&d).unwrap().commits(1).unwrap()[0].oid.clone();
    git(&d, &["checkout", "-q", "main"]);
    fs::write(d.join("b.txt"), "main\n").unwrap();
    git(&d, &["commit", "-qam", "main"]);
    assert_eq!(r.operation(), None);
    assert!(r.cherry_pick(&other).is_err(), "the pick conflicts");
    assert_eq!(Repo::discover(&d).unwrap().operation(), Some(Operation::CherryPick));
    Repo::discover(&d).unwrap().abort_operation().unwrap();
    assert_eq!(Repo::discover(&d).unwrap().operation(), None);
    assert!(r.merge("other").is_err());
    assert_eq!(Repo::discover(&d).unwrap().operation(), Some(Operation::Merge));
    // Resolve and continue.
    fs::write(d.join("b.txt"), "resolved\n").unwrap();
    git(&d, &["add", "b.txt"]);
    Repo::discover(&d).unwrap().continue_operation().unwrap();
    assert_eq!(Repo::discover(&d).unwrap().operation(), None);
    assert!(r.continue_operation().is_err());
}

#[test]
fn fetch_pull_and_push_against_a_local_remote() {
    let d = repo("net");
    let remote = std::env::temp_dir().join(format!("git-core-remote-{}", std::process::id()));
    let _ = fs::remove_dir_all(&remote);
    fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(&d, &["remote", "add", "origin", remote.to_str().unwrap()]);
    let r = Repo::discover(&d).unwrap();
    assert!(!r.has_upstream());
    // The first push sets the upstream.
    r.push(false).unwrap();
    assert!(Repo::discover(&d).unwrap().has_upstream());
    // A second clone pushes a commit; fetch and pull bring it here.
    let other = std::env::temp_dir().join(format!("git-core-clone-{}", std::process::id()));
    let _ = fs::remove_dir_all(&other);
    git(&std::env::temp_dir(), &["clone", "-q", remote.to_str().unwrap(), other.to_str().unwrap()]);
    fs::write(other.join("n.txt"), "n\n").unwrap();
    git(&other, &["add", "n.txt"]);
    git(&other, &["commit", "-qm", "from the clone"]);
    git(&other, &["push", "-q"]);
    r.fetch().unwrap();
    r.pull().unwrap();
    assert!(d.join("n.txt").exists());
    // Force push with lease and deleting a remote branch.
    git(&d, &["checkout", "-q", "-b", "topic"]);
    Repo::discover(&d).unwrap().push(false).unwrap();
    assert!(Repo::discover(&d).unwrap().refs().unwrap().iter().any(|x| x.name == "origin/topic"));
    Repo::discover(&d).unwrap().push(true).unwrap();
    git(&d, &["checkout", "-q", "main"]);
    Repo::discover(&d).unwrap().delete_remote_branch("origin/topic").unwrap();
    assert!(Repo::discover(&d).unwrap().delete_remote_branch("nonsense").is_err());
    // Renaming: a remote branch (push the new name, delete the old), a tag, and the remote itself.
    git(&d, &["checkout", "-q", "-b", "old-name"]);
    Repo::discover(&d).unwrap().push(false).unwrap();
    git(&d, &["checkout", "-q", "main"]);
    let r = Repo::discover(&d).unwrap();
    r.rename_remote_branch("origin/old-name", "new-name").unwrap();
    r.fetch().unwrap();
    let names: Vec<String> =
        Repo::discover(&d).unwrap().refs().unwrap().into_iter().map(|x| x.name).collect();
    assert!(
        names.contains(&"origin/new-name".to_string())
            && !names.contains(&"origin/old-name".to_string()),
        "{names:?}"
    );
    git(&d, &["tag", "t1"]);
    r.rename_tag("t1", "t2").unwrap();
    let names: Vec<String> =
        Repo::discover(&d).unwrap().refs().unwrap().into_iter().map(|x| x.name).collect();
    assert!(names.contains(&"t2".to_string()) && !names.contains(&"t1".to_string()));
    assert!(r.rename_tag("t2", "t2").is_err() && r.rename_tag("nope", "t3").is_err());
    assert!(r.rename_tag("t2", "-f").is_err() && r.rename_tag("t2", "a b").is_err());
    // An annotated tag keeps its message under the new name; a failed rename leaves the old tag alone.
    git(&d, &["tag", "-a", "ann", "-m", "release notes"]);
    r.rename_tag("ann", "ann2").unwrap();
    let body = std::process::Command::new("git")
        .args(["tag", "-n9", "-l", "ann2"])
        .current_dir(&d)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&body.stdout).contains("release notes"));
    assert!(!Repo::discover(&d).unwrap().refs().unwrap().iter().any(|x| x.name == "ann"));
    // Remote branches: not onto themselves (that would delete them), not HEAD, not an invalid name.
    let names: Vec<String> =
        Repo::discover(&d).unwrap().refs().unwrap().into_iter().map(|x| x.name).collect();
    assert!(names.contains(&"origin/new-name".to_string()), "{names:?}");
    assert!(r.rename_remote_branch("origin/new-name", "new-name").is_err());
    assert!(
        r.rename_remote_branch("origin/HEAD", "x").is_err()
            && r.rename_remote_branch("origin/new-name", "bad name").is_err()
    );
    assert!(r.rename_branch("main", "-x").is_err());
    // Several at once: all or none.
    git(&d, &["branch", "grp/a"]);
    git(&d, &["branch", "grp/b"]);
    git(&d, &["branch", "taken"]);
    let r = Repo::discover(&d).unwrap();
    let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
    assert!(
        r.rename_many(RefKind::Branch, &[pair("grp/a", "new/a"), pair("grp/b", "taken")]).is_err()
    );
    assert!(
        r.rename_many(RefKind::Branch, &[pair("grp/a", "same"), pair("grp/b", "same")]).is_err()
    );
    assert!(
        r.rename_many(RefKind::Branch, &[pair("grp/a", "new/a"), pair("grp/b", "bad name")])
            .is_err()
    );
    let names: Vec<String> =
        Repo::discover(&d).unwrap().refs().unwrap().into_iter().map(|x| x.name).collect();
    assert!(
        names.contains(&"grp/a".to_string())
            && names.contains(&"grp/b".to_string())
            && !names.contains(&"new/a".to_string()),
        "{names:?}"
    );
    r.rename_many(RefKind::Branch, &[pair("grp/a", "new/a"), pair("grp/b", "new/b")]).unwrap();
    let names: Vec<String> =
        Repo::discover(&d).unwrap().refs().unwrap().into_iter().map(|x| x.name).collect();
    assert!(
        names.contains(&"new/a".to_string())
            && names.contains(&"new/b".to_string())
            && !names.contains(&"grp/a".to_string())
    );
    r.rename_remote("origin", "upstream").unwrap();
    let names: Vec<String> =
        Repo::discover(&d).unwrap().refs().unwrap().into_iter().map(|x| x.name).collect();
    assert!(names.contains(&"upstream/main".to_string()), "{names:?}");
}

#[test]
fn submodules_are_listed_and_show_in_status() {
    let sub = repo("submodule-source");
    fs::write(sub.join("a.txt"), "a\n").unwrap();
    git(&sub, &["add", "."]);
    git(&sub, &["commit", "-m", "one"]);
    let dir = repo("submodule-host");
    fs::write(dir.join("f.txt"), "f\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "init"]);
    git(
        &dir,
        &["-c", "protocol.file.allow=always", "submodule", "add", sub.to_str().unwrap(), "mod"],
    );
    git(&dir, &["commit", "-m", "add submodule"]);
    let r = Repo::discover(&dir).unwrap();
    let list = r.submodules().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].path.as_str(), list[0].state), ("mod", SubmoduleState::Clean));
    assert!(!list[0].dirty);
    // An uncommitted file inside the submodule makes it a changed file of the host.
    fs::write(dir.join("mod/new.txt"), "x\n").unwrap();
    let r = Repo::discover(&dir).unwrap();
    assert!(r.submodules().unwrap()[0].dirty);
    assert!(r.status().unwrap().get(Path::new("mod")).is_some());
    // Removing it leaves no trace in the index or `.gitmodules`.
    r.submodule_remove("mod").unwrap();
    assert!(r.submodules().unwrap().is_empty());
}
