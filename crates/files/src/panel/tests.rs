use super::*;
use ratatui::crossterm::event::KeyModifiers;
use std::fs;
use std::process::Command;

fn tree(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("files-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("src/deep")).unwrap();
    fs::create_dir_all(d.join("docs")).unwrap();
    fs::write(d.join("README.md"), "r").unwrap();
    fs::write(d.join(".hidden"), "h").unwrap();
    fs::write(d.join("src/main.rs"), "m").unwrap();
    fs::write(d.join("src/deep/inner.txt"), "i").unwrap();
    d.canonicalize().unwrap()
}

fn key(c: KeyCode) -> Event {
    Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
}

/// Type a leader sequence: the leader key, then each key of `seq`.
fn leader(p: &mut Panel, seq: &str) {
    p.handle_event(&key(KeyCode::Char(' ')));
    for c in seq.chars() {
        p.handle_event(&key(KeyCode::Char(c)));
    }
}

fn names(p: &Panel) -> Vec<String> {
    p.rows.iter().map(|r| format!("{}{}", "  ".repeat(r.depth.into()), r.name)).collect()
}

fn panel(d: &Path) -> Panel {
    Panel::new(
        d,
        PanelOptions {
            hidden: false,
            git: false,
            linemode: Linemode::Off,
            bookmarks: None,
            background_bytes: None,
            leader: "space".into(),
            actions: Vec::new(),
        },
    )
}

#[test]
fn a_folder_with_several_kinds_of_change_shows_a_sign_for_each_in_a_fixed_order() {
    let d = std::env::temp_dir().join(format!("files-test-{}-kinds", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("src")).unwrap();
    let git = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(&d)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .status()
                .unwrap()
                .success()
        );
    };
    git(&["init", "-q", "-b", "main"]);
    fs::write(d.join("src/a.rs"), "a\n").unwrap();
    fs::write(d.join("src/b.rs"), "b\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "i"]);
    fs::write(d.join("src/a.rs"), "changed\n").unwrap();
    fs::remove_file(d.join("src/b.rs")).unwrap();
    fs::write(d.join("src/c.rs"), "new\n").unwrap();
    let d = d.canonicalize().unwrap();
    let p = &mut Panel::new(
        &d,
        PanelOptions {
            hidden: false,
            git: true,
            linemode: Linemode::Off,
            bookmarks: None,
            background_bytes: None,
            leader: "space".into(),
            actions: Vec::new(),
        },
    );
    let theme = Theme::default();
    for _ in 0..5 {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        p.render(Rect::new(0, 0, 40, 10), &mut buf, &theme, true);
        let text: Vec<String> =
            (0..10).map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect();
        assert!(text.iter().any(|l| l.contains("src") && l.contains("D M ?")), "{text:#?}");
        p.refresh();
    }
}

#[test]
fn the_active_indent_guide_is_the_scope_the_cursor_is_in() {
    let d = tree("guide");
    let mut p = panel(&d);
    p.reveal(&d.join("src/deep/inner.txt"));
    let at = |p: &Panel, name: &str| p.rows.iter().position(|r| r.name == name).unwrap();
    let (src, deep) = (at(&p, "src"), at(&p, "deep"));
    // On a file: the folder that holds it, from the folder row to the end of its contents.
    let s = p.active_scope().unwrap();
    assert_eq!((s.from, s.col, s.to), (deep, 1, at(&p, "inner.txt") + 1));
    // On an open folder: its own contents. On a top-level file: no guide.
    p.cursor = src;
    let s = p.active_scope().unwrap();
    assert_eq!((s.from, s.col), (src, 0));
    p.cursor = at(&p, "README.md");
    assert!(p.active_scope().is_none());
}

#[test]
fn wanted_width_fits_the_longest_name_on_screen() {
    let d = tree("wanted");
    let long = "a-name-that-is-forty-characters-long.txt";
    fs::write(d.join(long), "x").unwrap();
    let mut p = panel(&d);
    let theme = Theme::default();
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 12));
    p.render(Rect::new(0, 0, 30, 12), &mut buf, &theme, true);
    let want = p.wanted_width(&theme);
    assert!(want >= 40 + 2 + 4, "{want}");
    // Given that width the name is drawn whole.
    let mut wide = Buffer::empty(Rect::new(0, 0, want, 12));
    p.render(Rect::new(0, 0, want, 12), &mut wide, &theme, true);
    let text: String = (0..12)
        .map(|y| (0..want).map(|x| wide[(x, y)].symbol().to_string()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains(long), "{text}");
}

