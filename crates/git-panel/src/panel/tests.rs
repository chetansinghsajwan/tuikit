use super::*;
use ratatui::crossterm::event::KeyModifiers;
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
    assert!(
        Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .status()
            .unwrap()
            .success()
    );
}

/// A repo with: a.txt modified (unstaged), b.txt modified and staged, new.txt untracked.
fn repo(name: &str) -> PathBuf {
    identity();
    let d = std::env::temp_dir().join(format!("git-panel-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    fs::write(d.join("a.txt"), "a1\na2\n").unwrap();
    fs::write(d.join("b.txt"), "b1\nb2\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "init"]);
    fs::write(d.join("a.txt"), "a1\nA2 changed\n").unwrap();
    fs::write(d.join("b.txt"), "b1\nB2 staged\n").unwrap();
    git(&d, &["add", "b.txt"]);
    fs::write(d.join("new.txt"), "brand new\n").unwrap();
    d.canonicalize().unwrap()
}

fn key(c: KeyCode) -> Event {
    Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
}

fn press(p: &mut Panel, s: &str) -> Action {
    let mut last = Action::None;
    for c in s.chars() {
        last = p.handle_event(&key(KeyCode::Char(c)));
    }
    last
}

fn draw(p: &mut Panel) -> Vec<String> {
    let area = Rect::new(0, 0, 100, 16);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &Theme::default());
    (0..16).map(|y| (0..100).map(|x| b[(x, y)].symbol().to_string()).collect::<String>()).collect()
}

fn on(p: &mut Panel, name: &str) {
    let i = p
        .items
        .iter()
        .position(|&i| matches!(&p.rows[i], Row::File { change, .. } if change.rela == Path::new(name)))
        .unwrap_or_else(|| panic!("{name} listed"));
    p.cursor = i;
}

#[test]
fn one_tree_with_two_marks_per_file_and_icons() {
    let mut p = Panel::new(&repo("sections")).unwrap();
    let text = draw(&mut p).join("\n");
    assert!(
        !text.contains("Staged (") && !text.contains("Unstaged (") && !text.contains("Untracked ("),
        "no sections: {text}"
    );
    let pos = |s: &str| text.find(s).unwrap_or_else(|| panic!("{s} in\n{text}"));
    // Sorted by name; the marks sit at the right edge: the first is staged, the second not staged (`?` for a new file).
    assert!(pos("main") < pos("a.txt") && pos("a.txt") < pos("b.txt") && pos("b.txt") < pos("new.txt"), "{text}");
    assert!(text.contains(" M ") && text.contains("M  ") && text.contains(" ? "), "{text}");
    // A file that is partly staged is one row with both marks (`M M`).
    fs::write(p.root.join("b.txt"), "b1\nmore\n").unwrap();
    p.refresh();
    assert_eq!(p.all.iter().filter(|c| c.rela == Path::new("b.txt")).count(), 1);
    assert!(draw(&mut p).join("\n").contains("M M"));
}

#[test]
fn staged_marks_are_drawn_in_the_staged_color() {
    let mut p = Panel::new(&repo("stagedcolor")).unwrap();
    let theme = Theme::default();
    let area = Rect::new(0, 0, 100, 16);
    let mut b = Buffer::empty(area);
    p.render(area, &mut b, &theme);
    let success: ratatui::style::Color = theme.colors.success.into();
    let row = |name: &str| {
        (0..16)
            .find(|&y| (0..100).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().contains(name))
            .unwrap_or_else(|| panic!("{name} listed"))
    };
    // b.txt is staged, a.txt is changed but not staged: the mark of the staged one is green, the other is not.
    let green_mark = |y: u16| (0..100).any(|x| b[(x, y)].symbol() == "M" && b[(x, y)].style().fg == Some(success));
    assert!(green_mark(row("b.txt")), "staged mark");
    assert!(!green_mark(row("a.txt")), "unstaged mark");
}

#[test]
fn s_toggles_stage_and_unstage_and_the_cursor_stays_on_the_file() {
    let d = repo("toggle");
    let mut p = Panel::new(&d).unwrap();
    on(&mut p, "new.txt");
    assert_eq!(press(&mut p, "s"), Action::Changed);
    assert_eq!(p.current().map(|c| (c.rela.clone(), c.staged)), Some((PathBuf::from("new.txt"), Some(Kind::Added))));
    assert_eq!(press(&mut p, "s"), Action::Changed); // all staged: back out
    assert_eq!(p.current().map(|c| c.unstaged), Some(Some(Kind::Untracked)));
    // `u` only unstages.
    on(&mut p, "a.txt");
    assert_eq!(press(&mut p, "u"), Action::None);
    // A partly staged file: s stages the rest, s again unstages it.
    on(&mut p, "b.txt");
    fs::write(d.join("b.txt"), "b1\nmore\n").unwrap();
    p.refresh();
    assert_eq!(press(&mut p, "s"), Action::Changed);
    assert!(p.current().is_some_and(|c| c.unstaged.is_none() && c.staged.is_some()));
    assert_eq!(press(&mut p, "s"), Action::Changed);
    assert!(p.current().is_some_and(|c| c.staged.is_none()), "{:?}", p.current());
    // `a` toggles: it stages everything while something is left, and unstages it all once all is staged.
    assert_eq!(press(&mut p, "a"), Action::Changed);
    assert!(p.all.iter().all(|c| !c.open()));
    assert_eq!(press(&mut p, "a"), Action::Changed);
    assert!(p.all.iter().all(|c| c.staged.is_none()));
}

#[test]
fn the_tree_groups_by_folder_stages_a_folder_folds_and_toggles_to_a_list() {
    let d = repo("tree");
    fs::create_dir_all(d.join("src/deep/er")).unwrap();
    fs::write(d.join("src/one.rs"), "1\n").unwrap();
    fs::write(d.join("src/deep/er/two.rs"), "2\n").unwrap();
    let mut p = Panel::new(&d).unwrap();
    let text = draw(&mut p).join("\n");
    assert!(text.contains("src 2"), "{text}");
    assert!(text.contains("deep/er 1"), "a chain of single folders is one row: {text}");
    assert!(!text.contains("src/one.rs"), "files show their name only in the tree");
    // Enter on the folder folds it.
    let at = p
        .items
        .iter()
        .position(|&i| matches!(&p.rows[i], Row::Folder { path, .. } if path == Path::new("src")))
        .unwrap();
    p.cursor = at;
    p.handle_event(&key(KeyCode::Enter));
    let text = draw(&mut p).join("\n");
    assert!(text.contains("src 2") && !text.contains("one.rs"), "{text}");
    // s on the folder stages everything below it, s again unstages it.
    assert_eq!(press(&mut p, "s"), Action::Changed);
    assert!(p.all.iter().filter(|c| c.rela.starts_with("src")).all(|c| !c.open()), "{:?}", p.message);
    assert_eq!(press(&mut p, "s"), Action::Changed);
    assert!(p.all.iter().filter(|c| c.rela.starts_with("src")).all(|c| c.staged.is_none()));
    // t switches to the flat list with the full paths.
    press(&mut p, "t");
    let text = draw(&mut p).join("\n");
    assert!(text.contains("src/one.rs"), "{text}");
}

#[test]
fn x_asks_before_discarding_and_never_touches_staged_changes() {
    let d = repo("discard");
    let mut p = Panel::new(&d).unwrap();
    on(&mut p, "b.txt"); // staged
    press(&mut p, "d");
    assert!(matches!(p.mode, Mode::Normal), "staged files are not discarded");
    assert!(p.message.as_ref().is_some_and(|(_, err)| *err));

    on(&mut p, "a.txt");
    press(&mut p, "d");
    assert!(draw(&mut p).join("\n").contains("Discard a.txt? d again"));
    assert_eq!(press(&mut p, "n"), Action::None);
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "a1\nA2 changed\n");
    press(&mut p, "d");
    assert_eq!(press(&mut p, "d"), Action::Changed);
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "a1\na2\n");

    on(&mut p, "new.txt");
    press(&mut p, "d");
    assert!(draw(&mut p).join("\n").contains("Delete new.txt? d again"));
    assert_eq!(press(&mut p, "y"), Action::Changed);
    assert!(!d.join("new.txt").exists());
}

