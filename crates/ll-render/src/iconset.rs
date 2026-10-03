//! Icon sets: named, categorized collections of SVG icons in one
//! `.llabel-iconset` file (JSON). LabelLab ships some sets built in (see
//! [`builtin_sets`]); more can be registered at runtime (`ll-core` loads
//! imported sets from the user's data directory).
//!
//! Icons are addressed as `set:icon` (e.g. `iso7010:W012`). A bare
//! `icon` name is looked up in all registered sets, built-in sets first,
//! which keeps the names of the original symbol library (`warning`, …)
//! working.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::RenderError;

/// File extension of icon set files (without dot).
pub const ICONSET_EXTENSION: &str = "llabel-iconset";
/// Value of the `format` field that marks a file as an icon set.
pub const ICONSET_FORMAT: &str = "llabel-iconset";
/// Newest icon set format version this build reads and writes.
pub const ICONSET_VERSION: u32 = 1;

/// Text that is either the same in every language or translated:
/// `"Warnzeichen"` or `{"de": "Warnzeichen", "en": "Warning signs"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Localized(BTreeMap<String, String>),
}

impl Text {
    /// The text in `lang`, falling back to German, English, then any.
    pub fn get(&self, lang: &str) -> &str {
        match self {
            Text::Plain(s) => s,
            Text::Localized(map) => map
                .get(lang)
                .or_else(|| map.get("de"))
                .or_else(|| map.get("en"))
                .or_else(|| map.values().next())
                .map(String::as_str)
                .unwrap_or(""),
        }
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::Plain(s.to_owned())
    }
}

/// How an icon's grays become print dots.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Halftone {
    /// Hard threshold: light colors (e.g. safety yellow) become white,
    /// dark and saturated colors (red, blue, green, black) become ink.
    /// Clean edges for signs and pictograms.
    #[default]
    Threshold,
    /// Floyd-Steinberg error diffusion, for photo-like artwork.
    Dither,
}

/// A group of icons inside a set, e.g. "Warnzeichen".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: Text,
}

/// One icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Icon {
    /// Unique within the set, no `:`.
    pub id: String,
    pub name: Text,
    /// Id of one of the set's categories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Extra search words.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The SVG document.
    pub svg: String,
    /// Per-icon license/author/source, if they differ from the set's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// A `.llabel-iconset` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconSet {
    /// Always [`ICONSET_FORMAT`].
    pub format: String,
    pub version: u32,
    /// Short id used in symbol names (`id:icon`): lowercase letters,
    /// digits, `-` and `_`.
    pub id: String,
    pub name: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub halftone: Halftone,
    #[serde(default)]
    pub categories: Vec<Category>,
    pub icons: Vec<Icon>,
}