#[test]
fn lists_dirs_first_and_hides_dotfiles() {
    let p = panel(&tree("list"));
    assert_eq!(names(&p), vec!["docs", "src", "README.md"]);
}

#[test]
fn expand_collapse_and_left_navigation() {
    let mut p = panel(&tree("expand"));
    p.handle_event(&key(KeyCode::Down)); // src
    p.handle_event(&key(KeyCode::Right));
    assert_eq!(names(&p), vec!["docs", "src", "  deep", "  main.rs", "README.md"]);
    p.handle_event(&key(KeyCode::Down)); // deep
    p.handle_event(&key(KeyCode::Right));
    assert!(names(&p).contains(&"    inner.txt".to_string()));
    // h on an open folder collapses it; on a file or closed folder it jumps to the parent.
    p.handle_event(&key(KeyCode::Char('h')));
    assert!(!names(&p).contains(&"    inner.txt".to_string()));
    p.handle_event(&key(KeyCode::Char('h')));
    assert_eq!(p.current().unwrap().file_name().unwrap(), "src");
    p.handle_event(&key(KeyCode::Char('h')));
    assert_eq!(names(&p), vec!["docs", "src", "README.md"]);
}

#[test]
fn enter_opens_files_and_toggles_hidden() {
    let d = tree("open");
    let mut p = panel(&d);
    p.handle_event(&key(KeyCode::Char('G'))); // README.md
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Open(d.join("README.md")));
    p.handle_event(&key(KeyCode::Char('.')));
    assert!(names(&p).contains(&".hidden".to_string()));
    p.handle_event(&key(KeyCode::Char('.')));
    assert!(!names(&p).contains(&".hidden".to_string()));
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Close);
}

#[test]
fn reveal_expands_to_the_file_and_refresh_keeps_the_cursor() {
    let d = tree("reveal");
    let mut p = panel(&d);
    p.reveal(&d.join("src/deep/inner.txt"));
    assert_eq!(p.current().unwrap(), d.join("src/deep/inner.txt"));
    fs::write(d.join("src/zzz.txt"), "z").unwrap();
    p.refresh();
    assert!(names(&p).iter().any(|n| n.contains("zzz.txt")));
    assert_eq!(p.current().unwrap(), d.join("src/deep/inner.txt"));
    // Revealing something outside the root does nothing.
    p.reveal(Path::new("/etc/hostname"));
    assert_eq!(p.current().unwrap(), d.join("src/deep/inner.txt"));
}

#[test]
fn respects_gitignore_and_shows_git_state() {
    let d = tree("git");
    let git = |args: &[&str]| {
        assert!(
            Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(&d)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .status()
                .unwrap()
                .success()
        );
    };
    git(&["init", "-q", "-b", "main"]);
    fs::write(d.join(".gitignore"), "docs/\n").unwrap();
    git(&["add", "README.md", "src", ".gitignore"]);
    git(&["commit", "-q", "-m", "i"]);
    fs::write(d.join("README.md"), "changed").unwrap();
    fs::write(d.join("src/new.rs"), "n").unwrap();

    let mut p = Panel::new(&d, PanelOptions::default());
    assert!(!names(&p).contains(&"docs".to_string()), "gitignored folder is hidden: {:?}", names(&p));
    assert_eq!(p.ignored_hidden, 1);
    p.handle_event(&key(KeyCode::Char('I')));
    assert!(names(&p).contains(&"docs".to_string()));
    assert_eq!(p.ignored_hidden, 0);
    p.handle_event(&key(KeyCode::Char('I')));

    let area = Rect::new(0, 0, 30, 8);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default(), true);
    let row = |y: u16| (0..30).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>();
    let text: Vec<String> = (0..8).map(row).collect();
    // src has an untracked file beneath it (`?`), README.md is modified (M).
    assert!(
        text.iter().any(|l| l.contains("src") && l.trim_end().trim_end_matches('│').trim_end().ends_with('?')),
        "{text:?}"
    );
    assert!(text.iter().any(|l| l.contains("README.md") && l.contains('M')), "{text:?}");
}

