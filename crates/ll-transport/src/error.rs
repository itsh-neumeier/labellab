use thiserror::Error;

/// Errors raised while sending/receiving bytes over any transport.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("timed out waiting for {0} bytes")]
    Timeout(usize),

    #[error("transport is not connected")]
    NotConnected,

    #[error("platform error: {0}")]
    Platform(String),

    #[error("no Bluetooth device found for {0:?}")]
    DeviceNotFound(String),
}
