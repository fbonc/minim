use std::pin::Pin;
use std::sync::{Arc, Mutex};

use super::error::ProviderError;
use futures_core::Stream;
use futures_util::future::{AbortHandle, AbortRegistration, BoxFuture};
use ruru_types::{ContextCapture, Target};

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub prompt: Option<String>,
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
}

pub type ProviderStream =
    Pin<Box<dyn Stream<Item = Result<ProviderOutput, ProviderError>> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderOutput {
    TextDelta(String),
}

pub trait Provider: Send + Sync {
    fn configured_models(&self) -> Vec<String>;

    fn discover_models(&self) -> BoxFuture<'_, Result<Vec<String>, ProviderError>>;

    fn stream(&self, model: &str, request: ProviderRequest) -> ProviderStream;

    fn abort_request(&self);

    fn retry_request(&self) -> Option<ProviderStream>;
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RequestAbortController {
    handle: Arc<Mutex<Option<AbortHandle>>>,
}

impl RequestAbortController {
    pub(crate) fn register(&self) -> AbortRegistration {
        let (handle, registration) = AbortHandle::new_pair();
        let previous = self
            .handle
            .lock()
            .expect("provider request abort mutex poisoned")
            .replace(handle);

        if let Some(previous) = previous {
            previous.abort();
        }

        registration
    }

    pub(crate) fn abort(&self) {
        if let Some(handle) = self
            .handle
            .lock()
            .expect("provider request abort mutex poisoned")
            .take()
        {
            handle.abort();
        }
    }
}
