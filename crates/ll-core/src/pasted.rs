//! Images pasted from the clipboard. Image elements reference files, so a
//! pasted image is stored once in the data directory (`pasted/`), named by
//! a hash of its content so pasting the same image again reuses the file.

use std::path::{Path, PathBuf};

use crate::iconsets::data_dir;
use crate::CoreError;

/// File types accepted from the clipboard.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "svg"];

/// Folder for pasted images inside the data directory.
pub fn pasted_dir() -> Result<PathBuf, CoreError> {
    Ok(data_dir()?.join("pasted"))
}

/// Stores a pasted image and returns its path.
pub fn save_image(data: &[u8], extension: &str) -> Result<PathBuf, CoreError> {
    save_image_in(&pasted_dir()?, data, extension)
}

/// [`save_image`] into an explicit folder (tests).
pub fn save_image_in(dir: &Path, data: &[u8], extension: &str) -> Result<PathBuf, CoreError> {
    let ext = extension.to_ascii_lowercase();
    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(CoreError::Unsupported("image format from the clipboard"));
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{:016x}.{ext}", fnv1a(data)));
    if !path.exists() {
        std::fs::write(&path, data)?;
    }
    Ok(path)
}

/// 64-bit FNV-1a: stable across runs and platforms (unlike `DefaultHasher`).
fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_same_file() {
        let dir = std::env::temp_dir().join(format!("ll-pasted-{}", std::process::id()));
        let a = save_image_in(&dir, b"one", "PNG").unwrap();
        let b = save_image_in(&dir, b"one", "png").unwrap();
        let c = save_image_in(&dir, b"two", "png").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(std::fs::read(&a).unwrap(), b"one");
        assert!(a.to_string_lossy().ends_with(".png"));
        assert!(save_image_in(&dir, b"x", "exe").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
