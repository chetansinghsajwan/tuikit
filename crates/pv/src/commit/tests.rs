use super::utc;

#[test]
fn formats_utc() {
    assert_eq!(utc(0), "1970-01-01 00:00");
    assert_eq!(utc(1_793_108_415), "2026-10-27 13:40");
    assert_eq!(utc(951_782_400 + 3600 * 5 + 60 * 7), "2000-02-29 05:07"); // leap day
}
