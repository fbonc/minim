use std::sync::Arc;

use futures_channel::mpsc;
use futures_util::{Stream, StreamExt, stream};
use ruru_capture::new_capturer;
use ruru_config::KeychainCredentialStore;
use ruru_types::{ImageCapture, ModelSelection, ProviderId, Target, WindowBounds};

use crate::core::Core;
pub use crate::state::Capture;

mod core;
mod hotkey;
mod providers;
mod settings;
mod state;

pub type ProviderRequestId = u64;
pub use settings::{ProviderSettingsInfo, SettingsChange, SettingsSnapshot, SettingsUpdate};

pub type Sender = mpsc::UnboundedSender<Input>;

#[derive(Clone)]
pub enum Input {
    Submit {
        request_id: ProviderRequestId,
        prompt: Option<String>,
        model: Option<ModelSelection>,
    },
    AbortProviderRequest,
    RetryProviderRequest {
        request_id: ProviderRequestId,
    },
    RemoveTarget,
    SelectRegion,
    DiscoverModels(ProviderId),
    SaveKey(ProviderId, String),
    RemoveKey(ProviderId),
    SetModel {
        provider: ProviderId,
        model: String,
        enabled: bool,
    },
    SelectModel(ModelSelection),
}

#[derive(Debug, Clone)]
pub enum Output {
    ShowRequested {
        focused_window: Option<WindowBounds>,
    },
    CaptureCompleted,
    TargetCaptured(Target),
    RegionSelectionFinished(Result<Option<ImageCapture>, String>),
    AnswerChunk {
        request_id: ProviderRequestId,
        chunk: String,
    },
    AnswerCompleted {
        request_id: ProviderRequestId,
    },
    ProviderRequestFailed {
        request_id: ProviderRequestId,
        error: String,
    },
    RequestFailed(String),
    SettingsUpdated(SettingsUpdate),
    ModelsDiscovered {
        provider: ProviderId,
        result: Result<Vec<String>, String>,
    },
    SettingsFailed {
        provider: Option<ProviderId>,
        error: String,
    },
}

pub fn start() -> (Sender, impl Stream<Item = Output>, SettingsSnapshot) {
    let (sender, inputs) = mpsc::unbounded();
    let settings = Arc::new(settings::Settings::load(Arc::new(
        KeychainCredentialStore::default(),
    )));
    let initial_settings = settings.snapshot();
    let core = Arc::new(Core::new(settings.providers(), Arc::from(new_capturer())));
    let hotkey_outputs = hotkey::outputs(Arc::clone(&core));
    let responses = inputs
        .map(move |input| match input {
            Input::DiscoverModels(id) => settings.discover(Arc::clone(&core), id),
            Input::SaveKey(id, key) => match settings.save_key(id.clone(), key) {
                Ok(update) => {
                    core.forget_provider_request();
                    stream::iter([Output::SettingsUpdated(update)])
                        .chain(settings.discover(Arc::clone(&core), id))
                        .boxed()
                }
                Err(error) => settings_error(Some(id), error),
            },
            Input::RemoveKey(id) => match settings.remove_key(id.clone()) {
                Ok(update) => {
                    core.forget_provider_request();
                    stream::iter([Output::SettingsUpdated(update)]).boxed()
                }
                Err(error) => settings_error(Some(id), error),
            },
            Input::SetModel {
                provider,
                model,
                enabled,
            } => match settings.set_model(provider.clone(), model, enabled) {
                Ok(update) => {
                    core.forget_provider_request();
                    stream::iter([Output::SettingsUpdated(update)]).boxed()
                }
                Err(error) => settings_error(Some(provider), error),
            },
            Input::SelectModel(model) => match settings.select_model(model) {
                Ok(()) => stream::empty().boxed(),
                Err(error) => settings_error(None, error),
            },
            input => core.handle_input(input),
        })
        .flatten_unordered(None);

    (
        sender,
        stream::select(hotkey_outputs, responses),
        initial_settings,
    )
}

fn settings_error(
    provider: Option<ProviderId>,
    error: String,
) -> futures_util::stream::BoxStream<'static, Output> {
    stream::iter([Output::SettingsFailed { provider, error }]).boxed()
}