#[test]
fn renders_a_pane_with_the_root_name_and_scrolls() {
    let d = tree("render");
    for i in 0..30 {
        fs::write(d.join(format!("file{i:02}.txt")), "x").unwrap();
    }
    let mut p = panel(&d);
    let area = Rect::new(0, 0, 30, 8);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default(), true);
    let all: String =
        (0..8).map(|y| (0..30).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(all.contains("files-test-") && all.contains("docs"), "{all}");
    p.handle_event(&key(KeyCode::Char('G')));
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default(), true);
    let all: String =
        (0..8).map(|y| (0..30).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(all.contains("file29.txt") && !all.contains("docs"), "{all}");
}

fn type_text(p: &mut Panel, s: &str) {
    for c in s.chars() {
        p.handle_event(&key(KeyCode::Char(c)));
    }
}

fn cursor_on(p: &mut Panel, name: &str) {
    let i = p.rows.iter().position(|r| r.name == name).unwrap_or_else(|| panic!("{name} in {:?}", names(p)));
    p.cursor = i;
}

#[test]
fn a_creates_in_the_folder_under_the_cursor_and_reports_it() {
    let d = tree("create");
    let mut p = panel(&d);
    cursor_on(&mut p, "src");
    p.handle_event(&key(KeyCode::Char('a')));
    type_text(&mut p, "new.rs");
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Created(d.join("src/new.rs"), false));
    assert!(d.join("src/new.rs").is_file());
    // The folder opened and the cursor is on the new file.
    assert_eq!(p.current().unwrap(), d.join("src/new.rs"));
    // On a file the new item goes next to it; a trailing slash makes a folder.
    p.handle_event(&key(KeyCode::Char('a')));
    type_text(&mut p, "pkg/mod/");
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Created(d.join("src/pkg/mod"), true));
    assert!(d.join("src/pkg/mod").is_dir());
}

#[test]
fn creating_something_that_exists_shows_an_error_and_esc_cancels_the_prompt() {
    let d = tree("exists");
    let mut p = panel(&d);
    cursor_on(&mut p, "README.md");
    p.handle_event(&key(KeyCode::Char('a')));
    type_text(&mut p, "README.md");
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::None);
    assert!(p.message.as_ref().is_some_and(|(m, err)| *err && m.contains("already exists")), "{:?}", p.message);
    p.handle_event(&key(KeyCode::Char('a')));
    type_text(&mut p, "abc");
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::None);
    assert!(matches!(p.mode, Mode::Normal));
    assert!(!d.join("abc").exists());
    // Esc in normal mode still closes the panel.
    assert_eq!(p.handle_event(&key(KeyCode::Esc)), Action::Close);
}

#[test]
fn r_renames_with_the_name_prefilled_and_keeps_folders_open() {
    let d = tree("rename");
    let mut p = panel(&d);
    cursor_on(&mut p, "src");
    p.handle_event(&key(KeyCode::Right)); // open src
    cursor_on(&mut p, "src");
    p.handle_event(&key(KeyCode::Char('r')));
    for _ in 0..3 {
        p.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut p, "lib");
    assert_eq!(p.handle_event(&key(KeyCode::Enter)), Action::Moved(vec![(d.join("src"), d.join("lib"))]));
    assert!(d.join("lib/main.rs").is_file() && !d.join("src").exists());
    assert!(names(&p).contains(&"  main.rs".to_string()), "still expanded: {:?}", names(&p));
    assert_eq!(p.current().unwrap(), d.join("lib"));
}

