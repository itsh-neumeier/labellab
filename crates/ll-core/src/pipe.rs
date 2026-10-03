//! Pipe marker per DIN 2403: an arrow along the tape with the medium's
//! name, an optional second line, the flow direction and up to three hazard
//! pictograms. The printer prints one colour on the tape colour, so the
//! arrow shape is left free (it shows the tape colour, e.g. green for
//! water) and everything around it is printed (border in the ink colour).
//! The additional colour of some groups goes into the arrow tips: solid
//! (black ink) or hatched (stands for a colour the ink cannot show, red).

use ll_render::boxed::{self, TextLayout};
use ll_render::fonts::FaceSet;
use ll_render::{Bitmap, RenderError, TextAlign, VAlign};
use serde::{Deserialize, Serialize};

/// Most hazard pictograms on one marker.
pub const MAX_SYMBOLS: usize = 3;
/// Border between arrow and tape edge, as a share of the height.
const MARGIN_SHARE: f32 = 0.06;
/// Separator line between tip and body, as a share of the height.
const LINE_SHARE: f32 = 0.05;
/// Tip length as a share of the height (DIN look: about 0.75).
const TIP_SHARE: f32 = 0.75;
/// Share of the text height for the main line when there is a second line.
const MAIN_LINE_SHARE: f32 = 0.62;
/// Hatching period in multiples of the line width.
const HATCH_PERIOD: u32 = 3;

/// Flow direction: where the arrow tips are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipeDirection {
    #[default]
    Right,
    Left,
    Both,
}

/// Additional colour in the tips (DIN 2403 groups 4, 5, 8, 9).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipFill {
    #[default]
    None,
    /// Filled with ink (black additional colour on black ink).
    Solid,
    /// Hatched: an additional colour the ink cannot show (red).
    Hatched,
}

/// How the arrow is set off from the tape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipeFrame {
    /// Everything outside the arrow printed (DIN look).
    #[default]
    Filled,
    /// Only the arrow's outline printed.
    Outline,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// Content of a pipe marker element (see [`crate::label::Element::PipeMarker`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PipeMarker {
    /// Medium, e.g. "Trinkwasser" (placeholders allowed).
    pub text: String,
    /// Second line, e.g. "80 °C · PN 10".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sub_text: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub direction: PipeDirection,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tips: TipFill,
    #[serde(default, skip_serializing_if = "is_default")]
    pub frame: PipeFrame,
    /// Hazard pictograms as symbol names (`ghs:GHS02`), left in the body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<String>,
    /// DIN 2403 substance group 0–9 (editor hint for colours).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u8>,
    /// Own colours for coloured output (preview, A4 on a colour printer,
    /// PNG export). The label printer prints the cassette's colours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colors: Option<PipeColors>,
}

/// CSS colours (`#rrggbb`) of a pipe marker; unset = the group's colours.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipeColors {
    /// Arrow (substance) colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Text and border colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink: Option<String>,
    /// Additional colour in the tips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

/// Arrow geometry in dots inside a `w` x `h` box.
#[derive(Debug, Clone, Copy)]
struct Arrow {
    w: u32,
    h: u32,
    margin: u32,
    line: u32,
    /// Body between the tips (separator lines at these x).
    body_l: u32,
    body_r: u32,
    left_tip: bool,
    right_tip: bool,
}

impl Arrow {
    fn new(w: u32, h: u32, direction: PipeDirection) -> Self {
        let margin = ((h as f32 * MARGIN_SHARE).round() as u32).max(1);
        let line = ((h as f32 * LINE_SHARE).round() as u32).max(1);
        let tip = ((h as f32 * TIP_SHARE).round() as u32).min(w / 4);
        let left_tip = matches!(direction, PipeDirection::Left | PipeDirection::Both);
        let right_tip = matches!(direction, PipeDirection::Right | PipeDirection::Both);
        Self {
            w,
            h,
            margin,
            line,
            body_l: if left_tip { margin + tip } else { margin },
            body_r: if right_tip {
                w.saturating_sub(margin + tip)
            } else {
                w.saturating_sub(margin)
            },
            left_tip,
            right_tip,
        }
    }

    /// Half height of the arrow at column `x` shrunk by `inset` dots
    /// (`None` outside the arrow).
    fn half_at(&self, x: u32, inset: u32) -> Option<f32> {
        let full = (self.h as f32 / 2.0 - (self.margin + inset) as f32).max(0.0);
        let x = x as f32 + 0.5;
        let start = (self.margin + if self.left_tip { inset * 2 } else { inset }) as f32;
        let end =
            self.w as f32 - (self.margin + if self.right_tip { inset * 2 } else { inset }) as f32;
        if x < start || x >= end {
            return None;
        }
        let body_l = self.body_l as f32;
        let body_r = self.body_r as f32;
        let half = if self.left_tip && x < body_l {
            full * (x - start) / (body_l - start).max(1.0)
        } else if self.right_tip && x > body_r {
            full * (end - x) / (end - body_r).max(1.0)
        } else {
            full
        };
        Some(half)
    }

    fn inside(&self, x: u32, y: u32, inset: u32) -> bool {
        let centre = self.h as f32 / 2.0;
        self.half_at(x, inset)
            .is_some_and(|half| (y as f32 + 0.5 - centre).abs() <= half)
    }

