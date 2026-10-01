//! Native Bluetooth RFCOMM transport for Windows, via WinRT
//! (`Windows.Devices.Bluetooth.Rfcomm` + `Windows.Networking.Sockets`).
//!
//! Bypasses the legacy "Bluetooth SPP over virtual COM port" compatibility
//! shim entirely (see ADR-002, `docs/PROGRESS.md`: that shim reliably fails
//! to connect with `ERROR_SEM_TIMEOUT` on this project's test hardware,
//! reproduced identically by both .NET `SerialPort` and `tokio-serial`).
//!
//! Requires the device to already be paired in Windows (Settings ->
//! Bluetooth & devices). Programmatic pairing is a TODO for a later pass.

use std::time::Duration;

use windows::core::HSTRING;
use windows::Devices::Bluetooth::Rfcomm::{RfcommDeviceService, RfcommServiceId};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Networking::Sockets::StreamSocket;
use windows::Storage::Streams::{DataReader, DataWriter, InputStreamOptions};

use crate::{Transport, TransportError, TransportKind};

fn platform_err(e: windows::core::Error) -> TransportError {
    TransportError::Platform(e.message())
}

/// One paired Bluetooth device offering the Serial Port Profile (SPP).
#[derive(Debug, Clone)]
pub struct BluetoothDeviceInfo {
    pub id: String,
    /// The paired device's name (e.g. `PT-P710BT5265`), or the service
    /// name if that couldn't be resolved.
    pub name: String,
    /// The SPP service name as enumerated (e.g. `SPP SERVER`).
    pub service_name: String,
}

/// Lists paired devices that expose an RFCOMM SPP service. Does not filter
/// by name; the caller decides which one is the printer (model names vary,
/// see `ll_protocol::model`).
pub fn list_paired_devices() -> Result<Vec<BluetoothDeviceInfo>, TransportError> {
    let selector = RfcommDeviceService::GetDeviceSelector(
        &RfcommServiceId::SerialPort().map_err(platform_err)?,
    )
    .map_err(platform_err)?;
    let devices = DeviceInformation::FindAllAsyncAqsFilter(&selector)
        .map_err(platform_err)?
        .get()
        .map_err(platform_err)?;

    let mut out = Vec::new();
    for device in devices {
        let id = device.Id().map_err(platform_err)?.to_string_lossy();
        let service_name = device.Name().map_err(platform_err)?.to_string_lossy();
        // The enumeration yields the SPP *service* name ("SPP SERVER" on the
        // PT-P710BT); the paired device's own name ("PT-P710BT5265") is on
        // the service's BluetoothDevice. TODO(verify): not hardware-tested
        // yet; falls back to the service name if it can't be resolved.
        let device_name = RfcommDeviceService::FromIdAsync(&HSTRING::from(id.as_str()))
            .ok()
            .and_then(|op| op.get().ok())
            .and_then(|service| service.Device().ok())
            .and_then(|d| d.Name().ok())
            .map(|n| n.to_string_lossy())
            .filter(|n| !n.trim().is_empty());
        out.push(BluetoothDeviceInfo {
            id,
            name: device_name.unwrap_or_else(|| service_name.clone()),
            service_name,
        });
    }
    Ok(out)
}

/// A connection to the printer over native Bluetooth RFCOMM.
pub struct BluetoothTransport {
    socket: StreamSocket,
    writer: DataWriter,
    reader: DataReader,
}

impl BluetoothTransport {
    /// Connects to a paired device by its `DeviceInformation` id (as
    /// returned by [`list_paired_devices`]).
    ///
    /// `windows` 0.58's WinRT async operations don't implement `Future`;
    /// this blocks the calling (tokio) thread on `.get()` until the OS
    /// completes the operation. Fine for a one-shot CLI command; move to
    /// `spawn_blocking` before using this from the GUI event loop (M6).
    pub async fn connect(device_id: &str) -> Result<Self, TransportError> {
        let id = HSTRING::from(device_id);
        let service = RfcommDeviceService::FromIdAsync(&id)
            .map_err(platform_err)?
            .get()
            .map_err(platform_err)?;

        let socket = StreamSocket::new().map_err(platform_err)?;
        let host_name = service.ConnectionHostName().map_err(platform_err)?;
        let service_name = service.ConnectionServiceName().map_err(platform_err)?;
        socket
            .ConnectAsync(&host_name, &service_name)
            .map_err(platform_err)?
            .get()
            .map_err(platform_err)?;

        let output = socket.OutputStream().map_err(platform_err)?;
        let writer = DataWriter::CreateDataWriter(&output).map_err(platform_err)?;

        let input = socket.InputStream().map_err(platform_err)?;
        let reader = DataReader::CreateDataReader(&input).map_err(platform_err)?;
        reader
            .SetInputStreamOptions(InputStreamOptions::Partial)
            .map_err(platform_err)?;

        Ok(Self {
            socket,
            writer,
            reader,
        })
    }
}

#[async_trait::async_trait]
impl Transport for BluetoothTransport {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.writer.WriteBytes(data).map_err(platform_err)?;
        self.writer
            .StoreAsync()
            .map_err(platform_err)?
            .get()
            .map_err(platform_err)?;
        Ok(())
    }

    /// TODO: `windows` 0.58's WinRT async operations don't implement
    /// `Future`, so `timeout` isn't enforced here yet (relies on the OS's
    /// own RFCOMM read timeout). Revisit once a timeout-safe wrapper
    /// exists, see `connect`'s doc comment.
    async fn read_exact_timeout(
        &mut self,
        buf: &mut [u8],
        _timeout: Duration,
    ) -> Result<(), TransportError> {
        let loaded = self
            .reader
            .LoadAsync(buf.len() as u32)
            .map_err(platform_err)?
            .get()
            .map_err(platform_err)?;

        if (loaded as usize) < buf.len() {
            return Err(TransportError::Timeout(buf.len() - loaded as usize));
        }
        self.reader.ReadBytes(buf).map_err(platform_err)?;
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::BluetoothRfcomm
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.socket.Close().map_err(platform_err)?;
        Ok(())
    }
}
