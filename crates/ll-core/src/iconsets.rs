//! Installed icon sets: `.llabel-iconset` files imported by the user are
//! kept in `<data dir>/iconsets/` and registered with
//! [`ll_render::iconset`] at startup ([`load_installed`]), so CLI and GUI
//! see the same sets. Also builds sets from a folder of SVG files
//! ([`from_dir`]).
//!
//! Data directory: `LABELLAB_DATA_DIR` if set, else `%APPDATA%\LabelLab`
//! (Windows), `~/Library/Application Support/LabelLab` (macOS) or
//! `$XDG_DATA_HOME/labellab` / `~/.local/share/labellab` (Linux).

use std::path::{Path, PathBuf};

use ll_render::iconset::{self, Category, Icon, IconSet, Text};
use ll_render::ICONSET_EXTENSION;

use crate::CoreError;

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::IconSet(e.to_string())
}

/// LabelLab's per-user data directory (not created).
pub fn data_dir() -> Result<PathBuf, CoreError> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = env("LABELLAB_DATA_DIR") {
        return Ok(dir);
    }
    let dir = if cfg!(windows) {
        env("APPDATA").map(|d| d.join("LabelLab"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/LabelLab"))
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("labellab"))
    };
    dir.ok_or_else(|| err("no user data directory (HOME/APPDATA not set)"))
}

/// Where imported icon sets are stored.
pub fn iconset_dir() -> Result<PathBuf, CoreError> {
    Ok(data_dir()?.join("iconsets"))
}

/// Registers every installed icon set. Returns one message per file that
/// could not be loaded (the others still load).
pub fn load_installed() -> Vec<String> {
    let Ok(dir) = iconset_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new(); // nothing imported yet
    };
    let mut warnings = Vec::new();
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().and_then(|e| e.to_str()) != Some(ICONSET_EXTENSION) {
            continue;
        }
        let result = std::fs::read(&path)
            .map_err(err)
            .and_then(|data| IconSet::from_json(&data).map_err(err))
            .and_then(|set| iconset::register(set).map_err(err));
        if let Err(e) = result {
            warnings.push(format!("{}: {e}", path.display()));
        }
    }
    warnings
}

/// Imports an icon set file: validates it, copies it into
/// [`iconset_dir`] as `<id>.llabel-iconset` (replacing an older import
/// with the same id) and registers it. Returns the set.
pub fn import(path: &Path) -> Result<IconSet, CoreError> {
    let data = std::fs::read(path)?;
    let set = IconSet::from_json(&data).map_err(err)?;
    if iconset::is_builtin(&set.id) {
        return Err(err(format!(
            "the id {:?} belongs to a built-in set; give the set another id",
            set.id
        )));
    }
    let dir = iconset_dir()?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{}.{ICONSET_EXTENSION}", set.id)), &data)?;
    iconset::register(set.clone()).map_err(err)?;
    Ok(set)
}

/// Removes an imported icon set (file and registration).
pub fn remove(id: &str) -> Result<(), CoreError> {
    if iconset::is_builtin(id) {
        return Err(err(format!("{id:?} is built in and can't be removed")));
    }
    let file = iconset_dir()?.join(format!("{id}.{ICONSET_EXTENSION}"));
    let existed = file.exists();
    if existed {
        std::fs::remove_file(&file)?;
    }
    if !iconset::unregister(id) && !existed {
        return Err(err(format!("no imported icon set {id:?}")));
    }
    Ok(())
}

/// Turns a file name like `fire_exit-left` into a readable name.
fn title_from_stem(stem: &str) -> String {
    stem.replace(['_', '-'], " ")
}

