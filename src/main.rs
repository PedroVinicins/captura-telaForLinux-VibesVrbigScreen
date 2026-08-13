mod capture;
mod error;
mod frame;
mod pipewire;
mod portal;
mod vr;

use anyhow::Result;
use tokio::runtime::Runtime;

use capture::ScreenCapture;

const TARGET_FPS: u32 = 60;

fn main() -> Result<()> {
    // O runtime precisa permanecer vivo enquanto a sessão do portal estiver aberta.
    let runtime = Runtime::new()?;
    let capture = runtime.block_on(ScreenCapture::new(TARGET_FPS))?;

    vr::run(capture, runtime);

    Ok(())
}

