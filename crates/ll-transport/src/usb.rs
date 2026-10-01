//! USB transport via `nusb` (pure Rust, no libusb), see ADR-013.
//!
//! The printer exposes a USB printer-class interface (class `0x07`) with one
//! bulk OUT endpoint for commands/raster data and one bulk IN endpoint for
//! the 32-byte status block.
//!
//! Platform notes:
//! - Linux: the kernel's `usblp` driver usually binds the printer interface;
//!   it is detached on open (`detach_and_claim_interface`). Non-root access
//!   needs a udev rule (see `docs/PROTOCOL.md`).
//! - Windows: `nusb` can only claim interfaces bound to WinUSB. With
//!   Windows' generic `usbprint.sys` (or Brother's driver) bound, opening
//!   fails. TODO(verify): whether the PT-P710BT works with WinUSB bound
//!   (e.g. via Zadig) — not tested on hardware yet.

use std::time::Duration;

use nusb::descriptors::{ConfigurationDescriptor, TransferType};
use nusb::io::{EndpointRead, EndpointWrite};
use nusb::transfer::{Bulk, Direction, In, Out};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{Transport, TransportError, TransportKind};

/// USB interface class code for printers (USB-IF "Printer Class").
const USB_CLASS_PRINTER: u8 = 0x07;

/// Transfer size for the buffered endpoint reader/writer. Raster rows are
/// tiny (≤ 19 bytes), so this mostly bounds how much is batched per
/// transfer; a multiple of the 64-byte full-speed packet size.
const TRANSFER_SIZE: usize = 512;

/// Upper bound for one bulk write to complete. The printer stalls OUT
/// transfers while it is busy feeding/printing, so this is generous.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

fn platform_err(e: nusb::Error) -> TransportError {
    TransportError::Platform(e.to_string())
}

/// One USB device as seen during enumeration. Not filtered: the caller
/// matches `vendor_id`/`product_id` against `ll_protocol::model::MODELS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

/// Lists all USB devices currently attached.
pub async fn list_devices() -> Result<Vec<UsbDeviceInfo>, TransportError> {
    let devices = nusb::list_devices().await.map_err(platform_err)?;
    Ok(devices
        .map(|d| UsbDeviceInfo {
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            product: d.product_string().map(str::to_owned),
            serial_number: d.serial_number().map(str::to_owned),
        })
        .collect())
}

/// Interface and endpoint addresses of a printer-class interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrinterEndpoints {
    pub interface: u8,
    pub alt_setting: u8,
    pub bulk_out: u8,
    pub bulk_in: u8,
}

/// Finds the first printer-class interface that has both a bulk OUT and a
/// bulk IN endpoint (the IN endpoint is needed for the status block).
pub fn find_printer_endpoints(config: &ConfigurationDescriptor<'_>) -> Option<PrinterEndpoints> {
    config.interface_alt_settings().find_map(|alt| {
        if alt.class() != USB_CLASS_PRINTER {
            return None;
        }
        let bulk = |dir: Direction| {
            alt.endpoints()
                .find(|ep| ep.transfer_type() == TransferType::Bulk && ep.direction() == dir)
                .map(|ep| ep.address())
        };
        Some(PrinterEndpoints {
            interface: alt.interface_number(),
            alt_setting: alt.alternate_setting(),
            bulk_out: bulk(Direction::Out)?,
            bulk_in: bulk(Direction::In)?,
        })
    })
}

/// A connection to the printer over USB.
pub struct UsbTransport {
    writer: EndpointWrite<Bulk>,
    reader: EndpointRead<Bulk>,
}

