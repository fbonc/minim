use std::sync::Arc;

use futures_util::future::{AbortHandle, AbortRegistration};
use ruru_provider::{Provider, ProviderRequest};
use ruru_types::{ContextCapture, ImageCapture, Provenance, Target};

use crate::Output;

#[derive(Debug, Clone)]
pub struct Capture {
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
    pub provenance: Option<Provenance>,
    pub elapsed_ms: u32,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) capture_id: u64,
    current_capture: Option<Capture>,
    provider_request_id: u64,
    provider_abort: Option<AbortHandle>,
    request_provider: Option<Arc<dyn Provider>>,
}

impl State {
    #[cfg(test)]
    pub(crate) fn current_capture(&self) -> Option<&Capture> {
        self.current_capture.as_ref()
    }

    pub(crate) fn start_capture(&mut self) -> u64 {
        self.abort_provider_request();
        self.request_provider = None;
        self.capture_id = self.capture_id.wrapping_add(1);
        self.current_capture = None;
        self.capture_id
    }

    pub(crate) fn commit_capture(&mut self, capture_id: u64, capture: Capture) {
        if self.capture_id == capture_id {
            self.current_capture = Some(capture);
        }
    }

    pub(crate) fn apply_region(&mut self, capture_id: u64, image: ImageCapture) -> bool {
        if self.capture_id != capture_id {
            return false;
        }

        let Some(capture) = &mut self.current_capture else {
            return false;
        };

        capture.target = Some(Target::Image(image));
        true
    }

    pub(crate) fn remove_target(&mut self) {
        if let Some(capture) = &mut self.current_capture {
            capture.target = None;
        }
    }

    pub(crate) fn start_provider_request(
        &mut self,
        prompt: Option<String>,
        provider: Option<Arc<dyn Provider>>,
    ) -> (ProviderRequest, u64, AbortRegistration) {
        self.abort_provider_request();
        let request = ProviderRequest {
            prompt,
            target: self
                .current_capture
                .as_ref()
                .and_then(|capture| capture.target.clone()),
            context: self
                .current_capture
                .as_ref()
                .and_then(|capture| capture.context.clone()),
        };
        let (abort, registration) = AbortHandle::new_pair();
        self.provider_abort = Some(abort);
        self.request_provider = provider;
        (request, self.provider_request_id, registration)
    }

    pub(crate) fn start_provider_retry(
        &mut self,
    ) -> (Option<Arc<dyn Provider>>, u64, AbortRegistration) {
        self.abort_provider_request();
        let (abort, registration) = AbortHandle::new_pair();
        self.provider_abort = Some(abort);
        (
            self.request_provider.clone(),
            self.provider_request_id,
            registration,
        )
    }

    pub(crate) fn abort_provider_request(&mut self) {
        self.provider_request_id = self.provider_request_id.wrapping_add(1);
        let request_was_active = if let Some(abort) = self.provider_abort.take() {
            abort.abort();
            true
        } else {
            false
        };
        if request_was_active && let Some(provider) = &self.request_provider {
            provider.abort_request();
        }
    }

