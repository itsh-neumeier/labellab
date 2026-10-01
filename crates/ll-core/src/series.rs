//! Series printing from CSV data (M7, ADR-017).
//!
//! A template refers to CSV columns with `{{Column}}` placeholders in text,
//! QR/barcode data and image paths; `{{#}}` is the 1-based record number.
//! [`apply`] fills them in for one record, producing a plain [`Label`] that
//! goes through the normal render/print path (preview and print stay on
//! the same path).
//!
//! CSV files as exported by Excel are handled: `;`, `,` or tab delimiter
//! (detected from the header line), UTF-8 with or without BOM, and
//! Windows-1252 ("ANSI") as fallback when the bytes aren't valid UTF-8.

use std::ops::RangeInclusive;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::label::{Element, Label};
use crate::CoreError;

/// Placeholder for the record number (1-based).
pub const ROW_NUMBER_PLACEHOLDER: &str = "#";

/// A loaded CSV table: header names plus records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSet {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl DataSet {
    /// Reads a CSV file, see the module docs for supported formats.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        Self::from_bytes(&std::fs::read(path)?)
    }

    /// Parses CSV bytes; the first record is the header line.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        let text = decode(bytes);
        let delimiter = detect_delimiter(text.lines().next().unwrap_or_default());
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(text.as_bytes());
        let csv_err = |e: csv::Error| CoreError::Template(format!("CSV: {e}"));

        let headers: Vec<String> = reader
            .headers()
            .map_err(csv_err)?
            .iter()
            .map(|h| h.trim().to_string())
            .collect();
        let mut rows = Vec::new();
        for record in reader.records() {
            let record = record.map_err(csv_err)?;
            if record.iter().all(|f| f.trim().is_empty()) {
                continue; // trailing empty lines from spreadsheet exports
            }
            let mut row: Vec<String> = record.iter().map(str::to_string).collect();
            row.resize(headers.len(), String::new());
            rows.push(row);
        }
        Ok(Self { headers, rows })
    }

    /// Record numbers (1-based, inclusive) clamped to the data; `None`
    /// selects all records. Empty if nothing is left.
    pub fn select(&self, range: Option<RangeInclusive<usize>>) -> RangeInclusive<usize> {
        let last = self.rows.len();
        match range {
            None => 1..=last,
            Some(r) => (*r.start()).max(1)..=(*r.end()).min(last),
        }
    }
}

/// UTF-8 (BOM stripped) or, if invalid, Windows-1252.
fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| windows_1252(b)).collect(),
    }
}

/// Windows-1252 byte to char (0x80-0x9F differ from Latin-1).
fn windows_1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž',
        '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Picks the most frequent of `;`, `,` and tab in the header line
/// (outside quotes); `;` wins ties (German Excel default).
fn detect_delimiter(header: &str) -> u8 {
    let mut counts = [(b';', 0), (b',', 0), (b'\t', 0)];
    let mut quoted = false;
    for c in header.bytes() {
        if c == b'"' {
            quoted = !quoted;
        } else if !quoted {
            for (d, n) in counts.iter_mut() {
                if c == *d {
                    *n += 1;
                }
            }
        }
    }
    counts
        .iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0)
        .map_or(b';', |(d, _)| *d)
}

/// Replaces `{{Name}}` (column name, case-insensitive, surrounding spaces
/// ignored) and `{{#}}` (record number) in `text`. Unknown placeholders
/// stay as written so they're visible on the preview.
pub fn fill(text: &str, headers: &[String], row: &[String], number: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(len) = rest[start + 2..].find("}}") else {
            break;
        };
        out.push_str(&rest[..start]);
        let name = rest[start + 2..start + 2 + len].trim();
        let value = if name == ROW_NUMBER_PLACEHOLDER {
            Some(number.to_string())
        } else {
            headers
                .iter()
                .position(|h| h.eq_ignore_ascii_case(name))
                .map(|i| row.get(i).cloned().unwrap_or_default())
        };
        match value {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[start..start + 2 + len + 2]),
        }
        rest = &rest[start + 2 + len + 2..];
    }
    out.push_str(rest);
    out
}

