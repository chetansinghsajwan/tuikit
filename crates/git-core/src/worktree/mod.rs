//! Working-tree changes and the operations that change them (stage, unstage,
//! discard, commit). Reading goes through `gix`; writing runs the `git` command
//! line, as lazygit does, so hooks, signing and every edge case behave exactly as
//! the user's git does.

use crate::history::{FilePatch, RefKind, hunks};
use crate::lines::head_blob;
use crate::{Error, Kind, Repo, err};
use gix::bstr::ByteSlice;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How far `Repo::reset` goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    /// Keep the index and the work tree.
    Soft,
    /// Keep the work tree, reset the index.
    Mixed,
    /// Reset both: changes are lost.
    Hard,
}

/// An operation that stopped and waits for you (usually on conflicts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

/// Which comparison a file diff shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// `HEAD` against the index: what the next commit contains.
    Staged,
    /// The index against the work tree: what is not staged yet (an untracked
    /// file counts as all added).
    Unstaged,
}

mod branches;
mod commit;
mod diff;
mod head;
mod index;
mod operation;
mod owner;
mod remote;
mod replay;
mod run;
mod stash;
