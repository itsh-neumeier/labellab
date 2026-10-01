//! Serial (COM-port) transport: the Bluetooth-SPP fallback (virtual COM
//! port) and any directly wired serial connection, see `docs/PROTOCOL.md`.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::SerialPortBuilderExt;

use crate::{Transport, TransportError, TransportKind};

/// A connection to the printer over a serial/COM port.
pub struct SerialTransport {
    port: tokio_serial::SerialStream,
}

impl SerialTransport {
    /// Opens `path` (e.g. `COM9` on Windows, `/dev/rfcomm0` on Linux) at
    /// `baud_rate`. Virtual Bluetooth-SPP COM ports generally ignore the
    /// baud rate, but the OS API still requires one.
    pub fn open(path: &str, baud_rate: u32) -> Result<Self, TransportError> {
        let port = tokio_serial::new(path, baud_rate)
            .open_native_async()
            .map_err(|e| TransportError::Io(std::io::Error::other(e)))?;
        Ok(Self { port })
    }
}

#[async_trait::async_trait]
impl Transport for SerialTransport {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.port.write_all(data).await.map_err(TransportError::Io)
    }

    async fn read_exact_timeout(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<(), TransportError> {
        tokio::time::timeout(timeout, self.port.read_exact(buf))
            .await
            .map_err(|_| TransportError::Timeout(buf.len()))?
            .map_err(TransportError::Io)?;
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Serial
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

/// Lists available serial ports, e.g. `COM9` on Windows or `/dev/ttyUSB0`
/// on Linux. Includes Bluetooth-SPP ports exposed as a virtual COM port.
/// Does not distinguish printers from other serial devices yet (M4 adds
/// name-based filtering alongside native Bluetooth/USB discovery).
pub fn list_ports() -> Result<Vec<String>, TransportError> {
    tokio_serial::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .map_err(|e| TransportError::Io(std::io::Error::other(e)))
}
