//! Device discovery and status queries.
//!
//! Two transports exist today: serial (COM-port / BT-SPP fallback, see
//! `docs/PROTOCOL.md`) and, on Windows, native Bluetooth RFCOMM via WinRT
//! (bypasses the flaky BT-SPP virtual-COM-port shim, see ADR-002 and
//! `docs/PROGRESS.md`). Linux native Bluetooth (BlueZ) is still a TODO.
//! USB goes through `nusb` on both platforms (ADR-013).

use std::time::Duration;

use ll_protocol::model::{ModelInfo, MODELS};
use ll_protocol::{command, status::StatusBlock};
use ll_transport::usb::{UsbDeviceInfo, UsbTransport};
use ll_transport::{serial::SerialTransport, Transport};

use crate::CoreError;

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// How to reach a printer; shared by the CLI flags and the GUI's device
/// picker. Serialized with a `"kind"` tag for the GUI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Connection {
    /// COM port / `/dev/tty*`, incl. Bluetooth-SPP virtual ports.
    Serial { port: String, baud_rate: u32 },
    /// Native Bluetooth RFCOMM by OS device id (Windows only so far).
    Bluetooth { device_id: String },
    /// USB, picked by [`select_usb_printer`] (`None`: first found).
    Usb { spec: Option<String> },
}

/// Opens the transport described by `connection`.
pub async fn connect(connection: &Connection) -> Result<Box<dyn Transport>, CoreError> {
    match connection {
        Connection::Serial { port, baud_rate } => {
            Ok(Box::new(SerialTransport::open(port, *baud_rate)?))
        }
        Connection::Usb { spec } => Ok(Box::new(open_usb(spec.as_deref()).await?)),
        #[cfg(windows)]
        Connection::Bluetooth { device_id } => Ok(Box::new(
            ll_transport::bluetooth::BluetoothTransport::connect(device_id).await?,
        )),
        #[cfg(not(windows))]
        Connection::Bluetooth { .. } => Err(CoreError::Unsupported(
            "native Bluetooth is not implemented on this platform yet (BlueZ pending)",
        )),
    }
}

/// Connects, resets the printer and reads its status block.
pub async fn query_status_on(connection: &Connection) -> Result<StatusBlock, CoreError> {
    let mut transport = connect(connection).await?;
    let status = query_status(transport.as_mut()).await;
    transport.close().await?;
    status
}

/// Lists available serial ports (COM-ports on Windows, `/dev/tty*` on
/// Linux), including Bluetooth-SPP virtual COM ports.
pub fn list_serial_devices() -> Result<Vec<String>, CoreError> {
    Ok(ll_transport::serial::list_ports()?)
}

/// An attached USB device whose VID:PID matches a known model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbPrinter {
    pub model: &'static ModelInfo,
    pub serial_number: Option<String>,
}

impl UsbPrinter {
    /// `VVVV:PPPP` identifier, as accepted by [`select_usb_printer`].
    pub fn usb_id(&self) -> String {
        format!("{:04x}:{:04x}", self.model.usb_vid, self.model.usb_pid)
    }
}

/// Keeps only devices whose VID:PID is in `ll_protocol::model::MODELS`.
pub fn filter_usb_printers(devices: Vec<UsbDeviceInfo>) -> Vec<UsbPrinter> {
    devices
        .into_iter()
        .filter_map(|d| {
            MODELS
                .iter()
                .find(|m| m.usb_vid == d.vendor_id && m.usb_pid == d.product_id)
                .map(|model| UsbPrinter {
                    model,
                    serial_number: d.serial_number,
                })
        })
        .collect()
}

/// Lists attached USB printers of known models.
pub async fn list_usb_printers() -> Result<Vec<UsbPrinter>, CoreError> {
    Ok(filter_usb_printers(
        ll_transport::usb::list_devices().await?,
    ))
}

/// Picks a printer by `spec`: `None` takes the first one; otherwise `spec`
/// matches a model name (`PT-P710BT`), a `VVVV:PPPP` id or a serial number
/// (all case-insensitive).
pub fn select_usb_printer<'a>(
    printers: &'a [UsbPrinter],
    spec: Option<&str>,
) -> Option<&'a UsbPrinter> {
    let Some(spec) = spec else {
        return printers.first();
    };
    printers.iter().find(|p| {
        p.model.name.eq_ignore_ascii_case(spec)
            || p.usb_id().eq_ignore_ascii_case(spec)
            || p.serial_number
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(spec))
    })
}

/// Opens the USB printer selected by `spec` (see [`select_usb_printer`]).
pub async fn open_usb(spec: Option<&str>) -> Result<UsbTransport, CoreError> {
    let printers = list_usb_printers().await?;
    let printer = select_usb_printer(&printers, spec).ok_or(CoreError::NoDevice)?;
    Ok(UsbTransport::open(
        printer.model.usb_vid,
        printer.model.usb_pid,
        printer.serial_number.as_deref(),
    )
    .await?)
}

#[cfg(windows)]
pub use bluetooth::list_bluetooth_devices;

#[cfg(windows)]
mod bluetooth {
    use ll_transport::bluetooth::{self, BluetoothDeviceInfo};

    use super::*;

    /// Lists paired Bluetooth devices offering the Serial Port Profile.
    /// Not filtered by name; multiple PT-P7xx/E7xx models exist, see
    /// `ll_protocol::model`.
    pub fn list_bluetooth_devices() -> Result<Vec<BluetoothDeviceInfo>, CoreError> {
        Ok(bluetooth::list_paired_devices()?)
    }
}

pub(crate) async fn query_status(transport: &mut dyn Transport) -> Result<StatusBlock, CoreError> {
    transport.write_all(&command::invalidate()).await?;
    transport.write_all(&command::initialize()).await?;
    transport.write_all(&command::status_request()).await?;

    let mut buf = [0u8; ll_protocol::status::STATUS_BLOCK_LEN];
    transport
        .read_exact_timeout(&mut buf, STATUS_TIMEOUT)
        .await?;

    Ok(StatusBlock::parse(&buf)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb(vid: u16, pid: u16, serial: Option<&str>) -> UsbDeviceInfo {
        UsbDeviceInfo {
            vendor_id: vid,
            product_id: pid,
            product: None,
            serial_number: serial.map(str::to_owned),
        }
    }

    #[test]
    fn filters_unknown_usb_devices() {
        let printers = filter_usb_printers(vec![
            usb(0x046d, 0xc52b, None), // a mouse receiver
            usb(0x04f9, 0x20af, Some("A1")),
        ]);
        assert_eq!(printers.len(), 1);
        assert_eq!(printers[0].model.name, "PT-P710BT");
        assert_eq!(printers[0].usb_id(), "04f9:20af");
    }

    #[test]
    fn selects_usb_printer_by_spec() {
        let printers = filter_usb_printers(vec![
            usb(0x04f9, 0x20af, Some("A1")),
            usb(0x04f9, 0x224a, Some("B2")),
        ]);
        let pick = |spec| select_usb_printer(&printers, spec).map(|p| p.model.name);
        assert_eq!(pick(None), Some("PT-P710BT"));
        assert_eq!(pick(Some("pt-e720bt")), Some("PT-E720BT"));
        assert_eq!(pick(Some("04F9:224A")), Some("PT-E720BT"));
        assert_eq!(pick(Some("b2")), Some("PT-E720BT"));
        assert_eq!(pick(Some("nope")), None);
    }
}
