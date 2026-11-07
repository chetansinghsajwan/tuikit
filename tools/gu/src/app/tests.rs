use super::*;
use ratatui::crossterm::event::KeyModifiers;
use std::fs;
use std::process::Command;
use std::sync::Once;

fn identity() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        for (k, v) in [
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_SYSTEM", "/dev/null"),
        ] {
            // SAFETY: set once, before any test starts git, to constants.
            unsafe { std::env::set_var(k, v) };
        }
    });
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git").args(args).current_dir(dir).status().unwrap().success(), "git {args:?}");
}

fn repo(name: &str) -> PathBuf {
    identity();
    let d = std::env::temp_dir().join(format!("git-ui-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    fs::write(d.join("a.txt"), "1\n2\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "first commit"]);
    git(&d, &["branch", "other"]);
    d.canonicalize().unwrap()
}

/// The row of the Refs tree that shows the ref called `name`.
fn ref_row(app: &App, name: &str) -> usize {
    app.ref_rows
        .iter()
        .position(|r| matches!(r, RefRow::Item { index, .. } if app.branches[*index].name == name))
        .unwrap_or_else(|| panic!("no ref {name}"))
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn press(app: &mut App, keys: &str) {
    for c in keys.chars() {
        assert_eq!(app.handle_event(&key(KeyCode::Char(c))), Quit::No);
    }
}

fn screen(app: &mut App) -> Vec<String> {
    let area = Rect::new(0, 0, 120, 36);
    let mut buf = Buffer::empty(area);
    app.render(area, &mut buf);
    (0..36)
        .map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
        .collect()
}

fn head(d: &Path) -> String {
    Repo::discover(d).unwrap().branch().unwrap()
}

#[test]
fn the_commits_tab_shows_every_branch_and_searches() {
    let d = repo("tab");
    git(&d, &["checkout", "-q", "-b", "side"]);
    fs::write(d.join("b.txt"), "b\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "needle on a side branch"]);
    git(&d, &["checkout", "-q", "main"]);
    let mut app = App::new(&d).unwrap();
    // The Status tab lists the checked-out branch only.
    assert!(!screen(&mut app).join("\n").contains("needle"));
    // `]` opens the Commits tab: every branch, with the graph and the ref labels.
    press(&mut app, "]");
    let all = screen(&mut app).join("\n");
    assert!(all.contains("needle on a side branch") && all.contains("side") && all.contains("●"), "{all}");
    // `/` searches: the cursor lands on the match and the message counts it.
    press(&mut app, "/");
    type_text(&mut app, "needle");
    assert_eq!(app.commits[app.lists[1].cursor].subject, "needle on a side branch");
    assert!(app.message.as_ref().is_some_and(|(m, _)| m.contains("match 1 of 1")), "{:?}", app.message);
    // Esc clears the search first, then leaves the tab and restores the pane's scope.
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.search.is_none());
    press(&mut app, "[");
    assert!(!screen(&mut app).join("\n").contains("needle"));
}

#[test]
fn switch_to_takes_a_ref_a_chosen_option_or_a_commit_hash() {
    let d = repo("switch");
    git(&d, &["branch", "topic-one"]);
    git(&d, &["branch", "side-two"]);
    let mut app = App::new(&d).unwrap();
    let root = app.repo.commits(5).unwrap()[0].oid.clone();
    press(&mut app, " ");
    press(&mut app, "S");
    type_text(&mut app, "topic-one");
    assert_eq!(app.repo.branch().as_deref(), Some("topic-one"));
    // A fragment picks the first match.
    press(&mut app, " ");
    press(&mut app, "S");
    type_text(&mut app, "side");
    assert_eq!(app.repo.branch().as_deref(), Some("side-two"));
    // A hash detaches HEAD.
    press(&mut app, " ");
    press(&mut app, "S");
    type_text(&mut app, &root[..8]);
    assert_eq!(app.repo.branch(), None, "{:?}", app.message);
    // Nothing of that name.
    press(&mut app, " ");
    press(&mut app, "S");
    type_text(&mut app, "zzzz-nothing");
    assert!(app.message.as_ref().is_some_and(|(m, err)| *err && m.contains("no branch")));
}

#[test]
fn amend_adds_the_staged_file_and_enter_keeps_the_message() {
    let d = repo("amend");
    let mut app = App::new(&d).unwrap();
    fs::write(d.join("more.txt"), "x\n").unwrap();
    git(&d, &["add", "more.txt"]);
    let before = app.repo.commits(5).unwrap().len();
    press(&mut app, "2");
    press(&mut app, "A");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.repo.commits(5).unwrap().len(), before, "amended, not added");
    assert!(app.repo.status().unwrap().is_empty(), "{:?}", app.message);
}

