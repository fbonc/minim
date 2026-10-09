use iced::{Subscription, Task};
use ruru_types::ModelSelection;
use ruru_types::Target;

pub mod answering;
pub mod prompting;

mod latex;
mod style;
mod update;
mod view;

#[derive(Debug, Clone)]
pub enum Input {
    Prompting(prompting::Input),
    Answering(answering::Input),
    Show,
    CaptureCompleted,
    SetTarget(Target),
    CaptureFailed,
    AppendAnswer(String),
    FinishAnswer,
    FailAnswer(String),
    ProviderRequestAbortRequested,
    ProviderRequestRetryRequested,
    CopyAnswerRequested,
    RemoveTargetRequested,
    BackRequested,
    DismissRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Submitted {
        prompt: Option<String>,
        model: Option<ModelSelection>,
    },
    ModelSelected(ModelSelection),
    TargetRemoved,
    CaptureRegionRequested,
    ProviderRequestAbortRequested,
    ProviderRequestRetryRequested,
    CopyAnswerRequested(String),
    LinkClicked(String),
    ModeChanged(OverlayMode),
    Dismissed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverlayMode {
    #[default]
    Prompting,
    Answering,
}

#[derive(Debug)]
pub struct Overlay {
    pub visible: bool,
    pub target: Option<Target>,
    selected_model: Option<ModelSelection>,
    available_models: Vec<ModelSelection>,
    prompting: prompting::Prompting,
    answering: answering::Answering,
    capture_ready: bool,
    pub mode: OverlayMode,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            visible: false,
            target: None,
            selected_model: None,
            available_models: Vec::new(),
            prompting: prompting::Prompting::new(),
            answering: answering::Answering::default(),
            capture_ready: true,
            mode: OverlayMode::default(),
        }
    }
}

impl Overlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_selected_model(mut self, selected_model: Option<ModelSelection>) -> Self {
        self.selected_model = selected_model;
        self
    }

    pub fn with_models(mut self, models: Vec<ModelSelection>) -> Self {
        self.reconcile_models(&models);
        self
    }

    pub fn selected_model(&self) -> Option<&ModelSelection> {
        self.selected_model.as_ref()
    }

    pub fn reconcile_models(&mut self, models: &[ModelSelection]) {
        self.available_models = models.to_vec();
        if self
            .selected_model
            .as_ref()
            .is_none_or(|selected| !models.contains(selected))
        {
            self.selected_model = models.first().cloned();
        }
    }

    pub fn check_prompt_input_focus(&self) -> Task<Input> {
        self.prompting.check_focus().map(Input::Prompting)
    }

    pub fn subscription(&self) -> Subscription<Input> {
        if self.mode == OverlayMode::Answering {
            self.answering.subscription().map(Input::Answering)
        } else {
            Subscription::none()
        }
    }
}
