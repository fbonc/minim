use super::{Input, MainWindow, Output};

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
            Input::SettingsRequested => None,
        }
    }
}