/// Builds an icon set from a folder of `.svg` files. SVGs directly in
/// `dir` have no category; each subfolder becomes a category named after
/// it. Icon ids are the file names without `.svg`; SVGs are cleaned with
/// [`iconset::normalize_svg`].
pub fn from_dir(dir: &Path, id: &str, name: &str) -> Result<IconSet, CoreError> {
    let mut set = IconSet::new(id, name);
    let mut add = |path: &Path, category: Option<&str>| -> Result<(), CoreError> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| err(format!("bad file name {}", path.display())))?;
        let icon_id = stem.replace(':', "-");
        if set.icon(&icon_id).is_some() {
            return Err(err(format!("icon {icon_id:?} exists twice")));
        }
        let svg = iconset::normalize_svg(&std::fs::read(path)?)
            .map_err(|e| err(format!("{}: {e}", path.display())))?;
        set.icons.push(Icon {
            id: icon_id,
            name: Text::Plain(title_from_stem(stem)),
            category: category.map(str::to_owned),
            tags: Vec::new(),
            svg,
            license: None,
            author: None,
            source: None,
        });
        Ok(())
    };
    let is_svg = |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    };
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    let mut categories = Vec::new();
    for path in &entries {
        if path.is_dir() {
            let Some(cat) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let mut files: Vec<PathBuf> = std::fs::read_dir(path)?
                .flatten()
                .map(|e| e.path())
                .filter(|p| is_svg(p))
                .collect();
            files.sort();
            if files.is_empty() {
                continue;
            }
            let cat_id = cat.to_lowercase().replace(' ', "-");
            for file in &files {
                add(file, Some(&cat_id))?;
            }
            categories.push(Category {
                id: cat_id,
                name: Text::Plain(cat.to_owned()),
            });
        } else if is_svg(path) {
            add(path, None)?;
        }
    }
    set.categories = categories;
    if set.icons.is_empty() {
        return Err(err(format!("no .svg files in {}", dir.display())));
    }
    set.validate(false).map_err(err)?;
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="4"/></svg>"#;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("labellab-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn builds_set_from_folder_with_categories() {
        let dir = temp_dir("fromdir");
        std::fs::create_dir_all(dir.join("Netzwerk")).unwrap();
        std::fs::write(dir.join("loose.svg"), SVG).unwrap();
        std::fs::write(dir.join("Netzwerk/switch_port.svg"), SVG).unwrap();
        std::fs::write(dir.join("Netzwerk/readme.txt"), "x").unwrap();
        let set = from_dir(&dir, "my-set", "Meine Icons").unwrap();
        assert_eq!(set.icons.len(), 2);
        assert_eq!(set.categories[0].id, "netzwerk");
        let port = set.icon("switch_port").unwrap();
        assert_eq!(port.category.as_deref(), Some("netzwerk"));
        assert_eq!(port.name.get("de"), "switch port");
        // The result is a valid, loadable file.
        IconSet::from_json(set.to_json().unwrap().as_bytes()).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn imports_and_removes_sets() {
        // Only this test touches LABELLAB_DATA_DIR in this test binary.
        let data = temp_dir("data");
        std::env::set_var("LABELLAB_DATA_DIR", &data);

        let src = temp_dir("src");
        std::fs::write(src.join("a.svg"), SVG).unwrap();
        let set = from_dir(&src, "import-test", "Import").unwrap();
        let file = src.join("set.llabel-iconset");
        std::fs::write(&file, set.to_json().unwrap()).unwrap();

        import(&file).unwrap();
        assert!(data.join("iconsets/import-test.llabel-iconset").exists());
        assert!(iconset::resolve("import-test:a").is_some());

        iconset::unregister("import-test");
        assert!(load_installed().is_empty());
        assert!(
            iconset::resolve("import-test:a").is_some(),
            "reloaded from disk"
        );

        remove("import-test").unwrap();
        assert!(iconset::resolve("import-test:a").is_none());
        assert!(remove("material").is_err());

        let mut builtin = set.clone();
        builtin.id = "material".into();
        std::fs::write(&file, builtin.to_json().unwrap()).unwrap();
        assert!(import(&file).is_err());

        std::env::remove_var("LABELLAB_DATA_DIR");
        std::fs::remove_dir_all(&data).unwrap();
        std::fs::remove_dir_all(&src).unwrap();
    }
}
