use super::*;

#[test]
fn base64_matches_the_standard_examples() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn the_sequence_wraps_the_encoded_text() {
    assert_eq!(sequence("abc"), "\x1b]52;c;YWJj\x07");
}
