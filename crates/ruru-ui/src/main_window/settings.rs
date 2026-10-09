use iced::widget::{
    Id, button, checkbox, column, container, operation, pick_list, row, rule, scrollable, space,
    text, text_input,
};
use iced::{Center, Element, Fill, Task};
use ruru_types::ProviderId;

use super::{Output, style};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Providers,
}

#[derive(Clone)]
pub enum Input {
    SectionSelected(Section),
    ProviderSelected(ProviderId),
    ProviderRowToggled(ProviderId),
    ApiKeyChanged(ProviderId, String),
    SaveKeyRequested(ProviderId),
    RemoveKeyRequested(ProviderId),
    RefreshModelsRequested(ProviderId),
    ModelToggled(ProviderId, String, bool),
}

impl std::fmt::Debug for Input {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SectionSelected(section) => formatter
                .debug_tuple("SectionSelected")
                .field(section)
                .finish(),
            Self::ProviderSelected(id) => {
                formatter.debug_tuple("ProviderSelected").field(id).finish()
            }
            Self::ProviderRowToggled(id) => formatter
                .debug_tuple("ProviderRowToggled")
                .field(id)
                .finish(),
            Self::ApiKeyChanged(id, _) => formatter
                .debug_tuple("ApiKeyChanged")
                .field(id)
                .field(&"[redacted]")
                .finish(),
            Self::SaveKeyRequested(id) => {
                formatter.debug_tuple("SaveKeyRequested").field(id).finish()
            }
            Self::RemoveKeyRequested(id) => formatter
                .debug_tuple("RemoveKeyRequested")
                .field(id)
                .finish(),
            Self::RefreshModelsRequested(id) => formatter
                .debug_tuple("RefreshModelsRequested")
                .field(id)
                .finish(),
            Self::ModelToggled(id, model, enabled) => formatter
                .debug_tuple("ModelToggled")
                .field(id)
                .field(model)
                .field(enabled)
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ProviderChoice {
    id: ProviderId,
    name: String,
}

impl std::fmt::Display for ProviderChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.name)
    }
}

pub struct ProviderView {
    id: ProviderId,
    name: String,
    api_key_draft: String,
    credential_present: bool,
    discovered_models: Vec<String>,
    selected_models: Vec<String>,
    discovering: bool,
    error: Option<String>,
}

impl ProviderView {
    pub fn new(
        id: ProviderId,
        name: impl Into<String>,
        has_key: bool,
        models: Vec<String>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            api_key_draft: String::new(),
            credential_present: has_key,
            discovered_models: models.clone(),
            selected_models: models,
            discovering: false,
            error: None,
        }
    }

    fn detail_view(&self) -> Element<'_, Input> {
        let id = self.id.clone();
        let key_input = text_input("Paste an API key", &self.api_key_draft)
            .secure(true)
            .on_input(move |value| Input::ApiKeyChanged(id.clone(), value))
            .on_submit(Input::SaveKeyRequested(self.id.clone()))
            .padding(10)
            .style(style::key_input);
        let save_key = button(text(if self.credential_present {
            "Update key"
        } else {
            "Save key"
        }))
        .on_press_maybe(
            (!self.api_key_draft.trim().is_empty())
                .then(|| Input::SaveKeyRequested(self.id.clone())),
        )
        .padding([10, 14])
        .style(style::action_button);

        let key_controls = column![
            text("API key").size(style::SUBHEADING_SIZE),
            text(if self.credential_present {
                "API keys are saved in Keychain"
            } else {
                "Add an API key to discover models"
            })
            .size(style::BODY_TEXT_SIZE)
            .color(style::MUTED_COLOR),
            row![key_input, save_key].spacing(10).align_y(Center),
        ]
        .spacing(10);

        let model_header = row![
            text("Available models").size(style::SUBHEADING_SIZE),
            space().width(Fill),
            button(text("Refresh"))
                .on_press_maybe(
                    (self.credential_present && !self.discovering)
                        .then(|| Input::RefreshModelsRequested(self.id.clone()))
                )
                .padding([7, 10])
                .style(style::subtle_button),
        ]
        .align_y(Center);
        let mut model_choices = column![].spacing(10);

        if self.discovering {
            model_choices =
                model_choices.push(text("Discovering models…").color(style::MUTED_COLOR));
        } else if self.discovered_models.is_empty() {
            model_choices = model_choices
                .push(text("Refresh to discover available models.").color(style::MUTED_COLOR));
        } else {
            for model in &self.discovered_models {
                let id = self.id.clone();
                let model_id = model.clone();
                model_choices = model_choices.push(
                    checkbox(self.selected_models.contains(model))
                        .label(model.as_str())
                        .on_toggle(move |enabled| {
                            Input::ModelToggled(id.clone(), model_id.clone(), enabled)
                        })
                        .size(17)
                        .style(style::provider_checkbox),
                );
            }
        }

        let model_list: Element<'_, Input> =
            if self.discovered_models.len() > 8 && !self.discovering {
                scrollable(container(model_choices).padding(iced::Padding {
                    right: style::SCROLL_CONTENT_GAP,
                    ..Default::default()
                }))
                .height(style::MODEL_LIST_HEIGHT)
                .width(Fill)
                .style(crate::style::scroll)
                .into()
            } else {
                model_choices.into()
            };
        let model_list = container(model_list)
            .padding(12)
            .width(Fill)
            .max_width(style::MODEL_LIST_WIDTH)
            .style(style::model_list);
        let model_header = container(model_header)
            .width(Fill)
            .max_width(style::MODEL_LIST_WIDTH);
        let models = column![model_header, model_list].spacing(10);

        let mut content = column![key_controls]
            .spacing(style::SECTION_SPACING)
            .width(Fill);
        if let Some(error) = &self.error {
            content = content.push(text(error).color(style::DANGER_COLOR));
        }
        if self.credential_present {
            content = content
                .push(rule::horizontal(1).style(style::sidebar_rule))
                .push(models);
        }
        content.into()
    }
}