fn tab(app: &mut App) {
    app.handle_event(&key(KeyCode::Tab));
}

#[test]
fn l_on_a_commit_shows_its_files_as_a_tree_with_the_diff_of_each() {
    let d = repo("cfiles");
    fs::create_dir_all(d.join("src/deep")).unwrap();
    fs::write(d.join("src/a.rs"), "fn a() {}\n").unwrap();
    fs::write(d.join("src/deep/b.rs"), "fn b() {}\n").unwrap();
    fs::write(d.join("top.txt"), "t\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "add files"]);
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    press(&mut app, "l");
    let text = screen(&mut app).join("\n");
    assert!(
        text.contains("add files") && text.contains("src/") && text.contains("top.txt") && text.contains("+1"),
        "{text}"
    );
    // The diff of the file under the cursor shows beside the tree.
    let cf = app.commit_files.as_ref().unwrap();
    let at =
        cf.rows.iter().position(|r| matches!(r, FileRow::File { change, .. } if change.path == "src/a.rs")).unwrap();
    app.commit_files.as_mut().unwrap().cursor = at;
    let text = screen(&mut app).join("\n");
    assert!(text.contains("fn a()"), "{text}");
    // h folds and unfolds a folder, and goes to the parent; esc goes back to the commits.
    app.commit_files.as_mut().unwrap().cursor = 0;
    let open = app.commit_files.as_ref().unwrap().rows.len();
    press(&mut app, "h");
    assert!(app.commit_files.as_ref().unwrap().rows.len() < open, "the folder is folded");
    press(&mut app, "l");
    assert_eq!(app.commit_files.as_ref().unwrap().rows.len(), open);
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.commit_files.is_none());
    assert!(screen(&mut app).join("\n").contains("add files"));
}

#[test]
fn the_filter_popup_chooses_several_things_and_the_bar_shows_them() {
    let d = repo("filter");
    commit_file(&d, "b.txt", "b\n", "add feature alpha");
    commit_file(&d, "c.txt", "c\n", "fix: bug beta");
    commit_file(&d, "d.txt", "d\n", "feat(ui): gamma");
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    assert_eq!(app.commits.len(), 4);
    // By message: the text section is the first.
    press(&mut app, "f");
    assert!(screen(&mut app).join("\n").contains("Filter commits"));
    type_text(&mut app, "alpha");
    assert_eq!(app.commits.len(), 1, "{:?}", app.message);
    assert!(screen(&mut app).join("\n").contains("message alpha"), "the bar names the filter");
    press(&mut app, "F");
    assert_eq!(app.commits.len(), 4);
    // By category: feat, then feat and fix together.
    press(&mut app, "f");
    tab(&mut app); // authors
    tab(&mut app); // categories: feat, fix
    press(&mut app, " ");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.commits.len(), 1, "{:?}", app.message);
    assert_eq!(app.commits[0].subject, "feat(ui): gamma");
    press(&mut app, "f");
    tab(&mut app);
    tab(&mut app);
    app.handle_event(&key(KeyCode::Down));
    press(&mut app, " ");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.commits.len(), 2);
    assert!(screen(&mut app).join("\n").contains("category feat, fix"));
    // The popup shows what is set, and a date that excludes everything empties the list.
    press(&mut app, "f");
    for _ in 0..4 {
        tab(&mut app);
    }
    press(&mut app, " ");
    assert!(screen(&mut app).join("\n").contains("since: 1d"));
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.commits.len(), 2, "everything is newer than a day");
    app.filter.since = Some(("x".into(), i64::MAX));
    app.refilter("x");
    assert_eq!(app.commits.len(), 0);
}

