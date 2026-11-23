//! Fetch, pull and push.

use super::*;

impl App {
    /// The remote actions.
    pub(super) fn remote_action(&mut self, id: &str) -> bool {
        match id {
            "remote.fetch" => self.remote("fetching", "fetched", |r| r.fetch()),
            "remote.pull" => self.remote("pulling", "pulled", |r| r.pull()),
            "remote.push" => self.remote("pushing", "pushed", |r| r.push(false)),
            "remote.force_push" => self.mode = Mode::Confirm(Confirm::ForcePush),
            _ => return false,
        }
        true
    }

    /// Run a confirmed push.
    pub(super) fn remote_confirmed(&mut self, c: Confirm) {
        match c {
            Confirm::ForcePush => self.remote("force-pushing", "force-pushed", |r| r.push(true)),
            _ => {}
        }
    }
}
