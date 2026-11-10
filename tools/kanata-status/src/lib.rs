//! `kanata-status`: draws the keyboard from a kanata config and follows the live state
//! served by `kb daemon` (layer, held keys).

mod app;
pub mod kbd;
pub mod link;

pub use app::App;
