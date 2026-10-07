mod style;
mod update;
mod view;

#[derive(Debug, Clone)]
pub enum Input {
    Show,
    DismissRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Dismissed,
}

#[derive(Debug)]
pub struct MainWindow {
    pub visible: bool,
}

impl Default for MainWindow {
    fn default() -> Self {
        Self { visible: true }
    }
}

impl MainWindow {
    pub fn new() -> Self {
        Self::default()
    }
}
