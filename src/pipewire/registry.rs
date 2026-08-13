use anyhow::Result;
use pipewire::{
    core::CoreRc,
    registry::{RegistryRc, Listener},
};

pub struct RegistryState {
    pub registry: RegistryRc,
    _listener: Listener,
}

pub fn setup_registry(core: &CoreRc) -> Result<RegistryState> {
    let registry = core.get_registry_rc()?;

    let listener = registry
        .add_listener_local()
        .global(|global| {
            tracing::debug!(
                "Global ID: {}, Tipo: {}",
                global.id,
                global.type_
            );
        })
        .register();

    Ok(RegistryState {
        registry,
        _listener: listener,
    })
}