#[test]
fn since_understands_units_and_dates() {
    let now = 1_000_000_000;
    assert_eq!(parse_since("7d", now), Some(now - 7 * 86_400));
    assert_eq!(parse_since("2w", now), Some(now - 14 * 86_400));
    assert_eq!(parse_since("1970-01-02", now), Some(86_400));
    assert_eq!(parse_since("2000-03-01", now), Some(951_868_800));
    assert_eq!(parse_since("tomorrow", now), None);
    assert_eq!(parse_since("2026-13-01", now), None);
}

#[test]
fn shows_the_files_page_and_the_commits_and_refs_tabs() {
    let d = repo("panes");
    let mut app = App::new(&d).unwrap();
    // The first page is the changed files only, under the tab strip.
    let all = screen(&mut app).join("\n");
    for want in ["Status", "Commits", "Refs", "Files"] {
        assert!(all.contains(want), "{want} in\n{all}");
    }
    assert!(!all.contains("Stashes") && !all.contains("Branches"), "{all}");
    // Refs: the branches (and tags, remotes) and the stashes.
    press(&mut app, "3");
    let all = screen(&mut app).join("\n");
    assert!(all.contains("Stashes") && all.contains("main") && all.contains("other"), "{all}");
    // Commits: the graph with the refs as pills.
    press(&mut app, "2");
    let all = screen(&mut app).join("\n");
    assert!(all.contains("●") && all.contains("main") && all.contains("other"), "{all}");
}

#[test]
fn focus_moves_with_tab_and_digits_and_q_quits() {
    let d = repo("focus");
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    assert_eq!((app.screen, app.focus), (Screen::Commits, Focus::Commits));
    // Commits has one pane: Tab stays. Refs has two: Tab and Shift-Tab move between them.
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.focus, Focus::Commits);
    press(&mut app, "3");
    assert_eq!((app.screen, app.focus), (Screen::Refs, Focus::Branches));
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.focus, Focus::Stashes);
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.focus, Focus::Branches);
    app.handle_event(&Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)));
    assert_eq!(app.focus, Focus::Stashes);
    press(&mut app, "1");
    assert_eq!((app.screen, app.focus), (Screen::Status, Focus::Files));
    assert_eq!(app.handle_event(&key(KeyCode::Char('q'))), Quit::Yes);
}

