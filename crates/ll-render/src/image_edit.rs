//! Non-destructive edits for imported images: rotate/flip, crop, remove
//! the background (make it transparent = not printed), and choose how the
//! result becomes print dots. The source file is never changed; the edit
//! is stored with the label and applied on every render (preview = print).
//!
//! Order: rotate/flip → crop → background removal → composite on white →
//! gray. Crop coordinates therefore refer to the rotated/flipped image, as
//! shown in the editor.

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use image::{imageops, GrayImage, Luma, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::iconset::Halftone;
use crate::RenderError;

/// Crop rectangle as fractions (0..1) of the rotated/flipped image.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Which pixels become transparent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BackgroundRemoval {
    /// Background color; `None` = detected from the image border.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// 0..100: how different a color may be and still count as background.
    #[serde(default = "default_tolerance")]
    pub tolerance: u8,
    /// Only remove background connected to the image border (keeps inner
    /// areas of the same color, e.g. white inside a logo).
    #[serde(default = "default_true")]
    pub contiguous: bool,
}

fn default_tolerance() -> u8 {
    20
}

fn default_true() -> bool {
    true
}

/// All edits of an image element. The default changes nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageEdit {
    /// Clockwise quarter turns in degrees: 0, 90, 180, 270.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: u16,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip_h: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip_v: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<CropRect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<BackgroundRemoval>,
    /// `None` = dithering (photos); `threshold` for logos/line art.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halftone: Option<Halftone>,
    /// Gray level below which a pixel is ink with `threshold` (1..254).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<u8>,
}

fn is_zero(v: &u16) -> bool {
    *v == 0
}

impl ImageEdit {
    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }

    /// The halftone mode to use for this image (images dither by default).
    pub fn halftone(&self) -> Halftone {
        self.halftone.unwrap_or(Halftone::Dither)
    }
}

/// Loads an image as RGBA: PNG/JPEG/BMP via `image`, SVG rasterized
/// `svg_height_px` tall with a transparent background.
pub fn load_rgba(path: &Path, svg_height_px: u16) -> Result<RgbaImage, RenderError> {
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    if !is_svg {
        return Ok(image::open(path)
            .map_err(|e| RenderError::Image(e.to_string()))?
            .to_rgba8());
    }
    let tree = usvg::Tree::from_data(&std::fs::read(path)?, &usvg::Options::default())
        .map_err(|e| RenderError::Image(e.to_string()))?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 || svg_height_px == 0 {
        return Ok(RgbaImage::new(0, 0));
    }
    let scale = svg_height_px as f32 / size.height();
    let w = (size.width() * scale).round().max(1.0) as u32;
    let h = svg_height_px as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| RenderError::Image(format!("invalid SVG raster size {w}x{h}")))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut out = RgbaImage::new(w, h);
    for (i, px) in pixmap.pixels().iter().enumerate() {
        // tiny-skia stores premultiplied alpha.
        let c = px.demultiply();
        out.put_pixel(
            i as u32 % w,
            i as u32 / w,
            Rgba([c.red(), c.green(), c.blue(), c.alpha()]),
        );
    }
    Ok(out)
}

/// Applies rotation, flips and background removal (not the crop), for the
/// editor's view of the image.
pub fn transform_and_mask(img: RgbaImage, edit: &ImageEdit) -> RgbaImage {
    let mut img = match (edit.rotation / 90) % 4 {
        1 => imageops::rotate90(&img),
        2 => imageops::rotate180(&img),
        3 => imageops::rotate270(&img),
        _ => img,
    };
    if edit.flip_h {
        imageops::flip_horizontal_in_place(&mut img);
    }
    if edit.flip_v {
        imageops::flip_vertical_in_place(&mut img);
    }
    if let Some(bg) = &edit.background {
        remove_background(&mut img, bg);
    }
    img
}

/// Applies the whole edit (including the crop).
pub fn apply(img: RgbaImage, edit: &ImageEdit) -> RgbaImage {
    if edit.is_identity() {
        return img;
    }
    // Crop before background removal so "connected to the border" means
    // the border of the visible (cropped) image.
    let mut rotated = transform_and_mask(
        img,
        &ImageEdit {
            background: None,
            ..*edit
        },
    );
    if let Some(c) = edit.crop {
        rotated = crop(&rotated, c);
    }
    if let Some(bg) = &edit.background {
        remove_background(&mut rotated, bg);
    }
    rotated
}

fn crop(img: &RgbaImage, c: CropRect) -> RgbaImage {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let x0 = (c.x.clamp(0.0, 1.0) * w).round() as u32;
    let y0 = (c.y.clamp(0.0, 1.0) * h).round() as u32;
    let x1 = ((c.x + c.w).clamp(0.0, 1.0) * w).round() as u32;
    let y1 = ((c.y + c.h).clamp(0.0, 1.0) * h).round() as u32;
    if x1 <= x0 || y1 <= y0 {
        return img.clone();
    }
    imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image()
}

/// Most common border color (quantized), averaged within its bucket.
fn border_color(img: &RgbaImage) -> [u8; 3] {
    let (w, h) = img.dimensions();
    let mut buckets: HashMap<(u8, u8, u8), (u32, [u32; 3])> = HashMap::new();
    let mut add = |x: u32, y: u32| {
        let Rgba([r, g, b, a]) = *img.get_pixel(x, y);
        if a < 128 {
            return; // already transparent
        }
        let e = buckets.entry((r >> 4, g >> 4, b >> 4)).or_default();
        e.0 += 1;
        e.1[0] += r as u32;
        e.1[1] += g as u32;
        e.1[2] += b as u32;
    };
    for x in 0..w {
        add(x, 0);
        add(x, h.saturating_sub(1));
    }
    for y in 0..h {
        add(0, y);
        add(w.saturating_sub(1), y);
    }
    buckets
        .values()
        .max_by_key(|(n, _)| *n)
        .map(|(n, sum)| sum.map(|s| (s / n) as u8))
        .unwrap_or([255, 255, 255])
}

