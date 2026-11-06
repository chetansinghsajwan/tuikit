use super::*;

#[test]
fn the_cursor_in_a_block_gets_that_blocks_column_and_blank_lines_belong_to_it() {
    // fn a() {        0
    //     x;          1
    //                 (blank)
    //     if b {      1
    //         y;      2
    //     }           1
    // }               0
    let own = [Some(0), Some(4), None, Some(4), Some(8), Some(4), Some(0)];
    assert_eq!(indent_levels(&own), vec![0, 4, 4, 4, 8, 4, 0]);
    let s = code_scope(&own, 4, 4).unwrap(); // `y;` sits in the `if` block
    assert_eq!((s.from, s.to, s.col), (3, 5, 1));
    let s = code_scope(&own, 4, 1).unwrap(); // `x;` sits in the fn body
    assert_eq!((s.from, s.col), (0, 0));
    assert!(code_scope(&own, 4, 6).is_none(), "the closing line at the top has no scope");
}
