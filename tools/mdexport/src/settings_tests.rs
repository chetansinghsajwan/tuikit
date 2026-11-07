use super::*;

#[test]
fn settings_shape_the_page_and_the_fonts() {
    let s = Settings {
        paper: "letter".into(),
        margin_cm: 3.0,
        font: "Inter".into(),
        size_pt: 12.0,
        justify: false,
        page_numbers: false,
        ..Settings::default()
    };
    let src = typst_source_with("hi\n", None, &s);
    assert!(
        src.contains("paper: \"us-letter\"") && src.contains("margin: (x: 3cm") && !src.contains("numbering"),
        "{src}"
    );
    assert!(
        src.contains("font: (\"Inter\", \"Libertinus Serif\"), size: 12pt") && src.contains("justify: false"),
        "{src}"
    );
    // An unknown font still makes a PDF: the embedded one stands in.
    let pdf = to_pdf("# Hi\n", &Export { base: None, settings: s }).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
}