/// `label` with every placeholder filled from record `number` (1-based)
/// of `data`.
pub fn apply(label: &Label, data: &DataSet, number: usize) -> Label {
    let empty = Vec::new();
    let row = data.rows.get(number.wrapping_sub(1)).unwrap_or(&empty);
    let f = |s: &str| fill(s, &data.headers, row, number);
    let mut out = label.clone();
    for item in &mut out.elements {
        match &mut item.element {
            Element::Text { text, .. } => *text = f(text),
            Element::Qr { data } => *data = f(data),
            Element::Barcode { data, .. } => *data = f(data),
            Element::Image { path, .. } => {
                if let Some(p) = path.to_str() {
                    *path = f(p).into();
                }
            }
            Element::Symbol { name, .. } => *name = f(name),
        }
    }
    out
}

/// Placeholder names used in `label` (without braces), in order of first
/// use.
pub fn placeholders(label: &Label) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut scan = |s: &str| {
        let mut rest = s;
        while let Some(start) = rest.find("{{") {
            let Some(len) = rest[start + 2..].find("}}") else {
                break;
            };
            let name = rest[start + 2..start + 2 + len].trim().to_string();
            if !names.contains(&name) {
                names.push(name);
            }
            rest = &rest[start + 2 + len + 2..];
        }
    };
    for item in &label.elements {
        match &item.element {
            Element::Text { text, .. } => scan(text),
            Element::Qr { data } | Element::Barcode { data, .. } => scan(data),
            Element::Image { path, .. } => scan(&path.to_string_lossy()),
            Element::Symbol { name, .. } => scan(name),
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> DataSet {
        DataSet::from_bytes("Name;Raum\nServer 1;2.04\nSwitch;Keller\n".as_bytes()).unwrap()
    }

    #[test]
    fn parses_semicolon_csv() {
        let d = data();
        assert_eq!(d.headers, ["Name", "Raum"]);
        assert_eq!(d.rows, [["Server 1", "2.04"], ["Switch", "Keller"]]);
    }

    #[test]
    fn detects_comma_and_tab_and_quotes() {
        let d = DataSet::from_bytes(b"a,b\n\"x, y\",2\n").unwrap();
        assert_eq!(d.rows, [["x, y", "2"]]);
        let d = DataSet::from_bytes(b"a\tb\n1\t2\n").unwrap();
        assert_eq!(d.rows, [["1", "2"]]);
    }

    #[test]
    fn handles_bom_ansi_and_ragged_rows() {
        let d = DataSet::from_bytes(b"\xEF\xBB\xBFName;Ort\nA\n").unwrap();
        assert_eq!(d.headers, ["Name", "Ort"]);
        assert_eq!(d.rows, [["A", ""]]);
        // "Büro" in Windows-1252
        let d = DataSet::from_bytes(b"Raum\nB\xFCro \x80\n").unwrap();
        assert_eq!(d.rows, [["Büro €"]]);
    }

    #[test]
    fn fills_placeholders() {
        let d = data();
        let s = fill(
            "{{name}} / {{ Raum }} #{{#}} {{fehlt}}",
            &d.headers,
            &d.rows[1],
            2,
        );
        assert_eq!(s, "Switch / Keller #2 {{fehlt}}");
    }

    #[test]
    fn applies_to_every_element_type() {
        let label = Label {
            elements: vec![
                Element::text("{{Name}}").into(),
                Element::Qr {
                    data: "https://x/{{Raum}}".into(),
                }
                .into(),
            ],
            ..Label::default()
        };
        let out = apply(&label, &data(), 1);
        assert_eq!(out.elements[0].element, Element::text("Server 1"));
        assert_eq!(
            out.elements[1].element,
            Element::Qr {
                data: "https://x/2.04".into()
            }
        );
        assert_eq!(placeholders(&label), ["Name", "Raum"]);
    }

    #[test]
    fn selects_clamped_ranges() {
        let d = data();
        assert_eq!(d.select(None), 1..=2);
        assert_eq!(d.select(Some(0..=10)), 1..=2);
        assert!(d.select(Some(5..=9)).is_empty());
    }
}
