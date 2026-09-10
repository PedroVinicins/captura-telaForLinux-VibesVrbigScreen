use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[cfg(target_os = "linux")]
    #[error("Erro do portal: {0}")]
    Portal(#[from] ashpd::Error),

    #[cfg(target_os = "linux")]
    #[error("Estado inválido do portal: {0}")]
    PortalState(String),

    #[cfg(target_os = "linux")]
    #[error("Erro do PipeWire: {0}")]
    PipeWire(#[from] pipewire::Error),

    #[cfg(target_os = "linux")]
    #[error("Erro SPA: {0}")]
    Spa(String),

    #[cfg(target_os = "linux")]
    #[error("Erro de buffer: {0}")]
    Buffer(String),

    #[cfg(target_os = "macos")]
    #[error("Erro do ScreenCaptureKit: {0}")]
    ScreenCaptureKit(String),

    #[error("Erro de frame: {0}")]
    Frame(String),

    #[error("Erro de E/S: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CaptureError>;
