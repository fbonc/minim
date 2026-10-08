use super::{Input, MainWindow, Output, WindowMode};

impl MainWindow {
    pub fn update(&mut self, input: Input) -> Option<Output> {
        match input {
            Input::Show => {
                self.visible = true;
                None
            }
            Input::DismissRequested => {
                self.visible = false;
                Some(Output::Dismissed)
            }
            Input::SettingsRequested => {
                self.mode = WindowMode::Settings;
                None
            }
            Input::BackRequested => {
                self.mode = WindowMode::Home;
                None
            }
        }
    }
}
