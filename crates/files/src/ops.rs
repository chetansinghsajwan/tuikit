//! File operations behind the panel's keys. Pure functions on paths, no UI:
//! every one either does the whole job or returns an error and leaves things as
//! they were (as far as the filesystem allows).

use std::io;
use std::path::{Component, Path, PathBuf};

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.to_string())
}

/// A relative path that stays inside its base: no `..`, no root, not empty.
fn check_relative(name: &str) -> io::Result<&Path> {
    let p = Path::new(name.trim_end_matches('/'));
    if p.as_os_str().is_empty() {
        return Err(invalid("empty name"));
    }
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(invalid("name must be relative and stay inside the folder"));
    }
    Ok(p)
}

/// Create `name` inside `dir`: a file, or a folder when `name` ends with `/`.
/// Missing parent folders are created. Returns the new path and whether it is a folder.
pub fn create(dir: &Path, name: &str) -> io::Result<(PathBuf, bool)> {
    let rela = check_relative(name)?;
    let path = dir.join(rela);
    if path.symlink_metadata().is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", rela.display()),
        ));
    }
    let is_dir = name.ends_with('/');
    if is_dir {
        std::fs::create_dir_all(&path)?;
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    }
    Ok((path, is_dir))
}

/// Rename `from` to `name` inside `from`'s folder (`name` may contain
/// sub-folders, which are created). Never overwrites.
pub fn rename(from: &Path, name: &str) -> io::Result<PathBuf> {
    let rela = check_relative(name)?;
    let to = from.parent().ok_or_else(|| invalid("no parent folder"))?.join(rela);
    if to == from {
        return Ok(to);
    }
    if to.symlink_metadata().is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", rela.display()),
        ));
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(from, &to)?;
    Ok(to)
}

/// `dest`, or the first free `name copy`, `name copy 2`, ... next to it
/// (the extension stays last: `a copy.txt`).
pub fn unique(dest: PathBuf) -> PathBuf {
    unique_among(dest, &[])
}

/// `unique`, also avoiding the paths in `taken` (destinations already planned).
pub fn unique_among(dest: PathBuf, taken: &[PathBuf]) -> PathBuf {
    let free = |p: &Path| p.symlink_metadata().is_err() && !taken.iter().any(|t| t == p);
    if free(&dest) {
        return dest;
    }
    let stem = dest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = dest.extension().map(|e| format!(".{}", e.to_string_lossy()));
    for n in 1.. {
        let suffix = if n == 1 { " copy".to_string() } else { format!(" copy {n}") };
        let candidate =
            dest.with_file_name(format!("{stem}{suffix}{}", ext.as_deref().unwrap_or("")));
        if free(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// Copy a file, symlink or folder (recursively) to `dest`, which must not exist.
pub fn copy(src: &Path, dest: &Path) -> io::Result<()> {
    copy_progress(src, dest, &mut |_| true)
}

/// Like `copy`, reporting the bytes copied so far through `tick` (called per chunk
/// of a file); `tick` returns `false` to cancel, which ends in an `Interrupted` error.
pub fn copy_progress(src: &Path, dest: &Path, tick: &mut dyn FnMut(u64) -> bool) -> io::Result<()> {
    let meta = src.symlink_metadata()?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(src)?;
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, dest);
        #[cfg(not(unix))]
        return std::fs::copy(src, dest).map(|_| ());
    }
    if meta.is_dir() {
        if dest.starts_with(src) {
            return Err(invalid("cannot copy a folder into itself"));
        }
        std::fs::create_dir(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_progress(&entry.path(), &dest.join(entry.file_name()), tick)?;
        }
        return Ok(());
    }
    // A file, in chunks, so a big one reports as it goes and can be cancelled.
    use std::io::{Read, Write};
    let mut from = std::fs::File::open(src)?;
    let mut to = std::fs::File::create(dest)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            break;
        }
        to.write_all(&buf[..n])?;
        if !tick(n as u64) {
            drop(to);
            let _ = std::fs::remove_file(dest);
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
    }
    std::fs::set_permissions(dest, meta.permissions())
}