pub struct Settings {
    scroll_id: Id,
    providers: Vec<ProviderView>,
    active_provider: Option<ProviderId>,
    error: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scroll_id: Id::unique(),
            providers: Vec::new(),
            active_provider: None,
            error: None,
        }
    }
}

impl Settings {
    pub fn with_providers(mut self, providers: Vec<ProviderView>) -> Self {
        self.providers = providers;
        self
    }

    fn provider_mut(&mut self, id: &ProviderId) -> Option<&mut ProviderView> {
        self.providers
            .iter_mut()
            .find(|provider| &provider.id == id)
    }

    pub fn update(&mut self, input: Input) -> Option<Output> {
        match input {
            Input::SectionSelected(section) => Some(Output::JumpToSection(section)),
            Input::ProviderSelected(id) => {
                if self
                    .providers
                    .iter()
                    .any(|provider| provider.id == id && !provider.credential_present)
                {
                    self.active_provider = Some(id);
                }
                None
            }
            Input::ProviderRowToggled(id) => {
                if self.active_provider.as_ref() == Some(&id) {
                    self.active_provider = None;
                } else if self
                    .providers
                    .iter()
                    .any(|provider| provider.id == id && provider.credential_present)
                {
                    self.active_provider = Some(id.clone());
                    return Some(Output::DiscoverModels(id));
                }
                None
            }
            Input::ApiKeyChanged(id, value) => {
                if let Some(provider) = self.provider_mut(&id) {
                    provider.api_key_draft = value;
                    provider.error = None;
                }
                None
            }
            Input::SaveKeyRequested(id) => self.provider_mut(&id).and_then(|provider| {
                let key = provider.api_key_draft.trim();
                (!key.is_empty()).then(|| Output::SaveKey(id, key.to_owned()))
            }),
            Input::RemoveKeyRequested(id) => Some(Output::RemoveKey(id)),
            Input::RefreshModelsRequested(id) => Some(Output::DiscoverModels(id)),
            Input::ModelToggled(id, model, enabled) => Some(Output::SetModel {
                provider: id,
                model,
                enabled,
            }),
        }
    }

    pub fn key_saved(&mut self, id: &ProviderId) {
        if let Some(provider) = self.provider_mut(id) {
            provider.credential_present = true;
            provider.api_key_draft.clear();
            provider.error = None;
        }
    }

    pub fn key_removed(&mut self, id: &ProviderId) {
        if let Some(provider) = self.provider_mut(id) {
            provider.credential_present = false;
            provider.api_key_draft.clear();
            provider.discovered_models.clear();
            provider.discovering = false;
            provider.error = None;
        }
        if self.active_provider.as_ref() == Some(id) {
            self.active_provider = None;
        }
    }

    pub fn set_selected_models(&mut self, id: &ProviderId, models: Vec<String>) {
        if let Some(provider) = self.provider_mut(id) {
            if provider.discovered_models.is_empty() {
                provider.discovered_models = models.clone();
            }
            provider.selected_models = models;
        }
    }

    pub fn start_discovery(&mut self, id: &ProviderId) {
        if let Some(provider) = self.provider_mut(id) {
            provider.discovering = true;
            provider.error = None;
        }
    }

