//! Table element: a grid of cells with text, columns along the label and
//! rows across the tape, each with a relative size. Lines between the
//! cells and (optionally) around the table. Rendered by
//! [`render_table`] through the same text path as text elements.

use ll_render::boxed::{self, TextLayout};
use ll_render::{Bitmap, TextAlign, VAlign};
use serde::{Deserialize, Serialize};

/// Default grid line width in mm (about 1.5 print dots).
pub const DEFAULT_LINE_MM: f32 = 0.2;
/// Smallest and largest relative column/row size.
pub const MIN_RATIO: f32 = 0.1;
pub const MAX_RATIO: f32 = 20.0;
/// Most rows and columns (guards against typos).
pub const MAX_CELLS_PER_SIDE: usize = 50;
/// Clearance between a line and the cell text, mm.
const CELL_PAD_MM: f32 = 0.4;

fn default_line_mm() -> f32 {
    DEFAULT_LINE_MM
}

fn yes() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

/// Content of a table element (see [`crate::label::Element::Table`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// Rows from the top edge of the tape, each a list of cell texts
    /// (may contain `\n` and inline `**bold**`/`__italic__`). Rows may
    /// be shorter than the widest one; missing cells are empty.
    pub cells: Vec<Vec<String>>,
    /// Relative column widths; missing entries count as 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub col_ratios: Vec<f32>,
    /// Relative row heights; missing entries count as 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_ratios: Vec<f32>,
    /// Grid line width in mm; 0 = no lines.
    #[serde(default = "default_line_mm")]
    pub line_mm: f32,
    /// Line around the table (otherwise only lines between cells).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub frame: bool,
    /// Cells of the first row in bold (header).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub header: bool,
}

impl Table {
    /// An empty `rows` x `cols` table.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            cells: vec![vec![String::new(); cols.max(1)]; rows.max(1)],
            col_ratios: Vec::new(),
            row_ratios: Vec::new(),
            line_mm: DEFAULT_LINE_MM,
            frame: true,
            header: false,
        }
    }

    pub fn rows(&self) -> usize {
        self.cells.len().clamp(1, MAX_CELLS_PER_SIDE)
    }

    pub fn cols(&self) -> usize {
        self.cells
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(1)
            .clamp(1, MAX_CELLS_PER_SIDE)
    }

    /// Text of cell (`row`, `col`), empty if missing.
    pub fn cell(&self, row: usize, col: usize) -> &str {
        self.cells
            .get(row)
            .and_then(|r| r.get(col))
            .map_or("", String::as_str)
    }
}

fn ratio(ratios: &[f32], i: usize) -> f32 {
    match ratios.get(i) {
        Some(r) if r.is_finite() => r.clamp(MIN_RATIO, MAX_RATIO),
        _ => 1.0,
    }
}

/// `n + 1` edges from 0 to `total` (dots), split by `ratios`.
pub fn edges(ratios: &[f32], n: usize, total: u32) -> Vec<u32> {
    let n = n.max(1);
    let sum: f32 = (0..n).map(|i| ratio(ratios, i)).sum();
    let mut acc = 0.0;
    let mut out = vec![0];
    for i in 0..n {
        acc += ratio(ratios, i);
        out.push(((acc / sum) * total as f32).round() as u32);
    }
    out
}

/// Text style shared by all cells.
pub struct CellStyle<'a> {
    pub faces: &'a ll_render::fonts::FaceSet<'a>,
    pub bold_faces: &'a ll_render::fonts::FaceSet<'a>,
    /// One size for all cells in dots; `None` = largest fitting every cell.
    pub size_px: Option<f32>,
    pub align: TextAlign,
    pub valign: VAlign,
    pub line_spacing: f32,
}

/// Dot sizes derived from mm by the caller's canvas.
pub struct TableDots {
    pub line: u32,
    pub pad: u32,
}

impl TableDots {
    pub fn from_mm(table: &Table, mm: impl Fn(f32) -> u32) -> Self {
        let line = if table.line_mm > 0.0 {
            mm(table.line_mm).max(1)
        } else {
            0
        };
        Self {
            line,
            pad: line + mm(CELL_PAD_MM),
        }
    }
}