/// The bytes under these paths (files only), for a progress total.
pub fn total_size(paths: &[PathBuf]) -> u64 {
    let mut total = 0;
    let mut stack: Vec<PathBuf> = paths.to_vec();
    while let Some(p) = stack.pop() {
        let Ok(m) = p.symlink_metadata() else { continue };
        if m.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&p) {
                stack.extend(entries.flatten().map(|e| e.path()));
            }
        } else if m.is_file() {
            total += m.len();
        }
    }
    total
}

/// Move like `move_to`, reporting copied bytes through `tick` when it has to copy
/// (a rename inside one file system is instant and reports nothing).
pub fn move_progress(src: &Path, dest: &Path, tick: &mut dyn FnMut(u64) -> bool) -> io::Result<()> {
    if src.is_dir() && dest.starts_with(src) {
        return Err(invalid("cannot move a folder into itself"));
    }
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_progress(src, dest, tick)?;
            remove(src)
        }
    }
}

/// Delete permanently.
pub fn remove(path: &Path) -> io::Result<()> {
    if path.symlink_metadata()?.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// Move `src` to `dest` (which must not exist); across file systems this copies, then removes.
pub fn move_to(src: &Path, dest: &Path) -> io::Result<()> {
    if src.is_dir() && dest.starts_with(src) {
        return Err(invalid("cannot move a folder into itself"));
    }
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy(src, dest)?;
            remove(src)
        }
    }
}

/// Move to the trash (recoverable).
pub fn trash(paths: &[PathBuf]) -> Result<(), String> {
    trash::delete_all(paths).map_err(|e| e.to_string())
}

/// Names to show in the editor for a bulk rename: each path relative to `base`.
pub fn bulk_rename_text(base: &Path, paths: &[PathBuf]) -> String {
    paths.iter().map(|p| format!("{}\n", p.strip_prefix(base).unwrap_or(p).display())).collect()
}

/// Read back an edited bulk-rename text (one new path per line, relative to `base`,
/// same order as `olds`) and return the `(from, to)` pairs that change. Fails, and
/// changes nothing, on a wrong line count, a bad or duplicate name, or a name that
/// exists and is not itself being renamed away.
pub fn plan_bulk_rename(
    base: &Path,
    olds: &[PathBuf],
    edited: &str,
) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let lines: Vec<&str> = edited.lines().collect();
    if lines.len() != olds.len() {
        return Err(format!("expected {} names, got {}", olds.len(), lines.len()));
    }
    let mut moves = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (old, line) in olds.iter().zip(&lines) {
        let rela = check_relative(line).map_err(|e| format!("{line:?}: {e}"))?;
        let new = base.join(rela);
        if !seen.insert(new.clone()) {
            return Err(format!("{line:?} appears twice"));
        }
        if new != *old {
            moves.push((old.clone(), new));
        }
    }
    for (_, to) in &moves {
        if to.symlink_metadata().is_ok() && !olds.contains(to) {
            return Err(format!(
                "{} already exists",
                to.strip_prefix(base).unwrap_or(to).display()
            ));
        }
    }
    Ok(moves)
}

/// Carry out moves from `plan_bulk_rename`. Everything first moves to a temporary
/// name beside its source, so swaps and chains (`a→b`, `b→a`) work.
pub fn apply_moves(moves: &[(PathBuf, PathBuf)]) -> io::Result<()> {
    let mut staged = Vec::new();
    for (i, (from, to)) in moves.iter().enumerate() {
        let tmp = from.with_file_name(format!(".files-bulk-{}-{i}", std::process::id()));
        std::fs::rename(from, &tmp)?;
        staged.push((tmp, to));
    }
    for (tmp, to) in staged {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&tmp, to)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
