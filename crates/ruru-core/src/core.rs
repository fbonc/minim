use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures_util::future::{AbortRegistration, Abortable, ready};
use futures_util::stream::BoxStream;
use futures_util::{StreamExt, stream};
use ruru_capture::Capturer;
use ruru_provider::{ProviderError, ProviderOutput, ProviderStream};
use ruru_types::{ImageCapture, ModelSelection, Target, WindowBounds};

use crate::providers::Providers;
use crate::state::State;
use crate::{Capture, Input, Output, ProviderRequestId};

pub(crate) struct Core {
    state: Mutex<State>,
    providers: Providers,
    capturer: Arc<dyn Capturer>,
}

impl Core {
    pub(crate) fn new(providers: Providers, capturer: Arc<dyn Capturer>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            providers,
            capturer,
        }
    }

    pub(crate) fn handle_input(self: &Arc<Self>, input: Input) -> BoxStream<'static, Output> {
        match input {
            Input::Submit {
                request_id,
                prompt,
                model,
            } => {
                let Some(model) =
                    model.filter(|model| self.providers.configured_models().contains(model))
                else {
                    self.forget_provider_request();
                    return stream::once(async move {
                        Output::ProviderRequestFailed {
                            request_id,
                            error: "no available model selected".into(),
                        }
                    })
                    .boxed();
                };
                self.start_provider_stream(request_id, prompt, model)
            }
            Input::AbortProviderRequest => {
                self.abort_provider_request();
                stream::empty().boxed()
            }
            Input::RetryProviderRequest { request_id } => self.retry_provider_stream(request_id),
            Input::RemoveTarget => {
                self.remove_target();
                stream::empty().boxed()
            }
            Input::SelectRegion => {
                let core = Arc::clone(self);
                stream::once(async move {
                    let result = tokio::task::spawn_blocking(move || core.select_region())
                        .await
                        .map_err(|error| format!("region selection task failed: {error}"))
                        .and_then(|result| result);

                    Output::RegionSelectionFinished(result)
                })
                .boxed()
            }
            Input::DiscoverModels(_)
            | Input::SaveKey(_, _)
            | Input::RemoveKey(_)
            | Input::SetModel { .. }
            | Input::SelectModel(_) => unreachable!("settings inputs are handled by Settings"),
        }
    }

    pub(crate) fn capture(
        &self,
        on_capture_started: impl FnOnce(Option<WindowBounds>, Option<Target>),
    ) -> CaptureOutcome {
        let capture_id = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .start_capture();
        let started = Instant::now();
        let focused_window = self.capturer.capture_window_bounds().ok();
        let provenance = self.capturer.capture_provenance().ok();
        let (target, failure) = match self.capturer.capture_text() {
            Ok(capture) => (Some(Target::Text(capture)), None),
            Err(error) => (None, Some(format!("failed to capture text: {error:?}"))),
        };
        let pending_context = self.capturer.begin_context_capture().ok();
        on_capture_started(focused_window, target.clone());
        let context = pending_context.and_then(|context| context.complete().ok());
        let elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let capture = Capture {
            target: target.clone(),
            context,
            provenance,
            elapsed_ms,
        };

        self.state
            .lock()
            .expect("core state mutex poisoned")
            .commit_capture(capture_id, capture);

        CaptureOutcome { failure }
    }

    fn select_region(&self) -> Result<Option<ImageCapture>, String> {
        let capture_id = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .capture_id;
        let result = self
            .capturer
            .select_region()
            .map_err(|error| error.to_string())?;

        let Some(image) = result else {
            return Ok(None);
        };
        let applied = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .apply_region(capture_id, image.clone());

        Ok(applied.then_some(image))
    }

    fn remove_target(&self) {
        self.state
            .lock()
            .expect("core state mutex poisoned")
            .remove_target();
    }

    fn start_provider_stream(
        self: &Arc<Self>,
        output_request_id: ProviderRequestId,
        prompt: Option<String>,
        model: ModelSelection,
    ) -> BoxStream<'static, Output> {
        let provider = self.providers.resolve(&model);
        let (request, request_id, abort_registration) = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .start_provider_request(prompt, provider.clone());
        let provider_stream = match provider {
            Some(provider) => provider.stream(&model.model, request),
            None => stream::once(async move {
                Err(ProviderError::new(format!(
                    "provider `{}` is not configured",
                    model.provider
                )))
            })
            .boxed(),
        };
        self.provider_outputs(
            provider_stream,
            request_id,
            output_request_id,
            abort_registration,
        )
    }

    fn retry_provider_stream(
        self: &Arc<Self>,
        output_request_id: ProviderRequestId,
    ) -> BoxStream<'static, Output> {
        let (provider, request_id, abort_registration) = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .start_provider_retry();
        let provider_stream = provider
            .and_then(|provider| provider.retry_request())
            .unwrap_or_else(|| {
                stream::once(async {
                    Err(ProviderError::new(
                        "no provider request is available to retry",
                    ))
                })
                .boxed()
            });

        self.provider_outputs(
            provider_stream,
            request_id,
            output_request_id,
            abort_registration,
        )
    }

    fn provider_outputs(
        self: &Arc<Self>,
        provider_stream: ProviderStream,
        request_id: u64,
        output_request_id: ProviderRequestId,
        abort_registration: AbortRegistration,
    ) -> BoxStream<'static, Output> {
        let outputs = stream::unfold(
            ProviderResponseState::Streaming(provider_stream),
            move |response| async move {
                match response {
                    ProviderResponseState::Streaming(mut stream) => match stream.next().await {
                        Some(Ok(ProviderOutput::TextDelta(chunk))) => Some((
                            Output::AnswerChunk {
                                request_id: output_request_id,
                                chunk,
                            },
                            ProviderResponseState::Streaming(stream),
                        )),
                        Some(Err(error)) => Some((
                            Output::ProviderRequestFailed {
                                request_id: output_request_id,
                                error: error.to_string(),
                            },
                            ProviderResponseState::Finished,
                        )),
                        None => Some((
                            Output::AnswerCompleted {
                                request_id: output_request_id,
                            },
                            ProviderResponseState::Finished,
                        )),
                    },
                    ProviderResponseState::Finished => None,
                }
            },
        );
        let core = Arc::clone(self);

        Abortable::new(outputs, abort_registration)
            .filter_map(move |output| {
                let accepted = core.accept_provider_output(request_id, &output);
                ready(accepted.then_some(output))
            })
            .boxed()
    }

    fn abort_provider_request(&self) {
        self.state
            .lock()
            .expect("core state mutex poisoned")
            .abort_provider_request();
    }

    pub(crate) fn forget_provider_request(&self) {
        self.state
            .lock()
            .expect("core state mutex poisoned")
            .forget_provider_request();
    }

    fn accept_provider_output(&self, request_id: u64, output: &Output) -> bool {
        self.state
            .lock()
            .expect("core state mutex poisoned")
            .accept_provider_output(request_id, output)
    }
}