impl UsbTransport {
    /// Opens the first attached device with `vendor_id:product_id` (and
    /// `serial_number`, if given) and claims its printer interface.
    pub async fn open(
        vendor_id: u16,
        product_id: u16,
        serial_number: Option<&str>,
    ) -> Result<Self, TransportError> {
        let info = nusb::list_devices()
            .await
            .map_err(platform_err)?
            .find(|d| {
                d.vendor_id() == vendor_id
                    && d.product_id() == product_id
                    && serial_number.is_none_or(|s| d.serial_number() == Some(s))
            })
            .ok_or_else(|| {
                TransportError::DeviceNotFound(format!(
                    "{vendor_id:04x}:{product_id:04x}{}",
                    serial_number.map(|s| format!(" ({s})")).unwrap_or_default()
                ))
            })?;

        let device = info.open().await.map_err(platform_err)?;
        let endpoints = {
            let config = device
                .active_configuration()
                .map_err(|e| TransportError::Platform(e.to_string()))?;
            find_printer_endpoints(&config).ok_or_else(|| {
                TransportError::Platform(
                    "no printer-class interface with bulk IN/OUT endpoints".into(),
                )
            })?
        };

        let interface = device
            .detach_and_claim_interface(endpoints.interface)
            .await
            .map_err(platform_err)?;
        if endpoints.alt_setting != 0 {
            interface
                .set_alt_setting(endpoints.alt_setting)
                .await
                .map_err(platform_err)?;
        }

        let writer = interface
            .endpoint::<Bulk, Out>(endpoints.bulk_out)
            .map_err(platform_err)?
            .writer(TRANSFER_SIZE)
            .with_write_timeout(WRITE_TIMEOUT);
        let reader = interface
            .endpoint::<Bulk, In>(endpoints.bulk_in)
            .map_err(platform_err)?
            .reader(TRANSFER_SIZE);

        Ok(Self { writer, reader })
    }
}

#[async_trait::async_trait]
impl Transport for UsbTransport {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.writer.write_all(data).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn read_exact_timeout(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<(), TransportError> {
        tokio::time::timeout(timeout, self.reader.read_exact(buf))
            .await
            .map_err(|_| TransportError::Timeout(buf.len()))??;
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Usb
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.writer.flush().await?;
        self.reader.cancel_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(address, bmAttributes)` of one endpoint descriptor.
    type Ep = (u8, u8);

    /// Builds a configuration descriptor with the given interfaces, each a
    /// `(number, class, endpoints)` triple.
    fn config_bytes(interfaces: &[(u8, u8, &[Ep])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (number, class, endpoints) in interfaces {
            body.extend_from_slice(&[
                9,
                4, // INTERFACE
                *number,
                0, // bAlternateSetting
                endpoints.len() as u8,
                *class,
                1, // bInterfaceSubClass (printer)
                2, // bInterfaceProtocol (bidirectional)
                0,
            ]);
            for (address, attributes) in *endpoints {
                body.extend_from_slice(&[7, 5, *address, *attributes, 64, 0, 0]);
            }
        }
        let total = (9 + body.len()) as u16;
        let mut out = vec![
            9,
            2, // CONFIGURATION
            total.to_le_bytes()[0],
            total.to_le_bytes()[1],
            interfaces.len() as u8,
            1,
            0,
            0x80,
            50,
        ];
        out.extend(body);
        out
    }

    const BULK: u8 = 0x02;
    const INTERRUPT: u8 = 0x03;

    #[test]
    fn finds_bulk_endpoints_of_printer_interface() {
        let bytes = config_bytes(&[(0, USB_CLASS_PRINTER, &[(0x02, BULK), (0x81, BULK)])]);
        let config = ConfigurationDescriptor::new(&bytes).unwrap();
        assert_eq!(
            find_printer_endpoints(&config),
            Some(PrinterEndpoints {
                interface: 0,
                alt_setting: 0,
                bulk_out: 0x02,
                bulk_in: 0x81,
            })
        );
    }

    #[test]
    fn skips_non_printer_interfaces() {
        let bytes = config_bytes(&[
            (0, 0x08, &[(0x01, BULK), (0x82, BULK)]), // mass storage
            (1, USB_CLASS_PRINTER, &[(0x03, BULK), (0x84, BULK)]),
        ]);
        let config = ConfigurationDescriptor::new(&bytes).unwrap();
        let ep = find_printer_endpoints(&config).unwrap();
        assert_eq!((ep.interface, ep.bulk_out, ep.bulk_in), (1, 0x03, 0x84));
    }

    #[test]
    fn requires_bulk_in_endpoint() {
        let bytes = config_bytes(&[(0, USB_CLASS_PRINTER, &[(0x02, BULK), (0x81, INTERRUPT)])]);
        let config = ConfigurationDescriptor::new(&bytes).unwrap();
        assert_eq!(find_printer_endpoints(&config), None);
    }
}
