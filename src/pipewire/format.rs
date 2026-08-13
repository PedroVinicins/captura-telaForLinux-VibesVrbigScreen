use libspa::pod::{builder::Builder, Pod};

pub fn build_format_pod<'a>(_builder: &'a mut Builder<'a>) -> Option<&'a Pod> {
    // TODO:
    // libspa 0.10 não possui API segura para construir
    // um SPA_PARAM_Format de vídeo.
    //
    // Será necessário usar spa_sys ou criar um wrapper.

    None
}