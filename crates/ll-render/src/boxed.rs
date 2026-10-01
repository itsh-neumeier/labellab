//! Renders one element into a fixed box (free layout, M6 editor).
//!
//! Every function returns a box-local [`Bitmap`] with the same orientation
//! as the rest of the crate: `width_pins` = box height (tape-width axis),
//! `height_dots` = box width (feed/length axis). The caller blits it into
//! the label at the box position. Content is fitted into the box keeping
//! its aspect ratio and centered (text: aligned as requested).

use std::path::Path;

use fontdue::layout::{
    CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign,
};
use fontdue::Font;
use serde::{Deserialize, Serialize};

use crate::linear_barcode::encode_modules;
use crate::picture::{floyd_steinberg_dither, load_gray, render_svg_to_gray};
use crate::{render_qr, Bitmap, Face, QrErrorCorrection, RenderError, Symbology};

/// Alpha threshold (0-255) above which a rasterized pixel counts as ink.
const INK_THRESHOLD: u8 = 128;

/// Slant (dx per dy) of synthetic italic, about 12 degrees.
const SYNTHETIC_ITALIC_SLANT: f32 = 0.21;

/// Synthetic bold widens strokes by one dot per this many px of font size.
const SYNTHETIC_BOLD_PX_PER_DOT: f32 = 24.0;

/// Smallest font size (px = print dots per em) the auto-fit tries.
const MIN_AUTO_FONT_PX: f32 = 4.0;

/// Horizontal alignment of text lines inside their box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

impl From<TextAlign> for HorizontalAlign {
    fn from(a: TextAlign) -> Self {
        match a {
            TextAlign::Left => HorizontalAlign::Left,
            TextAlign::Center => HorizontalAlign::Center,
            TextAlign::Right => HorizontalAlign::Right,
        }
    }
}

/// Layout settings shared by measuring and rendering, so a size that
/// measured as fitting renders exactly the same way.
fn settings(max_w: Option<f32>, max_h: Option<f32>, align: TextAlign) -> LayoutSettings {
    LayoutSettings {
        max_width: max_w,
        max_height: max_h,
        horizontal_align: align.into(),
        vertical_align: if max_h.is_some() {
            VerticalAlign::Middle
        } else {
            VerticalAlign::Top
        },
        ..LayoutSettings::default()
    }
}

fn layout(font: &Font, text: &str, px: f32, settings: &LayoutSettings) -> Layout {
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(settings);
    layout.append(&[font], &TextStyle::new(text, px, 0));
    layout
}

/// Ink extent along the length axis (rightmost glyph pixel).
fn ink_width(layout: &Layout) -> f32 {
    layout
        .glyphs()
        .iter()
        .map(|g| g.x + g.width as f32)
        .fold(0.0, f32::max)
}

/// Number of lines `text` has from explicit line breaks alone.
fn hard_lines(text: &str) -> usize {
    text.split('\n').count()
}

/// Whether `text` at `px` fits `max_w` x `max_h` without any automatic
/// word wrap (`max_w = None`: height only).
fn fits(font: &Font, text: &str, px: f32, max_w: Option<f32>, max_h: f32) -> bool {
    let l = layout(font, text, px, &settings(max_w, None, TextAlign::Left));
    let lines = l.lines().map_or(0, Vec::len);
    l.height() <= max_h && lines <= hard_lines(text) && max_w.is_none_or(|w| ink_width(&l) <= w)
}

