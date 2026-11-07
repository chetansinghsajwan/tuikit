use super::*;

#[test]
fn ages_are_rounded_down_to_the_biggest_unit() {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        as i64;
    assert_eq!(ago(now), "now");
    assert_eq!(ago(now - 125), "2m ago");
    assert_eq!(ago(now - 3 * 3600 - 5), "3h ago");
    assert_eq!(ago(now - 2 * 86_400), "2d ago");
    assert_eq!(ago(now - 400 * 86_400), "1y ago");
    assert_eq!(ago(now + 100), "now", "the future is now");
}
