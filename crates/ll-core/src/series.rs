//! Series printing from CSV data (M7, ADR-017).
//!
//! A template refers to CSV columns with `{{Column}}` placeholders in text,
//! QR/barcode data and image paths; `{{#}}` is the 1-based record number,
//! `{{n}}`/`{{n:03}}`/`{{a}}`/`{{A}}` a running number or letter sequence
//! (start/step from [`Numbering`]), also usable without CSV.
//! [`apply`] fills them in for one record, producing a plain [`Label`] that
//! goes through the normal render/print path (preview and print stay on
//! the same path).
//!
//! CSV files as exported by Excel are handled: `;`, `,` or tab delimiter
//! (detected from the header line), UTF-8 with or without BOM, and
//! Windows-1252 ("ANSI") as fallback when the bytes aren't valid UTF-8.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::label::{Element, Label};
use crate::CoreError;

/// Widest zero padding of `{{n:000}}` (guards against huge allocations).
const MAX_NUMBER_WIDTH: usize = 32;

/// Placeholder for the record number (1-based).
pub const ROW_NUMBER_PLACEHOLDER: &str = "#";

/// A loaded CSV table: header names plus records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSet {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Folder of the CSV file: relative image paths in the cells (e.g.
    /// `bilder\\server1.png`) are taken from there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<PathBuf>,
}

impl DataSet {
    /// Reads a CSV file, see the module docs for supported formats.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let mut data = Self::from_bytes(&std::fs::read(path)?)?;
        data.base_dir = path.parent().map(Path::to_path_buf);
        Ok(data)
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
        Ok(Self {
            headers,
            rows,
            base_dir: None,
        })
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

/// Running number for `{{n}}` / `{{a}}` / `{{A}}`: label `k` (1-based)
/// gets `start + (k - 1) * step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Numbering {
    pub start: i64,
    pub step: i64,
}

impl Default for Numbering {
    fn default() -> Self {
        Self { start: 1, step: 1 }
    }
}

impl Numbering {
    /// The number for label `k` (1-based).
    pub fn value(&self, k: usize) -> i64 {
        self.start + (k as i64 - 1) * self.step
    }
}

/// Everything a placeholder can refer to for one label.
#[derive(Debug, Clone, Copy)]
pub struct Record<'a> {
    pub headers: &'a [String],
    pub row: &'a [String],
    /// 1-based position of the label in the series.
    pub number: usize,
    pub numbering: Numbering,
}

/// `1 -> a`, `26 -> z`, `27 -> aa` (spreadsheet-column style); `None`
/// below 1.
fn letters(mut n: i64, upper: bool) -> Option<String> {
    if n < 1 {
        return None;
    }
    let base = if upper { b'A' } else { b'a' };
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(base + (n % 26) as u8);
        n /= 26;
    }
    out.reverse();
    String::from_utf8(out).ok()
}

/// Value of one placeholder `name` (already trimmed), or `None` if
/// unknown. CSV columns win over the built-in `n`/`a`/`A` names.
fn resolve(name: &str, rec: &Record<'_>) -> Option<String> {
    if name == ROW_NUMBER_PLACEHOLDER {
        return Some(rec.number.to_string());
    }
    if let Some(i) = rec
        .headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(name))
    {
        return Some(rec.row.get(i).cloned().unwrap_or_default());
    }
    let value = rec.numbering.value(rec.number);
    let (key, format) = match name.split_once(':') {
        Some((k, f)) => (k.trim(), Some(f.trim())),
        None => (name, None),
    };
    match key {
        "n" => {
            let width = format
                .and_then(|f| f.strip_prefix('0').or(Some(f)))
                .and_then(|w| w.parse::<usize>().ok())
                .unwrap_or(0)
                .min(MAX_NUMBER_WIDTH);
            Some(if value < 0 {
                format!("-{:0width$}", -value, width = width)
            } else {
                format!("{value:0width$}")
            })
        }
        "a" => letters(value, false),
        "A" => letters(value, true),
        "datum" | "date" => now_formatted(format.unwrap_or("%d.%m.%Y")),
        "zeit" | "time" => now_formatted(format.unwrap_or("%H:%M")),
        _ => None,
    }
}

/// Current local date/time in a strftime `format`; `None` (placeholder
/// stays visible) if the format is invalid.
fn now_formatted(format: &str) -> Option<String> {
    use chrono::format::{Item, StrftimeItems};
    let items: Vec<Item> = StrftimeItems::new(format).collect();
    if items.iter().any(|i| matches!(i, Item::Error)) {
        return None;
    }
    Some(
        chrono::Local::now()
            .format_with_items(items.into_iter())
            .to_string(),
    )
}

/// Replaces placeholders in `text`: `{{Column}}` (CSV, case-insensitive,
/// spaces ignored), `{{#}}` (position in the series), `{{n}}` / `{{n:03}}`
/// (running number, optionally zero-padded), `{{a}}` / `{{A}}` (letter
/// sequence a..z, aa..), `{{datum}}` / `{{zeit}}` (current local date
/// `31.12.2026` / time `14:05`, optional strftime format such as
/// `{{datum:%Y-%m-%d}}`). Unknown placeholders stay as written so they're
/// visible on the preview.
pub fn fill(text: &str, rec: &Record<'_>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(len) = rest[start + 2..].find("}}") else {
            break;
        };
        out.push_str(&rest[..start]);
        let name = rest[start + 2..start + 2 + len].trim();
        match resolve(name, rec) {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[start..start + 2 + len + 2]),
        }
        rest = &rest[start + 2 + len + 2..];
    }
    out.push_str(rest);
    out
}

