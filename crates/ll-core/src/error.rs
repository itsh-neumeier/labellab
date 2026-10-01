use thiserror::Error;

/// Top-level errors surfaced by `ll-core` to the CLI and the GUI.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Protocol(#[from] ll_protocol::ProtocolError),

    #[error(transparent)]
    Transport(#[from] ll_transport::TransportError),

    #[error(transparent)]
    Render(#[from] ll_render::RenderError),

    #[error("invalid label template: {0}")]
    Template(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("not supported: {0}")]
    Unsupported(&'static str),

    #[error("no printer connected")]
    NoDevice,

    #[error(
        "printer reports an error (raw, bit meaning unverified): error1={error1:#04x} error2={error2:#04x}"
    )]
    PrinterError { error1: u8, error2: u8 },
}

impl CoreError {
    /// Stable, machine-readable error code (snake_case) so frontends can
    /// show a translated, user-friendly message instead of the raw text.
    pub fn code(&self) -> &'static str {
        use ll_protocol::ProtocolError as P;
        use ll_render::RenderError as R;
        use ll_transport::TransportError as T;
        match self {
            CoreError::Transport(T::Timeout(_)) => "timeout",
            CoreError::Transport(T::NotConnected) | CoreError::NoDevice => "no_device",
            CoreError::Transport(T::DeviceNotFound(_)) => "device_not_found",
            CoreError::Transport(T::Io(_)) => "connection",
            CoreError::Transport(T::Platform(_)) => "platform",
            CoreError::Protocol(P::UnsupportedTapeWidth(_)) => "tape_width",
            CoreError::Protocol(P::UnknownModel { .. }) => "unknown_model",
            CoreError::Protocol(_) => "protocol",
            CoreError::Render(R::HeightExceedsTape { .. }) => "too_tall",
            CoreError::Render(R::NoSystemFont | R::Font(_)) => "font",
            CoreError::Render(R::Image(_)) => "image",
            CoreError::Render(R::Barcode(_)) => "barcode",
            CoreError::Render(_) => "render",
            CoreError::Template(_) => "template",
            CoreError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => "file_not_found",
            CoreError::Io(_) => "io",
            CoreError::Unsupported(_) => "unsupported",
            CoreError::PrinterError { .. } => "printer_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        let timeout = CoreError::from(ll_transport::TransportError::Timeout(32));
        assert_eq!(timeout.code(), "timeout");
        let missing = CoreError::from(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert_eq!(missing.code(), "file_not_found");
        let printer = CoreError::PrinterError {
            error1: 1,
            error2: 0,
        };
        assert_eq!(printer.code(), "printer_error");
    }
}
