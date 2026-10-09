mod settings;

use ruru_types::ProviderId;

pub use settings::{ProviderView, Section};
mod style;
mod update;
mod view;

#[derive(Debug, Clone)]
pub enum Input {
    Show,
    DismissRequested,
    SettingsRequested,
    BackRequested,
    Settings(settings::Input),
}

#[derive(Clone, PartialEq, Eq)]
pub enum Output {
    Dismissed,
    JumpToSection(Section),
    DiscoverModels(ProviderId),
    SaveKey(ProviderId, String),
    RemoveKey(ProviderId),
    SetModel {
        provider: ProviderId,
        model: String,
        enabled: bool,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WindowMode {
    #[default]
    Home,
    Settings,
}

pub struct MainWindow {
    pub visible: bool,
    pub mode: WindowMode,
    settings: settings::Settings,
}

impl Default for MainWindow {
    fn default() -> Self {
        Self {
            visible: true,
            mode: WindowMode::default(),
            settings: settings::Settings::default(),
        }
    }
}

impl MainWindow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_providers(mut self, providers: Vec<ProviderView>) -> Self {
        self.settings = self.settings.with_providers(providers);
        self
    }

    pub fn jump_to_section<T>(&self, section: Section) -> iced::Task<T> {
        self.settings.jump_to_section(section)
    }

    pub fn key_saved(&mut self, id: &ProviderId) {
        self.settings.key_saved(id);
    }

    pub fn key_removed(&mut self, id: &ProviderId) {
        self.settings.key_removed(id);
    }

    pub fn set_models(&mut self, id: &ProviderId, models: Vec<String>) {
        self.settings.set_selected_models(id, models);
    }

    pub fn start_model_discovery(&mut self, id: &ProviderId) {
        self.settings.start_discovery(id);
    }

    pub fn finish_model_discovery(&mut self, id: &ProviderId, models: Vec<String>) {
        self.settings.finish_discovery(id, models);
    }

    pub fn provider_error(&mut self, id: &ProviderId, error: impl Into<String>) {
        self.settings.set_error(id, error.into());
    }

    pub fn settings_error(&mut self, error: impl Into<String>) {
        self.settings.set_global_error(error.into());
    }

    pub fn clear_settings_error(&mut self) {
        self.settings.clear_error();
    }
}