#[test]
fn d_asks_first_and_deletes_the_marked_items_only_when_pressed_again() {
    let d = tree("delete");
    let mut p = panel(&d);
    cursor_on(&mut p, "docs");
    p.handle_event(&key(KeyCode::Tab)); // mark docs, cursor moves down
    cursor_on(&mut p, "README.md");
    p.handle_event(&key(KeyCode::Tab)); // mark README.md
    assert_eq!(p.marked.len(), 2);
    p.handle_event(&key(KeyCode::Char('D'))); // permanent delete (the trash is not used in tests)
    assert!(matches!(p.mode, Mode::Confirm(ConfirmKind::Delete, _)));
    assert_eq!(p.handle_event(&key(KeyCode::Char('n'))), Action::None);
    assert!(d.join("docs").exists() && d.join("README.md").exists());
    p.handle_event(&key(KeyCode::Char('D')));
    let got = p.handle_event(&key(KeyCode::Char('D')));
    assert_eq!(got, Action::Deleted(vec![d.join("README.md"), d.join("docs")]));
    assert!(!d.join("docs").exists() && !d.join("README.md").exists());
    assert!(p.marked.is_empty());
}

#[test]
fn y_then_p_copies_and_x_then_p_moves_with_unique_names() {
    let d = tree("paste");
    let mut p = panel(&d);
    // Copy README.md into docs/, twice: the second one gets a new name.
    cursor_on(&mut p, "README.md");
    p.handle_event(&key(KeyCode::Char('y')));
    cursor_on(&mut p, "docs");
    assert_eq!(p.handle_event(&key(KeyCode::Char('p'))), Action::None);
    p.handle_event(&key(KeyCode::Char('p')));
    assert!(d.join("docs/README.md").is_file() && d.join("docs/README copy.md").is_file());
    assert!(d.join("README.md").is_file(), "a copy keeps the original");
    // Cut src/main.rs and paste it into docs/: reported as a move, open folders follow.
    cursor_on(&mut p, "src");
    p.handle_event(&key(KeyCode::Right));
    cursor_on(&mut p, "main.rs");
    p.handle_event(&key(KeyCode::Char('x')));
    cursor_on(&mut p, "docs");
    let got = p.handle_event(&key(KeyCode::Char('p')));
    assert_eq!(got, Action::Moved(vec![(d.join("src/main.rs"), d.join("docs/main.rs"))]));
    assert!(!d.join("src/main.rs").exists() && d.join("docs/main.rs").is_file());
    // The cut clipboard is spent.
    p.handle_event(&key(KeyCode::Char('p')));
    assert!(p.message.as_ref().is_some_and(|(m, err)| *err && m.contains("nothing to paste")), "{:?}", p.message);
}