pub(crate) struct CaptureOutcome {
    pub(crate) failure: Option<String>,
}

enum ProviderResponseState {
    Streaming(ProviderStream),
    Finished,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruru_provider::{MockProvider, Provider, ProviderRequest};
    use ruru_types::{
        ContextCapture, Provenance, ProviderId, TextCapture, TextCaptureMethod, WindowBounds,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    struct StubCapturer {
        context_completed: Option<Arc<AtomicBool>>,
    }

    struct RecordingProvider {
        requested_models: Mutex<Vec<String>>,
        aborted: AtomicBool,
        retried: AtomicBool,
    }

    impl Provider for RecordingProvider {
        fn configured_models(&self) -> Vec<String> {
            vec!["first".into(), "second".into()]
        }

        fn discover_models(
            &self,
        ) -> futures_util::future::BoxFuture<'_, Result<Vec<String>, ProviderError>> {
            Box::pin(async { Ok(self.configured_models()) })
        }

        fn stream(&self, model: &str, _request: ProviderRequest) -> ProviderStream {
            self.requested_models
                .lock()
                .expect("recorded models mutex poisoned")
                .push(model.to_owned());
            stream::empty().boxed()
        }

        fn abort_request(&self) {
            self.aborted.store(true, Ordering::SeqCst);
        }

        fn retry_request(&self) -> Option<ProviderStream> {
            self.retried.store(true, Ordering::SeqCst);
            Some(stream::empty().boxed())
        }
    }

