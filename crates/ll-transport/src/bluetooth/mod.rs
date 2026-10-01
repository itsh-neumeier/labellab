//! Native Bluetooth RFCOMM transport: WinRT on Windows, BlueZ (`bluer`)
//! on Linux. Both expose device listing, discovery of unpaired devices,
//! pairing and a [`crate::Transport`]; `ll-core` hides the differences
//! (sync WinRT calls vs. async D-Bus, service id vs. MAC + channel).

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    list_paired_devices, list_unpaired_devices, pair, BluetoothDeviceInfo, BluetoothTransport,
};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{discover, list_devices, pair, BluetoothDeviceInfo, BluetoothTransport};
