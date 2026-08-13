use std::os::fd::OwnedFd;

use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SourceType},
    PersistMode, Session,
};

use crate::error::{CaptureError, Result};

pub struct PortalSession {
    // A sessão deve ser destruída antes do proxy.
    _session: Session<'static, Screencast<'static>>,
    _screencast: Screencast<'static>,
    fd: Option<OwnedFd>,
    pub node_id: u32,
    pub width: u32,
    pub height: u32,
}

impl PortalSession {
    pub async fn new() -> Result<Self> {
        let screencast = Screencast::new().await?;
        let session = screencast.create_session().await?;

        screencast
            .select_sources(
                &session,
                CursorMode::Hidden,
                SourceType::Window.into(),
                false,
                None,
                PersistMode::DoNot,
            )
            .await?;

        let response = screencast.start(&session, None).await?.response()?;
        let stream = response
            .streams()
            .first()
            .ok_or_else(|| CaptureError::PortalState("nenhum stream foi selecionado".into()))?;

        let node_id = stream.pipe_wire_node_id();
        let (width, height) = match stream.size() {
            Some((width, height)) => (
                u32::try_from(width).map_err(|_| {
                    CaptureError::PortalState(format!(
                        "largura inválida informada pelo portal: {width}"
                    ))
                })?,
                u32::try_from(height).map_err(|_| {
                    CaptureError::PortalState(format!(
                        "altura inválida informada pelo portal: {height}"
                    ))
                })?,
            ),
            // O tamanho é opcional na resposta do portal e será negociado pelo
            // PipeWire. Zero aqui significa "ainda desconhecido".
            None => (0, 0),
        };

        let fd: OwnedFd = screencast.open_pipe_wire_remote(&session).await?.into();

        Ok(Self {
            _session: session,
            _screencast: screencast,
            fd: Some(fd),
            node_id,
            width,
            height,
        })
    }

    pub fn take_fd(&mut self) -> Result<OwnedFd> {
        self.fd.take().ok_or_else(|| {
            CaptureError::PortalState("o descritor PipeWire já foi consumido".into())
        })
    }
}