    impl Capturer for StubCapturer {
        fn capture_text(&self) -> ruru_capture::Result<TextCapture> {
            Ok(TextCapture {
                text: "selected text".into(),
                method: TextCaptureMethod::Accessibility,
            })
        }

        fn begin_context_capture(
            &self,
        ) -> ruru_capture::Result<ruru_capture::PendingContextCapture> {
            let context_completed = self.context_completed.clone();
            Ok(ruru_capture::PendingContextCapture::new(move || {
                if let Some(context_completed) = context_completed {
                    context_completed.store(true, Ordering::SeqCst);
                }
                Ok(ContextCapture {
                    image: ImageCapture {
                        png: b"context".to_vec(),
                        region: None,
                    },
                    accessibility_text: Some("surrounding context".into()),
                })
            }))
        }

        fn select_region(&self) -> ruru_capture::Result<Option<ImageCapture>> {
            Ok(Some(ImageCapture {
                png: b"png".to_vec(),
                region: None,
            }))
        }

        fn capture_provenance(&self) -> ruru_capture::Result<Provenance> {
            Ok(Provenance {
                app_name: "Example".into(),
                window_title: "Document".into(),
                path: Some("/tmp/example".into()),
            })
        }

        fn capture_window_bounds(&self) -> ruru_capture::Result<WindowBounds> {
            Ok(WindowBounds {
                x: 100.0,
                y: 200.0,
                w: 1200.0,
                h: 800.0,
            })
        }
    }

    fn test_providers() -> Providers {
        let providers = Providers::new();
        providers.insert(ProviderId::new("mock"), Arc::new(MockProvider::default()));
        providers
    }

    #[test]
    fn capture_orchestrates_and_commits_capture_data() {
        let core = Core::new(test_providers(), Arc::new(StubCapturer::default()));

        let mut initial_bounds = None;
        let mut initial_target = None;
        let outcome = core.capture(|bounds, target| {
            initial_bounds = bounds;
            initial_target = target;
        });

        assert!(outcome.failure.is_none());
        assert!(matches!(
            initial_target,
            Some(Target::Text(TextCapture { ref text, .. })) if text == "selected text"
        ));
        assert_eq!(
            initial_bounds,
            Some(WindowBounds {
                x: 100.0,
                y: 200.0,
                w: 1200.0,
                h: 800.0,
            })
        );

        let state = core.state.lock().expect("core state mutex poisoned");
        let capture = state.current_capture().expect("capture committed");
        assert!(matches!(
            capture.context,
            Some(ContextCapture {
                image: ImageCapture { ref png, .. },
                ..
            }) if png == b"context"
        ));
        assert_eq!(
            capture
                .provenance
                .as_ref()
                .map(|value| value.app_name.as_str()),
            Some("Example")
        );
    }

    #[test]
    fn requests_the_overlay_before_completing_context_capture() {
        let context_completed = Arc::new(AtomicBool::new(false));
        let core = Core::new(
            test_providers(),
            Arc::new(StubCapturer {
                context_completed: Some(Arc::clone(&context_completed)),
            }),
        );

        core.capture(|_, _| assert!(!context_completed.load(Ordering::SeqCst)));

        assert!(context_completed.load(Ordering::SeqCst));
    }

    #[test]
    fn region_selection_replaces_the_committed_target() {
        let core = Core::new(test_providers(), Arc::new(StubCapturer::default()));
        core.capture(|_, _| {});

        let selected = core.select_region().expect("region capture succeeds");

        assert!(selected.is_some());
        let state = core.state.lock().expect("core state mutex poisoned");
        assert!(matches!(
            state
                .current_capture()
                .and_then(|capture| capture.target.as_ref()),
            Some(Target::Image(_))
        ));
    }

