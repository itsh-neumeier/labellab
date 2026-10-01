//! Model table: per-model protocol constants and tape geometry.
//!
//! All values here must come from verified sources (Brother reference PDF or
//! hardware test), see `docs/PROTOCOL.md`. Unverified entries are marked
//! `TODO(verify)` and must not be used for production printing decisions
//! without a hardware test confirming them.

/// Print resolution in dots per inch, both across the head and along the
/// feed. Source: Raster Command Reference (180 dpi, documented), see
/// `docs/PROTOCOL.md`. Shared by every model in [`MODELS`].
pub const DOTS_PER_INCH: u32 = 180;

/// Converts a length in millimeters to print dots (rounded).
pub fn mm_to_dots(mm: f32) -> u32 {
    (mm.max(0.0) * DOTS_PER_INCH as f32 / 25.4).round() as u32
}

/// Printable pins and left pin offset for one tape width, in print dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TapeGeometry {
    pub width_mm: u8,
    pub printable_pins: u16,
    pub left_offset_pins: u16,
}

/// Static identification and geometry data for one printer model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelInfo {
    pub name: &'static str,
    pub usb_vid: u16,
    pub usb_pid: u16,
    pub status_series_byte: u8,
    pub status_model_byte: u8,
    pub head_pins: u16,
    pub tape_geometries: &'static [TapeGeometry],
}

// TODO(verify): Pin-Offsets/bedruckbare Pins je Bandbreite gegen echten
// PT-P710BT und Brothers Raster Command Reference pruefen (siehe
// docs/PROGRESS.md -> Hardware-Tests offen).
const PT_P710BT_GEOMETRIES: &[TapeGeometry] = &[
    TapeGeometry {
        width_mm: 3,
        printable_pins: 24,
        left_offset_pins: 52,
    },
    TapeGeometry {
        width_mm: 6,
        printable_pins: 32,
        left_offset_pins: 48,
    },
    TapeGeometry {
        width_mm: 9,
        printable_pins: 50,
        left_offset_pins: 39,
    },
    TapeGeometry {
        width_mm: 12,
        printable_pins: 70,
        left_offset_pins: 29,
    },
    TapeGeometry {
        width_mm: 18,
        printable_pins: 112,
        left_offset_pins: 8,
    },
    TapeGeometry {
        width_mm: 24,
        printable_pins: 128,
        left_offset_pins: 0,
    },
];

/// Known printer models, keyed by USB VID:PID and status series/model byte.
///
/// Source for VID/PID, series/model byte: Brother driver `Setup.ini` /
/// Sprachmonitor-INI (verified, see `docs/PROTOCOL.md`).
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "PT-P710BT",
        usb_vid: 0x04F9,
        usb_pid: 0x20AF,
        status_series_byte: 0x30,
        status_model_byte: 0x76,
        head_pins: 128,
        tape_geometries: PT_P710BT_GEOMETRIES,
    },
    ModelInfo {
        name: "PT-P715eBT",
        usb_vid: 0x04F9,
        usb_pid: 0x20C5,
        status_series_byte: 0x30,
        status_model_byte: 0x77,
        head_pins: 128,
        // TODO(verify): eigene Geometrie-Tabelle, aktuell P710BT-Werte uebernommen.
        tape_geometries: PT_P710BT_GEOMETRIES,
    },
    ModelInfo {
        name: "PT-E720BT",
        usb_vid: 0x04F9,
        usb_pid: 0x224A,
        status_series_byte: 0x30,
        status_model_byte: 0x81,
        head_pins: 128,
        // TODO(verify): eigene Geometrie-Tabelle, aktuell P710BT-Werte uebernommen.
        tape_geometries: PT_P710BT_GEOMETRIES,
    },
];

/// Looks up a model by the series/model byte pair reported in the status block.
pub fn find_by_status_bytes(series: u8, model: u8) -> Option<&'static ModelInfo> {
    MODELS
        .iter()
        .find(|m| m.status_series_byte == series && m.status_model_byte == model)
}

/// Looks up a model by its name (e.g. `"PT-P710BT"`), case-insensitive.
pub fn find_by_name(name: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.name.eq_ignore_ascii_case(name))
}