#[test]
fn the_refs_cursor_follows_its_ref_across_reloads_and_case_makes_folders() {
    let d = repo("cursor");
    git(&d, &["branch", "Feature/a"]);
    git(&d, &["branch", "feature/b"]);
    git(&d, &["branch", "Feature/c"]);
    let mut app = App::new(&d).unwrap();
    press(&mut app, "3");
    // `Feature/` and `feature/` are two folders, each once.
    let folders: Vec<String> = app
        .ref_rows
        .iter()
        .filter_map(|r| if let RefRow::Folder { name, .. } = r { Some(name.clone()) } else { None })
        .collect();
    assert_eq!(folders, vec!["Feature".to_string(), "feature".to_string()]);
    // A new branch sorts before the one under the cursor: the cursor stays on its ref.
    app.lists[0].cursor = ref_row(&app, "other");
    git(&d, &["branch", "aaa"]);
    app.reload_all();
    assert_eq!(app.current_ref().map(|b| b.name.as_str()), Some("other"));
    // A folder rename that cannot be done as a whole does nothing at all.
    git(&d, &["branch", "taken-a"]);
    app.reload_all();
    app.lists[0].cursor =
        app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "Feature")).unwrap();
    press(&mut app, "r");
    for _ in 0..7 {
        app.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut app, "x");
    assert!(
        app.branches.iter().any(|b| b.name == "x/a") && app.branches.iter().any(|b| b.name == "x/c"),
        "{:?}",
        app.message
    );
    let before: Vec<String> = app.branches.iter().map(|b| b.name.clone()).collect();
    app.lists[0].cursor =
        app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "x")).unwrap();
    press(&mut app, "r");
    for _ in 0..1 {
        app.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut app, "feature"); // `feature/b` exists, but x/a -> feature/a and x/c -> feature/c are free
    let after: Vec<String> = app.branches.iter().map(|b| b.name.clone()).collect();
    assert_ne!(before, after);
    // A name that is not valid says so and changes nothing.
    let snapshot = after.clone();
    app.lists[0].cursor =
        app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "feature")).unwrap();
    press(&mut app, "r");
    for _ in 0..7 {
        app.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut app, "bad name");
    assert!(app.message.as_ref().is_some_and(|(_, e)| *e), "{:?}", app.message);
    assert_eq!(app.branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>(), snapshot);
}

#[test]
fn tags_and_folders_of_refs_can_be_renamed() {
    let d = repo("renames");
    git(&d, &["branch", "feature/login"]);
    git(&d, &["branch", "feature/signup"]);
    git(&d, &["tag", "v1"]);
    let mut app = App::new(&d).unwrap();
    press(&mut app, "3");
    // A tag.
    app.lists[0].cursor = ref_row(&app, "v1");
    press(&mut app, "r");
    for _ in 0..2 {
        app.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut app, "v2");
    assert!(
        app.branches.iter().any(|b| b.name == "v2") && app.branches.iter().all(|b| b.name != "v1"),
        "{:?}",
        app.message
    );
    // A folder: everything under `feature/` moves to `feat/`.
    app.lists[0].cursor =
        app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "feature")).unwrap();
    press(&mut app, "r");
    for _ in 0..7 {
        app.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut app, "feat");
    let names: Vec<&str> = app.branches.iter().map(|b| b.name.as_str()).collect();
    assert!(
        names.contains(&"feat/login")
            && names.contains(&"feat/signup")
            && !names.iter().any(|n| n.starts_with("feature/")),
        "{names:?} {:?}",
        app.message
    );
    // A section cannot be renamed.
    app.lists[0].cursor = 0;
    press(&mut app, "r");
    assert!(matches!(app.mode, Mode::Normal) && app.message.as_ref().is_some_and(|(_, e)| *e));
}

#[test]
fn the_refs_are_a_tree_of_sections_and_folders_that_fold() {
    let d = repo("reftree");
    git(&d, &["branch", "feature/login"]);
    git(&d, &["branch", "feature/signup/form"]);
    git(&d, &["tag", "v1"]);
    let mut app = App::new(&d).unwrap();
    let shape = |app: &App| -> Vec<String> {
        app.ref_rows
            .iter()
            .map(|r| match r {
                RefRow::Section { label, .. } => label.to_string(),
                RefRow::Folder { name, depth, .. } => format!("{}{name}/", " ".repeat(*depth)),
                RefRow::Item { name, depth, .. } => format!("{}{name}", " ".repeat(*depth)),
            })
            .collect()
    };
    assert_eq!(
        shape(&app),
        vec!["Branches", " feature/", "  login", "  signup/", "   form", " main", " other", "Tags", " v1"]
    );
    // Enter on a folder folds it; the cursor stays on the folder.
    press(&mut app, "3");
    app.lists[0].cursor =
        app.ref_rows.iter().position(|r| matches!(r, RefRow::Folder { name, .. } if name == "feature")).unwrap();
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(shape(&app), vec!["Branches", " feature/", " main", " other", "Tags", " v1"]);
    assert!(matches!(app.ref_rows[app.lists[0].cursor], RefRow::Folder { .. }));
    // Enter again unfolds it, a section folds all its refs.
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.ref_rows.len(), 9);
    app.lists[0].cursor = 0;
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(shape(&app), vec!["Branches", "Tags", " v1"]);
}

