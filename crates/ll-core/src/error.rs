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
