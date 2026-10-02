//! Print history ("Erstellte Labels"): every printed label is kept as a
//! `.llabel` plus a small PNG preview in `<data dir>/history/`, listed in
//! `index.json`, newest first, at most [`MAX_ENTRIES`] entries.

use std::path::{Path, PathBuf};

use ll_protocol::model::ModelInfo;
use serde::{Deserialize, Serialize};

use crate::iconsets::data_dir;
use crate::label::{render_label_preview, Label};
use crate::CoreError;

/// How many printed labels are kept.
pub const MAX_ENTRIES: usize = 50;

/// One printed label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// File stem of the stored `.llabel`/`.png`.
    pub id: String,
    /// Local time of printing, `YYYY-MM-DD HH:MM:SS`.
    pub printed_at: String,
    /// Display name (file name or first text).
    pub name: String,
    pub model: String,
    pub width_mm: u8,
    /// Number of labels printed in the job.
    pub count: usize,
}

fn dir() -> Result<PathBuf, CoreError> {
    Ok(data_dir()?.join("history"))
}

/// The history, newest first (empty if nothing was printed yet).
pub fn list() -> Result<Vec<Entry>, CoreError> {
    list_in(&dir()?)
}

fn list_in(dir: &Path) -> Result<Vec<Entry>, CoreError> {
    match std::fs::read(dir.join("index.json")) {
        Ok(data) => serde_json::from_slice(&data)
            .map_err(|e| CoreError::Template(format!("history index: {e}"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Stores `label` (as designed, before placeholders are filled) with a
/// preview for `model`/`width_mm`, and trims the history.
pub fn record(
    label: &Label,
    model: &ModelInfo,
    width_mm: u8,
    name: &str,
    count: usize,
) -> Result<Entry, CoreError> {
    record_in(&dir()?, label, model, width_mm, name, count)
}

fn record_in(
    dir: &Path,
    label: &Label,
    model: &ModelInfo,
    width_mm: u8,
    name: &str,
    count: usize,
) -> Result<Entry, CoreError> {
    std::fs::create_dir_all(dir)?;
    let now = chrono::Local::now();
    let mut id = now.format("%Y%m%d-%H%M%S").to_string();
    let mut n = 1;
    while dir.join(format!("{id}.llabel")).exists() {
        n += 1;
        id = format!("{}-{n}", now.format("%Y%m%d-%H%M%S"));
    }
    label.save(&dir.join(format!("{id}.llabel")))?;
    // The preview is a convenience: a label that can't be rendered (e.g.
    // a missing image) is still kept.
    if let Ok(preview) = render_label_preview(label, model, width_mm, 1) {
        std::fs::write(dir.join(format!("{id}.png")), preview.png)?;
    }
    let entry = Entry {
        id,
        printed_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        name: name.to_owned(),
        model: model.name.to_owned(),
        width_mm,
        count,
    };
    let mut entries = list_in(dir).unwrap_or_default();
    entries.insert(0, entry.clone());
    for old in entries.split_off(MAX_ENTRIES.min(entries.len())) {
        for ext in ["llabel", "png"] {
            let _ = std::fs::remove_file(dir.join(format!("{}.{ext}", old.id)));
        }
    }
    let json = serde_json::to_vec_pretty(&entries)
        .map_err(|e| CoreError::Template(format!("history index: {e}")))?;
    std::fs::write(dir.join("index.json"), json)?;
    Ok(entry)
}

/// The stored label of entry `id`.
pub fn load(id: &str) -> Result<Label, CoreError> {
    load_in(&dir()?, id)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\', '.'])
}

fn load_in(dir: &Path, id: &str) -> Result<Label, CoreError> {
    if !valid_id(id) {
        return Err(CoreError::Template(format!("invalid history id {id:?}")));
    }
    Label::load(&dir.join(format!("{id}.llabel")))
}

/// The PNG preview of entry `id`, if there is one.
pub fn preview(id: &str) -> Option<Vec<u8>> {
    preview_in(&dir().ok()?, id)
}

fn preview_in(dir: &Path, id: &str) -> Option<Vec<u8>> {
    if !valid_id(id) {
        return None;
    }
    std::fs::read(dir.join(format!("{id}.png"))).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::Element;

    #[test]
    fn records_lists_loads_and_trims() {
        let dir = std::env::temp_dir().join(format!("labellab-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = ll_protocol::model::find_by_name("PT-P710BT").unwrap();
        let label = Label::single(Element::Fill);
        for i in 0..MAX_ENTRIES + 2 {
            record_in(&dir, &label, model, 12, &format!("L{i}"), 1).unwrap();
        }
        let entries = list_in(&dir).unwrap();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].name, format!("L{}", MAX_ENTRIES + 1));
        assert_eq!(load_in(&dir, &entries[0].id).unwrap(), label);
        assert!(preview_in(&dir, &entries[0].id).is_some());
        assert!(load_in(&dir, "../x").is_err());
        let files = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(files, 2 * MAX_ENTRIES + 1, "trimmed files + index");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
