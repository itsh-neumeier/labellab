//! Device discovery and status queries.
//!
//! Two transports exist today: serial (COM-port / BT-SPP fallback, see
//! `docs/PROTOCOL.md`) and, on Windows, native Bluetooth RFCOMM via WinRT
//! (bypasses the flaky BT-SPP virtual-COM-port shim, see ADR-002 and
//! `docs/PROGRESS.md`). Linux native Bluetooth (BlueZ) is still a TODO.

use std::time::Duration;

use ll_protocol::{command, status::StatusBlock};
use ll_transport::{serial::SerialTransport, Transport};

use crate::CoreError;

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// Lists available serial ports (COM-ports on Windows, `/dev/tty*` on
/// Linux), including Bluetooth-SPP virtual COM ports.
pub fn list_serial_devices() -> Result<Vec<String>, CoreError> {
    Ok(ll_transport::serial::list_ports()?)
}

/// Resets the printer and reads its status block over a serial connection.
pub async fn query_status_over_serial(
    port: &str,
    baud_rate: u32,
) -> Result<StatusBlock, CoreError> {
    let mut transport = SerialTransport::open(port, baud_rate)?;
    let status = query_status(&mut transport).await;
    transport.close().await?;
    status
}

#[cfg(windows)]
pub use bluetooth::{list_bluetooth_devices, query_status_over_bluetooth};

#[cfg(windows)]
mod bluetooth {
    use ll_transport::bluetooth::{self, BluetoothDeviceInfo, BluetoothTransport};

    use super::*;

    /// Lists paired Bluetooth devices offering the Serial Port Profile.
    /// Not filtered by name; multiple PT-P7xx/E7xx models exist, see
    /// `ll_protocol::model`.
    pub fn list_bluetooth_devices() -> Result<Vec<BluetoothDeviceInfo>, CoreError> {
        Ok(bluetooth::list_paired_devices()?)
    }

    /// Resets the printer and reads its status block over native
    /// Bluetooth RFCOMM.
    pub async fn query_status_over_bluetooth(device_id: &str) -> Result<StatusBlock, CoreError> {
        let mut transport = BluetoothTransport::connect(device_id).await?;
        let status = query_status(&mut transport).await;
        transport.close().await?;
        status
    }
}

async fn query_status(transport: &mut dyn Transport) -> Result<StatusBlock, CoreError> {
    transport.write_all(&command::invalidate()).await?;
    transport.write_all(&command::initialize()).await?;
    transport.write_all(&command::status_request()).await?;

    let mut buf = [0u8; ll_protocol::status::STATUS_BLOCK_LEN];
    transport
        .read_exact_timeout(&mut buf, STATUS_TIMEOUT)
        .await?;

    Ok(StatusBlock::parse(&buf)?)
}