fn invalid(msg: impl Into<String>) -> RenderError {
    RenderError::Image(format!("invalid icon set: {}", msg.into()))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

impl IconSet {
    /// An empty set with `id` and `name`.
    pub fn new(id: &str, name: impl Into<Text>) -> Self {
        Self {
            format: ICONSET_FORMAT.to_owned(),
            version: ICONSET_VERSION,
            id: id.to_owned(),
            name: name.into(),
            description: None,
            license: None,
            source: None,
            halftone: Halftone::default(),
            categories: Vec::new(),
            icons: Vec::new(),
        }
    }

    /// Parses and fully validates an icon set file (format, ids, every
    /// SVG). Use for files from outside the program.
    pub fn from_json(data: &[u8]) -> Result<Self, RenderError> {
        let set: IconSet =
            serde_json::from_slice(data).map_err(|e| invalid(format!("not readable: {e}")))?;
        set.validate(true)?;
        Ok(set)
    }

    /// Serializes the set as a `.llabel-iconset` file.
    pub fn to_json(&self) -> Result<String, RenderError> {
        serde_json::to_string_pretty(self).map_err(|e| invalid(e.to_string()))
    }

    /// Checks format, version, ids and categories; with `check_svg` also
    /// that every icon's SVG can be parsed.
    pub fn validate(&self, check_svg: bool) -> Result<(), RenderError> {
        if self.format != ICONSET_FORMAT {
            return Err(invalid(format!("format must be \"{ICONSET_FORMAT}\"")));
        }
        if self.version == 0 || self.version > ICONSET_VERSION {
            return Err(invalid(format!(
                "version {} is not supported (newest: {ICONSET_VERSION})",
                self.version
            )));
        }
        if !valid_id(&self.id) {
            return Err(invalid(format!(
                "set id {:?} may only contain a-z, 0-9, - and _",
                self.id
            )));
        }
        let categories: HashSet<&str> = self.categories.iter().map(|c| c.id.as_str()).collect();
        let mut ids = HashSet::new();
        for icon in &self.icons {
            if icon.id.is_empty() || icon.id.contains(':') {
                return Err(invalid(format!(
                    "icon id {:?} is empty or contains ':'",
                    icon.id
                )));
            }
            if !ids.insert(icon.id.as_str()) {
                return Err(invalid(format!("icon id {:?} is used twice", icon.id)));
            }
            if let Some(c) = &icon.category {
                if !categories.contains(c.as_str()) {
                    return Err(invalid(format!(
                        "icon {:?} uses unknown category {c:?}",
                        icon.id
                    )));
                }
            }
            if check_svg {
                usvg::Tree::from_str(&icon.svg, &usvg::Options::default())
                    .map_err(|e| invalid(format!("icon {:?}: {e}", icon.id)))?;
            }
        }
        Ok(())
    }

    pub fn icon(&self, id: &str) -> Option<&Icon> {
        self.icons.iter().find(|i| i.id == id)
    }
}

/// Cleans an SVG for storing in an icon set: parses it (system fonts are
/// loaded so text becomes paths) and writes it back as plain, compact SVG
/// without editor metadata, scripts or external references.
pub fn normalize_svg(svg: &[u8]) -> Result<String, RenderError> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(svg, &options).map_err(|e| invalid(e.to_string()))?;
    Ok(tree.to_string(&usvg::WriteOptions {
        coordinates_precision: 3,
        transforms_precision: 5,
        indent: usvg::Indent::None,
        attributes_indent: usvg::Indent::None,
        ..usvg::WriteOptions::default()
    }))
}

/// Bundled `.llabel-iconset` files (parsed lazily on first use).
const BUILTIN_FILES: &[&str] = &[
    include_str!("../assets/iconsets/iso7010.llabel-iconset"),
    include_str!("../assets/iconsets/iec60417.llabel-iconset"),
    include_str!("../assets/iconsets/ghs.llabel-iconset"),
];

/// The sets that ship with LabelLab: the original symbol library
/// (`material`) and the bundled `.llabel-iconset` files.
pub fn builtin_sets() -> Vec<IconSet> {
    let mut sets = vec![crate::symbols::material_set()];
    // Bundled files are covered by tests, so skipping a broken one only
    // guards against a bad build, it never hides user errors.
    sets.extend(
        BUILTIN_FILES
            .iter()
            .filter_map(|json| serde_json::from_str::<IconSet>(json).ok()),
    );
    sets
}

struct Registry {
    builtin: Vec<Arc<IconSet>>,
    user: Vec<Arc<IconSet>>,
}

fn registry() -> &'static RwLock<Registry> {
    static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        RwLock::new(Registry {
            builtin: builtin_sets().into_iter().map(Arc::new).collect(),
            user: Vec::new(),
        })
    })
}

/// Whether `id` belongs to a built-in set (those can't be replaced).
pub fn is_builtin(id: &str) -> bool {
    let reg = registry().read().unwrap_or_else(|e| e.into_inner());
    reg.builtin.iter().any(|s| s.id == id)
}

/// Adds `set` (or replaces a registered user set with the same id).
pub fn register(set: IconSet) -> Result<(), RenderError> {
    let mut reg = registry().write().unwrap_or_else(|e| e.into_inner());
    if reg.builtin.iter().any(|s| s.id == set.id) {
        return Err(invalid(format!(
            "set id {:?} is reserved for a built-in set",
            set.id
        )));
    }
    reg.user.retain(|s| s.id != set.id);
    reg.user.push(Arc::new(set));
    Ok(())
}

/// Removes a user set; `false` if none had that id.
pub fn unregister(id: &str) -> bool {
    let mut reg = registry().write().unwrap_or_else(|e| e.into_inner());
    let before = reg.user.len();
    reg.user.retain(|s| s.id != id);
    reg.user.len() != before
}

/// All registered sets, built-in first.
pub fn sets() -> Vec<Arc<IconSet>> {
    let reg = registry().read().unwrap_or_else(|e| e.into_inner());
    reg.builtin.iter().chain(reg.user.iter()).cloned().collect()
}