#[test]
fn c_commits_the_staged_files_with_the_typed_message() {
    let d = repo("commit");
    let mut p = Panel::new(&d).unwrap();
    // Empty message: refused, the prompt stays so nothing typed is lost.
    press(&mut p, "c");
    assert!(draw(&mut p).join("\n").contains("commit: "));
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::None);
    assert!(matches!(p.mode, Mode::Commit(_)) && p.message.as_ref().is_some_and(|(_, e)| *e));
    let shown = draw(&mut p).join("\n");
    assert!(
        shown.contains("empty commit message") && shown.contains("commit: "),
        "the refusal is visible above the prompt:\n{shown}"
    );
    press(&mut p, "fix b");
    assert!(draw(&mut p).join("\n").contains("commit: fix b"));
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Changed);
    let log = p.repo.commits(2).unwrap();
    assert_eq!(log[0].subject, "fix b");
    // What was staged is gone from the list; the other files remain.
    let text = draw(&mut p).join("\n");
    assert!(!text.contains("b.txt") && text.contains("a.txt") && text.contains("committed "), "{text}");
    // Esc cancels the prompt.
    press(&mut p, "c"); // nothing staged now
    assert!(p.message.as_ref().is_some_and(|(m, e)| *e && m.contains("nothing staged")));
}

