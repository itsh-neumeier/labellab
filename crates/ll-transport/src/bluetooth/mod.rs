//! Native Bluetooth RFCOMM transport. Windows uses WinRT directly; Linux
//! (BlueZ via `bluer`) is a TODO for a later pass (see `docs/PROGRESS.md`).

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{list_paired_devices, BluetoothDeviceInfo, BluetoothTransport};
