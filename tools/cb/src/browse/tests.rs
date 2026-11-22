//! Tests for the history rows.

use super::*;

/// A multi-line copy is one row.
#[test]
fn row_is_one_line() {
    assert_eq!(row("  a\n  b \n"), "a b");
}
