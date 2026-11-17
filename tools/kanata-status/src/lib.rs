//! `kanata-status`: draws the keyboard from a kanata config and follows the live state
//! served by `kb daemon` (layer, held keys).

/// The app state and drawing: what the screen shows for the daemon state and key presses.
mod app;
/// Reads the keyboard layout and layers out of a kanata config.
pub mod kbd;
/// Follows `kb daemon` over TCP and turns its lines into events.
pub mod link;

pub use app::App;
