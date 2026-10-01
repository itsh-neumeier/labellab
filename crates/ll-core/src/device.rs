//! Device discovery and status queries. Bluetooth/USB native discovery
//! lands in M4; for now devices are addressed by serial/COM port path
//! (the BT-SPP fallback, see `docs/PROTOCOL.md`).

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
    transport.write_all(&command::invalidate()).await?;
    transport.write_all(&command::initialize()).await?;
    transport.write_all(&command::status_request()).await?;

    let mut buf = [0u8; ll_protocol::status::STATUS_BLOCK_LEN];
    transport
        .read_exact_timeout(&mut buf, STATUS_TIMEOUT)
        .await?;
    transport.close().await?;

    Ok(StatusBlock::parse(&buf)?)
}
