//! The table editor: a grid of cells for csv, tsv and psv files. Move by cell, edit a cell, add, delete
//! and move rows and columns, sort, find, undo, save. The text model is `table::Table`, which helix uses too.

/// The grid view and its editing state (cursor, edits, sort, find, undo).
pub mod grid;
/// The editor's actions and default keys.
pub mod keys;
/// The text model of a csv, tsv or psv file, shared with helix.
pub mod table;
