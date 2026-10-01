//! `Transport` trait shared by all ways of talking to the printer, plus a
//! `MockTransport` for hardware-free tests.
//!
//! Real implementations (Serial in M2, native Bluetooth/USB in M4) live in
//! this crate behind the same trait so `ll-core` never depends on a
//! concrete transport.

mod error;
pub mod mock;
pub mod serial;

use std::time::Duration;

pub use error::TransportError;

/// Which concrete transport is behind a `Transport` instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    BluetoothRfcomm,
    Serial,
    Usb,
    Mock,
}

/// A byte-oriented connection to the printer.
#[async_trait::async_trait]
pub trait Transport: Send {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError>;
    async fn read_exact_timeout(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<(), TransportError>;
    fn kind(&self) -> TransportKind;
    async fn close(&mut self) -> Result<(), TransportError>;
}