/// Largest font size at which `text` (explicit line breaks only) fits
/// `max_w` x `max_h` dots. `max_w = None` fits the height only.
fn auto_font_px(font: &Font, text: &str, max_w: Option<f32>, max_h: f32) -> f32 {
    let (mut lo, mut hi) = (MIN_AUTO_FONT_PX, max_h.max(MIN_AUTO_FONT_PX) * 1.5);
    if !fits(font, text, lo, max_w, max_h) {
        return lo;
    }
    for _ in 0..14 {
        let mid = (lo + hi) / 2.0;
        if fits(font, text, mid, max_w, max_h) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Width in dots `text` needs at `size_px` (or, if `None`, at the largest
/// size fitting `box_h`), without wrapping. Used to give flow-layout text
/// its natural length.
pub fn text_natural_width(
    text: &str,
    face: &Face,
    box_h: u16,
    size_px: Option<f32>,
) -> Result<u32, RenderError> {
    let font = &face.font;
    let px = size_px.unwrap_or_else(|| auto_font_px(font, text, None, box_h as f32));
    let l = layout(font, text, px, &settings(None, None, TextAlign::Left));
    Ok(ink_width(&l).ceil().max(1.0) as u32 + 1)
}

/// Renders `text` into a `box_w` x `box_h` dot box. Explicit `\n` start a
/// new line. `size_px = None` picks the largest size that fits the box
/// without wrapping; a fixed size wraps long lines at word boundaries.
/// Lines are vertically centered; overflow is clipped.
pub fn text_in_box(
    text: &str,
    face: &Face,
    box_w: u32,
    box_h: u16,
    size_px: Option<f32>,
    align: TextAlign,
) -> Result<Bitmap, RenderError> {
    Ok(text_in_box_checked(text, face, box_w, box_h, size_px, align)?.0)
}

/// Ink this many dots outside the box still counts as fitting (glyph
/// position rounding), see [`text_in_box_checked`].
const CLIP_TOLERANCE_DOTS: i64 = 1;

/// Like [`text_in_box`], additionally reporting whether any ink was
/// clipped because the text does not fit the box.
pub fn text_in_box_checked(
    text: &str,
    face: &Face,
    box_w: u32,
    box_h: u16,
    size_px: Option<f32>,
    align: TextAlign,
) -> Result<(Bitmap, bool), RenderError> {
    let mut bitmap = Bitmap::new(box_h, box_w);
    let mut clipped = false;
    if text.trim().is_empty() || box_w == 0 || box_h == 0 {
        return Ok((bitmap, clipped));
    }
    let font = &face.font;
    let px = size_px.unwrap_or_else(|| auto_font_px(font, text, Some(box_w as f32), box_h as f32));
    // Synthetic italic: shear glyph pixels right by this fraction of their
    // height above the baseline. Synthetic bold: one extra dot of stroke
    // width per this many dots of font size (at least one).
    let shear = if face.synthetic_italic {
        SYNTHETIC_ITALIC_SLANT
    } else {
        0.0
    };
    let embolden = if face.synthetic_bold {
        ((px / SYNTHETIC_BOLD_PX_PER_DOT).round() as i32).max(1)
    } else {
        0
    };

    let layout = layout(
        font,
        text,
        px,
        &settings(Some(box_w as f32), Some(box_h as f32), align),
    );

    let baselines: Vec<f32> = layout
        .lines()
        .map(|lines| lines.iter().map(|l| l.baseline_y).collect())
        .unwrap_or_default();
    for glyph in layout.glyphs() {
        if glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        let baseline = baselines
            .iter()
            .copied()
            .find(|b| *b >= glyph.y)
            .unwrap_or(glyph.y + glyph.height as f32);
        let (_, coverage) = font.rasterize_config(glyph.key);
        for gy in 0..glyph.height {
            let pin = glyph.y.round() as i32 + gy as i32;
            let slant = ((baseline - pin as f32) * shear).round() as i32;
            for gx in 0..glyph.width {
                if coverage[gy * glyph.width + gx] < INK_THRESHOLD {
                    continue;
                }
                let base = glyph.x.round() as i64 + gx as i64;
                // Overflow is judged on the plain glyph (not the synthetic
                // slant/bold), with one dot of rounding tolerance.
                let tol = CLIP_TOLERANCE_DOTS;
                if base < -tol
                    || base >= box_w as i64 + tol
                    || i64::from(pin) < -tol
                    || i64::from(pin) >= i64::from(box_h) + tol
                {
                    clipped = true;
                }
                let line = base as i32 + slant;
                for extra in 0..=embolden {
                    if line + extra >= 0 && pin >= 0 {
                        bitmap.set_pixel(pin as u16, (line + extra) as u32, true);
                    }
                }
            }
        }
    }
    Ok((bitmap, clipped))
}

/// Copies `src` (box-local, possibly shorter) into a fresh `box_w` x
/// `box_h` bitmap, centered along the length axis.
fn center_in_box(src: &Bitmap, box_w: u32, box_h: u16) -> Bitmap {
    let mut out = Bitmap::new(box_h, box_w);
    let offset = (box_w as i32 - src.height_dots() as i32) / 2;
    out.blit(src, 0, offset, 0..box_h);
    out
}

/// Renders a QR code as large as fits the box (whole dots per module),
/// centered.
pub fn qr_in_box(
    data: &str,
    box_w: u32,
    box_h: u16,
    ec_level: QrErrorCorrection,
) -> Result<Bitmap, RenderError> {
    let side = (box_w.min(box_h as u32)) as u16;
    let offset = (box_h - side) / 2;
    let qr = render_qr(data, box_h, side, offset, ec_level)?;
    Ok(center_in_box(&qr, box_w, box_h))
}

/// Renders a linear barcode filling the box height. The module width is
/// the largest whole number of dots that fits `box_w` (at least 1; a box
/// narrower than the code is clipped), centered.
pub fn barcode_in_box(
    symbology: Symbology,
    data: &str,
    box_w: u32,
    box_h: u16,
) -> Result<Bitmap, RenderError> {
    let modules = encode_modules(symbology, data)?;
    let module_px = (box_w / modules.len().max(1) as u32).max(1);
    let mut code = Bitmap::new(box_h, modules.len() as u32 * module_px);
    for (i, &m) in modules.iter().enumerate() {
        if m == 0 {
            continue;
        }
        for s in 0..module_px {
            for pin in 0..box_h {
                code.set_pixel(pin, i as u32 * module_px + s, true);
            }
        }
    }
    Ok(center_in_box(&code, box_w, box_h))
}

/// Renders the image at `path` scaled to fit the box (aspect ratio kept),
/// Floyd-Steinberg dithered, centered.
pub fn image_in_box(
    path: &Path,
    box_w: u32,
    box_h: u16,
    invert: bool,
) -> Result<Bitmap, RenderError> {
    gray_in_box(&load_gray(path, box_h)?, box_w, box_h, invert)
}

/// Renders a bundled symbol (see [`crate::symbols`]) fitted into the box
/// like [`image_in_box`].
pub fn symbol_in_box(
    name: &str,
    box_w: u32,
    box_h: u16,
    invert: bool,
) -> Result<Bitmap, RenderError> {
    let data = crate::symbols::symbol_svg(name).ok_or_else(|| {
        RenderError::Image(format!(
            "unknown symbol '{name}', available: {}",
            crate::SYMBOL_NAMES.join(", ")
        ))
    })?;
    gray_in_box(&render_svg_to_gray(data, box_h)?, box_w, box_h, invert)
}

fn gray_in_box(
    gray: &image::GrayImage,
    box_w: u32,
    box_h: u16,
    invert: bool,
) -> Result<Bitmap, RenderError> {
    let (src_w, src_h) = gray.dimensions();
    let mut out = Bitmap::new(box_h, box_w);
    if src_w == 0 || src_h == 0 || box_w == 0 || box_h == 0 {
        return Ok(out);
    }
    let scale = (box_w as f32 / src_w as f32).min(box_h as f32 / src_h as f32);
    let new_w = ((src_w as f32 * scale).round() as u32).clamp(1, box_w);
    let new_h = ((src_h as f32 * scale).round() as u32).clamp(1, box_h as u32);
    let resized =
        image::imageops::resize(gray, new_w, new_h, image::imageops::FilterType::Triangle);
    let bits = floyd_steinberg_dither(&resized, invert);

    let line_off = (box_w - new_w) / 2;
    let pin_off = (box_h as u32 - new_h) / 2;
    for y in 0..new_h {
        for x in 0..new_w {
            if bits[(y * new_w + x) as usize] {
                out.set_pixel((pin_off + y) as u16, line_off + x, true);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! require_font {
        () => {
            match Face::default_face() {
                Ok(face) => face,
                Err(_) => {
                    eprintln!("skip: no system font found, see ll_render::fontsrc");
                    return;
                }
            }
        };
    }

    /// (first, last) pin with ink, or `None` if blank.
    fn pin_extent(bmp: &Bitmap) -> Option<(u16, u16)> {
        let pins: Vec<u16> = (0..bmp.width_pins())
            .filter(|&p| (0..bmp.height_dots()).any(|l| bmp.pixel(p, l)))
            .collect();
        Some((*pins.first()?, *pins.last()?))
    }

    /// (first, last) line with ink.
    fn line_extent(bmp: &Bitmap) -> Option<(u32, u32)> {
        let lines: Vec<u32> = (0..bmp.height_dots())
            .filter(|&l| (0..bmp.width_pins()).any(|p| bmp.pixel(p, l)))
            .collect();
        Some((*lines.first()?, *lines.last()?))
    }

    #[test]
    fn text_box_has_box_dimensions_and_ink() {
        let font = require_font!();
        let bmp = text_in_box("Hallo", &font, 200, 60, None, TextAlign::Center).unwrap();
        assert_eq!((bmp.width_pins(), bmp.height_dots()), (60, 200));
        assert!(pin_extent(&bmp).is_some());
    }

    #[test]
    fn reports_clipped_text() {
        let font = require_font!();
        let fits = text_in_box_checked("Hi", &font, 300, 60, Some(20.0), TextAlign::Left);
        assert!(!fits.unwrap().1);
        let auto = text_in_box_checked("Hallo Welt", &font, 300, 60, None, TextAlign::Left);
        assert!(!auto.unwrap().1);
        // 80 px text in a 40-dot-high box cannot fit.
        let tall = text_in_box_checked("Hallo", &font, 300, 40, Some(80.0), TextAlign::Left);
        assert!(tall.unwrap().1);
    }

    #[test]
    fn bigger_font_size_uses_more_height() {
        let font = require_font!();
        let small = text_in_box("H", &font, 300, 100, Some(20.0), TextAlign::Left).unwrap();
        let large = text_in_box("H", &font, 300, 100, Some(60.0), TextAlign::Left).unwrap();
        let h = |b: &Bitmap| pin_extent(b).map(|(a, z)| z - a).unwrap();
        assert!(h(&large) > 2 * h(&small));
    }

    #[test]
    fn two_lines_stack_vertically() {
        let font = require_font!();
        let one = text_in_box("AB", &font, 300, 100, Some(30.0), TextAlign::Left).unwrap();
        let two = text_in_box("AB\nAB", &font, 300, 100, Some(30.0), TextAlign::Left).unwrap();
        let h = |b: &Bitmap| pin_extent(b).map(|(a, z)| z - a).unwrap();
        assert!(h(&two) > h(&one) + 20);
    }

    #[test]
    fn alignment_moves_text_along_the_box() {
        let font = require_font!();
        let left = text_in_box("i", &font, 300, 60, Some(30.0), TextAlign::Left).unwrap();
        let right = text_in_box("i", &font, 300, 60, Some(30.0), TextAlign::Right).unwrap();
        assert!(line_extent(&left).unwrap().0 < 30);
        assert!(line_extent(&right).unwrap().1 > 270);
    }

    #[test]
    fn auto_size_fits_inside_box() {
        let font = require_font!();
        let bmp = text_in_box("Ein langer Text", &font, 120, 60, None, TextAlign::Center).unwrap();
        let (_, last) = line_extent(&bmp).unwrap();
        assert!(last < 120);
    }

    #[test]
    fn auto_size_never_wraps_or_clips() {
        let font = require_font!();
        // Box exactly as wide as the natural text: must stay one line and
        // keep ink away from the top/bottom edges.
        let w = text_natural_width("LabelLab", &font, 70, None).unwrap();
        let bmp = text_in_box("LabelLab", &font, w, 70, None, TextAlign::Center).unwrap();
        let (top, bottom) = pin_extent(&bmp).unwrap();
        assert!(top > 0 && bottom < 69, "clipped: {top}..{bottom}");
        let single = text_in_box("LabelLab", &font, w * 3, 70, None, TextAlign::Center).unwrap();
        let h = |b: &Bitmap| pin_extent(b).map(|(a, z)| z - a).unwrap();
        assert!(h(&bmp) <= h(&single) + 2, "wrapped onto a second line");
    }

    #[test]
    fn qr_is_centered_square() {
        let bmp = qr_in_box("hello", 200, 60, QrErrorCorrection::Medium).unwrap();
        let (l0, l1) = line_extent(&bmp).unwrap();
        assert!(l0 > 60 && l1 < 140, "centered along the length: {l0}..{l1}");
    }

    #[test]
    fn symbol_fits_box_and_unknown_errors() {
        let bmp = symbol_in_box("warning", 200, 60, false).unwrap();
        assert_eq!((bmp.width_pins(), bmp.height_dots()), (60, 200));
        assert!(pin_extent(&bmp).is_some());
        assert!(symbol_in_box("nope", 10, 10, false).is_err());
    }

    #[test]
    fn barcode_scales_module_width_to_box() {
        let narrow = barcode_in_box(Symbology::Code128, "AB", 100, 40).unwrap();
        let wide = barcode_in_box(Symbology::Code128, "AB", 400, 40).unwrap();
        let len = |b: &Bitmap| line_extent(b).map(|(a, z)| z - a).unwrap();
        assert!(len(&wide) >= 3 * len(&narrow));
        assert_eq!(pin_extent(&wide), Some((0, 39)), "bars fill the box height");
    }
}
