//! Native Bluetooth RFCOMM transport for Linux via BlueZ (`bluer`, D-Bus).
//!
//! Devices are addressed by their MAC address (`AA:BB:CC:DD:EE:FF`). The
//! RFCOMM channel is passed in by the caller (`ll-core` takes it from the
//! model table) because `bluer` has no SDP client to look it up.
//!
//! Not yet tested against real hardware (see `docs/PROGRESS.md`).

use std::str::FromStr;
use std::time::Duration;

use bluer::agent::{Agent, AgentHandle};
use bluer::rfcomm::{SocketAddr, Stream};
use bluer::{Address, Session};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{Transport, TransportError, TransportKind};

/// How long [`discover`] scans for nearby devices.
const DISCOVERY_TIME: Duration = Duration::from_secs(8);

fn platform_err(e: bluer::Error) -> TransportError {
    TransportError::Platform(e.to_string())
}

/// One Bluetooth device known to BlueZ.
#[derive(Debug, Clone)]
pub struct BluetoothDeviceInfo {
    /// MAC address, used as the connection id.
    pub id: String,
    /// Device name (alias), e.g. `PT-P710BT5265`.
    pub name: String,
    /// Same as `name` (Windows lists the SPP service name separately).
    pub service_name: String,
    pub paired: bool,
}

fn parse_address(id: &str) -> Result<Address, TransportError> {
    Address::from_str(id.trim())
        .map_err(|_| TransportError::DeviceNotFound(format!("invalid Bluetooth address {id:?}")))
}

async fn adapter(session: &Session) -> Result<bluer::Adapter, TransportError> {
    let adapter = session.default_adapter().await.map_err(platform_err)?;
    adapter.set_powered(true).await.map_err(platform_err)?;
    Ok(adapter)
}

/// Devices BlueZ already knows (paired, or seen during a recent scan).
pub async fn list_devices() -> Result<Vec<BluetoothDeviceInfo>, TransportError> {
    let session = Session::new().await.map_err(platform_err)?;
    let adapter = adapter(&session).await?;
    let mut out = Vec::new();
    for addr in adapter.device_addresses().await.map_err(platform_err)? {
        let device = adapter.device(addr).map_err(platform_err)?;
        let name = device.alias().await.map_err(platform_err)?;
        let paired = device.is_paired().await.map_err(platform_err)?;
        out.push(BluetoothDeviceInfo {
            id: addr.to_string(),
            service_name: name.clone(),
            name,
            paired,
        });
    }
    Ok(out)
}

/// Scans for nearby devices for a few seconds, then returns everything
/// BlueZ knows (including the newly found ones).
pub async fn discover() -> Result<Vec<BluetoothDeviceInfo>, TransportError> {
    use tokio_stream::StreamExt;

    let session = Session::new().await.map_err(platform_err)?;
    let adapter = adapter(&session).await?;
    {
        // Discovery runs while the event stream is alive; the block ends it.
        let events = adapter.discover_devices().await.map_err(platform_err)?;
        tokio::pin!(events);
        let deadline = tokio::time::sleep(DISCOVERY_TIME);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                ev = events.next() => if ev.is_none() { break },
            }
        }
    }
    list_devices().await
}

/// Pairs (and trusts) the device, answering PIN requests with `pin` and
/// confirming passkeys automatically.
pub async fn pair(id: &str, pin: &str) -> Result<(), TransportError> {
    let addr = parse_address(id)?;
    let session = Session::new().await.map_err(platform_err)?;
    let pin = pin.to_string();
    let agent = Agent {
        request_default: true,
        request_pin_code: Some(Box::new(move |_| {
            let pin = pin.clone();
            Box::pin(async move { Ok(pin) })
        })),
        request_confirmation: Some(Box::new(|_| Box::pin(async { Ok(()) }))),
        request_authorization: Some(Box::new(|_| Box::pin(async { Ok(()) }))),
        authorize_service: Some(Box::new(|_| Box::pin(async { Ok(()) }))),
        ..Agent::default()
    };
    let _handle: AgentHandle = session.register_agent(agent).await.map_err(platform_err)?;
    let adapter = adapter(&session).await?;
    let device = adapter.device(addr).map_err(platform_err)?;
    if !device.is_paired().await.map_err(platform_err)? {
        device.pair().await.map_err(platform_err)?;
    }
    device.set_trusted(true).await.map_err(platform_err)?;
    Ok(())
}

/// A connection to the printer over BlueZ RFCOMM.
pub struct BluetoothTransport {
    stream: Stream,
}

impl BluetoothTransport {
    /// Connects to `id` (MAC address) on RFCOMM `channel`.
    pub async fn connect(id: &str, channel: u8) -> Result<Self, TransportError> {
        let addr = parse_address(id)?;
        let stream = Stream::connect(SocketAddr::new(addr, channel))
            .await
            .map_err(TransportError::Io)?;
        Ok(Self { stream })
    }
}

#[async_trait::async_trait]
impl Transport for BluetoothTransport {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.stream
            .write_all(data)
            .await
            .map_err(TransportError::Io)
    }

    async fn read_exact_timeout(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<(), TransportError> {
        tokio::time::timeout(timeout, self.stream.read_exact(buf))
            .await
            .map_err(|_| TransportError::Timeout(buf.len()))?
            .map_err(TransportError::Io)?;
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::BluetoothRfcomm
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.stream.shutdown().await.map_err(TransportError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mac_addresses() {
        assert!(parse_address("b4:22:00:eb:96:6f").is_ok());
        assert!(parse_address("not a mac").is_err());
    }
}
