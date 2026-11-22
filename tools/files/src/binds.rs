//! The default keys of the standalone explorer's own actions.

use ui::keys::DefaultBinding;

/// The default keys of [`ACTIONS`].
pub(crate) const DEFAULT_BINDINGS: &[DefaultBinding] =
    &[DefaultBinding { id: "view.properties", keys: &["leader v i"] }];