#[test]
fn branches_checkout_create_and_delete() {
    let d = repo("branches");
    let mut app = App::new(&d).unwrap();
    press(&mut app, "3");
    // main (checked out) is first, `other` second: j then Enter switches.
    press(&mut app, "j");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(head(&d), "other");
    // New branch through the prompt.
    press(&mut app, "n");
    press(&mut app, "topic");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(head(&d), "topic");
    // Delete `other` (not checked out) with a confirmation.
    app.lists[0].cursor = ref_row(&app, "other");
    press(&mut app, "d");
    assert!(matches!(app.mode, Mode::Confirm(_)));
    press(&mut app, "y");
    assert!(app.branches.iter().all(|b| b.name != "other"), "{:?}", app.branches);
    // The checked-out branch cannot be deleted.
    app.lists[0].cursor = ref_row(&app, "topic");
    press(&mut app, "d");
    assert!(matches!(app.mode, Mode::Normal) && app.message.as_ref().is_some_and(|(_, e)| *e));
}

#[test]
fn stash_new_apply_pop_and_drop() {
    let d = repo("stash");
    let mut app = App::new(&d).unwrap();
    fs::write(d.join("a.txt"), "changed\n").unwrap();
    press(&mut app, "4");
    press(&mut app, "n");
    press(&mut app, "wip");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.stashes.len(), 1, "{:?}", app.message);
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "1\n2\n");
    press(&mut app, "p");
    assert!(app.stashes.is_empty());
    assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "changed\n");
    // Drop asks first.
    press(&mut app, "n");
    press(&mut app, "again");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.stashes.len(), 1);
    press(&mut app, "d");
    press(&mut app, "n"); // anything but y cancels
    assert_eq!(app.stashes.len(), 1);
    press(&mut app, "d");
    press(&mut app, "y");
    assert!(app.stashes.is_empty());
}

#[test]
fn a_branch_and_its_remote_branch_share_a_pill() {
    let r = |name: &str, kind| RefInfo {
        name: name.into(),
        kind,
        oid: "x".into(),
        is_head: false,
        subject: String::new(),
        time: 0,
    };
    let refs = [
        r("main", RefKind::Branch),
        r("origin/main", RefKind::Remote),
        r("origin/other", RefKind::Remote),
        r("v1", RefKind::Tag),
    ];
    let got: Vec<(String, Vec<String>)> = pills(&refs).into_iter().map(|(r, o)| (r.name, o)).collect();
    assert_eq!(
        got,
        vec![
            ("main".to_string(), vec!["origin".to_string()]),
            ("v1".to_string(), vec![]),
            ("origin/other".to_string(), vec![])
        ]
    );
    assert_eq!(pill_text(&refs[0], &["origin".to_string()]), "main · origin");
}

#[test]
fn initials_and_author_colors() {
    assert_eq!(initials("Chetan Singh Sajwan"), "CS");
    assert_eq!(initials("root"), "ro");
    assert_eq!(initials("Ada"), "Ad");
    assert_eq!(initials("  "), "?");
    let t = Theme::default();
    assert_eq!(author_color(&t, "Ada Lovelace"), author_color(&t, "Ada Lovelace"));
}

#[test]
fn the_files_pane_still_stages_and_commits() {
    let d = repo("files");
    let mut app = App::new(&d).unwrap();
    fs::write(d.join("a.txt"), "1\n2\n3\n").unwrap();
    app.reload_all();
    press(&mut app, "s"); // stage the highlighted file
    press(&mut app, "c");
    press(&mut app, "second");
    app.handle_event(&key(KeyCode::Enter));
    press(&mut app, "2"); // the commits load when their tab is shown
    assert_eq!(app.commits.len(), 2, "{:?}", app.message);
    assert_eq!(app.commits[0].subject, "second");
}

