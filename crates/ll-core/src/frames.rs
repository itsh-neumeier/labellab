//! Installed decorative frame sets (`.llabel-frames`, see
//! [`ll_render::decor`]): imported sets and the user's own frames live in
//! `<data dir>/frames/` and are registered at startup ([`load_installed`]).
//! Frames made in the editor go into the set [`USER_SET_ID`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ll_render::decor::{self, FrameDef, FrameSet, FRAMES_EXTENSION, FRAMES_FORMAT};
use ll_render::iconset::Text;

use crate::iconsets::data_dir;
use crate::CoreError;

/// Set id of frames made in the frame editor.
pub const USER_SET_ID: &str = "eigene";

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::IconSet(format!("frames: {e}"))
}

/// Where frame sets are stored.
pub fn frames_dir() -> Result<PathBuf, CoreError> {
    Ok(data_dir()?.join("frames"))
}

fn set_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{FRAMES_EXTENSION}"))
}

/// Registers every installed frame set; one message per file that failed.
pub fn load_installed() -> Vec<String> {
    let Ok(dir) = frames_dir() else {
        return Vec::new();
    };
    load_from(&dir)
}

fn load_from(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().and_then(|e| e.to_str()) != Some(FRAMES_EXTENSION) {
            continue;
        }
        let result = std::fs::read(&path)
            .map_err(err)
            .and_then(|data| FrameSet::from_json(&data).map_err(err))
            .and_then(|set| decor::register(set).map_err(err));
        if let Err(e) = result {
            warnings.push(format!("{}: {e}", path.display()));
        }
    }
    warnings
}

/// Imports a `.llabel-frames` file (copied into [`frames_dir`]).
pub fn import(path: &Path) -> Result<FrameSet, CoreError> {
    import_into(&frames_dir()?, path)
}

fn import_into(dir: &Path, path: &Path) -> Result<FrameSet, CoreError> {
    let data = std::fs::read(path)?;
    let set = FrameSet::from_json(&data).map_err(err)?;
    if decor::is_builtin(&set.id) {
        return Err(err(format!(
            "the id {:?} belongs to a built-in set",
            set.id
        )));
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(set_file(dir, &set.id), &data)?;
    decor::register(set.clone()).map_err(err)?;
    Ok(set)
}

/// Removes an installed set (file and registration).
pub fn remove(id: &str) -> Result<(), CoreError> {
    if decor::is_builtin(id) {
        return Err(err(format!("{id:?} is built in and can't be removed")));
    }
    let file = set_file(&frames_dir()?, id);
    if file.exists() {
        std::fs::remove_file(&file)?;
    }
    decor::unregister(id);
    Ok(())
}

fn user_set(dir: &Path) -> Result<FrameSet, CoreError> {
    match std::fs::read(set_file(dir, USER_SET_ID)) {
        Ok(data) => FrameSet::from_json(&data).map_err(err),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(FrameSet {
            format: FRAMES_FORMAT.into(),
            version: 1,
            id: USER_SET_ID.into(),
            name: Text::Localized(BTreeMap::from([
                ("de".into(), "Eigene Rahmen".into()),
                ("en".into(), "My frames".into()),
            ])),
            license: None,
            frames: Vec::new(),
        }),
        Err(e) => Err(e.into()),
    }
}

fn write_user_set(dir: &Path, set: FrameSet) -> Result<(), CoreError> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(&set).map_err(err)?;
    std::fs::write(set_file(dir, USER_SET_ID), json)?;
    decor::register(set).map_err(err)
}

/// Adds or replaces (same id) a frame in the user's own set.
pub fn save_user_frame(frame: FrameDef) -> Result<(), CoreError> {
    save_user_frame_in(&frames_dir()?, frame)
}

fn save_user_frame_in(dir: &Path, frame: FrameDef) -> Result<(), CoreError> {
    let mut set = user_set(dir)?;
    set.frames.retain(|f| f.id != frame.id);
    set.frames.push(frame);
    write_user_set(dir, set)
}

/// Deletes a frame from the user's own set.
pub fn delete_user_frame(id: &str) -> Result<(), CoreError> {
    let dir = frames_dir()?;
    let mut set = user_set(&dir)?;
    set.frames.retain(|f| f.id != id);
    write_user_set(&dir, set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(w: u32) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="100"><rect width="{w}" height="10"/></svg>"#
        )
    }

    #[test]
    fn user_frames_are_saved_registered_and_reloaded() {
        let dir = std::env::temp_dir().join(format!("ll-frames-{}", std::process::id()));
        let frame = FrameDef {
            id: "test".into(),
            name: Text::Plain("Test".into()),
            start: svg(10),
            middle: svg(20),
            end: svg(10),
        };
        save_user_frame_in(&dir, frame.clone()).unwrap();
        assert_eq!(decor::resolve("eigene:test"), Some(frame.clone()));
        // Replacing keeps one frame with that id.
        save_user_frame_in(&dir, frame).unwrap();
        assert_eq!(user_set(&dir).unwrap().frames.len(), 1);
        decor::unregister(USER_SET_ID);
        assert!(load_from(&dir).is_empty());
        assert!(decor::resolve("eigene:test").is_some());
        // Built-in ids can't be imported over.
        let builtin = dir.join("b.llabel-frames");
        std::fs::write(
            &builtin,
            br#"{"format":"labellab-frames","version":1,"id":"basis","name":"x","frames":[]}"#,
        )
        .unwrap();
        assert!(import_into(&dir.join("sub"), &builtin).is_err());
        decor::unregister(USER_SET_ID);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
