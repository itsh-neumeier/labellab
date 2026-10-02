//! Import of `.lbx` files from the manufacturer's label editor (ADR-038).
//!
//! An `.lbx` file is a ZIP archive with `label.xml` (the design) and the
//! embedded images. Everything here is derived from example files (no
//! specification used, see `docs/IMPORT-LBX.md`), so unknown parts are
//! skipped with a [`LbxWarning`] instead of failing the import.
//!
//! Each `style:sheet` becomes a sheet of a [`Document`]. Lengths in the
//! XML are points (1/72 in). In landscape, `x` runs along the tape from
//! the cut, `y` across it from the tape edge; `style:backGround` is the
//! printable area, so LabelLab's `y` starts at its `y`.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use ll_render::image_edit::BackgroundRemoval;
use ll_render::richtext::{BOLD_MARK, ITALIC_MARK};
use ll_render::{Halftone, ImageEdit, ShapeKind, Symbology, TextAlign, VAlign};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

use crate::document::{Document, Sheet, DOCUMENT_FORMAT_VERSION};
use crate::label::{Element, Item, Label, Rect};
use crate::CoreError;

/// Millimeters per point.
const MM_PER_PT: f32 = 25.4 / 72.0;
/// Line width for table grids and frames when the file gives none, mm.
const DEFAULT_LINE_MM: f32 = 0.2;

/// Something that could not be taken over exactly. `kind` is a stable
/// code the GUI translates; `detail` names the object (sheet, text …).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LbxWarning {
    pub kind: &'static str,
    pub detail: String,
}

/// Result of an import: the document plus what was approximated.
#[derive(Debug, Clone, Serialize)]
pub struct LbxImport {
    pub document: Document,
    pub warnings: Vec<LbxWarning>,
}

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Template(format!("lbx: {e}"))
}

// ---------------------------------------------------------------- XML tree

#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: HashMap<String, String>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// First descendant (depth first) with this name.
    fn find(&self, name: &str) -> Option<&Node> {
        self.children.iter().find_map(|c| {
            if c.name == name {
                Some(c)
            } else {
                c.find(name)
            }
        })
    }

    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }

    /// A length attribute like `"12.5pt"`, in mm.
    fn mm(&self, key: &str) -> Option<f32> {
        parse_pt(self.attr(key)?).map(|pt| pt * MM_PER_PT)
    }
}

fn parse_pt(v: &str) -> Option<f32> {
    v.trim().trim_end_matches("pt").trim().parse().ok()
}

fn parse_xml(xml: &str) -> Result<Node, CoreError> {
    let mut reader = Reader::from_str(xml);
    let mut stack = vec![Node::default()];
    let open = |e: &quick_xml::events::BytesStart| -> Result<Node, CoreError> {
        let mut node = Node {
            name: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
            ..Node::default()
        };
        for a in e.attributes() {
            let a = a.map_err(err)?;
            let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
            let value = a.unescape_value().map_err(err)?.into_owned();
            node.attrs.insert(key, value);
        }
        Ok(node)
    };
    loop {
        match reader.read_event().map_err(err)? {
            Event::Start(e) => stack.push(open(&e)?),
            Event::Empty(e) => {
                let node = open(&e)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            }
            Event::End(_) => {
                let node = stack.pop().ok_or_else(|| err("unbalanced XML"))?;
                stack
                    .last_mut()
                    .ok_or_else(|| err("unbalanced XML"))?
                    .children
                    .push(node);
            }
            Event::Text(t) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&t.unescape().map_err(err)?);
                }
            }
            Event::CData(t) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let root = stack.pop().ok_or_else(|| err("empty XML"))?;
    root.children
        .into_iter()
        .next()
        .ok_or_else(|| err("empty XML"))
}

// ---------------------------------------------------------------- import

/// Imports `path`; embedded images are written to
/// `<data dir>/imported/<file name>-<content hash>/`, so importing another
/// file with the same name never replaces images a saved label uses.
pub fn import(path: &Path) -> Result<LbxImport, CoreError> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "import".into());
    let hash = crate::pasted::fnv1a(&std::fs::read(path)?);
    let dir = crate::paths::data_dir()?.join("imported").join(format!(
        "{}-{:08x}",
        sanitize(&stem),
        hash as u32
    ));
    import_into(path, &dir)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// [`import`] with an explicit folder for the images.