#[test]
fn the_leader_menu_palette_and_help_work_in_every_pane() {
    let d = repo("leader");
    let mut app = App::new(&d).unwrap();
    // Files pane: its own table; Space then Space asks the host for the palette.
    press_raw(&mut app, ' ');
    assert!(app.files.menu().is_some());
    assert_eq!(app.handle_event(&key(KeyCode::Char(' '))), Quit::Palette);
    let ids: Vec<&str> = app.keys_rows().iter().map(|r| r.id).collect();
    assert!(ids.contains(&"commit.new") && ids.contains(&"focus.branches"), "{ids:?}");
    // Branches pane: the leader menu lists the branch actions, a leader key runs one.
    press(&mut app, "3");
    press_raw(&mut app, ' ');
    let menu = match app.focus {
        Focus::Branches => app.maps[0].menu().unwrap(),
        _ => panic!(),
    };
    let names: Vec<&str> = menu.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"new branch") && names.contains(&"merge") && names.contains(&"commands"), "{names:?}");
    press_raw(&mut app, 'n');
    assert!(matches!(app.mode, Mode::Prompt(PromptKind::NewBranch, _)));
    app.handle_event(&key(KeyCode::Esc));
    // The palette rows of a pane hold the basic keys too (they are filtered when shown).
    let rows = app.keys_rows();
    assert!(
        rows.iter().any(|r| r.id == "nav.down" && r.basic) && rows.iter().any(|r| r.id == "branch.merge" && !r.basic)
    );
    // The palette's pick runs in the pane; `?` opens the help, any other key closes it with Esc.
    assert_eq!(app.run_palette("branch.new"), Quit::No);
    assert!(matches!(app.mode, Mode::Prompt(PromptKind::NewBranch, _)));
    app.handle_event(&key(KeyCode::Esc));
    press(&mut app, "?");
    assert!(app.help.is_some());
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.help.is_none());
    // Stash pane has its own actions.
    press(&mut app, "4");
    let ids: Vec<&str> = app.keys_rows().iter().map(|r| r.id).collect();
    assert!(ids.contains(&"stash.apply") && !ids.contains(&"branch.merge"), "{ids:?}");
}

fn press_raw(app: &mut App, c: char) {
    app.handle_event(&key(KeyCode::Char(c)));
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_event(&key(KeyCode::Char(c)));
    }
    app.handle_event(&key(KeyCode::Enter));
}

fn commit_file(d: &Path, name: &str, body: &str, msg: &str) {
    fs::write(d.join(name), body).unwrap();
    git(d, &["add", name]);
    git(d, &["commit", "-qm", msg]);
}

#[test]
fn capital_m_shows_the_diff_from_a_commit_to_the_work_tree() {
    let d = repo("workdiff");
    commit_file(&d, "b.txt", "two\n", "second");
    fs::write(d.join("b.txt"), "two changed\n").unwrap();
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    press(&mut app, "G");
    press(&mut app, "M");
    let text = screen(&mut app).join("\n");
    assert!(text.contains("work tree") && text.contains("b.txt") && text.contains("two changed"), "{text}");
    press(&mut app, "M");
    assert!(!app.work);
}

#[test]
fn the_refs_column_fits_the_widest_commit_and_the_free_space_goes_to_what_is_left() {
    let d = repo("cols");
    commit_file(&d, "b.txt", "two\n", "second");
    git(&d, &["tag", "a-rather-long-tag-name"]);
    git(&d, &["tag", "another-long-tag-name"]);
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    app.fit_columns(120);
    assert!(app.refs_w > 24, "the refs of the widest commit all fit: {}", app.refs_w);
    // Without the message the refs take the rest of the row.
    app.cols.subject = false;
    app.fit_columns(120);
    assert!(app.refs_w > app.refs_need, "{} vs {}", app.refs_w, app.refs_need);
    // In a narrow list the message keeps its room and the refs give way.
    app.cols.subject = true;
    app.fit_columns(60);
    assert!(app.refs_w <= 60 - 24, "{}", app.refs_w);
}

