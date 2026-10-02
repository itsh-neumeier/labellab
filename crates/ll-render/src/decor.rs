//! Decorative frames built from three SVG segments: a start piece, a
//! middle piece repeated along the label and an end piece (like the
//! segment frames of other label editors). Each segment is drawn at the
//! full printable height; the middle pieces are stretched slightly so a
//! whole number of them fills the space between start and end.
//!
//! Frames come in sets (`.llabel-frames` JSON). The built-in set is drawn
//! for LabelLab (MIT); users add their own sets or frames.

use std::sync::{Arc, OnceLock, RwLock};

use image::imageops::{self, FilterType};
use image::GrayImage;
use serde::{Deserialize, Serialize};

use crate::iconset::Text;
use crate::picture::render_svg_to_gray;
use crate::{Bitmap, RenderError};

/// `format` value of a frame set file.
pub const FRAMES_FORMAT: &str = "labellab-frames";
/// File extension of frame set files.
pub const FRAMES_EXTENSION: &str = "llabel-frames";

/// Gray level below which a rasterized pixel is ink.
const INK_THRESHOLD: u8 = 128;

const BUILTIN_FILES: &[&str] = &[include_str!("../assets/frames/basis.llabel-frames")];

/// One frame: three SVG segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameDef {
    /// Id within the set: lowercase letters, digits, `-`, `_`.
    pub id: String,
    pub name: Text,
    /// Left end (label start).
    pub start: String,
    /// Repeated along the label.
    pub middle: String,
    /// Right end (label end).
    pub end: String,
}

/// A set of frames (one `.llabel-frames` file).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameSet {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub name: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default)]
    pub frames: Vec<FrameDef>,
}

fn invalid(msg: impl Into<String>) -> RenderError {
    RenderError::Image(format!("invalid frame set: {}", msg.into()))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

impl FrameSet {
    /// Parses and checks a frame set (format, ids, every SVG parses).
    pub fn from_json(data: &[u8]) -> Result<Self, RenderError> {
        let set: FrameSet = serde_json::from_slice(data).map_err(|e| invalid(e.to_string()))?;
        set.check()?;
        Ok(set)
    }

    fn check(&self) -> Result<(), RenderError> {
        if self.format != FRAMES_FORMAT {
            return Err(invalid(format!("format must be {FRAMES_FORMAT:?}")));
        }
        if !valid_id(&self.id) {
            return Err(invalid(format!("bad set id {:?}", self.id)));
        }
        for f in &self.frames {
            if !valid_id(&f.id) {
                return Err(invalid(format!("bad frame id {:?}", f.id)));
            }
            for svg in [&f.start, &f.middle, &f.end] {
                usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default())
                    .map_err(|e| invalid(format!("frame {:?}: {e}", f.id)))?;
            }
        }
        Ok(())
    }
}

struct Registry {
    builtin: Vec<Arc<FrameSet>>,
    user: Vec<Arc<FrameSet>>,
}

fn registry() -> &'static RwLock<Registry> {
    static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        RwLock::new(Registry {
            builtin: BUILTIN_FILES
                .iter()
                .filter_map(|json| FrameSet::from_json(json.as_bytes()).ok())
                .map(Arc::new)
                .collect(),
            user: Vec::new(),
        })
    })
}

/// Whether `id` belongs to a built-in set.
pub fn is_builtin(id: &str) -> bool {
    let reg = registry().read().unwrap_or_else(|e| e.into_inner());
    reg.builtin.iter().any(|s| s.id == id)
}

/// Adds or replaces a user set.
pub fn register(set: FrameSet) -> Result<(), RenderError> {
    set.check()?;
    let mut reg = registry().write().unwrap_or_else(|e| e.into_inner());
    if reg.builtin.iter().any(|s| s.id == set.id) {
        return Err(invalid(format!("set id {:?} is built in", set.id)));
    }
    reg.user.retain(|s| s.id != set.id);
    reg.user.push(Arc::new(set));
    Ok(())
}

/// Removes a user set; true if it was registered.
pub fn unregister(id: &str) -> bool {
    let mut reg = registry().write().unwrap_or_else(|e| e.into_inner());
    let before = reg.user.len();
    reg.user.retain(|s| s.id != id);
    before != reg.user.len()
}

/// All sets, built-in first.
pub fn sets() -> Vec<Arc<FrameSet>> {
    let reg = registry().read().unwrap_or_else(|e| e.into_inner());
    reg.builtin.iter().chain(reg.user.iter()).cloned().collect()
}

/// Looks up `set:frame`.
pub fn resolve(name: &str) -> Option<FrameDef> {
    let (set_id, frame_id) = name.split_once(':')?;
    sets()
        .into_iter()
        .find(|s| s.id == set_id)?
        .frames
        .iter()
        .find(|f| f.id == frame_id)
        .cloned()
}