pub fn import_into(path: &Path, image_dir: &Path) -> Result<LbxImport, CoreError> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(err)?;
    let mut xml = String::new();
    zip.by_name("label.xml")
        .map_err(|_| err("label.xml missing – not a label file"))?
        .read_to_string(&mut xml)?;
    let root = parse_xml(&xml)?;
    let body = root
        .child("pt:body")
        .ok_or_else(|| err("pt:body missing"))?;
    let mut ctx = Context {
        zip: &mut zip,
        image_dir,
        images: HashMap::new(),
        warnings: Vec::new(),
        fonts: ll_render::fonts::families(),
        sheet: String::new(),
        shrink: Vec::new(),
    };
    let mut sheets = Vec::new();
    for sheet in body.children_named("style:sheet") {
        ctx.shrink.clear();
        let mut sheet = ctx.sheet(sheet)?;
        shrink_to_fit(&mut sheet, &ctx.shrink);
        sheets.push(sheet);
    }
    // The same note for several objects is listed once.
    let mut seen = std::collections::HashSet::new();
    ctx.warnings
        .retain(|w| seen.insert((w.kind, w.detail.clone())));
    if sheets.is_empty() {
        return Err(err("no sheets"));
    }
    // Open on the sheet that was current in the editor.
    if let Some(current) = body.attr("currentSheet") {
        if let Some(i) = sheets.iter().position(|s| s.name == current) {
            sheets.swap(0, i);
        }
    }
    Ok(LbxImport {
        document: Document {
            version: DOCUMENT_FORMAT_VERSION,
            sheets,
        },
        warnings: ctx.warnings,
    })
}

struct Context<'a, R: Read + std::io::Seek> {
    zip: &'a mut zip::ZipArchive<R>,
    image_dir: &'a Path,
    /// Embedded file name → extracted path.
    images: HashMap<String, PathBuf>,
    warnings: Vec<LbxWarning>,
    fonts: Vec<String>,
    /// Name of the sheet being imported (for warnings).
    sheet: String,
    /// Elements of the current sheet whose text shrinks to fit its frame.
    shrink: Vec<usize>,
}

