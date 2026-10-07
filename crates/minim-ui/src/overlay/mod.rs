use iced::{Subscription, Task};
use minim_provider::ModelSelection;
use minim_types::Target;

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
    TargetRemoved,
    CaptureRegionRequested,
    ProviderRequestAbortRequested,
    ProviderRequestRetryRequested,
    LinkClicked(String),
    PhaseChanged(Phase),
    Dismissed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Prompting,
    Answering,
}

#[derive(Debug)]
pub struct Overlay {
    pub visible: bool,
    pub target: Option<Target>,
    selected_model: Option<ModelSelection>,
    prompting: prompting::Prompting,
    answering: answering::Answering,
    capture_ready: bool,
    pub phase: Phase,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            visible: false,
            target: None,
            selected_model: None,
            prompting: prompting::Prompting::new(),
            answering: answering::Answering::default(),
            capture_ready: true,
            phase: Phase::default(),
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

    pub fn check_prompt_input_focus(&self) -> Task<Input> {
        self.prompting.check_focus().map(Input::Prompting)
    }

    pub fn subscription(&self) -> Subscription<Input> {
        if self.phase == Phase::Answering {
            self.answering.subscription().map(Input::Answering)
        } else {
            Subscription::none()
        }
    }
}
