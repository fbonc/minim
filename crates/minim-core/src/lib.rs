use std::sync::Arc;

use futures_channel::mpsc;
use futures_util::{Stream, StreamExt, stream};
use minim_capture::new_capturer;
use minim_provider::{ModelSelection, ProviderRegistry};
use minim_types::{ImageCapture, Target, WindowBounds};

use crate::core::Core;
pub use crate::state::Capture;

mod core;
mod hotkey;
mod state;

pub type Sender = mpsc::Sender<Input>;

#[derive(Debug, Clone)]
pub enum Input {
    Submit {
        prompt: Option<String>,
        model: ModelSelection,
    },
    AbortProviderRequest,
    RetryProviderRequest,
    RemoveTarget,
    SelectRegion,
}

#[derive(Debug, Clone)]
pub enum Output {
    ShowRequested {
        focused_window: Option<WindowBounds>,
    },
    CaptureCompleted,
    TargetCaptured(Target),
    RegionSelectionFinished(Result<Option<ImageCapture>, String>),
    AnswerChunk(String),
    AnswerCompleted,
    RequestFailed(String),
}

pub fn start(providers: ProviderRegistry) -> (Sender, impl Stream<Item = Output>) {
    let (sender, inputs) = mpsc::channel(1);
    let core = Arc::new(Core::new(providers, Arc::from(new_capturer())));
    let hotkey_outputs = hotkey::outputs(Arc::clone(&core));
    let responses = inputs
        .map(move |input| core.handle_input(input))
        .flatten_unordered(None);

    (sender, stream::select(hotkey_outputs, responses))
}
