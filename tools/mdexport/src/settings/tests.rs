use super::*;

#[test]
fn a_partial_file_keeps_the_other_defaults() {
    let s: Settings = toml::from_str("font = \"Inter\"\nsize_pt = 12.0\n").unwrap();
    assert_eq!((s.font.as_str(), s.size_pt, s.paper.as_str(), s.justify), ("Inter", 12.0, "a4", true));
    assert!(Settings::default().lines().contains("mono=DejaVu Sans Mono"));
}
