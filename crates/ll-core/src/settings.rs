//! App settings (UI choices, printer, tape, print options) as one
//! encrypted file, see [`crate::paths::settings_file`] (ADR-036).
//!
//! Format: `LLS1` + 12-byte nonce + ChaCha20-Poly1305 ciphertext of the
//! settings as JSON. The key is built into LabelLab, so this keeps people
//! from casually reading or editing the file and detects any change (a
//! modified file is rejected and the defaults are used); it is not a
//! secret against someone who takes the program apart.

use std::collections::BTreeMap;
use std::path::Path;

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use crate::CoreError;

/// Settings: key → JSON value (the GUI decides the keys).
pub type Settings = BTreeMap<String, serde_json::Value>;

const MAGIC: &[u8; 4] = b"LLS1";
const NONCE_LEN: usize = 12;
/// Built-in key (see the module docs for what it protects against).
const KEY: [u8; 32] = *b"LabelLab/ITSH-Neumeier/settings1";

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Settings(e.to_string())
}

fn cipher() -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(&KEY))
}

/// Encrypts `settings` into the file format.
pub fn encrypt(settings: &Settings) -> Result<Vec<u8>, CoreError> {
    let json = serde_json::to_vec(settings).map_err(err)?;
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let data = cipher().encrypt(&nonce, json.as_slice()).map_err(err)?;
    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + data.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&data);
    Ok(out)
}

/// Decrypts the file format; fails on a foreign or modified file.
pub fn decrypt(data: &[u8]) -> Result<Settings, CoreError> {
    let rest = data
        .strip_prefix(MAGIC)
        .ok_or_else(|| err("not a LabelLab settings file"))?;
    if rest.len() < NONCE_LEN {
        return Err(err("settings file too short"));
    }
    let (nonce, body) = rest.split_at(NONCE_LEN);
    let json = cipher()
        .decrypt(Nonce::from_slice(nonce), body)
        .map_err(|_| err("settings file was modified or is damaged"))?;
    serde_json::from_slice(&json).map_err(err)
}

/// Reads `path`: no file = empty settings; a modified or broken file is an
/// error (callers fall back to defaults and overwrite it on the next save).
pub fn load_from(path: &Path) -> Result<Settings, CoreError> {
    match std::fs::read(path) {
        Ok(data) => decrypt(&data),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::new()),
        Err(e) => Err(e.into()),
    }
}

/// Writes `settings` to `path` (via a temporary file, so a crash never
/// leaves half a file).
pub fn save_to(path: &Path, settings: &Settings) -> Result<(), CoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("settings.tmp");
    std::fs::write(&tmp, encrypt(settings)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// [`load_from`] the default settings file.
pub fn load() -> Result<Settings, CoreError> {
    load_from(&crate::paths::settings_file()?)
}

/// [`save_to`] the default settings file.
pub fn save(settings: &Settings) -> Result<(), CoreError> {
    save_to(&crate::paths::settings_file()?, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Settings {
        let mut s = Settings::new();
        s.insert("labellab.lang".into(), "de".into());
        s.insert("print.copies".into(), 3.into());
        s
    }

    #[test]
    fn round_trip_is_unreadable_and_tamper_proof() {
        let data = encrypt(&sample()).unwrap();
        assert!(data.starts_with(MAGIC));
        // Not plain text.
        assert!(!String::from_utf8_lossy(&data).contains("labellab.lang"));
        assert_eq!(decrypt(&data).unwrap(), sample());
        // Any changed byte is rejected.
        let mut bad = data.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(decrypt(&bad).is_err());
        assert!(decrypt(b"{\"a\":1}").is_err());
        // Two saves use different nonces.
        assert_ne!(encrypt(&sample()).unwrap(), data);
    }

    #[test]
    fn files_load_empty_when_missing_and_save_atomically() {
        let dir = std::env::temp_dir().join(format!("ll-settings-{}", std::process::id()));
        let file = dir.join("LabelLab.settings");
        assert!(load_from(&file).unwrap().is_empty());
        save_to(&file, &sample()).unwrap();
        assert_eq!(load_from(&file).unwrap(), sample());
        assert!(!file.with_extension("settings.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
