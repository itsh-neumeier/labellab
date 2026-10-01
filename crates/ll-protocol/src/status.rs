//! Parser for the 32-byte status block returned by `ESC i S`.
//!
//! Byte offsets and meanings per `docs/PROTOCOL.md`. Fields whose bit
//! layout is not yet verified against the real PT-P710BT (error bits,
//! media type codes) are exposed as raw bytes rather than decoded enums.

use crate::error::ProtocolError;

pub const STATUS_BLOCK_LEN: usize = 32;

/// Status-type byte (offset 18): what kind of status report this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusType {
    Reply,
    PrintingComplete,
    Error,
    PhaseChange,
    Unknown(u8),
}

impl From<u8> for StatusType {
    fn from(byte: u8) -> Self {
        match byte {
            0x00 => StatusType::Reply,
            0x01 => StatusType::PrintingComplete,
            0x02 => StatusType::Error,
            0x06 => StatusType::PhaseChange,
            other => StatusType::Unknown(other),
        }
    }
}

/// Parsed 32-byte status block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBlock {
    raw: [u8; STATUS_BLOCK_LEN],
}

impl StatusBlock {
    /// Parses a status block. Fails if the length is wrong or the header
    /// byte (offset 0) is not the printer's `0x80` status marker.
    pub fn parse(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() != STATUS_BLOCK_LEN {
            return Err(ProtocolError::InvalidStatusLength(data.len()));
        }
        let head_mark = data[0];
        if head_mark != 0x80 {
            return Err(ProtocolError::InvalidStatusHeader(head_mark));
        }
        let mut raw = [0u8; STATUS_BLOCK_LEN];
        raw.copy_from_slice(data);
        Ok(Self { raw })
    }

    /// The full, unparsed 32-byte block.
    pub fn raw(&self) -> &[u8; STATUS_BLOCK_LEN] {
        &self.raw
    }

    /// Error information 1 (offset 8). Bit meaning not yet verified, see
    /// `docs/PROTOCOL.md` -> Hardware-Tests offen.
    pub fn error1(&self) -> u8 {
        self.raw[8]
    }

    /// Error information 2 (offset 9). Bit meaning not yet verified.
    pub fn error2(&self) -> u8 {
        self.raw[9]
    }

    pub fn has_error(&self) -> bool {
        self.error1() != 0 || self.error2() != 0
    }

    /// Tape/media width in mm (offset 10). Verified against hardware
    /// (9 mm band recognized correctly, see `docs/PROTOCOL.md`).
    pub fn media_width_mm(&self) -> u8 {
        self.raw[10]
    }

    /// Raw media type byte (offset 11). Code-to-type mapping not yet
    /// verified.
    pub fn media_type(&self) -> u8 {
        self.raw[11]
    }

    pub fn status_type(&self) -> StatusType {
        self.raw[18].into()
    }

    /// Phase type (offset 19). Raw for now; phase sub-codes not verified.
    pub fn phase_type(&self) -> u8 {
        self.raw[19]
    }

    /// Tape color code (offset 24), see [`crate::media::TAPE_COLORS`].
    pub fn tape_color(&self) -> u8 {
        self.raw[24]
    }

    /// Text/print color code (offset 25), see [`crate::media::TEXT_COLORS`].
    pub fn text_color(&self) -> u8 {
        self.raw[25]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> [u8; STATUS_BLOCK_LEN] {
        // Based on the 2026-10-01 hardware test: 9 mm band recognized over
        // the BT SPP COM port (see docs/PROGRESS.md "Bekannte Fakten").
        let mut b = [0u8; STATUS_BLOCK_LEN];
        b[0] = 0x80;
        b[10] = 9;
        b
    }

    #[test]
    fn parses_hardware_fixture() {
        let status = StatusBlock::parse(&fixture()).unwrap();
        assert_eq!(status.media_width_mm(), 9);
        assert!(!status.has_error());
        assert_eq!(status.status_type(), StatusType::Reply);
    }

    #[test]
    fn rejects_wrong_length() {
        let err = StatusBlock::parse(&[0u8; 10]).unwrap_err();
        assert!(matches!(err, ProtocolError::InvalidStatusLength(10)));
    }

    #[test]
    fn rejects_wrong_header() {
        let mut data = fixture();
        data[0] = 0x00;
        let err = StatusBlock::parse(&data).unwrap_err();
        assert!(matches!(err, ProtocolError::InvalidStatusHeader(0x00)));
    }

    #[test]
    fn decodes_status_type() {
        let mut data = fixture();
        data[18] = 0x02;
        let status = StatusBlock::parse(&data).unwrap();
        assert_eq!(status.status_type(), StatusType::Error);
    }
}