/// `label` with every placeholder filled for label `number` (1-based) of
/// a series: from that CSV record if `data` is given, and the running
/// number from `numbering`.
pub fn apply(label: &Label, data: Option<&DataSet>, number: usize, numbering: Numbering) -> Label {
    let empty: Vec<String> = Vec::new();
    let (headers, row) = match data {
        Some(d) => (
            d.headers.as_slice(),
            d.rows
                .get(number.wrapping_sub(1))
                .unwrap_or(&empty)
                .as_slice(),
        ),
        None => (empty.as_slice(), empty.as_slice()),
    };
    let rec = Record {
        headers,
        row,
        number,
        numbering,
    };
    let f = |s: &str| fill(s, &rec);
    let mut out = label.clone();
    for item in &mut out.elements {
        match &mut item.element {
            Element::Text { text, .. } => *text = f(text),
            Element::Qr { data } => *data = f(data),
            Element::Barcode { data, .. } => *data = f(data),
            Element::Image { path, .. } => {
                if let Some(p) = path.to_str() {
                    let filled = PathBuf::from(f(p));
                    // A path from a CSV cell may be relative to the CSV file.
                    let from_cell = p.contains("{{") && filled.is_relative();
                    *path = match data.and_then(|d| d.base_dir.as_ref()) {
                        Some(base) if from_cell && !filled.as_os_str().is_empty() => {
                            base.join(filled)
                        }
                        _ => filled,
                    };
                }
            }
            Element::Symbol { name, .. } => *name = f(name),
            Element::FuseBox { fields, .. } => {
                for field in fields {
                    field.text = f(&field.text);
                }
            }
            Element::Table { table, .. } => {
                for cell in table.cells.iter_mut().flatten() {
                    *cell = f(cell);
                }
            }
            Element::PipeMarker { marker, .. } => {
                marker.text = f(&marker.text);
                marker.sub_text = f(&marker.sub_text);
            }
            Element::Fill | Element::Shape { .. } => {}
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
            Element::FuseBox { fields, .. } => fields.iter().for_each(|f| scan(&f.text)),
            Element::Table { table, .. } => table.cells.iter().flatten().for_each(|c| scan(c)),
            Element::PipeMarker { marker, .. } => {
                scan(&marker.text);
                scan(&marker.sub_text);
            }
            Element::Fill | Element::Shape { .. } => {}
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
    fn image_path_from_a_column_is_relative_to_the_csv() {
        let mut d = DataSet::from_bytes("Name;Bild\nA;bilder/a.png\nB;/abs/b.png\nC;\n".as_bytes())
            .unwrap();
        d.base_dir = Some(PathBuf::from("/daten"));
        let label = Label::single(Element::Image {
            path: "{{Bild}}".into(),
            invert: false,
            brightness: 0,
            contrast: 0,
            edit: Default::default(),
        });
        let path = |n| match &apply(&label, Some(&d), n, Numbering::default()).elements[0].element {
            Element::Image { path, .. } => path.clone(),
            _ => unreachable!(),
        };
        assert_eq!(path(1), PathBuf::from("/daten/bilder/a.png"));
        assert_eq!(path(2), PathBuf::from("/abs/b.png"));
        // Empty cell: no image (rendered as an empty area).
        assert_eq!(path(3), PathBuf::from(""));
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

    fn rec<'a>(d: &'a DataSet, k: usize, numbering: Numbering) -> Record<'a> {
        Record {
            headers: &d.headers,
            row: &d.rows[k - 1],
            number: k,
            numbering,
        }
    }

    #[test]
    fn fills_placeholders() {
        let d = data();
        let s = fill(
            "{{name}} / {{ Raum }} #{{#}} {{fehlt}}",
            &rec(&d, 2, Numbering::default()),
        );
        assert_eq!(s, "Switch / Keller #2 {{fehlt}}");
    }

    #[test]
    fn date_and_time_placeholders() {
        let d = data();
        let r = rec(&d, 1, Numbering::default());
        let date = fill("{{datum}}", &r);
        assert!(
            date.len() == 10 && date.as_bytes()[2] == b'.' && date.as_bytes()[5] == b'.',
            "{date}"
        );
        let time = fill("{{zeit}}", &r);
        assert!(time.len() == 5 && time.as_bytes()[2] == b':', "{time}");
        assert_eq!(fill("{{date:%Y}}", &r).len(), 4);
        // An invalid format leaves the placeholder visible.
        assert_eq!(fill("{{datum:%Q}}", &r), "{{datum:%Q}}");
    }

    #[test]
    fn running_numbers_and_letters() {
        let d = data();
        let numbering = Numbering { start: 9, step: 2 };
        let s = fill("SW-{{n:03}} {{n}} {{A}}{{a}}", &rec(&d, 2, numbering));
        assert_eq!(s, "SW-011 11 Kk");
        assert_eq!(letters(26, true).as_deref(), Some("Z"));
        assert_eq!(letters(27, true).as_deref(), Some("AA"));
        assert_eq!(letters(0, true), None);
        let neg = fill("{{n:02}}", &rec(&d, 1, Numbering { start: -3, step: 1 }));
        assert_eq!(neg, "-03");
    }

    #[test]
    fn numbering_without_csv() {
        let label = Label {
            elements: vec![Element::text("Port {{n:02}}").into()],
            ..Label::default()
        };
        let out = apply(&label, None, 3, Numbering { start: 1, step: 1 });
        assert_eq!(out.elements[0].element, Element::text("Port 03"));
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
        let out = apply(&label, Some(&data()), 1, Numbering::default());
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
