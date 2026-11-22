//! The standalone explorer's own actions.

use ui::keys::Action;

/// The standalone app's own actions, registered next to the panel's. The tabs come from the shell.
pub(crate) const ACTIONS: &[Action] = &[Action {
    id: "view.properties",
    name: "properties",
    desc: "show or hide the properties pane",
    basic: false,
}];
