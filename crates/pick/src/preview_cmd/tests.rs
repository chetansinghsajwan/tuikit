use super::*;

fn cmd(c: &str, d: Option<&str>) -> PreviewCmd {
    PreviewCmd { cmd: c.into(), delimiter: d.map(Into::into) }
}

#[test]
fn expands_whole_line_fields_and_ranges_with_quoting() {
    let p = cmd("echo {} / {1} / {2..} / {9} / {x}", None);
    assert_eq!(p.expand("12 it's  fine"), r"echo '12 it'\''s  fine' / '12' / 'it'\''s fine' / '' / {x}");
    let e = cmd("show {1} {2..}", Some("="));
    assert_eq!(e.expand("PATH=/a:/b=c"), "show 'PATH' '/a:/b=c'");
}

#[test]
fn runs_the_command_and_reports_timeouts_and_stderr() {
    assert_eq!(cmd("printf %s {2}", None).run("a b c"), "b");
    assert!(cmd("echo oops >&2", None).run("x").contains("oops"));
    assert_eq!(cmd("sleep 5", None).run("x"), "preview timed out");
}
