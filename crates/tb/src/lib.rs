//! The table editor: a grid of cells for csv, tsv and psv files. Move by cell, edit a cell, add, delete
//! and move rows and columns, sort, find, undo, save. The text model is `table::Table`, which helix uses too.

pub mod grid;
pub mod keys;
pub mod table;
