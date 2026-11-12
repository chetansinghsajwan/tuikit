use super::*;

fn tmp(name: &str, body: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("pv-tab-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

fn flat(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_delimiter_is_sniffed_and_quoted_fields_stay_whole() {
    assert_eq!(sniff("a,b,c\n1,2,3\n"), b',');
    assert_eq!(sniff("a;b;c\n1;2;3\n"), b';');
    assert_eq!(sniff("a\tb\n1\t2\n"), b'\t');
    assert_eq!(sniff("a|b\n1|2\n"), b'|');
    let p = tmp("q.csv", "name,note\nann,\"one, two\"\nbob,plain\n");
    let t = flat(&render(&p, kind(&p).unwrap(), &Theme::default(), &Options::default()).unwrap());
    assert!(t.contains("one, two") && t.contains("csv · 2 columns · 2 of 2 rows"), "{t}");
    assert!(t.contains("╭") && t.contains("name"), "{t}");
}

#[test]
fn numbers_align_right_and_big_files_are_capped_with_a_footer() {
    let mut body = String::from("id,item\n");
    for i in 0..500 {
        body.push_str(&format!("{i},thing {i}\n"));
    }
    let p = tmp("big.csv", &body);
    let lines = render(&p, kind(&p).unwrap(), &Theme::default(), &Options::default()).unwrap();
    let t = flat(&lines);
    assert!(t.contains("200 of 500 rows") && t.contains("… 300 more rows"), "{t}");
    assert!(t.contains("│  0 │") || t.contains("│   0 │"), "the id column is right aligned: {t}");
    assert!(lines.len() < 220);
}

#[test]
fn many_columns_are_cut_to_what_fits_and_tsv_uses_tabs() {
    let head: Vec<String> = (0..40).map(|i| format!("column{i}")).collect();
    let p = tmp("wide.tsv", &format!("{}\n{}\n", head.join("\t"), head.join("\t")));
    let t = flat(
        &render(
            &p,
            kind(&p).unwrap(),
            &Theme::default(),
            &Options { width: Some(60), height: None, git: false, focus: None },
        )
        .unwrap(),
    );
    assert!(t.contains("columns not shown"), "{t}");
    assert!(t.lines().all(|l| unicode_width::UnicodeWidthStr::width(l) <= 60), "{t}");
    assert_eq!(kind(Path::new("a.xlsx")), Some(Kind::Sheet));
    assert_eq!(kind(Path::new("a.rs")), None);
}
