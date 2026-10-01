//! Command builders for the Brother raster protocol.
//!
//! Byte layouts come from the public Raster Command Reference, see
//! `docs/PROTOCOL.md` for the per-command verification status. Anything
//! whose bit meaning is not yet confirmed against the real PT-P710BT is
//! called out with `TODO(verify)` and must not be trusted for a production
//! print until a hardware test confirms it.

/// 100 zero bytes: clears the printer's command buffer / parser state.
pub fn invalidate() -> Vec<u8> {
    vec![0x00; 100]
}

/// `ESC @`: resets the printer to its power-on state.
pub fn initialize() -> Vec<u8> {
    vec![0x1B, 0x40]
}

/// `ESC i S`: requests a 32-byte status block (see [`crate::status`]).
pub fn status_request() -> Vec<u8> {
    vec![0x1B, 0x69, 0x53]
}

/// `ESC i a 01`: switches the printer into raster graphics mode.
pub fn switch_to_raster_mode() -> Vec<u8> {
    vec![0x1B, 0x69, 0x61, 0x01]
}

/// Parameters for the `ESC i z` print-information command.
pub struct PrintInformation {
    pub media_width_mm: u8,
    pub raster_lines: u32,
    pub is_first_page: bool,
}

impl PrintInformation {
    /// `ESC i z n1..n10`.
    ///
    /// TODO(verify): `n1` validity-flag bit assignment (media type/width/
    /// length valid, quality priority, recovery) against the real
    /// PT-P710BT; only "media width valid" is set here.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0x1B, 0x69, 0x7A];
        out.push(0b0000_0010); // n1: validity flags, bit1 = media width valid
        out.push(0x00); // n2: media type, 0 = unspecified
        out.push(self.media_width_mm); // n3: media width (mm)
        out.push(0x00); // n4: media length (mm), 0 = continuous tape
        out.extend_from_slice(&self.raster_lines.to_le_bytes()); // n5..n8
        out.push(if self.is_first_page { 0x00 } else { 0x01 }); // n9
        out.push(0x00); // n10: fixed
        out
    }
}

/// `ESC i M n`: Various Mode. Bit 6 = auto-cut (verified position, see
/// `docs/PROTOCOL.md`); other bits not yet used. Sent per page, so a
/// multi-page job can cut after selected pages only.
pub fn various_mode(auto_cut: bool) -> Vec<u8> {
    let mut flags = 0u8;
    if auto_cut {
        flags |= 1 << 6;
    }
    vec![0x1B, 0x69, 0x4D, flags]
}

/// `0C`: end of a page that is not the last one in the job (no feed).
/// Source: Raster Command Reference, "Print command" (documented).
pub fn print_page() -> Vec<u8> {
    vec![0x0C]
}

/// `ESC i d n1 n2`: margin/feed amount in print dots (LE16).
pub fn margin(dots: u16) -> Vec<u8> {
    let le = dots.to_le_bytes();
    vec![0x1B, 0x69, 0x64, le[0], le[1]]
}

/// `M 02`: selects TIFF/PackBits compression for subsequent raster lines.
pub fn select_packbits_compression() -> Vec<u8> {
    vec![0x4D, 0x02]
}

/// `G n1 n2 <data>`: one PackBits-compressed raster line (length LE16).
pub fn raster_line(compressed: &[u8]) -> Vec<u8> {
    let len = (compressed.len() as u16).to_le_bytes();
    let mut out = Vec::with_capacity(3 + compressed.len());
    out.push(0x47);
    out.extend_from_slice(&len);
    out.extend_from_slice(compressed);
    out
}

/// `Z`: one blank raster line.
pub fn empty_row() -> Vec<u8> {
    vec![0x5A]
}

/// `FF`-style print: prints and feeds the final page.
pub fn print_with_feed() -> Vec<u8> {
    vec![0x1A]
}

/// Prints the current page without feeding (used for chain printing).
pub fn print_without_feed() -> Vec<u8> {
    vec![0x0C]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidate_is_100_zero_bytes() {
        assert_eq!(invalidate(), vec![0x00; 100]);
    }

    #[test]
    fn initialize_bytes() {
        assert_eq!(initialize(), vec![0x1B, 0x40]);
    }

    #[test]
    fn status_request_bytes() {
        assert_eq!(status_request(), vec![0x1B, 0x69, 0x53]);
    }

    #[test]
    fn various_mode_sets_bit6_for_autocut() {
        assert_eq!(various_mode(true), vec![0x1B, 0x69, 0x4D, 0b0100_0000]);
        assert_eq!(various_mode(false), vec![0x1B, 0x69, 0x4D, 0x00]);
    }

    #[test]
    fn margin_is_little_endian() {
        assert_eq!(margin(0x0102), vec![0x1B, 0x69, 0x64, 0x02, 0x01]);
    }

    #[test]
    fn raster_line_length_prefix_is_little_endian() {
        let data = vec![0xAA; 300];
        let out = raster_line(&data);
        assert_eq!(out[0], 0x47);
        assert_eq!(u16::from_le_bytes([out[1], out[2]]), 300);
        assert_eq!(&out[3..], data.as_slice());
    }

    #[test]
    fn print_information_layout() {
        let info = PrintInformation {
            media_width_mm: 9,
            raster_lines: 42,
            is_first_page: true,
        };
        let bytes = info.to_bytes();
        assert_eq!(&bytes[0..3], &[0x1B, 0x69, 0x7A]);
        assert_eq!(bytes[5], 9, "n3: media width");
        assert_eq!(
            u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]),
            42,
            "n5..n8: raster line count"
        );
        assert_eq!(bytes[11], 0x00, "n9: is_first_page");
        assert_eq!(bytes[12], 0x00, "n10: fixed");
        assert_eq!(bytes.len(), 13);
    }
}