    pub(crate) fn accept_provider_output(&mut self, request_id: u64, output: &Output) -> bool {
        if self.provider_request_id != request_id || self.provider_abort.is_none() {
            return false;
        }

        if matches!(output, Output::AnswerCompleted | Output::RequestFailed(_)) {
            self.provider_abort = None;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruru_types::{Provenance, TextCapture, TextCaptureMethod};

    fn image() -> ImageCapture {
        ImageCapture {
            png: b"png".to_vec(),
            region: None,
        }
    }

    fn context() -> ContextCapture {
        ContextCapture {
            image: image(),
            accessibility_text: Some("surrounding context".into()),
        }
    }

    #[test]
    fn provider_request_uses_prompt_target_and_context() {
        let target = Target::Text(TextCapture {
            text: "selected text".into(),
            method: TextCaptureMethod::Accessibility,
        });
        let context = context();
        let mut state = State::default();
        let capture_id = state.start_capture();
        state.commit_capture(
            capture_id,
            Capture {
                target: Some(target),
                context: Some(context),
                provenance: Some(Provenance {
                    app_name: "Example".into(),
                    window_title: "Document".into(),
                    path: Some("/tmp/example".into()),
                }),
                elapsed_ms: 12,
            },
        );

        let (request, _, _) = state.start_provider_request(Some("explain this".into()), None);

        assert_eq!(request.prompt.as_deref(), Some("explain this"));
        assert!(matches!(
            request.target,
            Some(Target::Text(TextCapture { ref text, .. })) if text == "selected text"
        ));
        assert!(matches!(
            request.context,
            Some(ContextCapture { image: ImageCapture { ref png, .. }, .. }) if png == b"png"
        ));
    }

    #[test]
    fn removing_target_preserves_context_for_provider_request() {
        let context = context();
        let mut state = State::default();
        let capture_id = state.start_capture();
        state.commit_capture(
            capture_id,
            Capture {
                target: Some(Target::Text(TextCapture {
                    text: "selected text".into(),
                    method: TextCaptureMethod::Accessibility,
                })),
                context: Some(context),
                provenance: None,
                elapsed_ms: 0,
            },
        );

        state.remove_target();
        let (request, _, _) =
            state.start_provider_request(Some("explain the context".into()), None);

        assert!(request.target.is_none());
        assert_eq!(request.prompt.as_deref(), Some("explain the context"));
        assert!(matches!(
            request.context,
            Some(ContextCapture { image: ImageCapture { ref png, .. }, .. }) if png == b"png"
        ));
    }

    #[test]
    fn beginning_a_capture_discards_the_previous_capture() {
        let mut state = State::default();
        let first_id = state.start_capture();
        state.commit_capture(
            first_id,
            Capture {
                target: Some(Target::Image(image())),
                context: None,
                provenance: None,
                elapsed_ms: 0,
            },
        );

        state.start_capture();

        assert!(state.current_capture.is_none());
    }

    #[test]
    fn stale_region_capture_does_not_replace_the_current_target() {
        let mut state = State::default();
        let stale_id = state.start_capture();
        let current_id = state.start_capture();
        state.commit_capture(
            current_id,
            Capture {
                target: Some(Target::Text(TextCapture {
                    text: "current target".into(),
                    method: TextCaptureMethod::Accessibility,
                })),
                context: None,
                provenance: None,
                elapsed_ms: 0,
            },
        );

        assert!(!state.apply_region(stale_id, image()));
        assert!(matches!(
            state.current_capture.and_then(|capture| capture.target),
            Some(Target::Text(TextCapture { ref text, .. })) if text == "current target"
        ));
    }

    #[test]
    fn region_capture_replaces_text_target_and_preserves_context() {
        let context = context();
        let mut state = State::default();
        let capture_id = state.start_capture();
        state.commit_capture(
            capture_id,
            Capture {
                target: Some(Target::Text(TextCapture {
                    text: "selected text".into(),
                    method: TextCaptureMethod::Accessibility,
                })),
                context: Some(context),
                provenance: None,
                elapsed_ms: 0,
            },
        );

        assert!(state.apply_region(capture_id, image()));
        let (request, _, _) = state.start_provider_request(None, None);

        assert!(matches!(request.target, Some(Target::Image(_))));
        assert!(matches!(
            request.context,
            Some(ContextCapture { image: ImageCapture { ref png, .. }, .. }) if png == b"png"
        ));
    }

    #[test]
    fn newer_provider_request_aborts_the_previous_request() {
        let mut state = State::default();
        let (_, first_id, first_registration) = state.start_provider_request(None, None);
        let first_abort = first_registration.handle();

        let (_, second_id, _) = state.start_provider_request(None, None);

        assert!(first_abort.is_aborted());
        assert_ne!(first_id, second_id);
        assert!(!state.accept_provider_output(first_id, &Output::AnswerChunk("stale".into())));
        assert!(state.accept_provider_output(second_id, &Output::AnswerChunk("current".into())));
    }

    #[test]
    fn beginning_a_capture_aborts_the_active_provider_request() {
        let mut state = State::default();
        let (_, request_id, registration) = state.start_provider_request(None, None);
        let abort = registration.handle();

        state.start_capture();

        assert!(abort.is_aborted());
        assert!(!state.accept_provider_output(request_id, &Output::AnswerChunk("stale".into())));
    }

    #[test]
    fn terminal_provider_output_closes_the_active_request() {
        let mut state = State::default();
        let (_, request_id, _) = state.start_provider_request(None, None);

        assert!(state.accept_provider_output(request_id, &Output::AnswerCompleted));
        assert!(!state.accept_provider_output(request_id, &Output::AnswerChunk("late".into())));
    }
}
