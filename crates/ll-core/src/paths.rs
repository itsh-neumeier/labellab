//! Where LabelLab keeps its files (ADR-036).
//!
//! Portable by default: next to the executable live the encrypted settings
//! file [`SETTINGS_FILE`] and the data folder [`DATA_FOLDER`] (imported
//! icon sets and frames, own frames, print history, pasted images). If the
//! executable's folder is not writable (e.g. installed under
//! `C:\Program Files`), the per-user folder is used instead
//! (`%APPDATA%\LabelLab`, `~/.local/share/labellab`, …).
//!
//! Overrides: `LABELLAB_DATA_DIR` (data folder), `LABELLAB_SETTINGS`
//! (settings file).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::CoreError;

/// Data folder next to the executable.
pub const DATA_FOLDER: &str = "llappdata";
/// Settings file next to the executable.
pub const SETTINGS_FILE: &str = "LabelLab.settings";

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The per-user data folder (LabelLab before portable storage).
pub fn user_dir() -> Result<PathBuf, CoreError> {
    let dir = if cfg!(windows) {
        env_path("APPDATA").map(|d| d.join("LabelLab"))
    } else if cfg!(target_os = "macos") {
        env_path("HOME").map(|h| h.join("Library/Application Support/LabelLab"))
    } else {
        env_path("XDG_DATA_HOME")
            .or_else(|| env_path("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("labellab"))
    };
    dir.ok_or_else(|| CoreError::IconSet("no user data directory (HOME/APPDATA not set)".into()))
}

/// Whether files can be created in `dir`.
fn writable(dir: &Path) -> bool {
    let probe = dir.join(".labellab-write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// The executable's folder if it is writable (portable mode), checked once.
pub fn portable_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.to_path_buf();
        writable(&dir).then_some(dir)
    })
    .as_deref()
}

/// Copies `from` into `to` (files and folders), skipping what exists.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if !target.exists() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// LabelLab's data folder (not necessarily created yet). In portable mode
/// data from the per-user folder is copied over once, so nothing imported
/// earlier is lost.
pub fn data_dir() -> Result<PathBuf, CoreError> {
    if let Some(dir) = env_path("LABELLAB_DATA_DIR") {
        return Ok(dir);
    }
    let Some(base) = portable_dir() else {
        return user_dir();
    };
    let dir = base.join(DATA_FOLDER);
    static MIGRATED: OnceLock<()> = OnceLock::new();
    MIGRATED.get_or_init(|| {
        if !dir.exists() {
            if let Ok(old) = user_dir() {
                if old.is_dir() {
                    let _ = copy_tree(&old, &dir);
                }
            }
        }
    });
    Ok(dir)
}

/// The settings file: next to the executable, else in the per-user folder.
pub fn settings_file() -> Result<PathBuf, CoreError> {
    if let Some(file) = env_path("LABELLAB_SETTINGS") {
        return Ok(file);
    }
    match portable_dir() {
        Some(base) => Ok(base.join(SETTINGS_FILE)),
        None => Ok(user_dir()?.join(SETTINGS_FILE)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_tree_keeps_existing_files() {
        let root = std::env::temp_dir().join(format!("ll-paths-{}", std::process::id()));
        let (from, to) = (root.join("old"), root.join("new"));
        std::fs::create_dir_all(from.join("frames")).unwrap();
        std::fs::write(from.join("frames/a.llabel-frames"), "old").unwrap();
        std::fs::write(from.join("b.txt"), "old").unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(to.join("b.txt"), "new").unwrap();
        copy_tree(&from, &to).unwrap();
        assert_eq!(
            std::fs::read_to_string(to.join("frames/a.llabel-frames")).unwrap(),
            "old"
        );
        assert_eq!(std::fs::read_to_string(to.join("b.txt")).unwrap(), "new");
        assert!(writable(&to));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