#[test]
fn footer_shows_the_prompt_and_messages_in_the_render() {
    let d = tree("footer");
    let mut p = panel(&d);
    let area = Rect::new(0, 0, 40, 8);
    let draw = |p: &mut Panel| {
        let mut b = Buffer::empty(area);
        p.render(area, &mut b, &Theme::default(), true);
        (0..8).map(|y| (0..40).map(|x| b[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect::<String>()
    };
    p.handle_event(&key(KeyCode::Char('a')));
    type_text(&mut p, "foo");
    assert!(draw(&mut p).contains("new: foo"));
    p.handle_event(&key(KeyCode::Esc));
    cursor_on(&mut p, "README.md");
    p.handle_event(&key(KeyCode::Char('D')));
    assert!(draw(&mut p).contains("README.md? (D"));
    p.handle_event(&key(KeyCode::Char('n')));
    p.handle_event(&key(KeyCode::Char('y')));
    p.handle_event(&key(KeyCode::Char('p')));
    assert!(draw(&mut p).contains("pasted 1 item"));
}

#[test]
fn bookmarks_save_jump_and_delete() {
    let d = tree("bookmarks");
    let mut p = panel(&d);
    leader(&mut p, "bs");
    p.handle_event(&key(KeyCode::Char('a'))); // bookmark a = docs
    leader(&mut p, "bj");
    p.handle_event(&key(KeyCode::Char('a')));
    assert_eq!(p.root(), d.join("docs"));
    leader(&mut p, "ch"); // up again
    assert_eq!(p.root(), d);
    leader(&mut p, "bd");
    p.handle_event(&key(KeyCode::Char('a'))); // delete it
    leader(&mut p, "bj");
    p.handle_event(&key(KeyCode::Char('a')));
    assert_eq!(p.root(), d, "a deleted bookmark goes nowhere");
    // The footer waits for the key and lists the bookmarks.
    leader(&mut p, "bs");
    assert!(p.is_busy());
    p.handle_event(&key(KeyCode::Esc));
}

#[test]
fn columns_show_size_and_time_for_files_when_wide() {
    let d = tree("columns");
    std::fs::write(d.join("README.md"), vec![b'x'; 2048]).unwrap();
    let mut p = Panel::new(
        &d,
        PanelOptions {
            hidden: false,
            git: false,
            linemode: Linemode::SizeTime,
            bookmarks: None,
            background_bytes: None,
            leader: "space".into(),
            actions: Vec::new(),
        },
    );
    let area = Rect::new(0, 0, 70, 8);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default(), true);
    let text: Vec<String> = (0..8).map(|y| (0..70).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
    assert!(text.iter().any(|l| l.contains("README.md") && l.contains("2.0K")), "{text:?}");
    // Narrow panes (a sidebar) drop the columns.
    let area = Rect::new(0, 0, 30, 8);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default(), true);
    let text: Vec<String> = (0..8).map(|y| (0..30).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
    assert!(!text.iter().any(|l| l.contains("2.0K")), "{text:?}");
}

#[test]
fn linemodes_cycle_and_show_permissions_and_folder_sizes() {
    let d = tree("linemode");
    std::fs::write(d.join("src/main.rs"), vec![b'x'; 3000]).unwrap();
    let mut p = Panel::new(
        &d,
        PanelOptions {
            hidden: false,
            git: false,
            linemode: Linemode::SizeTime,
            bookmarks: None,
            background_bytes: None,
            leader: "space".into(),
            actions: Vec::new(),
        },
    );
    let area = Rect::new(0, 0, 70, 8);
    let draw = |p: &mut Panel| {
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, &Theme::default(), true);
        (0..8).map(|y| (0..70).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>()
    };
    draw(&mut p); // starts counting the visible folders
    for _ in 0..300 {
        if p.size_signature().iter().all(|(_, (_, done))| *done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        p.tick();
    }
    let text = draw(&mut p);
    // src holds main.rs (3000 B) and a deeper file: counted, no `+` left.
    assert!(text.iter().any(|l| l.contains("src") && l.contains("2.9K") && !l.contains('+')), "{text:?}");
    leader(&mut p, "vc");
    let text = draw(&mut p);
    assert!(text.iter().any(|l| l.contains("src") && l.contains("rwx")), "{text:?}");
    leader(&mut p, "vc");
    let text = draw(&mut p);
    assert!(!text.iter().any(|l| l.contains("rwx") || l.contains("KiB")), "{text:?}");
}

#[test]
fn a_big_paste_runs_in_the_background_and_reports_when_done() {
    let d = tree("bgpaste");
    std::fs::write(d.join("big.bin"), vec![7u8; 3 << 20]).unwrap();
    let mut p = Panel::new(
        &d,
        PanelOptions {
            hidden: false,
            git: false,
            linemode: Linemode::Off,
            bookmarks: None,
            background_bytes: Some(0),
            leader: "space".into(),
            actions: Vec::new(),
        },
    );
    let find = |p: &mut Panel, name: &str| {
        let rows = p.rows.iter().position(|r| r.name == name).unwrap();
        p.cursor = rows;
    };
    // Cut big.bin into docs/: a move, so the end produces `Moved`.
    find(&mut p, "big.bin");
    p.handle_event(&key(KeyCode::Char('x')));
    find(&mut p, "docs");
    assert_eq!(p.handle_event(&key(KeyCode::Char('p'))), Action::None, "returns at once");
    for _ in 0..500 {
        p.tick();
        if p.job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(d.join("docs/big.bin").is_file() && !d.join("big.bin").exists());
    // The end waits for the next event.
    assert_eq!(
        p.handle_event(&key(KeyCode::Char('R'))),
        Action::Moved(vec![(d.join("big.bin"), d.join("docs/big.bin"))])
    );

    // A copy (yank) of a folder with a few files, also in the background.
    find(&mut p, "src");
    p.handle_event(&key(KeyCode::Char('y')));
    find(&mut p, "docs");
    p.handle_event(&key(KeyCode::Char('p')));
    for _ in 0..500 {
        p.tick();
        if p.job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(d.join("docs/src/main.rs").is_file() && d.join("src/main.rs").is_file());
}

#[test]
fn the_leader_opens_a_menu_and_space_no_longer_toggles_folders() {
    let d = tree("leader");
    let mut p = panel(&d);
    assert!(p.menu().is_none());
    p.handle_event(&key(KeyCode::Char(' ')));
    let menu = p.menu().expect("the leader starts a sequence");
    let names: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
    for want in ["find everywhere", "grep", "view", "bookmarks", "cd", "open with"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    // The popup is drawn over the screen by the host.
    let area = Rect::new(0, 0, 60, 30);
    let mut buf = Buffer::empty(area);
    ui::keys::render_menu(area, &mut buf, &Theme::default(), &menu);
    let text: String =
        (0..30).map(|y| (0..60).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(text.contains("grep") && text.contains("bookmarks"), "{text}");
    // A group opens a deeper menu; Esc closes it without doing anything.
    p.handle_event(&key(KeyCode::Char('v')));
    assert_eq!(p.menu().unwrap().title, "Quick Keys · v");
    p.handle_event(&key(KeyCode::Esc));
    assert!(p.menu().is_none());
    // Space on a folder row does not expand it any more (Enter, l and h do).
    let rows = p.rows.len();
    p.handle_event(&key(KeyCode::Char(' ')));
    p.handle_event(&key(KeyCode::Char('z'))); // not bound: the sequence ends
    assert_eq!(p.rows.len(), rows);
    assert!(p.menu().is_some());
    assert!(p.message.as_ref().is_some_and(|(m, err)| *err && m.contains("not bound")), "{:?}", p.message);
    p.handle_event(&key(KeyCode::Esc));
    assert!(p.menu().is_none());
    // Host actions come back as commands; the leader key is configurable.
    let acts =
        vec![ui::keys::Action { id: "tab.new", name: "new tab", desc: "x", basic: false, keys: &["leader t n"] }];
    let mut p = Panel::new(
        &d,
        PanelOptions {
            hidden: false,
            git: false,
            linemode: Linemode::Off,
            bookmarks: None,
            background_bytes: None,
            leader: ",".into(),
            actions: acts,
        },
    );
    p.handle_event(&key(KeyCode::Char(',')));
    p.handle_event(&key(KeyCode::Char('t')));
    assert_eq!(p.handle_event(&key(KeyCode::Char('n'))), Action::Command("tab.new"));
}

#[test]
fn scrolling_keeps_the_parent_folders_pinned_at_the_top() {
    let d = tree("pinned");
    std::fs::create_dir_all(d.join("src/deep/er")).unwrap();
    for i in 0..30 {
        std::fs::write(d.join(format!("src/deep/er/f{i:02}.txt")), "x").unwrap();
    }
    let mut p = panel(&d);
    p.reveal(&d.join("src/deep/er/f29.txt"));
    let area = Rect::new(0, 0, 40, 8);
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default(), true);
    let text: Vec<String> =
        (0..8).map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect();
    // Border, then the folders above the first scrolling row, outermost first, then files.
    assert!(text[1].contains("src") && !text[1].contains("deep"), "{text:#?}");
    assert!(text[2].contains("deep") && text[3].contains("er"), "{text:#?}");
    assert!(text.iter().any(|l| l.contains("f29.txt")), "the cursor row stays in view: {text:#?}");
    assert_eq!(text.iter().filter(|l| l.contains("src")).count(), 1, "nothing is drawn twice: {text:#?}");
    // At the top nothing is pinned.
    p.handle_event(&key(KeyCode::Char('g')));
    let mut buf = Buffer::empty(area);
    p.render(area, &mut buf, &Theme::default(), true);
    let text: Vec<String> =
        (0..8).map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect();
    assert!(text[1].contains("docs"), "{text:#?}");
}
