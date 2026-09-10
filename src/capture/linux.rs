use tokio::runtime::Runtime;
use tracing::info;

use crate::{error::Result, frame::Frame, pipewire::PipeWireStream, portal::PortalSession};

pub struct ScreenCapture {
    // A captura e a sessão devem ser destruídas antes do runtime.
    _portal: PortalSession,
    stream: PipeWireStream,
    _runtime: Runtime,
    running: bool,
}

impl ScreenCapture {
    pub fn new(fps: u32) -> Result<Self> {
        // O runtime precisa permanecer vivo enquanto a sessão do portal estiver aberta.
        let runtime = Runtime::new()?;
        let mut portal = runtime.block_on(PortalSession::new())?;
        let node_id = portal.node_id;
        let width = portal.width;
        let height = portal.height;
        let fd = portal.take_fd()?;

        info!(node_id, width, height, "Portal conectado");

        let stream = PipeWireStream::new(fd, node_id, width, height, fps)?;

        Ok(Self {
            _portal: portal,
            stream,
            _runtime: runtime,
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
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
