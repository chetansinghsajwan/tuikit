//! The default keys of the shell's own actions.

use crate::keys::DefaultBinding;

/// The default keys of [`NAV`].
pub(super) const NAV: &[DefaultBinding] = &[
    DefaultBinding { id: "nav.up", keys: &["k", "up", "ctrl-p"] },
    DefaultBinding { id: "nav.down", keys: &["j", "down", "ctrl-n"] },
    DefaultBinding { id: "nav.up_step", keys: &["ctrl-k"] },
    DefaultBinding { id: "nav.down_step", keys: &["ctrl-j"] },
    DefaultBinding { id: "nav.page_up", keys: &["pageup"] },
    DefaultBinding { id: "nav.page_down", keys: &["pagedown"] },
    DefaultBinding { id: "nav.top", keys: &["g", "home"] },
    DefaultBinding { id: "nav.bottom", keys: &["G", "end"] },
];

/// The default keys of [`TABS`].
pub(super) const TABS: &[DefaultBinding] = &[
    DefaultBinding { id: "tab.next", keys: &["]", "leader t l"] },
    DefaultBinding { id: "tab.prev", keys: &["[", "leader t h"] },
];

/// The default keys of [`DYNAMIC_TABS`].
pub(super) const DYNAMIC_TABS: &[DefaultBinding] = &[
    DefaultBinding { id: "tab.new", keys: &["leader t n"] },
    DefaultBinding { id: "tab.reopen", keys: &["leader t r"] },
    DefaultBinding { id: "tab.duplicate", keys: &["leader t d"] },
    DefaultBinding { id: "tab.move_left", keys: &["leader t ["] },
    DefaultBinding { id: "tab.move_right", keys: &["leader t ]"] },
    DefaultBinding { id: "tab.close", keys: &["leader t c"] },
];

/// The default keys of [`SHELL`].
pub(super) const SHELL: &[DefaultBinding] = &[
    DefaultBinding { id: "palette", keys: &["leader space"] },
    DefaultBinding { id: "help", keys: &["?"] },
];