/// Renders `table` into a `w` x `h` dot box; returns the bitmap and
/// whether any cell text was clipped.
pub fn render_table(
    table: &Table,
    w: u32,
    h: u16,
    style: &CellStyle,
    dots: &TableDots,
) -> Result<(Bitmap, bool), ll_render::RenderError> {
    let mut out = Bitmap::new(h, w);
    if w == 0 || h == 0 {
        return Ok((out, false));
    }
    let (rows, cols) = (table.rows(), table.cols());
    let xs = edges(&table.col_ratios, cols, w);
    let ys = edges(&table.row_ratios, rows, h as u32);
    let cell_box = |r: usize, c: usize| -> Option<(u32, u32, u32, u16)> {
        let x = xs[c] + dots.pad;
        let y = ys[r] + dots.pad;
        let cw = (xs[c + 1] - xs[c]).checked_sub(2 * dots.pad)?;
        let ch = (ys[r + 1] - ys[r]).checked_sub(2 * dots.pad)?;
        (cw > 0 && ch > 0).then(|| (x, y, cw, ch.min(u16::MAX as u32) as u16))
    };
    let faces_for = |r: usize| {
        if table.header && r == 0 {
            style.bold_faces
        } else {
            style.faces
        }
    };
    let filled: Vec<(usize, usize)> = (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .filter(|&(r, c)| !table.cell(r, c).trim().is_empty())
        .collect();
    let px = style.size_px.unwrap_or_else(|| {
        filled
            .iter()
            .filter_map(|&(r, c)| {
                let (_, _, cw, ch) = cell_box(r, c)?;
                Some(boxed::text_fit_px(
                    table.cell(r, c),
                    faces_for(r),
                    cw,
                    ch,
                    style.line_spacing,
                ))
            })
            .fold(f32::INFINITY, f32::min)
    });
    let mut clipped = false;
    for &(r, c) in &filled {
        let Some((x, y, cw, ch)) = cell_box(r, c) else {
            clipped = true;
            continue;
        };
        let (text, cut) = boxed::text_in_box_checked(
            table.cell(r, c),
            faces_for(r),
            cw,
            ch,
            &TextLayout {
                size_px: Some(px),
                align: style.align,
                valign: style.valign,
                line_spacing: style.line_spacing,
            },
        )?;
        clipped |= cut;
        out.blit(&text, y as i32, x as i32, 0..h);
    }
    draw_grid(&mut out, &xs, &ys, dots.line, table.frame);
    Ok((out, clipped))
}

/// Lines on the inner edges (and the outer ones with `frame`), centered
/// on the edge and kept inside the box.
fn draw_grid(out: &mut Bitmap, xs: &[u32], ys: &[u32], line: u32, frame: bool) {
    let (h, len) = (out.width_pins() as u32, out.height_dots());
    if line == 0 || h == 0 || len == 0 {
        return;
    }
    let inner = |edges: &[u32]| -> Vec<u32> {
        let last = edges.len().saturating_sub(1);
        edges
            .iter()
            .enumerate()
            .filter(|&(i, _)| frame || (i != 0 && i != last))
            .map(|(_, &e)| e)
            .collect()
    };
    let span = |edge: u32, size: u32| {
        let t = line.min(size);
        let start = edge.saturating_sub(t / 2).min(size - t);
        start..start + t
    };
    for x in inner(xs) {
        for l in span(x, len) {
            for pin in 0..h {
                out.set_pixel(pin as u16, l, true);
            }
        }
    }
    for y in inner(ys) {
        for pin in span(y, h) {
            for l in 0..len {
                out.set_pixel(pin as u16, l, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_follow_ratios_and_end_at_total() {
        assert_eq!(edges(&[], 4, 100), vec![0, 25, 50, 75, 100]);
        assert_eq!(edges(&[1.0, 3.0], 2, 100), vec![0, 25, 100]);
        // Missing and broken entries count as 1.
        assert_eq!(edges(&[f32::NAN], 2, 10), vec![0, 5, 10]);
    }

    #[test]
    fn size_follows_the_widest_row() {
        let t = Table {
            cells: vec![vec!["a".into()], vec!["b".into(), "c".into(), "d".into()]],
            ..Table::new(1, 1)
        };
        assert_eq!((t.rows(), t.cols()), (2, 3));
        assert_eq!(t.cell(0, 2), "");
        assert_eq!(t.cell(1, 2), "d");
    }

    #[test]
    fn grid_lines_with_and_without_frame() {
        let mut out = Bitmap::new(20, 40);
        draw_grid(&mut out, &[0, 20, 40], &[0, 10, 20], 2, false);
        // Inner lines only.
        assert!(out.pixel(5, 20) && out.pixel(10, 5));
        assert!(!out.pixel(0, 5) && !out.pixel(5, 0));
        let mut framed = Bitmap::new(20, 40);
        draw_grid(&mut framed, &[0, 20, 40], &[0, 10, 20], 2, true);
        assert!(framed.pixel(0, 5) && framed.pixel(5, 0) && framed.pixel(19, 39));
    }
}