#[test]
fn enter_opens_the_file_and_the_preview_shows_the_right_side_of_the_diff() {
    let d = repo("preview");
    let mut p = Panel::new(&d).unwrap();
    on(&mut p, "a.txt");
    let text = draw(&mut p).join("\n");
    assert!(text.contains("A2 changed") && !text.contains("B2 staged"), "unstaged diff of a.txt:\n{text}");
    on(&mut p, "b.txt");
    let text = draw(&mut p).join("\n");
    assert!(text.contains("B2 staged"), "staged diff of b.txt:\n{text}");
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Open(d.join("b.txt")));
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Close);
}

#[test]
fn a_clean_repo_and_a_non_repo() {
    let d = std::env::temp_dir().join(format!("git-panel-test-{}-clean", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    assert!(Panel::new(&d).is_none());
    git(&d, &["init", "-q", "-b", "main"]);
    let mut p = Panel::new(&d).unwrap();
    assert!(draw(&mut p).join("\n").contains("clean: nothing to commit"));
    assert_eq!(press(&mut p, "j"), Action::None); // navigation on an empty list is harmless
}

#[test]
fn space_opens_the_leader_menu_with_the_actions_and_the_palette_and_help_ask_the_host() {
    let d = repo("leader");
    let mut p = Panel::new(&d).unwrap();
    assert_eq!(press(&mut p, " "), Action::None);
    let menu = p.menu().expect("a leader menu");
    let names: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
    for want in ["stage or unstage", "unstage", "commit", "discard", "commands"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    // Space then Space is the palette; `?` is the help; the host's own actions come back as commands.
    assert_eq!(press(&mut p, " "), Action::Palette);
    assert_eq!(press(&mut p, "?"), Action::Help);
    let extra = vec![ui::keys::Action { id: "pane.next", name: "next pane", desc: "x", basic: true, keys: &["tab"] }];
    let mut p = Panel::with_actions(&d, "space", extra, &[]).unwrap();
    assert_eq!(
        p.handle_event(&Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))),
        Action::Command("pane.next")
    );
    // The rows for a palette: the basic keys are marked.
    assert!(
        p.keys_rows().iter().any(|r| r.id == "nav.down" && r.basic)
            && p.keys_rows().iter().any(|r| r.id == "commit.new" && !r.basic)
    );
}
