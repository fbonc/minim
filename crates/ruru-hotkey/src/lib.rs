#[cfg(target_os = "macos")]
mod macos;

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
    Ok(Box::new(macos::MacosHotkey::new(accelerator)?))
}