/// Lengths to 1/100 mm (points convert to long fractions).
fn round_mm(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// Smallest size a shrinking text is reduced to, in pt.
const MIN_SHRINK_PT: f32 = 4.0;
/// Factor per step when shrinking a text that does not fit.
const SHRINK_STEP: f32 = 0.92;

/// Texts marked "shrink to fit" keep their size when they fit and are made
/// smaller step by step when they overflow their frame, like in the
/// original editor. Measured with the real renderer on the sheet's tape.
fn shrink_to_fit(sheet: &mut Sheet, shrink: &[usize]) {
    if shrink.is_empty() {
        return;
    }
    let width = sheet.width_mm.unwrap_or(12);
    let Some(model) = ll_protocol::model::MODELS
        .iter()
        .find(|m| m.tape_geometries.iter().any(|g| g.width_mm == width))
    else {
        return;
    };
    for _ in 0..20 {
        let Ok(preview) = crate::label::render_label_preview(&sheet.label, model, width, 1) else {
            return;
        };
        let mut changed = false;
        for &i in preview.overflowing.iter().filter(|i| shrink.contains(i)) {
            if let Some(Element::Text {
                size_pt: Some(size),
                ..
            }) = sheet
                .label
                .elements
                .get_mut(i)
                .map(|item| &mut item.element)
            {
                if *size > MIN_SHRINK_PT {
                    *size = ((*size * SHRINK_STEP).max(MIN_SHRINK_PT) * 10.0).round() / 10.0;
                    changed = true;
                }
            }
        }
        if !changed {
            return;
        }
    }
}

/// Text style from `text:ptFontInfo`.
#[derive(Debug, Clone, PartialEq)]
struct FontStyle {
    family: Option<String>,
    bold: bool,
    italic: bool,
    size_pt: Option<f32>,
}

fn font_style(info: Option<&Node>) -> FontStyle {
    let log = info.and_then(|i| i.child("text:logFont"));
    let ext = info.and_then(|i| i.child("text:fontExt"));
    FontStyle {
        family: log
            .and_then(|l| l.attr("name"))
            .filter(|n| !n.is_empty())
            .map(str::to_owned),
        bold: log
            .and_then(|l| l.attr("weight"))
            .and_then(|w| w.parse::<u32>().ok())
            .is_some_and(|w| w >= 600),
        italic: log.and_then(|l| l.attr("italic")) == Some("true"),
        size_pt: ext.and_then(|e| e.attr("size")).and_then(parse_pt),
    }
}

/// `text` with `**`/`__` around the runs whose style differs from `base`
/// (`text:stringItem charLen` runs, counted in characters).
fn styled_text(text: &str, base: &FontStyle, runs: &[(usize, FontStyle)]) -> String {
    if runs
        .iter()
        .all(|(_, s)| s.bold == base.bold && s.italic == base.italic)
    {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut pos = 0;
    for (len, style) in runs {
        let end = (pos + len).min(chars.len());
        let part: String = chars[pos..end].iter().collect();
        let mut wrapped = part;
        if style.italic != base.italic {
            wrapped = format!("{ITALIC_MARK}{wrapped}{ITALIC_MARK}");
        }
        if style.bold != base.bold {
            wrapped = format!("{BOLD_MARK}{wrapped}{BOLD_MARK}");
        }
        out.push_str(&wrapped);
        pos = end;
    }
    out.extend(chars[pos.min(chars.len())..].iter());
    out
}

/// Nearest tape width any supported model prints on (model table).
fn nearest_tape(mm: f32) -> u8 {
    ll_protocol::model::MODELS
        .iter()
        .flat_map(|m| m.tape_geometries.iter().map(|g| g.width_mm))
        .min_by(|a, b| (*a as f32 - mm).abs().total_cmp(&(*b as f32 - mm).abs()))
        .unwrap_or(12)
}

impl<R: Read + std::io::Seek> Context<'_, R> {
    fn warn(&mut self, kind: &'static str, detail: impl Into<String>) {
        let detail = detail.into();
        let detail = if detail.is_empty() {
            self.sheet.clone()
        } else {
            format!("{}: {detail}", self.sheet)
        };
        self.warnings.push(LbxWarning { kind, detail });
    }

    fn sheet(&mut self, node: &Node) -> Result<Sheet, CoreError> {
        let name = node.attr("name").unwrap_or("Blatt").to_owned();
        self.sheet = name.clone();
        let paper = node.child("style:paper");
        let bg = node.child("style:backGround");
        let tape_mm = paper.and_then(|p| p.mm("width")).unwrap_or(12.0);
        if paper.and_then(|p| p.attr("orientation")) == Some("portrait") {
            self.warn("portrait", "");
        }
        let origin_y = bg
            .and_then(|b| b.mm("y"))
            .or_else(|| paper.and_then(|p| p.mm("marginLeft")))
            .unwrap_or(0.0);
        let mut label = Label {
            padding_start_mm: paper.and_then(|p| p.mm("marginTop")).map(round_mm),
            padding_mm: paper
                .and_then(|p| p.mm("marginBottom"))
                .map_or(1.0, round_mm),
            ..Label::default()
        };
        if paper.and_then(|p| p.attr("autoLength")) == Some("false") {
            label.min_length_mm = paper.and_then(|p| p.mm("height")).map(round_mm);
            label.fixed_length = true;
        }
        if let Some(objects) = node.child("pt:objects") {
            for object in &objects.children {
                self.object(object, origin_y, &mut label.elements)?;
            }
        }
        Ok(Sheet {
            name,
            width_mm: Some(nearest_tape(tape_mm)),
            label,
        })
    }

    /// Box of an object, in LabelLab coordinates.
    fn rect(style: &Node, origin_y: f32) -> Rect {
        let r = round_mm;
        Rect {
            x_mm: r(style.mm("x").unwrap_or(0.0)),
            y_mm: r(style.mm("y").unwrap_or(origin_y) - origin_y),
            w_mm: r(style.mm("width").unwrap_or(10.0)),
            h_mm: r(style.mm("height").unwrap_or(5.0)),
        }
    }

    fn item(element: Element, rect: Rect, style: &Node) -> Item {
        let angle = style
            .attr("angle")
            .and_then(|a| a.parse::<f32>().ok())
            .unwrap_or(0.0);
        let rotation = ((angle.rem_euclid(360.0) / 90.0).round() as u16 % 4) * 90;
        Item {
            rect: Some(rect),
            rotation,
            ..element.into()
        }
    }

    fn object(&mut self, node: &Node, origin_y: f32, out: &mut Vec<Item>) -> Result<(), CoreError> {
        let Some(style) = node.child("pt:objectStyle") else {
            self.warn("unknown", node.name.clone());
            return Ok(());
        };
        let rect = Self::rect(style, origin_y);
        match node.name.as_str() {
            "text:text" => {
                let shrink = node
                    .child("text:textControl")
                    .and_then(|c| c.attr("shrink"))
                    == Some("true");
                if shrink {
                    self.shrink.push(out.len());
                }
                out.push(self.text(node, rect, style, None));
            }
            "text:datetime" => {
                let mode = node
                    .child("text:dateTimeStyle")
                    .and_then(|d| d.attr("mode"))
                    .unwrap_or("DATE");
                let placeholder = if mode == "TIME" {
                    "{{zeit}}"
                } else {
                    "{{datum}}"
                };
                out.push(self.text(node, rect, style, Some(placeholder)));
            }
            "barcode:barcode" => out.push(self.barcode(node, rect, style)),
            "image:image" => {
                if let Some(item) = self.image(node, rect, style)? {
                    out.push(item);
                }
            }
            "table:table" => self.table(node, rect, style, out),
            "draw:frame" => {
                self.warn("frame", "");
                out.push(Self::item(
                    outline(ShapeKind::Rectangle, style),
                    rect,
                    style,
                ));
            }
            "image:clipart" => {
                let name = node
                    .child("image:clipartStyle")
                    .and_then(|c| c.attr("originalName"))
                    .unwrap_or("")
                    .to_owned();
                self.warn("clipart", name);
            }
            other => {
                // Simple drawing objects by name; anything else is reported.
                let lower = other.to_ascii_lowercase();
                let shape = if lower.contains("ellipse") || lower.contains("circle") {
                    Some(ShapeKind::Ellipse)
                } else if lower.contains("round") {
                    Some(ShapeKind::RoundedRectangle)
                } else if lower.contains("rect") {
                    Some(ShapeKind::Rectangle)
                } else if lower.contains("line") {
                    Some(ShapeKind::Line)
                } else {
                    None
                };
                match shape {
                    Some(kind) => out.push(Self::item(outline(kind, style), rect, style)),
                    None => self.warn("unknown", other.to_owned()),
                }
            }
        }
        Ok(())
    }

    fn text(&mut self, node: &Node, rect: Rect, style: &Node, fixed: Option<&str>) -> Item {
        let base = font_style(node.child("text:ptFontInfo"));
        let raw = node
            .child("pt:data")
            .map(|d| d.text.replace("\r\n", "\n").replace('\r', "\n"))
            .unwrap_or_default();
        let text = match fixed {
            Some(t) => t.to_owned(),
            None => {
                let runs: Vec<(usize, FontStyle)> = node
                    .children_named("text:stringItem")
                    .map(|s| {
                        let len = s.attr("charLen").and_then(|l| l.parse().ok()).unwrap_or(0);
                        (len, font_style(s.child("text:ptFontInfo")))
                    })
                    .collect();
                styled_text(&raw, &base, &runs)
            }
        };
        if let Some(family) = &base.family {
            if !self.fonts.iter().any(|f| f.eq_ignore_ascii_case(family)) {
                let family = family.clone();
                self.warn("font", family);
            }
        }
        let align_node = node
            .child("text:textAlign")
            .or_else(|| node.child("text:dateTimeStyle"));
        let align = match align_node.and_then(|a| a.attr("horizontalAlignment")) {
            Some("LEFT") | Some("JUSTIFY") => TextAlign::Left,
            Some("RIGHT") => TextAlign::Right,
            _ => TextAlign::Center,
        };
        let valign = match align_node.and_then(|a| a.attr("verticalAlignment")) {
            Some("TOP") => Some(VAlign::Top),
            Some("BOTTOM") => Some(VAlign::Bottom),
            _ => None,
        };
        if node
            .child("text:textStyle")
            .and_then(|s| s.attr("vertical"))
            == Some("true")
        {
            self.warn("vertical_text", raw.lines().next().unwrap_or("").to_owned());
        }
        let element = Element::Text {
            text,
            size_pt: base.size_pt,
            align,
            font: base.family,
            bold: base.bold,
            italic: base.italic,
            line_spacing: None,
        };
        Item {
            valign,
            ..Self::item(element, rect, style)
        }
    }

    fn barcode(&mut self, node: &Node, rect: Rect, style: &Node) -> Item {
        let data = node
            .child("pt:data")
            .map(|d| d.text.clone())
            .unwrap_or_default();
        let protocol = node
            .child("barcode:barcodeStyle")
            .and_then(|b| b.attr("protocol"))
            .unwrap_or("")
            .to_ascii_uppercase();
        let symbology = match protocol.as_str() {
            "CODE39" => Some(Symbology::Code39),
            "CODE128" | "GS1-128" | "UCC/EAN128" => Some(Symbology::Code128),
            "EAN13" | "JAN13" => Some(Symbology::Ean13),
            "EAN8" | "JAN8" => Some(Symbology::Ean8),
            "UPCA" | "UPC-A" => Some(Symbology::UpcA),
            "ITF" | "I-2/5" | "ITF14" => Some(Symbology::Itf),
            _ => None,
        };
        let element = match symbology {
            Some(symbology) => Element::Barcode { symbology, data },
            None => {
                if protocol != "QRCODE" {
                    self.warn("barcode", protocol.clone());
                }
                Element::Qr { data }
            }
        };
        Self::item(element, rect, style)
    }

    fn image(&mut self, node: &Node, rect: Rect, style: &Node) -> Result<Option<Item>, CoreError> {
        let Some(img) = node.child("image:imageStyle") else {
            return Ok(None);
        };
        let Some(file) = img.attr("fileName").map(str::to_owned) else {
            self.warn("image", img.attr("originalName").unwrap_or("").to_owned());
            return Ok(None);
        };
        let path = match self.extract(&file) {
            Ok(p) => p,
            Err(e) => {
                self.warn("image", format!("{file}: {e}"));
                return Ok(None);
            }
        };
        let effect = img.child("image:effect");
        let level = |key: &str| -> i8 {
            effect
                .and_then(|e| e.attr(key))
                .and_then(|v| v.parse::<i32>().ok())
                .map(|v| ((v - 50) * 2).clamp(-100, 100) as i8)
                .unwrap_or(0)
        };
        let mono = img.child("image:mono");
        let mut edit = ImageEdit::default();
        match mono.and_then(|m| m.attr("operationKind")) {
            Some("BINARY") => {
                edit.halftone = Some(Halftone::Threshold);
                edit.threshold = mono
                    .and_then(|m| m.attr("threshold"))
                    .and_then(|t| t.parse::<u8>().ok())
                    .map(|t| t.clamp(1, 254));
            }
            Some(_) => edit.halftone = Some(Halftone::Dither),
            None => {}
        }
        if let Some(t) = img
            .child("image:transparent")
            .filter(|t| t.attr("flag") == Some("true"))
        {
            edit.background = Some(BackgroundRemoval {
                color: t.attr("color").and_then(parse_color),
                tolerance: 10,
                contiguous: false,
            });
        }
        if img.child("image:trimming").and_then(|t| t.attr("flag")) == Some("true") {
            self.warn("crop", img.attr("originalName").unwrap_or(&file).to_owned());
        }
        let element = Element::Image {
            path,
            invert: mono.and_then(|m| m.attr("reverse")) == Some("1"),
            brightness: level("brightness"),
            contrast: level("contrast"),
            edit,
        };
        Ok(Some(Self::item(element, rect, style)))
    }

    /// Writes an embedded file to the image folder (BMP converted to PNG,
    /// they are often huge); repeated references reuse it.
    fn extract(&mut self, name: &str) -> Result<PathBuf, CoreError> {
        if let Some(p) = self.images.get(name) {
            return Ok(p.clone());
        }
        let mut data = Vec::new();
        self.zip
            .by_name(name)
            .map_err(|_| err(format!("{name} missing in archive")))?
            .read_to_end(&mut data)?;
        std::fs::create_dir_all(self.image_dir)?;
        let safe = sanitize(
            Path::new(name)
                .file_stem()
                .map_or("image".into(), |s| s.to_string_lossy())
                .as_ref(),
        );
        let is_bmp = name.to_ascii_lowercase().ends_with(".bmp");
        let path = if is_bmp {
            let img = image::load_from_memory(&data).map_err(err)?;
            // `.bmp.png`, so it cannot clash with a `.png` of the same name.
            let path = self.image_dir.join(format!("{safe}.bmp.png"));
            img.save(&path).map_err(err)?;
            path
        } else {
            let ext = Path::new(name)
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|| "png".into());
            let path = self.image_dir.join(format!("{safe}.{ext}"));
            std::fs::write(&path, &data)?;
            path
        };
        self.images.insert(name.to_owned(), path.clone());
        Ok(path)
    }

    /// Table: outline, grid lines and cell texts.
    fn table(&mut self, node: &Node, rect: Rect, style: &Node, out: &mut Vec<Item>) {
        let line = style
            .child("pt:pen")
            .and_then(|p| p.mm("widthX"))
            .unwrap_or(DEFAULT_LINE_MM)
            .max(0.1);
        let grid = node.child("table:gridPosition");
        let positions = |key: &str| -> Vec<f32> {
            grid.and_then(|g| g.attr(key))
                .map(|v| {
                    v.split_whitespace()
                        .filter_map(parse_pt)
                        .map(|p| p * MM_PER_PT)
                        .collect()
                })
                .unwrap_or_default()
        };
        let (xs, ys) = (positions("x"), positions("y"));
        out.push(Self::item(
            Element::Shape {
                shape: ShapeKind::Rectangle,
                stroke_mm: line,
                filled: false,
            },
            rect,
            style,
        ));
        let fill = |r: Rect| -> Item {
            Item {
                rect: Some(r),
                ..Element::Fill.into()
            }
        };
        let inner = |v: &[f32]| v.len().saturating_sub(1).max(1);
        for x in xs.iter().skip(1).take(inner(&xs) - 1) {
            out.push(fill(Rect {
                x_mm: rect.x_mm + x - line / 2.0,
                y_mm: rect.y_mm,
                w_mm: line,
                h_mm: rect.h_mm,
            }));
        }
        for y in ys.iter().skip(1).take(inner(&ys) - 1) {
            out.push(fill(Rect {
                x_mm: rect.x_mm,
                y_mm: rect.y_mm + y - line / 2.0,
                w_mm: rect.w_mm,
                h_mm: line,
            }));
        }
        // Cell texts: cells are numbered from 1; spans cover several grid steps.
        let at = |v: &[f32], i: usize| v.get(i).copied();
        if let Some(cells) = node.child("table:cells") {
            for cell in cells.children_named("table:cell") {
                let Some(data) = cell.find("pt:data").map(|d| d.text.trim().to_owned()) else {
                    continue;
                };
                if data.is_empty() {
                    continue;
                }
                let num = |k: &str| cell.attr(k).and_then(|v| v.parse::<usize>().ok());
                let (cx, cy) = (num("addressX").unwrap_or(1), num("addressY").unwrap_or(1));
                let (sx, sy) = (num("spanX").unwrap_or(1), num("spanY").unwrap_or(1));
                let (Some(x0), Some(x1), Some(y0), Some(y1)) = (
                    at(&xs, cx - 1),
                    at(&xs, cx - 1 + sx),
                    at(&ys, cy - 1),
                    at(&ys, cy - 1 + sy),
                ) else {
                    continue;
                };
                let font = font_style(cell.find("text:ptFontInfo"));
                out.push(Item {
                    rect: Some(Rect {
                        x_mm: rect.x_mm + x0,
                        y_mm: rect.y_mm + y0,
                        w_mm: x1 - x0,
                        h_mm: y1 - y0,
                    }),
                    ..Element::Text {
                        text: data,
                        size_pt: font.size_pt,
                        align: TextAlign::Center,
                        font: font.family,
                        bold: font.bold,
                        italic: font.italic,
                        line_spacing: None,
                    }
                    .into()
                });
            }
        }
    }
}

