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

    /// Series code (offset 3); with [`Self::model_byte`] identifies the
    /// model (see [`crate::model::find_by_status_bytes`]).
    pub fn series_byte(&self) -> u8 {
        self.raw[3]
    }

    /// Model code (offset 4).
    pub fn model_byte(&self) -> u8 {
        self.raw[4]
    }

    /// The model this status came from, if it is in the model table.
    pub fn model(&self) -> Option<&'static crate::model::ModelInfo> {
        crate::model::find_by_status_bytes(self.series_byte(), self.model_byte())
    }

    /// Notification number (offset 22): `0x01` cover opened, `0x02` cover
    /// closed, `0x00` none. Documented, TODO(verify) on hardware.
    pub fn notification(&self) -> u8 {
        self.raw[22]
    }

    /// Ids of the error bits set in bytes 8 and 9 (see [`ERROR_BITS`]).
    pub fn error_ids(&self) -> Vec<&'static str> {
        ERROR_BITS
            .iter()
            .filter(|b| self.raw[b.byte] & b.mask != 0)
            .map(|b| b.id)
            .collect()
    }
}

/// One documented error bit of the status block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorBit {
    /// Status byte offset (8 = error information 1, 9 = error information 2).
    pub byte: usize,
    pub mask: u8,
    /// Stable, language-neutral id (the GUI translates it).
    pub id: &'static str,
}

const fn bit(byte: usize, mask: u8, id: &'static str) -> ErrorBit {
    ErrorBit { byte, mask, id }
}

/// Error bits per Brother's Raster Command Reference (PT-E550W/P750W/P710BT,
/// status table "Error information 1/2"). Status: documented; on a real
/// PT-P710BT only "no error" (`0x00 0x00`) is confirmed so far.
/// TODO(verify): provoke errors (no cassette, cover open, weak battery) and
/// compare, see `docs/PROTOCOL.md`.
pub const ERROR_BITS: &[ErrorBit] = &[
    bit(8, 0x01, "no_media"),
    bit(8, 0x04, "cutter_jam"),
    bit(8, 0x08, "weak_battery"),
    bit(8, 0x40, "high_voltage_adapter"),
    bit(9, 0x01, "wrong_media"),
    bit(9, 0x10, "cover_open"),
    bit(9, 0x20, "overheating"),
];

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
    fn decodes_model_and_documented_error_bits() {
        let mut b = fixture();
        b[3] = 0x30;
        b[4] = 0x76;
        b[8] = 0x08;
        b[9] = 0x10;
        let status = StatusBlock::parse(&b).unwrap();
        assert_eq!(status.model().map(|m| m.name), Some("PT-P710BT"));
        assert_eq!(status.error_ids(), vec!["weak_battery", "cover_open"]);
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
