mod capture;
mod error;
mod frame;
#[cfg(target_os = "linux")]
mod pipewire;
#[cfg(target_os = "linux")]
mod portal;
mod vr;

use anyhow::Result;

const TARGET_FPS: u32 = 60;

fn main() -> Result<()> {
    vr::run(TARGET_FPS)?;

    Ok(())
}
