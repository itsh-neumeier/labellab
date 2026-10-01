//! In-memory transport for hardware-free tests. Records every byte written
//! and serves pre-programmed responses to reads.

use std::collections::VecDeque;
use std::time::Duration;

use crate::{Transport, TransportError, TransportKind};

/// Records all written bytes and replays queued responses on read.
#[derive(Debug, Default)]
pub struct MockTransport {
    written: Vec<u8>,
    responses: VecDeque<Vec<u8>>,
    closed: bool,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues bytes to be returned by the next `read_exact_timeout` call(s).
    pub fn push_response(&mut self, data: impl Into<Vec<u8>>) {
        self.responses.push_back(data.into());
    }

    /// All bytes written so far, in order.
    pub fn written(&self) -> &[u8] {
        &self.written
    }
}

#[async_trait::async_trait]
impl Transport for MockTransport {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if self.closed {
            return Err(TransportError::NotConnected);
        }
        self.written.extend_from_slice(data);
        Ok(())
    }

    async fn read_exact_timeout(
        &mut self,
        buf: &mut [u8],
        _timeout: Duration,
    ) -> Result<(), TransportError> {
        if self.closed {
            return Err(TransportError::NotConnected);
        }
        let Some(chunk) = self.responses.pop_front() else {
            return Err(TransportError::Timeout(buf.len()));
        };
        if chunk.len() != buf.len() {
            return Err(TransportError::Timeout(buf.len()));
        }
        buf.copy_from_slice(&chunk);
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Mock
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_written_bytes() {
        let mut t = MockTransport::new();
        t.write_all(&[0x1B, 0x40]).await.unwrap();
        assert_eq!(t.written(), &[0x1B, 0x40]);
    }

    #[tokio::test]
    async fn replays_queued_response() {
        let mut t = MockTransport::new();
        t.push_response(vec![0x80; 32]);
        let mut buf = [0u8; 32];
        t.read_exact_timeout(&mut buf, Duration::from_millis(10))
            .await
            .unwrap();
        assert_eq!(buf[0], 0x80);
    }

    #[tokio::test]
    async fn errors_after_close() {
        let mut t = MockTransport::new();
        t.close().await.unwrap();
        assert!(t.write_all(&[0x00]).await.is_err());
    }
}
