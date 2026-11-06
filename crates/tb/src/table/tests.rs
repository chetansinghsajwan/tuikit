use super::*;

const SAMPLE: &str = "id,name,note\n1,\"a,b\",x\n22,\"say \"\"hi\"\"\",\n";

#[test]
fn a_file_that_is_not_changed_is_written_back_exactly() {
    for text in [SAMPLE, "a,b\r\nc,d\r\n", "a,b\nc,d", "", "x\n\ny\n", "\"multi\nline\",z\n"] {
        assert_eq!(Table::parse(text, ',').to_text(), text, "{text:?}");
    }
}

#[test]
fn quotes_protect_delimiters_and_values_are_unquoted() {
    let t = Table::parse(SAMPLE, ',');
    assert_eq!((t.row_count(), t.col_count()), (3, 3));
    assert_eq!(t.value(1, 1), "a,b");
    assert_eq!(t.value(2, 1), "say \"hi\"");
    assert_eq!(t.value(2, 2), "");
    assert_eq!(Table::parse("\"multi\nline\",z\n", ',').value(0, 0), "multi\nline");
}

#[test]
fn set_quotes_when_it_has_to_and_edits_keep_the_rest() {
    let mut t = Table::parse("a,b\nc,d\n", ',');
    t.set(0, 1, "x,y");
    assert_eq!(t.to_text(), "a,\"x,y\"\nc,d\n");
    t.set(3, 2, "z");
    assert_eq!(t.row_count(), 4);
    assert_eq!(t.value(3, 2), "z");
}

#[test]
fn rows_and_columns_are_inserted_deleted_and_moved() {
    let mut t = Table::parse("a,b,c\n1,2,3\n", ',');
    t.insert_col(1);
    assert_eq!(t.to_text(), "a,,b,c\n1,,2,3\n");
    t.delete_col(1);
    t.move_col(0, 2);
    assert_eq!(t.to_text(), "b,c,a\n2,3,1\n");
    t.insert_row(1);
    assert_eq!(t.to_text(), "b,c,a\n,,\n2,3,1\n");
    t.move_row(1, 2);
    t.delete_row(2);
    assert_eq!(t.to_text(), "b,c,a\n2,3,1\n");
}

#[test]
fn sort_is_numeric_keeps_the_header_and_puts_empty_last() {
    let mut t = Table::parse("n,s\n10,b\n9,a\n,z\nx,c\n", ',');
    t.sort_by(0, false, true);
    assert_eq!(t.to_text(), "n,s\n9,a\n10,b\nx,c\n,z\n");
    t.sort_by(0, true, true);
    assert_eq!(t.to_text(), "n,s\nx,c\n10,b\n9,a\n,z\n");
}

#[test]
fn offsets_and_cells_agree() {
    let t = Table::parse("ab,c\nd,ef\n", ',');
    assert_eq!(t.char_offset(0, 1), 3);
    assert_eq!(t.char_offset(1, 1), 7);
    assert_eq!(t.cell_at(3), (0, 1));
    assert_eq!(t.cell_at(6), (1, 0));
    assert_eq!(t.cell_at(8), (1, 1));
}
