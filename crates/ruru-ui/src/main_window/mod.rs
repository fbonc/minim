mod style;
mod update;
mod view;

#[derive(Debug, Clone)]
pub enum Input {
    Show,
    DismissRequested,
    SettingsRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Dismissed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WindowMode {
    #[default]
    Home,
    Settings,
}

#[derive(Debug)]
pub struct MainWindow {
    pub visible: bool,
    pub mode: WindowMode,
}

impl Default for MainWindow {
    fn default() -> Self {
        Self {
            visible: true,
            mode: WindowMode::default(),
        }
    }
}

impl MainWindow {
    pub fn new() -> Self {
        Self::default()
    }
}
