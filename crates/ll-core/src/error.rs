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

    #[error("no printer connected")]
    NoDevice,
}
