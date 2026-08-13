use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("Erro do portal: {0}")]
    Portal(#[from] ashpd::Error),

    #[error("Estado inválido do portal: {0}")]
    PortalState(String),

    #[error("Erro do PipeWire: {0}")]
    PipeWire(#[from] pipewire::Error),

    #[error("Erro SPA: {0}")]
    Spa(String),

    #[error("Erro de buffer: {0}")]
    Buffer(String),

    #[error("Erro de frame: {0}")]
    Frame(String),

    #[error("Erro de E/S: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CaptureError>;