#[test]
fn z_folds_the_file_at_the_top_of_the_diff_and_w_ignores_whitespace() {
    let d = repo("fold");
    commit_file(&d, "b.txt", "two\n", "second");
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    press(&mut app, "p");
    let before = screen(&mut app).join("\n");
    assert!(before.contains("⋯ line"), "{before}");
    press(&mut app, "z");
    let folded = screen(&mut app).join("\n");
    assert!(folded.contains("folded"), "{folded}");
    press(&mut app, "z");
    assert!(!screen(&mut app).join("\n").contains("folded"));
    press(&mut app, "w");
    assert!(pv::diff_ignoring_whitespace());
    press(&mut app, "w");
    assert!(!pv::diff_ignoring_whitespace());
}

#[test]
fn m_marks_a_commit_and_the_preview_shows_the_diff_to_the_highlighted_one() {
    let d = repo("compare");
    commit_file(&d, "b.txt", "two\n", "second");
    commit_file(&d, "c.txt", "three\n", "third");
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    // Newest first: mark the oldest, then move back to the newest.
    press(&mut app, "G");
    press(&mut app, "m");
    press(&mut app, "g");
    let text = screen(&mut app).join("\n");
    assert!(text.contains("from") && text.contains("to") && text.contains("b.txt") && text.contains("c.txt"), "{text}");
    // m on the marked commit, or Esc, clears it.
    press(&mut app, "G");
    press(&mut app, "m");
    assert!(app.compare.is_none());
}

#[test]
fn commit_actions_check_out_tag_branch_reword_reset_and_revert() {
    let d = repo("commits");
    commit_file(&d, "b.txt", "two\n", "second");
    let mut app = App::new(&d).unwrap();
    press(&mut app, "2");
    // Reword the last commit.
    press(&mut app, "r");
    for _ in 0..6 {
        app.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut app, "reworded");
    assert_eq!(app.commits[0].subject, "reworded", "{:?}", app.message);
    // Tag and branch at the first commit.
    press(&mut app, "j");
    press(&mut app, "t");
    type_text(&mut app, "v0");
    assert!(app.branches.iter().any(|b| b.name == "v0"), "{:?}", app.message);
    press(&mut app, "b");
    type_text(&mut app, "from-first");
    assert_eq!(app.repo.branch().as_deref(), Some("from-first"));
    // Check out the head commit of main again (detached), then cherry-pick the reworded one elsewhere.
    press(&mut app, "k");
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.repo.branch(), None, "a detached HEAD");
    git(&d, &["switch", "-q", "main"]);
    app.reload_all();
    // Revert asks first; reset hard asks first, soft does not.
    press(&mut app, "R");
    assert!(matches!(app.mode, Mode::Confirm(Confirm::Revert(_))));
    press(&mut app, "y");
    assert_eq!(app.commits.len(), 3, "{:?}", app.message);
    app.lists[1].cursor = 2;
    press(&mut app, " ");
    press(&mut app, "z");
    press(&mut app, "h");
    assert!(matches!(app.mode, Mode::Confirm(Confirm::Reset { .. })));
    press(&mut app, "y");
    assert_eq!(app.commits.len(), 1, "{:?}", app.message);
}

