use tracing::info;

use crate::{error::Result, frame::Frame, pipewire::PipeWireStream, portal::PortalSession};

pub struct ScreenCapture {
    // Mantém a permissão e a sessão D-Bus vivas durante toda a captura.
    _portal: PortalSession,
    stream: PipeWireStream,
    running: bool,
}

impl ScreenCapture {
    pub async fn new(fps: u32) -> Result<Self> {
        let mut portal = PortalSession::new().await?;
        let node_id = portal.node_id;
        let width = portal.width;
        let height = portal.height;
        let fd = portal.take_fd()?;

        info!(node_id, width, height, "Portal conectado");

        let stream = PipeWireStream::new(fd, node_id, width, height, fps)?;

        Ok(Self {
            _portal: portal,
            stream,
            running: true,
        })
    }

    pub fn try_receive_frame(&mut self) -> Option<Frame> {
        if !self.running {
            return None;
        }

        self.stream.try_receive_frame()
    }

    pub fn stop(&mut self) {
        if !self.running {
            return;
        }

        self.running = false;
        self.stream.stop();
        info!("Captura parada");
    }

    pub fn width(&self) -> u32 {
        self.stream.width()
    }

    pub fn height(&self) -> u32 {
        self.stream.height()
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
