//! Tests for choosing the clipboard backend.

use super::*;

/// An environment with nothing set.
fn bare() -> Env {
    Env { macos: false, wayland_display: None, x11: false, default_wayland_socket: false }
}

/// macOS wins over everything else.
#[test]
fn macos_is_chosen_first() {
    let env = Env { macos: true, wayland_display: Some("w".into()), x11: true, ..bare() };
    assert_eq!(choose(&env), Kind::MacOs);
}

/// A set `WAYLAND_DISPLAY` is used as is, ahead of X11.
#[test]
fn wayland_display_is_used() {
    let env = Env { wayland_display: Some("wayland-1".into()), x11: true, ..bare() };
    assert_eq!(choose(&env), Kind::Wayland("wayland-1".into()));
}

/// A service on WSLg has no `WAYLAND_DISPLAY` but a `wayland-0` socket: it still gets Wayland.
#[test]
fn default_socket_stands_in_for_the_variable() {
    let env = Env { default_wayland_socket: true, ..bare() };
    assert_eq!(choose(&env), Kind::Wayland("wayland-0".into()));
}

/// Only `DISPLAY` means X11; nothing at all falls back to the terminal.
#[test]
fn x11_then_terminal() {
    assert_eq!(choose(&Env { x11: true, ..bare() }), Kind::X11);
    assert_eq!(choose(&bare()), Kind::Terminal);
}
