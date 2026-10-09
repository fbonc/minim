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
                self.settings.discard_draft();
                Some(Output::Dismissed)
            }
            Input::SettingsRequested => {
                self.mode = WindowMode::Settings;
                None
            }
            Input::BackRequested => {
                self.mode = WindowMode::Home;
                self.settings.discard_draft();
                None
            }
            Input::Settings(input) => self.settings.update(input),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_and_back_switch_modes() {
        let mut window = MainWindow::new();

        assert!(window.update(Input::SettingsRequested).is_none());
        assert_eq!(window.mode, WindowMode::Settings);

        assert!(window.update(Input::BackRequested).is_none());
        assert_eq!(window.mode, WindowMode::Home);
    }
}
