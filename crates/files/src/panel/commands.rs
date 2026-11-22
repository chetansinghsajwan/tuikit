//! The actions of every domain of the file panel, in the order the help lists them.

use ui::keys::Action;

/// The actions of movement and folding.
pub(super) const NAV: &[Action] = &[
    Action { id: "nav.up", name: "up", desc: "move the cursor up", basic: true },
    Action { id: "nav.down", name: "down", desc: "move the cursor down", basic: true },
    Action {
        id: "nav.up_step",
        name: "up a step",
        desc: "move the cursor up several rows",
        basic: true,
    },
    Action {
        id: "nav.down_step",
        name: "down a step",
        desc: "move the cursor down several rows",
        basic: true,
    },
    Action { id: "nav.page_up", name: "page up", desc: "move a page up", basic: true },
    Action { id: "nav.page_down", name: "page down", desc: "move a page down", basic: true },
    Action { id: "nav.top", name: "top", desc: "go to the first row", basic: true },
    Action { id: "nav.bottom", name: "bottom", desc: "go to the last row", basic: true },
    Action {
        id: "tree.collapse",
        name: "collapse",
        desc: "collapse the folder, or go to the parent folder's row",
        basic: true,
    },
];

/// The actions of actions on the row under the cursor or the marked rows.
pub(super) const ITEM: &[Action] = &[
    Action {
        id: "item.open",
        name: "open",
        desc: "open the file, or expand the folder",
        basic: true,
    },
    Action {
        id: "item.mark",
        name: "mark",
        desc: "mark or unmark the row for a multi-row action",
        basic: true,
    },
    Action {
        id: "item.create",
        name: "new",
        desc: "create a file (a trailing / makes a folder)",
        basic: false,
    },
    Action { id: "item.rename", name: "rename", desc: "rename the row", basic: false },
    Action {
        id: "item.delete",
        name: "delete",
        desc: "move the marked rows to the trash (asks first)",
        basic: false,
    },
    Action {
        id: "item.delete_forever",
        name: "delete forever",
        desc: "delete the marked rows permanently (asks first)",
        basic: false,
    },
    Action { id: "item.copy", name: "copy", desc: "copy the marked rows", basic: false },
    Action { id: "item.cut", name: "cut", desc: "cut the marked rows", basic: false },
    Action {
        id: "item.paste",
        name: "paste",
        desc: "paste into the folder under the cursor",
        basic: false,
    },
    Action {
        id: "item.open_with",
        name: "open with",
        desc: "open the row with a command you type",
        basic: false,
    },
    Action {
        id: "item.bulk_rename",
        name: "bulk rename",
        desc: "rename the marked rows together in your editor",
        basic: false,
    },
];

/// The actions of search.
pub(super) const SEARCH: &[Action] = &[
    Action {
        id: "search.here",
        name: "find here",
        desc: "find names in the folder under the cursor only, not in its subfolders (the picker)",
        basic: false,
    },
    Action {
        id: "search.files",
        name: "find everywhere",
        desc: "find files by name in the root and every subfolder (the picker)",
        basic: false,
    },
    Action {
        id: "search.grep",
        name: "grep",
        desc: "search file contents (the picker)",
        basic: false,
    },
];

/// The actions of what the list shows.
pub(super) const VIEW: &[Action] = &[
    Action {
        id: "view.hidden",
        name: "hidden files",
        desc: "show or hide dot files",
        basic: false,
    },
    Action {
        id: "view.ignored",
        name: "ignored files",
        desc: "show or hide the entries .gitignore and the other ignore files hide",
        basic: false,
    },
    Action {
        id: "view.columns",
        name: "columns",
        desc: "cycle the columns: permissions, size and time / size and time / permissions and time / off",
        basic: false,
    },
    Action {
        id: "view.refresh",
        name: "refresh",
        desc: "reread the folders and the git status",
        basic: false,
    },
    Action {
        id: "view.tables_as_text",
        name: "tables as text",
        desc: "show CSV, TSV and spreadsheets in the preview as text instead of tables (and back)",
        basic: false,
    },
];

/// The actions of which folder is the root.
pub(super) const ROOT: &[Action] = &[
    Action {
        id: "root.into",
        name: "cd into",
        desc: "make the folder under the cursor the root",
        basic: false,
    },
    Action {
        id: "root.focus",
        name: "focus folder",
        desc: "make the folder the cursor row sits in the root, so only its own entries show",
        basic: false,
    },
    Action { id: "root.up", name: "cd up", desc: "make the parent folder the root", basic: false },
];

/// The actions of bookmarks.
pub(super) const BOOKMARK: &[Action] = &[
    Action {
        id: "bookmark.set",
        name: "save bookmark",
        desc: "save the folder under the cursor under a key you type",
        basic: false,
    },
    Action {
        id: "bookmark.jump",
        name: "go to bookmark",
        desc: "make a bookmarked folder the root",
        basic: false,
    },
    Action {
        id: "bookmark.delete",
        name: "delete bookmark",
        desc: "delete a bookmark",
        basic: false,
    },
];

/// The actions that belong to no domain: the palette, the help, back and quit.
pub(super) const GENERAL: &[Action] = &[
    Action {
        id: "palette",
        name: "commands",
        desc: "search every action by name and run it",
        basic: true,
    },
    Action { id: "help", name: "keys", desc: "list every action with its keys", basic: false },
    Action { id: "back", name: "back", desc: "clear the marks, else close", basic: true },
    Action { id: "quit", name: "quit", desc: "close the explorer", basic: true },
];