    /// Inside one of the tips, `gap` dots clear of its edges and the separator.
    fn in_tip(&self, x: u32, y: u32, gap: u32) -> bool {
        let sep = self.line / 2 + gap;
        let in_left = self.left_tip && x + sep < self.body_l;
        let in_right = self.right_tip && x > self.body_r + sep;
        (in_left || in_right) && self.inside(x, y, gap)
    }

    fn on_separator(&self, x: u32) -> bool {
        let half = self.line / 2;
        let near = |edge: u32| x + half >= edge && x < edge + self.line - half;
        (self.left_tip && near(self.body_l)) || (self.right_tip && near(self.body_r))
    }
}

/// Fonts for the marker's text (regular for the second line).
pub struct MarkerStyle<'a> {
    pub faces: &'a FaceSet<'a>,
    pub sub_faces: &'a FaceSet<'a>,
    /// Main text size in dots; `None` = largest that fits.
    pub size_px: Option<f32>,
}

/// Renders `marker` into a `w` x `h` dot box; `symbol` renders a pictogram
/// into a square of the given size. Returns the bitmap and whether text
/// was clipped.
pub fn render_marker(
    marker: &PipeMarker,
    w: u32,
    h: u16,
    style: &MarkerStyle,
    symbol: &dyn Fn(&str, u32) -> Result<Bitmap, RenderError>,
) -> Result<(Bitmap, bool), RenderError> {
    let mut out = Bitmap::new(h, w);
    let hh = h as u32;
    if w == 0 || hh == 0 {
        return Ok((out, false));
    }
    let arrow = Arrow::new(w, hh, marker.direction);
    let gap = arrow.line + arrow.line / 2;
    for x in 0..w {
        let separator = arrow.on_separator(x);
        for y in 0..hh {
            let inside = arrow.inside(x, y, 0);
            let ink = match marker.frame {
                PipeFrame::Filled => !inside || separator,
                PipeFrame::Outline => {
                    (inside && !arrow.inside(x, y, arrow.line)) || (inside && separator)
                }
            };
            let tip = match marker.tips {
                TipFill::None => false,
                TipFill::Solid => arrow.in_tip(x, y, gap),
                TipFill::Hatched => {
                    arrow.in_tip(x, y, gap) && (x + y) % (arrow.line * HATCH_PERIOD) < arrow.line
                }
            };
            if ink || tip {
                out.set_pixel(y as u16, x, true);
            }
        }
    }

    // Content area: the body inside the border, clear of the separators.
    let pad = arrow.margin + arrow.line;
    let left = arrow.body_l
        + if arrow.left_tip {
            arrow.line + pad
        } else {
            pad
        };
    let right = arrow.body_r.saturating_sub(if arrow.right_tip {
        arrow.line + pad
    } else {
        pad
    });
    let top = arrow.margin + arrow.line;
    let inner_h = hh.saturating_sub(2 * top);
    if right <= left || inner_h == 0 {
        return Ok((out, false));
    }
    let mut x = left;
    for name in marker.symbols.iter().take(MAX_SYMBOLS) {
        if x + inner_h > right {
            break;
        }
        let pic = symbol(name, inner_h)?;
        out.blit(&pic, top as i32, x as i32, 0..h);
        x += inner_h + pad;
    }
    let text_w = right.saturating_sub(x);
    let mut clipped = false;
    if text_w > 0 && !marker.text.trim().is_empty() {
        let with_sub = !marker.sub_text.trim().is_empty();
        let main_h = if with_sub {
            (inner_h as f32 * MAIN_LINE_SHARE) as u32
        } else {
            inner_h
        };
        let layout = |size_px| TextLayout {
            size_px,
            align: TextAlign::Center,
            valign: VAlign::Middle,
            line_spacing: 1.0,
        };
        let (main, cut) = boxed::text_in_box_checked(
            &marker.text,
            style.faces,
            text_w,
            main_h.min(u16::MAX as u32) as u16,
            &layout(style.size_px),
        )?;
        clipped |= cut;
        out.blit(&main, top as i32, x as i32, 0..h);
        if with_sub {
            let sub_h = inner_h - main_h;
            let (sub, cut) = boxed::text_in_box_checked(
                &marker.sub_text,
                style.sub_faces,
                text_w,
                sub_h.min(u16::MAX as u32) as u16,
                &layout(None),
            )?;
            clipped |= cut;
            out.blit(&sub, (top + main_h) as i32, x as i32, 0..h);
        }
    }
    Ok((out, clipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_shape_follows_the_direction() {
        let a = Arrow::new(400, 100, PipeDirection::Right);
        // Body is full height, the tip narrows to the right end.
        assert!(a.inside(100, 50, 0) && a.inside(100, 10, 0));
        assert!(a.inside(390, 50, 0) && !a.inside(390, 20, 0));
        // No tip on the left: straight end.
        assert!(a.inside(8, 10, 0));
        let both = Arrow::new(400, 100, PipeDirection::Both);
        assert!(!both.inside(8, 10, 0) && both.inside(8, 50, 0));
    }

    #[test]
    fn tips_are_filled_clear_of_their_edges() {
        let a = Arrow::new(400, 100, PipeDirection::Right);
        assert!(a.in_tip(a.body_r + 20, 50, 4));
        assert!(!a.in_tip(a.body_r + 1, 50, 4), "separator stays free");
        assert!(!a.in_tip(200, 50, 4), "body is not a tip");
    }
}
