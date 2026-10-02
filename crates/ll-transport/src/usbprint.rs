//! Windows USB transport through the system's printer class driver
//! (`usbprint.sys`), which Windows binds to the printer automatically.
//!
//! `nusb` needs WinUSB on the interface and fails with "incompatible driver
//! is installed for this device" when `usbprint.sys` (or the manufacturer's
//! driver) owns it (user report 2026-10-02). `usbprint.sys` exposes a
//! device interface (`GUID_DEVINTERFACE_USBPRINT`) that can be opened like a
//! file: writes go to the bulk OUT endpoint, reads come from bulk IN.
//!
//! TODO(verify): reading the 32-byte status through `usbprint.sys` on a real
//! PT-P710BT (bidirectional support) — see `docs/PROTOCOL.md`.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Foundation::HWND;

use crate::{Transport, TransportError, TransportKind};

/// Device interface class of `usbprint.sys` printers.
const GUID_DEVINTERFACE_USBPRINT: GUID = GUID::from_u128(0x28d78fad_5a12_11d1_ae5b_0000f803a8c2);

/// Pause between reads while waiting for the status block.
const READ_POLL: Duration = Duration::from_millis(10);

/// Device paths (`\\?\usb#vid_04f9&pid_20af#...#{28d78fad-...}`) of all
/// present `usbprint.sys` printers.
pub fn device_paths() -> Result<Vec<String>, TransportError> {
    let mut paths = Vec::new();
    // SAFETY: plain SetupAPI enumeration; every buffer passed is sized as
    // the API requests and the device info set is destroyed at the end.
    unsafe {
        let set = SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_USBPRINT),
            PCWSTR::null(),
            HWND::default(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
        .map_err(|e| TransportError::Platform(e.to_string()))?;
        for index in 0.. {
            let mut data = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInterfaces(set, None, &GUID_DEVINTERFACE_USBPRINT, index, &mut data)
                .is_err()
            {
                break; // no more interfaces
            }
            let mut needed = 0u32;
            // First call only reports the size (and "fails" doing so).
            let _ = SetupDiGetDeviceInterfaceDetailW(set, &data, None, 0, Some(&mut needed), None);
            if needed == 0 {
                continue;
            }
            // u32 buffer: the detail struct needs 4-byte alignment.
            let mut buf = vec![0u32; (needed as usize).div_ceil(4)];
            let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(set, &data, Some(detail), needed, None, None)
                .is_err()
            {
                continue;
            }
            let start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
            let max = (needed as usize).saturating_sub(4) / 2;
            let chars = std::slice::from_raw_parts(start, max);
            let len = chars.iter().position(|&c| c == 0).unwrap_or(max);
            paths.push(String::from_utf16_lossy(&chars[..len]));
        }
        let _ = SetupDiDestroyDeviceInfoList(set);
    }
    Ok(paths)
}

/// The device path of the printer with this VID/PID, if `usbprint.sys`
/// serves it.
pub fn find(vendor_id: u16, product_id: u16) -> Result<Option<String>, TransportError> {
    let needle = format!("vid_{vendor_id:04x}&pid_{product_id:04x}");
    Ok(device_paths()?
        .into_iter()
        .find(|p| p.to_ascii_lowercase().contains(&needle)))
}

/// An open `usbprint.sys` printer. File I/O blocks, so it runs on tokio's
/// blocking pool.
pub struct UsbPrintTransport {
    file: Arc<Mutex<File>>,
}

impl UsbPrintTransport {
    pub fn open(path: &str) -> Result<Self, TransportError> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }
}

fn poisoned() -> TransportError {
    TransportError::Platform("usbprint handle lock poisoned".into())
}

#[async_trait::async_trait]
impl Transport for UsbPrintTransport {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let file = self.file.clone();
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || -> Result<(), TransportError> {
            let mut f = file.lock().map_err(|_| poisoned())?;
            f.write_all(&data)?;
            f.flush()?;
            Ok(())
        })
        .await
        .map_err(|e| TransportError::Platform(e.to_string()))?
    }

    async fn read_exact_timeout(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<(), TransportError> {
        let file = self.file.clone();
        let len = buf.len();
        let read = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, TransportError> {
            let deadline = Instant::now() + timeout;
            let mut out = vec![0u8; len];
            let mut got = 0;
            let mut f = file.lock().map_err(|_| poisoned())?;
            while got < len {
                let n = f.read(&mut out[got..])?;
                got += n;
                if got < len {
                    if Instant::now() >= deadline {
                        return Err(TransportError::Timeout(len));
                    }
                    if n == 0 {
                        std::thread::sleep(READ_POLL);
                    }
                }
            }
            Ok(out)
        });
        let data = tokio::time::timeout(timeout + Duration::from_secs(1), read)
            .await
            .map_err(|_| TransportError::Timeout(len))?
            .map_err(|e| TransportError::Platform(e.to_string()))??;
        buf.copy_from_slice(&data);
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Usb
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
