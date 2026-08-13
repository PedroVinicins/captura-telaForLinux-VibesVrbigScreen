use anyhow::Result;
use pipewire::{
    context::ContextRc,
    core::CoreRc,
    main_loop::MainLoopRc,
};
use std::os::fd::OwnedFd;

pub struct PwEnvironment {
    pub mainloop: MainLoopRc,
    pub context: ContextRc,
    pub core: CoreRc,
}

impl PwEnvironment {
    pub fn new(fd: OwnedFd) -> Result<Self> {
        // O pipewire::init() deve ser chamado antes de criar este ambiente.

        let mainloop = MainLoopRc::new(None)?;
        let context = ContextRc::new(&mainloop, None)?;

        let core = context.connect_fd_rc(fd, None)?;

        Ok(Self {
            mainloop,
            context,
            core,
        })
    }
}

// VAI SE ESSA 