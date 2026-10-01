//! Finds and loads a usable system font.
//!
//! No family/weight/fallback-chain selection yet (that's future work for
//! M5/M6) — this just picks the first of a short list of common default
//! fonts per OS, so text uses a real typeface instead of the M3 placeholder
//! stencil. CI installs `fonts-dejavu-core` on `ubuntu-latest` so the Linux
//! candidate path is deterministic there.

use std::path::{Path, PathBuf};

use crate::RenderError;

#[cfg(windows)]
const CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\segoeui.ttf",
    r"C:\Windows\Fonts\arial.ttf",
    r"C:\Windows\Fonts\calibri.ttf",
    r"C:\Windows\Fonts\tahoma.ttf",
];

#[cfg(target_os = "linux")]
const CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
];

#[cfg(not(any(windows, target_os = "linux")))]
const CANDIDATES: &[&str] = &[];

/// Returns the first candidate system font that exists on disk.
pub fn default_font_path() -> Option<PathBuf> {
    CANDIDATES
        .iter()
        .map(Path::new)
        .find(|p| p.exists())
        .map(Path::to_path_buf)
}

/// Loads the default system font's raw bytes.
pub fn load_default_font() -> Result<Vec<u8>, RenderError> {
    let path = default_font_path().ok_or(RenderError::NoSystemFont)?;
    Ok(std::fs::read(path)?)
}
