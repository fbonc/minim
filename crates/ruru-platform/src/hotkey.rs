use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::{Error, Hotkey};

pub struct MacosHotkey {
    _manager: GlobalHotKeyManager,
    id: u32,
}

impl MacosHotkey {
    pub fn new(accelerator: &str) -> Result<Self, Error> {
        let hotkey: HotKey = accelerator
            .parse()
            .map_err(|e| Error::Register(format!("invalid accelerator {accelerator:?}: {e}")))?;
        let manager = GlobalHotKeyManager::new().map_err(|e| Error::Register(e.to_string()))?;
        manager
            .register(hotkey)
            .map_err(|e| Error::Register(e.to_string()))?;
        Ok(Self {
            _manager: manager,
            id: hotkey.id(),
        })
    }
}

impl Hotkey for MacosHotkey {
    fn recv(&self) -> Result<(), Error> {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            let event = receiver.recv().map_err(|e| Error::Recv(e.to_string()))?;
            if is_registered_press(&event, self.id) {
                return Ok(());
            }
        }
    }
}

fn is_registered_press(event: &GlobalHotKeyEvent, registered_id: u32) -> bool {
    event.id() == registered_id && event.state() == HotKeyState::Pressed
}
