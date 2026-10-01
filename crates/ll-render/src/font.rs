//! Placeholder 5x5 pixel stencil font: space, `0`-`9`, `A`-`Z`, `. , - : !`.
//!
//! This is **not** the real renderer (that's M5: `cosmic-text`/`fontdue`,
//! real typography, lowercase, Unicode). It exists so M3 can prove the
//! full print pipeline (render -> PackBits -> protocol -> transport) with
//! real, if crude, text on real tape. Unsupported characters render blank.
//!
//! Each glyph is 5 rows of 5 bits (bit 4 = leftmost column, `1` = ink).

pub const GLYPH_WIDTH: u32 = 5;
pub const GLYPH_HEIGHT: u32 = 5;

/// Returns the 5-row bitmap for `c` (case-insensitive), or an all-blank
/// glyph for anything not in the placeholder set (space included).
pub fn glyph(c: char) -> [u8; 5] {
    match c.to_ascii_uppercase() {
        '0' => [0b01110, 0b10011, 0b10101, 0b11001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00010, 0b00100, 0b11111],
        '3' => [0b11110, 0b00001, 0b00110, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b11111, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b11110],
        '6' => [0b00110, 0b01000, 0b11110, 0b10001, 0b01110],
        '7' => [0b11111, 0b00010, 0b00100, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b01110, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b01111, 0b00001, 0b01110],
        'A' => [0b01110, 0b10001, 0b11111, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b11110, 0b10001, 0b11110],
        'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b01111],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000],
        'G' => [0b01111, 0b10000, 0b10011, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b11111, 0b10001, 0b10001],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b11111],
        'J' => [0b00111, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b11100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10011, 0b01111],
        'R' => [0b11110, 0b10001, 0b11110, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
        'Y' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100],
        ',' => [0b00000, 0b00000, 0b00000, 0b00100, 0b01000],
        '-' => [0b00000, 0b00000, 0b11111, 0b00000, 0b00000],
        ':' => [0b00000, 0b00100, 0b00000, 0b00100, 0b00000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        _ => [0; 5],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_and_unknown_are_blank() {
        assert_eq!(glyph(' '), [0; 5]);
        assert_eq!(glyph('@'), [0; 5]);
    }

    #[test]
    fn letter_i_is_symmetric_bar() {
        let g = glyph('I');
        assert_eq!(g[0], 0b11111);
        assert_eq!(g[4], 0b11111);
        assert_eq!(g[1], 0b00100);
    }

    #[test]
    fn lowercase_maps_to_uppercase() {
        assert_eq!(glyph('a'), glyph('A'));
    }
}