/// An icon found by [`resolve`].
#[derive(Debug, Clone)]
pub struct ResolvedIcon {
    pub set: Arc<IconSet>,
    pub index: usize,
}

impl ResolvedIcon {
    pub fn icon(&self) -> &Icon {
        &self.set.icons[self.index]
    }

    /// Full name `set:icon`.
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.set.id, self.icon().id)
    }
}

/// Looks up `set:icon`, or a bare `icon` in all sets (built-in first).
pub fn resolve(name: &str) -> Option<ResolvedIcon> {
    let all = sets();
    let (set_id, icon_id) = match name.split_once(':') {
        Some((s, i)) => (Some(s), i),
        None => (None, name),
    };
    all.into_iter()
        .filter(|s| set_id.is_none_or(|id| s.id == id))
        .find_map(|set| {
            let index = set.icons.iter().position(|i| i.id == icon_id)?;
            Some(ResolvedIcon { set, index })
        })
}

/// Error for a symbol name that no registered set contains.
pub(crate) fn unknown_symbol(name: &str) -> RenderError {
    RenderError::Image(format!(
        "unknown symbol '{name}' (list them with `labellab symbols`; icons from an imported icon set need that set installed)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#;

    fn sample(id: &str) -> IconSet {
        let mut set = IconSet::new(id, "Test");
        set.categories.push(Category {
            id: "shapes".into(),
            name: Text::Localized(BTreeMap::from([
                ("de".into(), "Formen".into()),
                ("en".into(), "Shapes".into()),
            ])),
        });
        set.icons.push(Icon {
            id: "square".into(),
            name: "Quadrat".into(),
            category: Some("shapes".into()),
            tags: vec![],
            svg: SQUARE.into(),
            license: None,
            author: None,
            source: None,
        });
        set
    }

    #[test]
    fn round_trips_and_validates() {
        let set = sample("test-rt");
        let json = set.to_json().unwrap();
        assert!(json.contains(r#""format": "llabel-iconset""#));
        assert_eq!(IconSet::from_json(json.as_bytes()).unwrap(), set);
        assert_eq!(set.categories[0].name.get("en"), "Shapes");
        assert_eq!(set.categories[0].name.get("fr"), "Formen");
    }

    #[test]
    fn rejects_bad_files() {
        let mut bad = sample("test-bad");
        bad.icons[0].svg = "<svg".into();
        assert!(IconSet::from_json(bad.to_json().unwrap().as_bytes()).is_err());

        let mut bad = sample("Bad Id");
        bad.id = "Bad Id".into();
        assert!(bad.validate(false).is_err());

        let mut bad = sample("test-cat");
        bad.icons[0].category = Some("nope".into());
        assert!(bad.validate(false).is_err());

        assert!(IconSet::from_json(b"{\"format\":\"other\"}").is_err());
    }

    #[test]
    fn registers_resolves_and_unregisters() {
        register(sample("test-reg")).unwrap();
        let found = resolve("test-reg:square").unwrap();
        assert_eq!(found.full_name(), "test-reg:square");
        assert!(unregister("test-reg"));
        assert!(resolve("test-reg:square").is_none());
    }

    #[test]
    fn builtin_ids_are_reserved_and_old_names_resolve() {
        assert!(register(sample("material")).is_err());
        assert_eq!(resolve("warning").unwrap().set.id, "material");
        assert_eq!(resolve("material:warning").unwrap().icon().id, "warning");
    }

    #[test]
    fn normalize_strips_metadata_and_keeps_shapes() {
        let raw = br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="10" height="10"><metadata>junk</metadata><rect inkscape:label="x" width="10" height="10" fill="red"/></svg>"#;
        let clean = normalize_svg(raw).unwrap();
        assert!(!clean.contains("metadata") && !clean.contains("inkscape"));
        assert!(clean.contains("<path"));
        usvg::Tree::from_str(&clean, &usvg::Options::default()).unwrap();
    }

    #[test]
    fn bundled_sets_are_valid() {
        for json in BUILTIN_FILES {
            let set = IconSet::from_json(json.as_bytes()).unwrap();
            assert!(!set.icons.is_empty());
        }
        assert_eq!(builtin_sets().len(), 1 + BUILTIN_FILES.len());
    }
}
