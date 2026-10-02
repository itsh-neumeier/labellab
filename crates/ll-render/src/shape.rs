//! Simple vector shapes (line, rectangle, rounded rectangle, ellipse) drawn
//! into a box, as outline or filled, with a hard threshold so edges stay
//! crisp on 1-bit tape.

use serde::{Deserialize, Serialize};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::{Bitmap, RenderError};

/// Which shape to draw.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    /// Straight line through the middle of the box, along the label.
    #[default]
    Line,
    Rectangle,
    RoundedRectangle,
    Ellipse,
}

/// Corner radius of a rounded rectangle as a fraction of the shorter side.
const ROUNDED_CORNER: f32 = 0.25;

/// Draws `kind` into a `box_w` (along the label) x `box_h` (across the
/// tape) dot box. `stroke` is the line width in dots; `filled` fills the
/// shape (ignored for lines).
pub fn shape_in_box(
    kind: ShapeKind,
    box_w: u32,
    box_h: u16,
    stroke: f32,
    filled: bool,
) -> Result<Bitmap, RenderError> {
    let mut out = Bitmap::new(box_h, box_w);
    if box_w == 0 || box_h == 0 {
        return Ok(out);
    }
    let mut pixmap = Pixmap::new(box_w, box_h as u32)
        .ok_or_else(|| RenderError::Image(format!("invalid shape size {box_w}x{box_h}")))?;
    let (w, h) = (box_w as f32, box_h as f32);
    let stroke = stroke.max(1.0).min(w.min(h));
    // Outlines are inset by half the line width so they stay inside the box.
    let inset = if filled && kind != ShapeKind::Line {
        0.0
    } else {
        stroke / 2.0
    };
    let path = match kind {
        ShapeKind::Line => {
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, h / 2.0);
            pb.line_to(w, h / 2.0);
            pb.finish()
        }
        ShapeKind::Rectangle => {
            Rect::from_ltrb(inset, inset, w - inset, h - inset).map(PathBuilder::from_rect)
        }
        ShapeKind::RoundedRectangle => {
            let r = (w.min(h) * ROUNDED_CORNER).max(stroke);
            rounded_rect(inset, inset, w - inset, h - inset, r)
        }
        ShapeKind::Ellipse => {
            Rect::from_ltrb(inset, inset, w - inset, h - inset).and_then(PathBuilder::from_oval)
        }
    };
    let Some(path) = path else {
        return Ok(out); // degenerate box
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    if filled && kind != ShapeKind::Line {
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    } else {
        let line = Stroke {
            width: stroke,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &line, Transform::identity(), None);
    }
    for (i, px) in pixmap.pixels().iter().enumerate() {
        if px.alpha() >= 128 {
            out.set_pixel((i as u32 / box_w) as u16, i as u32 % box_w, true);
        }
    }
    Ok(out)
}

fn rounded_rect(l: f32, t: f32, r: f32, b: f32, radius: f32) -> Option<tiny_skia::Path> {
    let radius = radius.min((r - l) / 2.0).min((b - t) / 2.0).max(0.0);
    // Cubic approximation of a quarter circle.
    let k = radius * 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(l + radius, t);
    pb.line_to(r - radius, t);
    pb.cubic_to(r - radius + k, t, r, t + radius - k, r, t + radius);
    pb.line_to(r, b - radius);
    pb.cubic_to(r, b - radius + k, r - radius + k, b, r - radius, b);
    pb.line_to(l + radius, b);
    pb.cubic_to(l + radius - k, b, l, b - radius + k, l, b - radius);
    pb.line_to(l, t + radius);
    pb.cubic_to(l, t + radius - k, l + radius - k, t, l + radius, t);
    pb.close();
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(b: &Bitmap) -> usize {
        (0..b.height_dots())
            .map(|l| (0..b.width_pins()).filter(|&p| b.pixel(p, l)).count())
            .sum()
    }

    #[test]
    fn outline_has_hollow_middle_and_filled_does_not() {
        let rect = shape_in_box(ShapeKind::Rectangle, 100, 50, 3.0, false).unwrap();
        assert!(rect.pixel(0, 50) && !rect.pixel(25, 50));
        let filled = shape_in_box(ShapeKind::Rectangle, 100, 50, 3.0, true).unwrap();
        assert!(filled.pixel(25, 50));
        assert_eq!(ink(&filled), 100 * 50);
    }

    #[test]
    fn line_runs_through_the_middle() {
        let line = shape_in_box(ShapeKind::Line, 100, 50, 4.0, false).unwrap();
        assert!(line.pixel(25, 5) && line.pixel(25, 95));
        assert!(!line.pixel(5, 50));
    }

    #[test]
    fn ellipse_and_rounded_leave_corners_blank() {
        for kind in [ShapeKind::Ellipse, ShapeKind::RoundedRectangle] {
            let s = shape_in_box(kind, 100, 50, 2.0, true).unwrap();
            assert!(!s.pixel(0, 0), "{kind:?} corner");
            assert!(s.pixel(25, 50), "{kind:?} center");
        }
    }
}
