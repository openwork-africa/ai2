use std::path::Path;

pub fn in_service() -> bool {
    std::env::var_os(::setup::files::UI_SERVICE_ENV).is_some()
}

pub fn port() -> u16 {
    ::setup::files::ui_port(std::env::var(::setup::files::UI_PORT_ENV).ok())
}

pub fn running(home: &Path, port: u16, page: Option<&str>) -> Option<String> {
    if port == 0 {
        return None;
    }
    apps::Ui::find(home, port)?;
    let nonce = crate::nonce::load(home).ok()?;
    Some(crate::server::address(port, &nonce, page))
}

pub fn reuse(home: &Path, port: u16, page: Option<&str>) -> Option<String> {
    if in_service() {
        return None;
    }
    running(home, port, page)
}