#[test]
fn branches_rebase_rename_and_delete_tags() {
    let d = repo("branchops");
    git(&d, &["switch", "-q", "-c", "feat"]);
    commit_file(&d, "f.txt", "f\n", "feature");
    git(&d, &["switch", "-q", "main"]);
    commit_file(&d, "m.txt", "m\n", "main moves");
    git(&d, &["tag", "v9"]);
    git(&d, &["switch", "-q", "feat"]);
    let mut app = App::new(&d).unwrap();
    press(&mut app, "3");
    let at = |app: &App, name: &str| ref_row(app, name);
    // Rebase feat onto main.
    app.lists[0].cursor = at(&app, "main");
    press(&mut app, "B");
    assert!(matches!(app.mode, Mode::Confirm(Confirm::Rebase(_))));
    press(&mut app, "y");
    assert!(d.join("m.txt").exists() && d.join("f.txt").exists(), "{:?}", app.message);
    // Rename it.
    app.lists[0].cursor = at(&app, "feat");
    press(&mut app, "r");
    for _ in 0..4 {
        app.handle_event(&key(KeyCode::Backspace));
    }
    type_text(&mut app, "topic");
    assert!(app.branches.iter().any(|b| b.name == "topic"), "{:?}", app.message);
    // A tag is deleted through the same key, with a confirmation.
    app.lists[0].cursor = at(&app, "v9");
    press(&mut app, "d");
    assert!(matches!(app.mode, Mode::Confirm(Confirm::DeleteTag(_))));
    press(&mut app, "y");
    assert!(app.branches.iter().all(|b| b.name != "v9"));
}

#[test]
fn remote_commands_run_in_the_background_and_report() {
    let d = repo("remote");
    let remote = std::env::temp_dir().join(format!("git-ui-remote-{}", std::process::id()));
    let _ = fs::remove_dir_all(&remote);
    fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(&d, &["remote", "add", "origin", remote.to_str().unwrap()]);
    let mut app = App::new(&d).unwrap();
    let wait = |app: &mut App| {
        for _ in 0..500 {
            if !app.tick() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    // Space r P pushes (the first push sets the upstream) without blocking the screen.
    press_raw(&mut app, ' ');
    press_raw(&mut app, 'r');
    press_raw(&mut app, 'P');
    assert!(app.job.is_some());
    wait(&mut app);
    assert_eq!(app.message.as_ref().map(|(m, e)| (m.as_str(), *e)), Some(("pushed", false)), "{:?}", app.message);
    assert!(app.repo.has_upstream());
    // Fetch and pull report too; force push asks first.
    for k in ['f', 'p'] {
        press_raw(&mut app, ' ');
        press_raw(&mut app, 'r');
        press_raw(&mut app, k);
        wait(&mut app);
    }
    assert_eq!(app.message.as_ref().map(|(m, _)| m.as_str()), Some("pulled"));
    press_raw(&mut app, ' ');
    press_raw(&mut app, 'r');
    press_raw(&mut app, 'F');
    assert!(matches!(app.mode, Mode::Confirm(Confirm::ForcePush)));
    press(&mut app, "n");
    assert!(app.job.is_none());
}

#[test]
fn a_stopped_operation_shows_in_the_status_line_and_can_be_aborted() {
    let d = repo("stopped");
    git(&d, &["switch", "-q", "other"]);
    commit_file(&d, "b.txt", "other\n", "other");
    git(&d, &["switch", "-q", "main"]);
    commit_file(&d, "b.txt", "main\n", "main");
    let mut app = App::new(&d).unwrap();
    press(&mut app, "3");
    app.lists[0].cursor = ref_row(&app, "other");
    press(&mut app, "M");
    press(&mut app, "y"); // the merge conflicts
    assert_eq!(app.operation_name(), Some("MERGING"), "{:?}", app.message);
    press(&mut app, "j"); // clears the failure message the merge left
    let shown = screen(&mut app).join("\n");
    assert!(shown.contains("MERGING"), "{shown}");
    press_raw(&mut app, ' ');
    press_raw(&mut app, 'o');
    press_raw(&mut app, 'a');
    assert_eq!(app.operation_name(), None, "{:?}", app.message);
}
