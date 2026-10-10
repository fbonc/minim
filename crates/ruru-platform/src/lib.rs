#[cfg(target_os = "macos")]
mod hotkey;
#[cfg(target_os = "macos")]
mod reopen;

#[cfg(target_os = "macos")]
pub use reopen::install as install_reopen_handler;

#[derive(Debug)]
pub enum Error {
    Register(String),
    Recv(String),
}

pub trait Hotkey: Send + Sync {
    fn recv(&self) -> Result<(), Error>;
}

#[cfg(target_os = "macos")]
pub fn new_hotkey(accelerator: &str) -> Result<Box<dyn Hotkey>, Error> {
    Ok(Box::new(hotkey::MacosHotkey::new(accelerator)?))
}