/// Outline shape with the object's pen width.
fn outline(shape: ShapeKind, style: &Node) -> Element {
    Element::Shape {
        shape,
        stroke_mm: style
            .child("pt:pen")
            .and_then(|p| p.mm("widthX"))
            .unwrap_or(DEFAULT_LINE_MM)
            .max(0.1),
        filled: false,
    }
}

fn parse_color(v: &str) -> Option<[u8; 3]> {
    let hex = v.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A small `.lbx` built here (no files of the manufacturer's editor).
    fn sample(dir: &Path) -> PathBuf {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<pt:document xmlns:pt="p" xmlns:style="s" xmlns:text="t" xmlns:barcode="b" xmlns:image="i" xmlns:table="tb" xmlns:draw="d">
<pt:body currentSheet="Zwei">
<style:sheet name="Eins">
<style:paper width="68pt" height="283.6pt" marginLeft="8.4pt" marginTop="5.6pt" marginRight="8.4pt" marginBottom="5.6pt" orientation="landscape" autoLength="false"/>
<style:backGround x="5.6pt" y="8.4pt" width="272.4pt" height="51.2pt"/>
<pt:objects>
<text:text><pt:objectStyle x="56.8pt" y="31pt" width="221.2pt" height="28.6pt" angle="0"/>
<text:ptFontInfo><text:logFont name="Nicht Installiert Sans" weight="400" italic="false"/><text:fontExt size="18pt"/></text:ptFontInfo>
<text:textAlign horizontalAlignment="RIGHT" verticalAlignment="BOTTOM"/>
<pt:data>AKKU&#10;SCHRAUBER</pt:data>
<text:stringItem charLen="4"><text:ptFontInfo><text:logFont weight="700"/></text:ptFontInfo></text:stringItem>
<text:stringItem charLen="10"><text:ptFontInfo><text:logFont weight="400"/></text:ptFontInfo></text:stringItem>
</text:text>
<barcode:barcode><pt:objectStyle x="10pt" y="11.6pt" width="100pt" height="40pt"/><barcode:barcodeStyle protocol="EAN13"/><pt:data>012345678901</pt:data></barcode:barcode>
<barcode:barcode><pt:objectStyle x="120pt" y="11.6pt" width="30pt" height="30pt"/><barcode:barcodeStyle protocol="AZTECCODE"/><pt:data>42</pt:data></barcode:barcode>
<image:image><pt:objectStyle x="5.6pt" y="8.4pt" width="51.2pt" height="51.2pt"/>
<image:imageStyle originalName="bild.bmp" fileName="Object0.bmp"><image:effect brightness="60" contrast="50"/><image:mono operationKind="BINARY" threshold="100"/></image:imageStyle></image:image>
<image:image><pt:objectStyle x="60pt" y="8.4pt" width="20pt" height="20pt"/><image:imageStyle originalName="again.bmp" fileName="Object0.bmp"/></image:image>
<image:clipart><pt:objectStyle x="200pt" y="8.4pt" width="20pt" height="20pt"/><image:clipartStyle originalName="FONT,Dingbats,1,1"/></image:clipart>
</pt:objects>
</style:sheet>
<style:sheet name="Zwei">
<style:paper width="34pt" height="2834pt" marginTop="2.8pt" marginBottom="2.8pt" autoLength="true"/>
<style:backGround x="2.8pt" y="4.2pt" width="1000pt" height="25.6pt"/>
<pt:objects>
<text:datetime><pt:objectStyle x="10pt" y="4.2pt" width="100pt" height="25.6pt"/><text:dateTimeStyle mode="TIME" horizontalAlignment="LEFT"/></text:datetime>
<table:table><pt:objectStyle x="120pt" y="4.2pt" width="30pt" height="20pt"/><table:gridPosition x="0pt 15pt 30pt" y="0pt 10pt 20pt"/>
<table:cells><table:cell addressX="2" addressY="1" spanX="1" spanY="1"><pt:data>A1</pt:data></table:cell></table:cells></table:table>
<text:text><pt:objectStyle x="200pt" y="4.2pt" width="60pt" height="20pt"/><text:ptFontInfo><text:fontExt size="18pt"/></text:ptFontInfo>
<text:textControl control="LONGTEXTFIXED" shrink="true"/><pt:data>VIEL ZU LANGER TEXT</pt:data></text:text>
</pt:objects>
</style:sheet>
</pt:body></pt:document>"##;
        let mut bmp = Vec::new();
        image::RgbImage::from_pixel(4, 4, image::Rgb([0, 0, 0]))
            .write_to(&mut std::io::Cursor::new(&mut bmp), image::ImageFormat::Bmp)
            .unwrap();
        let path = dir.join("test.lbx");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("label.xml", opts).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
        zip.start_file("Object0.bmp", opts).unwrap();
        zip.write_all(&bmp).unwrap();
        zip.finish().unwrap();
        path
    }

    #[test]
    fn imports_sheets_objects_and_reports_what_is_approximated() {
        let dir = std::env::temp_dir().join(format!("ll-lbx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = sample(&dir);
        let result = import_into(&path, &dir.join("img")).unwrap();
        let doc = &result.document;
        // Current sheet first.
        let names: Vec<_> = doc.sheets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Zwei", "Eins"]);
        let eins = &doc.sheets[1];
        assert_eq!(eins.width_mm, Some(24));
        assert!(eins.label.fixed_length);
        assert!((eins.label.min_length_mm.unwrap() - 100.05).abs() < 0.1);
        assert!((eins.label.padding_start_mm.unwrap() - 1.98).abs() < 0.01);

        let items = &eins.label.elements;
        // Text: bold first word as inline markup, right/bottom aligned, y from the printable area.
        let Element::Text {
            text,
            size_pt,
            align,
            ..
        } = &items[0].element
        else {
            panic!("text expected");
        };
        assert_eq!(text, "**AKKU**\nSCHRAUBER");
        assert_eq!(*size_pt, Some(18.0));
        assert_eq!(*align, TextAlign::Right);
        assert_eq!(items[0].valign, Some(VAlign::Bottom));
        let r = items[0].rect.unwrap();
        assert!((r.x_mm - 20.04).abs() < 0.02 && (r.y_mm - 7.97).abs() < 0.02);
        // Barcodes: EAN-13 kept, Aztec becomes a QR code (warned).
        assert!(matches!(
            items[1].element,
            Element::Barcode {
                symbology: Symbology::Ean13,
                ..
            }
        ));
        assert!(matches!(items[2].element, Element::Qr { .. }));
        // Image: BMP converted to PNG once, levels and threshold mapped.
        let Element::Image {
            path: img,
            brightness,
            edit,
            ..
        } = &items[3].element
        else {
            panic!("image expected");
        };
        assert!(img.extension().unwrap() == "png" && img.exists());
        assert_eq!(*brightness, 20);
        assert_eq!(edit.threshold, Some(100));
        let Element::Image { path: again, .. } = &items[4].element else {
            panic!("image expected");
        };
        assert_eq!(again, img);
        assert_eq!(items.len(), 5, "clipart is skipped");

        let zwei = &doc.sheets[0];
        assert_eq!(zwei.width_mm, Some(12));
        assert!(!zwei.label.fixed_length);
        let Element::Text { text, .. } = &zwei.label.elements[0].element else {
            panic!("datetime → text");
        };
        assert_eq!(text, "{{zeit}}");
        // Table: outline + one inner line each way + one cell text.
        assert!(matches!(
            zwei.label.elements[1].element,
            Element::Shape { .. }
        ));
        assert_eq!(zwei.label.elements.len(), 6);
        let Element::Text { text, .. } = &zwei.label.elements[4].element else {
            panic!("cell text expected");
        };
        assert_eq!(text, "A1");
        // "Shrink to fit": made smaller until it fits its frame.
        let Element::Text { size_pt, .. } = &zwei.label.elements[5].element else {
            panic!("text expected");
        };
        assert!(size_pt.unwrap() < 18.0);

        let kinds: Vec<_> = result.warnings.iter().map(|w| w.kind).collect();
        assert!(
            kinds.contains(&"barcode") && kinds.contains(&"clipart") && kinds.contains(&"font")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_files_without_label_xml() {
        let dir = std::env::temp_dir().join(format!("ll-lbx-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.lbx");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file("other.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.finish().unwrap();
        assert!(import_into(&path, &dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
