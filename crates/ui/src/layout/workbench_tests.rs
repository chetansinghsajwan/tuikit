use super::*;

#[test]
fn the_ribbon_is_the_whole_bottom_row_and_the_docks_stop_above_it() {
    let main = Rect::new(0, 0, 120, 40);
    let w = workbench(main, 30, 25, 0);
    assert_eq!(w.ribbon, Rect::new(0, 39, 120, 1));
    assert_eq!(w.left, Rect::new(0, 0, 30, 39));
    assert_eq!(w.right, Rect::new(95, 0, 25, 39));
    assert_eq!(w.editor, Rect::new(30, 0, 65, 39));
    assert_eq!(w.bottom.height, 0);
}

#[test]
fn a_bottom_dock_sits_under_the_editor_and_the_ribbon_stays_below_it() {
    let w = workbench(Rect::new(0, 0, 120, 40), 30, 25, 12);
    assert_eq!(w.bottom, Rect::new(30, 27, 65, 12));
    assert_eq!(w.editor, Rect::new(30, 0, 65, 27));
    assert_eq!((w.left.height, w.right.height), (39, 39));
    assert_eq!(w.ribbon, Rect::new(0, 39, 120, 1), "never moves");
    assert_eq!(w.bottom.bottom(), w.ribbon.y, "the dock ends right above the ribbon");
}

#[test]
fn closed_docks_take_no_room_and_big_ones_are_limited() {
    let main = Rect::new(2, 1, 90, 30);
    let w = workbench(main, 0, 0, 0);
    assert_eq!(w.editor, Rect::new(2, 1, 90, 29));
    assert_eq!(w.ribbon, Rect::new(2, 30, 90, 1));
    assert_eq!((w.left.width, w.right.width, w.left.height, w.right.height), (0, 0, 0, 0));
    // At most two fifths of the width per side (but the editor keeps 20 columns) and half of the height.
    let w = workbench(main, 80, 80, 100);
    assert_eq!((w.left.width, w.right.width), (35, 35));
    assert_eq!(w.bottom.height, 14);
    assert!(w.editor.width >= 20);
    // The pieces never overlap or leave the area.
    for r in [w.left, w.right, w.bottom, w.editor, w.ribbon] {
        assert!(r.x >= main.x && r.right() <= main.right() && r.y >= main.y && r.bottom() <= main.bottom(), "{r:?}");
    }
    assert!(w.left.right() <= w.editor.x && w.editor.right() <= w.right.x && w.editor.bottom() <= w.bottom.y && w.bottom.bottom() <= w.ribbon.y);
}

#[test]
fn tiny_screens_keep_a_usable_editor_and_drop_the_ribbon_last() {
    let w = workbench(Rect::new(0, 0, 40, 10), 30, 30, 8);
    assert!(w.editor.width >= 20 && w.editor.height >= 4, "{w:?}");
    assert_eq!(w.ribbon.height, 1);
    // With almost no rows left there is no ribbon, and nothing overlaps.
    let w = workbench(Rect::new(0, 0, 40, 4), 10, 10, 10);
    assert_eq!(w.ribbon.height, 0);
    assert_eq!(w.editor.height, 4);
}