    pub fn finish_discovery(&mut self, id: &ProviderId, models: Vec<String>) {
        if let Some(provider) = self.provider_mut(id) {
            provider.discovered_models = models;
            provider.discovering = false;
            provider.error = None;
        }
    }

    pub fn set_error(&mut self, id: &ProviderId, error: String) {
        if let Some(provider) = self.provider_mut(id) {
            provider.discovering = false;
            provider.error = Some(error);
        }
    }

    pub fn set_global_error(&mut self, error: String) {
        self.error = Some(error);
    }
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn discard_draft(&mut self) {
        for provider in &mut self.providers {
            provider.api_key_draft.clear();
        }
        self.active_provider = None;
    }

    pub fn jump_to_section<T>(&self, section: Section) -> Task<T> {
        match section {
            Section::Providers => {
                operation::snap_to(self.scroll_id.clone(), operation::RelativeOffset::START)
            }
        }
    }

    pub fn view(&self) -> Element<'_, Input> {
        let navigation = column![
            button(text("Providers & Models").size(style::SECTION_TEXT_SIZE))
                .on_press(Input::SectionSelected(Section::Providers))
                .width(Fill)
                .padding([10, 12])
                .style(style::selected_section),
        ]
        .width(Fill);

        let content = column![self.provider_view()].width(Fill);
        row![
            container(navigation)
                .padding(iced::Padding {
                    top: style::CONTENT_SPACING,
                    ..Default::default()
                })
                .width(style::SIDEBAR_WIDTH),
            rule::vertical(1).style(style::sidebar_rule),
            container(
                scrollable(container(content).padding(iced::Padding {
                    right: style::SCROLL_CONTENT_GAP,
                    ..Default::default()
                }))
                .id(self.scroll_id.clone())
                .width(Fill)
                .height(Fill)
                .style(crate::style::scroll),
            )
            .padding(iced::Padding {
                top: style::CONTENT_SPACING,
                ..Default::default()
            })
            .width(Fill)
            .height(Fill),
        ]
        .spacing(style::SETTINGS_COLUMN_SPACING)
        .height(Fill)
        .into()
    }

    fn provider_view(&self) -> Element<'_, Input> {
        let mut content: iced::widget::Column<'_, Input> = column![
            text("Providers & Models").size(style::PAGE_HEADING_SIZE),
            text("Connect providers and choose which models appear in the overlay.")
                .size(style::BODY_TEXT_SIZE)
                .color(style::MUTED_COLOR),
        ]
        .spacing(style::SECTION_SPACING)
        .width(Fill);

        let mut selected = column![text("Overlay models").size(style::SUBHEADING_SIZE)].spacing(10);
        let mut has_selected_models = false;
        for provider in self
            .providers
            .iter()
            .filter(|provider| provider.credential_present)
        {
            for model in &provider.selected_models {
                has_selected_models = true;
                selected = selected.push(
                    row![
                        text(model).width(Fill),
                        text(&provider.name).color(style::MUTED_COLOR),
                        button(text("×"))
                            .on_press(Input::ModelToggled(
                                provider.id.clone(),
                                model.clone(),
                                false
                            ))
                            .padding([4, 8])
                            .style(style::subtle_button),
                    ]
                    .spacing(12)
                    .align_y(Center),
                );
            }
        }
        if !has_selected_models {
            selected = selected.push(text("No models selected yet.").color(style::MUTED_COLOR));
        }
        content = content.push(
            container(selected)
                .padding(style::SECTION_PADDING)
                .width(Fill)
                .style(style::provider_card),
        );

        let choices = self
            .providers
            .iter()
            .filter(|provider| !provider.credential_present)
            .map(|provider| ProviderChoice {
                id: provider.id.clone(),
                name: provider.name.clone(),
            })
            .collect::<Vec<_>>();
        let add_control: Element<'_, Input> = if choices.is_empty() {
            button(text("Add a provider"))
                .padding([6, 12])
                .style(style::action_button)
                .into()
        } else {
            pick_list(choices, None::<ProviderChoice>, |choice| {
                Input::ProviderSelected(choice.id)
            })
            .placeholder("Add a provider")
            .padding([6, 10])
            .style(style::add_provider_picker)
            .menu_style(style::add_provider_menu)
            .into()
        };

        let mut providers = column![
            row![
                text("Providers").size(style::SUBHEADING_SIZE),
                space().width(Fill),
                add_control,
            ]
            .align_y(Center),
        ]
        .spacing(12);

        let mut has_provider_row = false;
        for provider in self.providers.iter().filter(|provider| {
            provider.credential_present || self.active_provider.as_ref() == Some(&provider.id)
        }) {
            has_provider_row = true;
            let expanded = self.active_provider.as_ref() == Some(&provider.id);
            let status: Element<'_, Input> = if provider.credential_present {
                button(text("×").size(style::SUBHEADING_SIZE))
                    .on_press(Input::RemoveKeyRequested(provider.id.clone()))
                    .padding([2, 6])
                    .style(style::subtle_button)
                    .into()
            } else {
                text("Not connected").color(style::MUTED_COLOR).into()
            };
            providers = providers.push(
                row![
                    button(
                        row![
                            text(if expanded { "▾" } else { "▸" }),
                            text(&provider.name).width(Fill),
                        ]
                        .spacing(12)
                        .align_y(Center)
                    )
                    .on_press(Input::ProviderRowToggled(provider.id.clone()))
                    .width(Fill)
                    .padding([8, 4])
                    .style(style::subtle_button),
                    status,
                ]
                .spacing(12)
                .align_y(Center),
            );
            if expanded {
                providers = providers.push(container(provider.detail_view()).width(Fill).padding(
                    iced::Padding {
                        top: 4.0,
                        right: 4.0,
                        bottom: 12.0,
                        left: 28.0,
                    },
                ));
            }
        }
        if !has_provider_row {
            providers = providers.push(text("No providers connected.").color(style::MUTED_COLOR));
        }
        content = content.push(
            container(providers)
                .padding(style::SECTION_PADDING)
                .width(Fill)
                .style(style::provider_card),
        );
        if let Some(error) = &self.error {
            content = content.push(text(error).color(style::DANGER_COLOR));
        }
        container(content).width(Fill).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_requests_scroll_to_provider_section() {
        let mut settings = Settings::default();

        assert!(matches!(
            settings.update(Input::SectionSelected(Section::Providers)),
            Some(Output::JumpToSection(Section::Providers))
        ));
    }

    #[test]
    fn add_provider_opens_an_unconnected_provider() {
        let id = ProviderId::new("openai");
        let mut settings = Settings::default().with_providers(vec![ProviderView::new(
            id.clone(),
            "OpenAI",
            false,
            Vec::new(),
        )]);

        assert!(
            settings
                .update(Input::ProviderSelected(id.clone()))
                .is_none()
        );
        assert_eq!(settings.active_provider, Some(id.clone()));

        settings.key_saved(&id);
        assert!(settings.providers[0].credential_present);
        assert_eq!(settings.active_provider, Some(id));
    }

    #[test]
    fn managing_a_connected_provider_starts_discovery() {
        let id = ProviderId::new("openai");
        let mut settings = Settings::default().with_providers(vec![ProviderView::new(
            id.clone(),
            "OpenAI",
            true,
            Vec::new(),
        )]);

        assert!(matches!(
            settings.update(Input::ProviderRowToggled(id.clone())),
            Some(Output::DiscoverModels(provider)) if provider == id
        ));
        assert_eq!(settings.active_provider, Some(id.clone()));
        assert!(settings.update(Input::ProviderRowToggled(id)).is_none());
        assert_eq!(settings.active_provider, None);
    }

    #[test]
    fn provider_key_drafts_are_independent() {
        let first = ProviderId::new("first");
        let second = ProviderId::new("second");
        let mut settings = Settings::default().with_providers(vec![
            ProviderView::new(first.clone(), "First", false, Vec::new()),
            ProviderView::new(second.clone(), "Second", false, Vec::new()),
        ]);

        settings.update(Input::ApiKeyChanged(first.clone(), "first-key".into()));
        settings.update(Input::ApiKeyChanged(second.clone(), "second-key".into()));
        assert!(
            matches!(settings.update(Input::SaveKeyRequested(first.clone())), Some(Output::SaveKey(id, key)) if id == first && key == "first-key")
        );
        assert!(
            matches!(settings.update(Input::SaveKeyRequested(second.clone())), Some(Output::SaveKey(id, key)) if id == second && key == "second-key")
        );
    }

    #[test]
    fn key_is_submitted_only_on_commit_and_is_redacted_from_debug() {
        let id = ProviderId::new("test");
        let mut settings = Settings::default().with_providers(vec![ProviderView::new(
            id.clone(),
            "Test",
            false,
            Vec::new(),
        )]);
        assert!(
            settings
                .update(Input::ApiKeyChanged(id.clone(), " sk-test ".into()))
                .is_none()
        );
        assert!(
            matches!(settings.update(Input::SaveKeyRequested(id.clone())), Some(Output::SaveKey(provider, key)) if provider == id && key == "sk-test")
        );
        assert!(!format!("{:?}", Input::ApiKeyChanged(id, "sk-test".into())).contains("sk-test"));
    }
}
