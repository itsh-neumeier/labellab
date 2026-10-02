//! Tape and text colors reported in the status block (bytes 24 and 25).
//!
//! Source: Brother Raster Command Reference PT-E550W/P750W/P710BT v1.02,
//! status tables (8) "Tape color information" and (9) "Text color
//! information" (documented). Hardware-confirmed so far: white tape
//! (`0x01`) with black text (`0x08`) on a PT-P710BT, see
//! `docs/PROTOCOL.md`.

/// One color code with a stable, language-neutral id (the GUI maps ids to
/// display names and preview colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorCode {
    pub code: u8,
    pub id: &'static str,
}

/// Tape (background) colors, status byte 24.
pub const TAPE_COLORS: &[ColorCode] = &[
    ColorCode {
        code: 0x01,
        id: "white",
    },
    ColorCode {
        code: 0x02,
        id: "other",
    },
    ColorCode {
        code: 0x03,
        id: "clear",
    },
    ColorCode {
        code: 0x04,
        id: "red",
    },
    ColorCode {
        code: 0x05,
        id: "blue",
    },
    ColorCode {
        code: 0x06,
        id: "yellow",
    },
    ColorCode {
        code: 0x07,
        id: "green",
    },
    ColorCode {
        code: 0x08,
        id: "black",
    },
    ColorCode {
        code: 0x09,
        id: "clear_white_text",
    },
    ColorCode {
        code: 0x20,
        id: "matte_white",
    },
    ColorCode {
        code: 0x21,
        id: "matte_clear",
    },
    ColorCode {
        code: 0x22,
        id: "matte_silver",
    },
    ColorCode {
        code: 0x23,
        id: "satin_gold",
    },
    ColorCode {
        code: 0x24,
        id: "satin_silver",
    },
    ColorCode {
        code: 0x30,
        id: "blue_d",
    },
    ColorCode {
        code: 0x31,
        id: "red_d",
    },
    ColorCode {
        code: 0x40,
        id: "fluorescent_orange",
    },
    ColorCode {
        code: 0x41,
        id: "fluorescent_yellow",
    },
    ColorCode {
        code: 0x50,
        id: "berry_pink",
    },
    ColorCode {
        code: 0x51,
        id: "light_gray",
    },
    ColorCode {
        code: 0x52,
        id: "lime_green",
    },
    ColorCode {
        code: 0x60,
        id: "yellow_f",
    },
    ColorCode {
        code: 0x61,
        id: "pink_f",
    },
    ColorCode {
        code: 0x62,
        id: "blue_f",
    },
    ColorCode {
        code: 0x70,
        id: "white_heat_shrink",
    },
    ColorCode {
        code: 0x90,
        id: "white_flex",
    },
    ColorCode {
        code: 0x91,
        id: "yellow_flex",
    },
    ColorCode {
        code: 0xF0,
        id: "cleaning",
    },
    ColorCode {
        code: 0xF1,
        id: "stencil",
    },
    ColorCode {
        code: 0xFF,
        id: "incompatible",
    },
];

/// Text (ink) colors, status byte 25.
pub const TEXT_COLORS: &[ColorCode] = &[
    ColorCode {
        code: 0x01,
        id: "white",
    },
    ColorCode {
        code: 0x02,
        id: "other",
    },
    ColorCode {
        code: 0x04,
        id: "red",
    },
    ColorCode {
        code: 0x05,
        id: "blue",
    },
    ColorCode {
        code: 0x08,
        id: "black",
    },
    ColorCode {
        code: 0x0A,
        id: "gold",
    },
    ColorCode {
        code: 0x62,
        id: "blue_f",
    },
    ColorCode {
        code: 0xF0,
        id: "cleaning",
    },
    ColorCode {
        code: 0xF1,
        id: "stencil",
    },
    ColorCode {
        code: 0xFF,
        id: "incompatible",
    },
];

/// Media type (status byte 11) ids per Brother's Raster Command Reference
/// (documented). Hardware-confirmed: `0x01` with a laminated TZe tape.
pub const MEDIA_TYPES: &[ColorCode] = &[
    ColorCode {
        code: 0x00,
        id: "none",
    },
    ColorCode {
        code: 0x01,
        id: "laminated",
    },
    ColorCode {
        code: 0x03,
        id: "non_laminated",
    },
    ColorCode {
        code: 0x11,
        id: "heat_shrink_2_1",
    },
    ColorCode {
        code: 0x17,
        id: "heat_shrink_3_1",
    },
    ColorCode {
        code: 0xFF,
        id: "incompatible",
    },
];

/// Id of media type `code`, or `None` if not in the table.
pub fn media_type_id(code: u8) -> Option<&'static str> {
    MEDIA_TYPES.iter().find(|c| c.code == code).map(|c| c.id)
}

/// Id of tape color `code`, or `None` if not in the table.
pub fn tape_color_id(code: u8) -> Option<&'static str> {
    TAPE_COLORS.iter().find(|c| c.code == code).map(|c| c.id)
}

/// Id of text color `code`, or `None` if not in the table.
pub fn text_color_id(code: u8) -> Option<&'static str> {
    TEXT_COLORS.iter().find(|c| c.code == code).map(|c| c.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_observed_codes_map_to_white_and_black() {
        assert_eq!(tape_color_id(0x01), Some("white"));
        assert_eq!(text_color_id(0x08), Some("black"));
        assert_eq!(tape_color_id(0x03), Some("clear"));
        assert_eq!(text_color_id(0x0A), Some("gold"));
        assert_eq!(tape_color_id(0x42), None);
    }
}