/// Width in dots of one segment drawn `height` dots high.
fn segment_width(svg: &str, height: u16) -> Result<u32, RenderError> {
    let tree = usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default())
        .map_err(|e| RenderError::Image(e.to_string()))?;
    let size = tree.size();
    if size.height() <= 0.0 {
        return Ok(0);
    }
    Ok((size.width() / size.height() * height as f32).round() as u32)
}

/// Widths in dots of the start and end pieces at `height` dots (the space
/// they take from the label ends).
pub fn end_widths(frame: &FrameDef, height: u16) -> Result<(u32, u32), RenderError> {
    Ok((
        segment_width(&frame.start, height)?,
        segment_width(&frame.end, height)?,
    ))
}

/// ORs a grayscale segment into `out` at line `x`.
fn blit_gray(out: &mut Bitmap, gray: &GrayImage, x: u32) {
    for (gx, gy, p) in gray.enumerate_pixels() {
        if p.0[0] < INK_THRESHOLD && gy <= u16::MAX as u32 {
            let line = x + gx;
            if line < out.height_dots() {
                out.set_pixel(gy as u16, line, true);
            }
        }
    }
}

/// Renders `frame` as a `length` x `height` dot bitmap (box-local:
/// `width_pins` = `height`, `height_dots` = `length`).
pub fn render_frame(frame: &FrameDef, length: u32, height: u16) -> Result<Bitmap, RenderError> {
    let mut out = Bitmap::new(height, length);
    if length == 0 || height == 0 {
        return Ok(out);
    }
    let start = render_svg_to_gray(frame.start.as_bytes(), height)?;
    let end = render_svg_to_gray(frame.end.as_bytes(), height)?;
    let middle = render_svg_to_gray(frame.middle.as_bytes(), height)?;
    let (sw, ew) = (start.width(), end.width());
    blit_gray(&mut out, &start, 0);
    blit_gray(&mut out, &end, length.saturating_sub(ew));
    let avail = length.saturating_sub(sw + ew);
    if avail > 0 && middle.width() > 0 {
        let count = ((avail as f32 / middle.width() as f32).round() as u32).max(1);
        let tile = avail as f32 / count as f32;
        for k in 0..count {
            let x0 = sw + (k as f32 * tile).round() as u32;
            let x1 = sw + ((k + 1) as f32 * tile).round() as u32;
            if x1 > x0 {
                let piece = imageops::resize(&middle, x1 - x0, height as u32, FilterType::Triangle);
                blit_gray(&mut out, &piece, x0);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_set_parses_and_resolves() {
        let all = sets();
        let basis = all.iter().find(|s| s.id == "basis").unwrap();
        assert!(basis.frames.len() >= 8);
        assert!(resolve("basis:brace").is_some());
        assert!(resolve("basis:nope").is_none());
        assert!(is_builtin("basis"));
    }

    #[test]
    fn frame_fills_the_length_with_ends_in_place() {
        let frame = resolve("basis:arrow").unwrap();
        let (h, len) = (70u16, 600u32);
        let b = render_frame(&frame, len, h).unwrap();
        assert_eq!(b.height_dots(), len);
        // Top band (middle pieces) is inked across the middle of the label.
        let top = (100..500)
            .filter(|&l| (0..8).any(|p| b.pixel(p, l)))
            .count();
        assert!(top > 380, "top band coverage {top}");
        // The end piece (arrow tip) reaches near the label end at mid height.
        let tip = (len - 30..len).any(|l| (h / 2 - 3..h / 2 + 3).any(|p| b.pixel(p, l)));
        assert!(tip);
        let (sw, ew) = end_widths(&frame, h).unwrap();
        assert!(sw > 0 && ew > sw);
    }

    #[test]
    fn raster_segments_from_the_paint_editor_render() {
        // A 4x10 black PNG embedded the way the frame editor stores drawings.
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAKCAYAAACT+/8OAAAAEUlEQVR4nGNgYGD4j4YHpwAAAGEn2cKAeeIAAAAASUVORK5CYII=";
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="10" viewBox="0 0 4 10"><image width="4" height="10" image-rendering="optimizeSpeed" href="data:image/png;base64,{png}"/></svg>"#
        );
        let normalized = crate::iconset::normalize_svg(svg.as_bytes()).unwrap();
        let frame = FrameDef {
            id: "r".into(),
            name: Text::Plain("R".into()),
            start: normalized.clone(),
            middle: normalized.clone(),
            end: normalized,
        };
        let b = render_frame(&frame, 200, 50).unwrap();
        assert!(b.pixel(25, 2) && b.pixel(25, 100) && b.pixel(25, 197));
    }

    #[test]
    fn rejects_bad_sets() {
        let bad = br#"{"format":"x","version":1,"id":"a","name":"A","frames":[]}"#;
        assert!(FrameSet::from_json(bad).is_err());
        let bad_svg = br#"{"format":"labellab-frames","version":1,"id":"a","name":"A",
            "frames":[{"id":"f","name":"F","start":"<nope","middle":"<svg/>","end":"<svg/>"}]}"#;
        assert!(FrameSet::from_json(bad_svg).is_err());
    }
}