    #[test]
    fn routes_each_request_using_its_model_selection() {
        let providers = Providers::new();
        let provider = Arc::new(RecordingProvider {
            requested_models: Mutex::new(Vec::new()),
            aborted: AtomicBool::new(false),
            retried: AtomicBool::new(false),
        });
        let provider_id = ProviderId::new("recording");
        providers.insert(provider_id.clone(), provider.clone());
        let core = Arc::new(Core::new(providers, Arc::new(StubCapturer::default())));

        let _outputs = core.start_provider_stream(
            1,
            Some("explain".into()),
            ModelSelection::new(provider_id, "second"),
        );

        assert_eq!(
            *provider
                .requested_models
                .lock()
                .expect("recorded models mutex poisoned"),
            vec!["second"]
        );
    }

    #[test]
    fn provider_outputs_keep_the_app_request_id() {
        let core = Arc::new(Core::new(
            test_providers(),
            Arc::new(StubCapturer::default()),
        ));
        let mut outputs = core.start_provider_stream(
            42,
            None,
            ModelSelection::new(ProviderId::new("mock"), "mock"),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime should start");

        let output = runtime
            .block_on(outputs.next())
            .expect("provider should produce output");

        assert!(matches!(output, Output::AnswerChunk { request_id: 42, .. }));
    }

    #[test]
    fn missing_model_failure_keeps_the_app_request_id() {
        let providers = Providers::new();
        let provider = Arc::new(RecordingProvider {
            requested_models: Mutex::new(Vec::new()),
            aborted: AtomicBool::new(false),
            retried: AtomicBool::new(false),
        });
        let provider_id = ProviderId::new("recording");
        providers.insert(provider_id.clone(), provider.clone());
        let core = Arc::new(Core::new(providers, Arc::new(StubCapturer::default())));
        let _previous =
            core.start_provider_stream(41, None, ModelSelection::new(provider_id, "first"));
        let mut outputs = core.handle_input(Input::Submit {
            request_id: 42,
            prompt: None,
            model: None,
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should start");

        let output = runtime
            .block_on(outputs.next())
            .expect("missing model should produce an error");

        assert!(matches!(
            output,
            Output::ProviderRequestFailed { request_id: 42, .. }
        ));
        assert!(provider.aborted.load(Ordering::SeqCst));
    }

    #[test]
    fn aborting_stops_the_active_provider_request() {
        let providers = Providers::new();
        let provider = Arc::new(RecordingProvider {
            requested_models: Mutex::new(Vec::new()),
            aborted: AtomicBool::new(false),
            retried: AtomicBool::new(false),
        });
        let provider_id = ProviderId::new("recording");
        providers.insert(provider_id.clone(), provider.clone());
        let core = Arc::new(Core::new(providers, Arc::new(StubCapturer::default())));
        let _outputs = core.start_provider_stream(
            1,
            Some("explain".into()),
            ModelSelection::new(provider_id, "first"),
        );

        let _outputs = core.handle_input(Input::AbortProviderRequest);

        assert!(provider.aborted.load(Ordering::SeqCst));
    }

    #[test]
    fn retrying_uses_the_previous_provider_request() {
        let providers = Providers::new();
        let provider = Arc::new(RecordingProvider {
            requested_models: Mutex::new(Vec::new()),
            aborted: AtomicBool::new(false),
            retried: AtomicBool::new(false),
        });
        let provider_id = ProviderId::new("recording");
        providers.insert(provider_id.clone(), provider.clone());
        let core = Arc::new(Core::new(providers, Arc::new(StubCapturer::default())));
        let _outputs = core.start_provider_stream(
            1,
            Some("explain".into()),
            ModelSelection::new(provider_id, "first"),
        );
        core.abort_provider_request();

        let _outputs = core.handle_input(Input::RetryProviderRequest { request_id: 2 });

        assert!(provider.retried.load(Ordering::SeqCst));
    }
}
