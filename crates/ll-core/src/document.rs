//! A `.llabel` file with several sheets (like tabs in a spreadsheet): each
//! sheet is a complete [`Label`] with a name and, optionally, the tape
//! width it was designed for.
//!
//! On disk a single-sheet document is written as a plain label (format
//! version 2), so such files stay readable by older versions; documents
//! with more sheets use `{"version": 3, "sheets": [...]}`. Older versions
//! reject version 3 with a clear "newer format" error instead of reading an
//! empty label.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::label::Label;
use crate::CoreError;

/// Format version of multi-sheet documents.
pub const DOCUMENT_FORMAT_VERSION: u32 = 3;

/// One sheet of a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    /// Tape width the sheet was designed for, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<u8>,
    pub label: Label,
}

/// A `.llabel` file: one or more sheets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "document_version")]
    pub version: u32,
    pub sheets: Vec<Sheet>,
}

fn document_version() -> u32 {
    DOCUMENT_FORMAT_VERSION
}

fn invalid(msg: impl Into<String>) -> CoreError {
    CoreError::Template(msg.into())
}

impl Document {
    /// A document with one sheet.
    pub fn single(name: &str, label: Label) -> Self {
        Self {
            version: DOCUMENT_FORMAT_VERSION,
            sheets: vec![Sheet {
                name: name.to_owned(),
                width_mm: None,
                label,
            }],
        }
    }

    /// Reads a document or a plain (single-sheet) label. `default_name`
    /// names the sheet of a plain label.
    pub fn from_json(json: &str, default_name: &str) -> Result<Self, CoreError> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        if value.get("sheets").is_none() {
            return Ok(Self::single(default_name, Label::from_json(json)?));
        }
        let doc: Document = serde_json::from_value(value).map_err(|e| invalid(e.to_string()))?;
        if doc.version > DOCUMENT_FORMAT_VERSION {
            return Err(invalid(format!(
                "format version {} is newer than supported ({DOCUMENT_FORMAT_VERSION})",
                doc.version
            )));
        }
        if doc.sheets.is_empty() {
            return Err(invalid("document has no sheets"));
        }
        // Validate every sheet like a standalone label.
        for sheet in &doc.sheets {
            Label::from_json(&sheet.label.to_json()?)?;
        }
        Ok(doc)
    }

    /// Serializes: a plain label for one sheet, else a version-3 document.
    pub fn to_json(&self) -> Result<String, CoreError> {
        match self.sheets.as_slice() {
            [only] => only.label.to_json(),
            [] => Err(invalid("document has no sheets")),
            _ => {
                let mut doc = self.clone();
                doc.version = DOCUMENT_FORMAT_VERSION;
                for sheet in &mut doc.sheets {
                    // Store each label with the current label format version.
                    sheet.label = Label::from_json(&sheet.label.to_json()?)?;
                }
                serde_json::to_string_pretty(&doc).map_err(|e| invalid(e.to_string()))
            }
        }
    }

    /// Reads a `.llabel` file (any sheet count) and resolves relative image
    /// paths against its directory.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let json = std::fs::read_to_string(path)?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Label");
        let mut doc = Self::from_json(&json, name)?;
        if let Some(dir) = path.parent() {
            for sheet in &mut doc.sheets {
                sheet.label.resolve_paths(dir);
            }
        }
        Ok(doc)
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    /// The sheet chosen by `selector`: a 1-based number or a sheet name
    /// (case-insensitive); `None` = the first sheet.
    pub fn sheet(&self, selector: Option<&str>) -> Result<&Sheet, CoreError> {
        let Some(sel) = selector else {
            return self
                .sheets
                .first()
                .ok_or_else(|| invalid("document has no sheets"));
        };
        if let Ok(n) = sel.trim().parse::<usize>() {
            if let Some(sheet) = n.checked_sub(1).and_then(|i| self.sheets.get(i)) {
                return Ok(sheet);
            }
        }
        self.sheets
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(sel.trim()))
            .ok_or_else(|| {
                invalid(format!(
                    "no sheet {sel:?} (sheets: {})",
                    self.sheets
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::Element;

    #[test]
    fn single_sheet_is_written_as_plain_label() {
        let doc = Document::single("A", Label::single(Element::text("x")));
        let json = doc.to_json().unwrap();
        assert!(!json.contains("sheets"));
        let back = Document::from_json(&json, "Datei").unwrap();
        assert_eq!(back.sheets.len(), 1);
        assert_eq!(back.sheets[0].name, "Datei");
        assert_eq!(back.sheets[0].label, doc.sheets[0].label);
    }

    #[test]
    fn multi_sheet_round_trips_and_selects() {
        let mut doc = Document::single("Kabel", Label::single(Element::text("K1")));
        doc.sheets.push(Sheet {
            name: "Ports".into(),
            width_mm: Some(24),
            label: Label::single(Element::text("P1")),
        });
        let json = doc.to_json().unwrap();
        assert!(json.contains(r#""version": 3"#) && json.contains(r#""sheets""#));
        let back = Document::from_json(&json, "x").unwrap();
        assert_eq!(back, doc);
        assert_eq!(back.sheet(Some("2")).unwrap().name, "Ports");
        assert_eq!(back.sheet(Some("ports")).unwrap().width_mm, Some(24));
        assert_eq!(back.sheet(None).unwrap().name, "Kabel");
        assert!(back.sheet(Some("3")).is_err());
        // An older reader that only knows labels refuses it clearly.
        assert!(Label::from_json(&json).is_err());
    }

    #[test]
    fn rejects_empty_and_newer_documents() {
        assert!(Document::from_json(r#"{"sheets": []}"#, "x").is_err());
        assert!(Document::from_json(r#"{"version": 9, "sheets": []}"#, "x").is_err());
    }
}
