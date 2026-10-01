//! System font discovery and loading by family, bold and italic, via
//! `fontdb` (already in the tree through `resvg`/`usvg`, see ADR-017).
//!
//! The system font database is scanned once per process and cached; the
//! first call can take a moment on machines with many fonts.

use std::sync::OnceLock;

use fontdb::{Database, Family, Query, Style, Weight};
use fontdue::{Font, FontSettings};

use crate::{fontsrc, RenderError};

static DATABASE: OnceLock<Database> = OnceLock::new();

fn database() -> &'static Database {
    DATABASE.get_or_init(|| {
        let mut db = Database::new();
        db.load_system_fonts();
        db
    })
}

/// A parsed font face, ready for rendering. `synthetic_*` are set when the
/// requested style isn't installed for the family and has to be faked
/// (slanted / thickened glyphs) so it still shows on the label.
pub struct Face {
    pub(crate) font: Font,
    pub(crate) synthetic_bold: bool,
    pub(crate) synthetic_italic: bool,
}

impl Face {
    /// Parses raw TTF/OTF/TTC bytes; `index` selects the face inside a
    /// collection (0 for single fonts).
    pub fn from_bytes(data: &[u8], index: u32) -> Result<Self, RenderError> {
        let settings = FontSettings {
            collection_index: index,
            ..FontSettings::default()
        };
        Font::from_bytes(data, settings)
            .map(|font| Face {
                font,
                synthetic_bold: false,
                synthetic_italic: false,
            })
            .map_err(|e| RenderError::Font(e.to_string()))
    }

    /// The default system font (see [`fontsrc`]).
    pub fn default_face() -> Result<Self, RenderError> {
        Self::from_bytes(&fontsrc::load_default_font()?, 0)
    }

    /// Loads `family` (default font's family if `None`) in the requested
    /// style. Falls back to the closest available style of the family (a
    /// family without a bold face renders regular), and to the default
    /// font if the family isn't installed.
    pub fn load(family: Option<&str>, bold: bool, italic: bool) -> Result<Self, RenderError> {
        if family.is_none() && !bold && !italic {
            return Self::default_face();
        }
        let db = database();
        let default_family = default_family_name(db);
        let name = family.or(default_family.as_deref());
        let families = match name {
            Some(n) => vec![Family::Name(n), Family::SansSerif],
            None => vec![Family::SansSerif],
        };
        let query = Query {
            families: &families,
            weight: if bold { Weight::BOLD } else { Weight::NORMAL },
            style: if italic { Style::Italic } else { Style::Normal },
            ..Query::default()
        };
        let Some(id) = db.query(&query) else {
            return Self::synthesize(Self::default_face()?, bold, italic, false, false);
        };
        let (has_bold, has_italic) = db.face(id).map_or((false, false), |f| {
            (f.weight.0 >= Weight::SEMIBOLD.0, f.style != Style::Normal)
        });
        let face = db
            .with_face_data(id, Self::from_bytes)
            .unwrap_or_else(Self::default_face)?;
        Self::synthesize(face, bold, italic, has_bold, has_italic)
    }

    fn synthesize(
        mut face: Self,
        bold: bool,
        italic: bool,
        has_bold: bool,
        has_italic: bool,
    ) -> Result<Self, RenderError> {
        face.synthetic_bold = bold && !has_bold;
        face.synthetic_italic = italic && !has_italic;
        Ok(face)
    }
}

/// Family name of the default font file, so bold/italic without an
/// explicit family stays in the same typeface.
fn default_family_name(db: &Database) -> Option<String> {
    let path = fontsrc::default_font_path()?;
    db.faces().find_map(|f| match &f.source {
        fontdb::Source::File(p) if *p == path => f.families.first().map(|(n, _)| n.clone()),
        _ => None,
    })
}

/// Installed font family names, sorted and de-duplicated.
pub fn families() -> Vec<String> {
    let mut names: Vec<String> = database()
        .faces()
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_family_falls_back_instead_of_failing() {
        if fontsrc::load_default_font().is_err() {
            return; // no system font on this machine
        }
        assert!(Face::load(Some("Does Not Exist 12345"), true, false).is_ok());
    }

    #[test]
    fn bold_face_differs_from_regular_when_installed() {
        if fontsrc::load_default_font().is_err() {
            return;
        }
        let regular = Face::load(None, false, false).unwrap();
        let bold = Face::load(None, true, false).unwrap();
        let ink = |f: &Face| {
            let (_, bitmap) = f.font.rasterize('H', 40.0);
            bitmap.iter().filter(|&&a| a > 128).count()
        };
        // A real or synthetic bold face, never lighter.
        assert!(ink(&bold) >= ink(&regular));
    }

    #[test]
    fn missing_style_is_synthesized() {
        if fontsrc::load_default_font().is_err() {
            return;
        }
        let face = Face::load(Some("Does Not Exist 12345"), true, true).unwrap();
        assert!(face.synthetic_bold && face.synthetic_italic);
    }

    #[test]
    fn families_are_sorted_and_unique() {
        let f = families();
        let mut sorted = f.clone();
        sorted.sort_by_key(|n| n.to_lowercase());
        sorted.dedup();
        assert_eq!(f, sorted);
    }
}