/// Makes background pixels fully transparent.
pub fn remove_background(img: &mut RgbaImage, bg: &BackgroundRemoval) {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let target = bg.color.unwrap_or_else(|| border_color(img));
    // tolerance 0..100 → RGB distance 0..~441.
    let max_dist = bg.tolerance.min(100) as f32 * 4.42;
    let matches = |p: &Rgba<u8>| {
        if p[3] < 128 {
            return true;
        }
        let d: f32 = (0..3)
            .map(|i| (p[i] as f32 - target[i] as f32).powi(2))
            .sum::<f32>()
            .sqrt();
        d <= max_dist
    };
    if !bg.contiguous {
        for p in img.pixels_mut() {
            if matches(p) {
                p[3] = 0;
            }
        }
        return;
    }
    let mut seen = vec![false; (w * h) as usize];
    let mut queue = VecDeque::new();
    let seed = |x: u32, y: u32, seen: &mut Vec<bool>, queue: &mut VecDeque<(u32, u32)>| {
        let i = (y * w + x) as usize;
        if !seen[i] {
            seen[i] = true;
            queue.push_back((x, y));
        }
    };
    for x in 0..w {
        seed(x, 0, &mut seen, &mut queue);
        seed(x, h - 1, &mut seen, &mut queue);
    }
    for y in 0..h {
        seed(0, y, &mut seen, &mut queue);
        seed(w - 1, y, &mut seen, &mut queue);
    }
    while let Some((x, y)) = queue.pop_front() {
        let p = img.get_pixel_mut(x, y);
        if !matches(p) {
            continue;
        }
        p[3] = 0;
        if x > 0 {
            seed(x - 1, y, &mut seen, &mut queue);
        }
        if x + 1 < w {
            seed(x + 1, y, &mut seen, &mut queue);
        }
        if y > 0 {
            seed(x, y - 1, &mut seen, &mut queue);
        }
        if y + 1 < h {
            seed(x, y + 1, &mut seen, &mut queue);
        }
    }
}

/// Composites on white (transparent = not printed) and converts to gray.
pub fn to_gray_on_white(img: &RgbaImage) -> GrayImage {
    let mut out = GrayImage::new(img.width(), img.height());
    for (x, y, Rgba([r, g, b, a])) in img.enumerate_pixels() {
        let a = *a as u32;
        let mix = |c: u8| (c as u32 * a + 255 * (255 - a)) / 255;
        let luma = (mix(*r) * 299 + mix(*g) * 587 + mix(*b) * 114) / 1000;
        out.put_pixel(x, y, Luma([luma as u8]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10x10 white image with a black 4x4 square in the middle and a
    /// white hole in that square.
    fn sample() -> RgbaImage {
        RgbaImage::from_fn(10, 10, |x, y| {
            let inside = (3..7).contains(&x) && (3..7).contains(&y);
            let hole = x == 5 && y == 5;
            if inside && !hole {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        })
    }

    #[test]
    fn contiguous_removal_keeps_inner_holes() {
        let mut img = sample();
        remove_background(
            &mut img,
            &BackgroundRemoval {
                color: None,
                tolerance: 10,
                contiguous: true,
            },
        );
        assert_eq!(img.get_pixel(0, 0)[3], 0, "outside removed");
        assert_eq!(img.get_pixel(4, 4)[3], 255, "black kept");
        assert_eq!(img.get_pixel(5, 5)[3], 255, "inner white kept");

        let mut all = sample();
        remove_background(
            &mut all,
            &BackgroundRemoval {
                color: Some([255, 255, 255]),
                tolerance: 10,
                contiguous: false,
            },
        );
        assert_eq!(all.get_pixel(5, 5)[3], 0, "inner white removed too");
    }

    #[test]
    fn crop_and_rotate() {
        let edit = ImageEdit {
            rotation: 90,
            crop: Some(CropRect {
                x: 0.3,
                y: 0.3,
                w: 0.4,
                h: 0.4,
            }),
            ..ImageEdit::default()
        };
        let out = apply(sample(), &edit);
        assert_eq!(out.dimensions(), (4, 4));
        assert_eq!(out.get_pixel(0, 0)[0], 0, "only the black square remains");
    }

    #[test]
    fn transparency_composites_to_white() {
        let img = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 0]));
        let gray = to_gray_on_white(&img);
        assert_eq!(gray.get_pixel(0, 0)[0], 255);
    }

    #[test]
    fn edit_round_trips_as_json_and_default_is_empty() {
        assert_eq!(serde_json::to_string(&ImageEdit::default()).unwrap(), "{}");
        let edit = ImageEdit {
            flip_h: true,
            background: Some(BackgroundRemoval {
                color: Some([10, 20, 30]),
                tolerance: 15,
                contiguous: false,
            }),
            halftone: Some(Halftone::Threshold),
            threshold: Some(100),
            ..ImageEdit::default()
        };
        let json = serde_json::to_string(&edit).unwrap();
        assert_eq!(serde_json::from_str::<ImageEdit>(&json).unwrap(), edit);
    }
}
