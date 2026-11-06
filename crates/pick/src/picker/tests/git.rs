use super::super::*;
use crate::source::{GitSource, spawn_git};
use ratatui::crossterm::event::KeyEvent;
use std::process::Command;

fn git(dir: &std::path::Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

fn repo(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pick-git-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    std::fs::write(d.join("a.txt"), "one\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "first commit"]);
    git(&d, &["tag", "v1"]);
    std::fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
    git(&d, &["commit", "-qam", "add the second line"]);
    d.canonicalize().unwrap()
}

fn picker(root: &PathBuf, what: GitSource, query: &str) -> Picker {
    let opts = PickerOptions { mode: Mode::Fuzzy, paths: false, root: root.clone(), preview: true, git: false, query: query.into() };
    let mut p = Picker::new(opts, Arc::new(|| {}));
    spawn_git(p.injector(), root.clone(), what);
    for _ in 0..300 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        p.nucleo.tick(10);
        if p.nucleo.snapshot().item_count() > 0 && !p.nucleo.tick(10).running {
            break;
        }
    }
    p
}

fn enter(p: &mut Picker) -> Action {
    p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)))
}

#[test]
fn refs_list_branches_and_tags_and_print_names() {
    let d = repo("refs");
    let mut p = picker(&d, GitSource::Refs, "");
    let names: Vec<String> = p.nucleo.snapshot().matched_items(..).map(|i| i.data.git.as_ref().unwrap().name.clone()).collect();
    assert_eq!(names, vec!["main".to_string(), "v1".to_string()]);
    assert_eq!(enter(&mut p), Action::Accept(vec!["main".into()])); // head branch is first
}

#[test]
fn query_matches_name_or_subject() {
    let d = repo("query");
    let mut p = picker(&d, GitSource::Refs, "first");
    for _ in 0..50 {
        p.nucleo.tick(10);
    }
    // The tag points at "first commit": found through the subject.
    assert_eq!(enter(&mut p), Action::Accept(vec!["v1".into()]));
}

#[test]
fn commits_print_full_hash_and_preview_shows_the_commit() {
    let d = repo("commits");
    let mut p = picker(&d, GitSource::Commits, "");
    let log = git_core::Repo::discover(&d).unwrap().commits(5).unwrap();
    assert_eq!(log.len(), 2);
    let Action::Accept(v) = enter(&mut p) else { panic!() };
    assert_eq!(v, vec![log[0].oid.clone()]);
    assert_eq!(v[0].len(), 40);

    let area = Rect::new(0, 0, 100, 16);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default());
    let text: String = (0..16).map(|y| (0..100).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(text.contains("add the second line") && text.contains("·") && text.contains("1 file") && text.contains("+1"), "{text}");
}

#[test]
fn commit_preview_scrolls_with_ctrl_d_and_u() {
    let d = repo("scroll");
    // A commit with a long patch.
    let body: String = (1..=80).map(|n| format!("line {n}\n")).collect();
    std::fs::write(d.join("long.txt"), body).unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "long file"]);
    let mut p = picker(&d, GitSource::Commits, "");
    let area = Rect::new(0, 0, 100, 16);
    let draw = |p: &mut Picker| -> String {
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, &Theme::default());
        (0..16).map(|y| (0..100).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    };
    let top = draw(&mut p);
    assert!(top.contains("long file") && !top.contains("line 70"), "{top}");
    let ctrl = |c| Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    for _ in 0..12 {
        p.handle_event(&ctrl('d'));
        draw(&mut p);
    }
    let deep = draw(&mut p);
    assert!(deep.contains("line 70") || deep.contains("line 7"), "{deep}");
    for _ in 0..40 {
        p.handle_event(&ctrl('u'));
    }
    assert!(draw(&mut p).contains("long file"));
    // Moving to another commit resets the scroll.
    p.handle_event(&ctrl('d'));
    draw(&mut p);
    p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    draw(&mut p);
    assert_eq!(p.pscroll, 0);
}

#[test]
fn stashes_print_their_name() {
    let d = repo("stash");
    std::fs::write(d.join("a.txt"), "wip\n").unwrap();
    git(&d, &["stash", "push", "-q", "-m", "work in progress"]);
    let mut p = picker(&d, GitSource::Stashes, "");
    assert_eq!(enter(&mut p), Action::Accept(vec!["stash@{0}".into()]));
}
