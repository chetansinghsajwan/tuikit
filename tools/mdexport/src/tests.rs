use super::*;

#[test]
fn markup_for_headings_emphasis_lists_code_links_and_escapes() {
    let src = typst_source(
        "# Title\n\nSome *em* **strong** `a#b` [x](http://y.z) and 1 < 2 *.\n\n- one\n- two\n\n1. a\n2. b\n\n```rust\nlet x = \"q\";\n```\n",
        None,
    );
    let body = src.split_once("\n\n").map(|_| &src[preamble(&Settings::default()).len()..]).unwrap();
    assert!(body.contains("= Title"), "{body}");
    assert!(body.contains("#emph[em] #strong[strong] #raw(\"a#b\") #link(\"http://y.z\")[x]"), "{body}");
    assert!(body.contains("1 \\< 2 \\*."), "{body}");
    assert!(body.contains("- one\n- two") && body.contains("1. a\n2. b"), "{body}");
    assert!(body.contains("#raw(block: true, lang: \"rust\", \"let x = \\\"q\\\";\")"), "{body}");
}

#[test]
fn tables_quotes_alerts_and_footnotes() {
    let md = "| a | b |\n|:--|--:|\n| 1 | 2 |\n\n> quoted\n\n> [!NOTE]\n> heads up\n\nText[^n].\n\n[^n]: the note\n";
    let src = typst_source(md, None);
    assert!(src.contains("#table(columns: 2, align: (left, right,)"), "{src}");
    assert!(src.contains("table.header([a],[b],),") && src.contains("[1],[2],"), "{src}");
    assert!(src.contains("#quote(block: true)[quoted"), "{src}");
    assert!(src.contains("[Note]") && src.contains("heads up"), "{src}");
    assert!(src.contains("Text#footnote[the note]."), "{src}");
}

#[test]
fn a_pdf_comes_out() {
    let md = "# Hello\n\nA paragraph with *emphasis*, `code`, a [link](https://example.org) and a list:\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```\n\n> [!TIP]\n> done[^1]\n\n[^1]: note\n";
    let pdf = to_pdf(md, &Export::default()).unwrap();
    assert!(pdf.starts_with(b"%PDF-"), "not a PDF");
    assert!(pdf.len() > 2000);
}
