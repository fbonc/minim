use super::answering;
use super::prompting;
use super::{Input, Output, Overlay, Phase};

impl Overlay {
    pub fn update(&mut self, input: Input) -> Option<Output> {
        match input {
            Input::Prompting(prompting::Input::SubmitRequested) if self.capture_ready => {
                self.phase = Phase::Answering;
                self.answering.reset();
                Some(Output::Submitted {
                    prompt: self.commit_prompt(),
                    model: self.selected_model.clone(),
                })
            }
            Input::Prompting(prompting::Input::CaptureRegionRequested) => {
                self.prompting
                    .update(prompting::Input::CaptureRegionRequested);
                Some(Output::CaptureRegionRequested)
            }
            Input::Prompting(prompting::Input::ModelSelected(selection)) => {
                self.selected_model = Some(selection);
                None
            }
            Input::Prompting(input) => {
                self.prompting.update(input);
                None
            }
            Input::Answering(answering::Input::LinkClicked(uri)) => Some(Output::LinkClicked(uri)),
            Input::Answering(input) => {
                self.answering.update(input);
                None
            }
            Input::Show => {
                self.visible = true;
                self.target = None;
                self.capture_ready = false;
                self.phase = Phase::Prompting;
                self.prompting.reset();
                None
            }
            Input::CaptureCompleted => {
                self.capture_ready = true;
                None
            }
            Input::SetTarget(target) => {
                self.target = Some(target);
                None
            }
            Input::RemoveTargetRequested if self.phase == Phase::Prompting => {
                self.target.take().map(|_| Output::TargetRemoved)
            }
            Input::RemoveTargetRequested => None,
            Input::CaptureFailed => {
                self.prompting.update(prompting::Input::CaptureFailed);
                None
            }
            Input::AppendAnswer(chunk) => {
                self.answering.push_token(&chunk);
                None
            }
            Input::FinishAnswer => {
                self.answering.finish();
                None
            }
            Input::FailAnswer(error) => {
                self.answering.fail(error);
                None
            }
            Input::ProviderRequestAbortRequested
                if self.phase == Phase::Answering && self.answering.is_streaming() =>
            {
                self.answering.finish();
                Some(Output::ProviderRequestAbortRequested)
            }
            Input::ProviderRequestAbortRequested => None,
            Input::ProviderRequestRetryRequested
                if self.phase == Phase::Answering && !self.answering.is_streaming() =>
            {
                self.answering.reset();
                Some(Output::ProviderRequestRetryRequested)
            }
            Input::ProviderRequestRetryRequested => None,
            Input::BackRequested => {
                self.phase = Phase::Prompting;
                Some(Output::PhaseChanged(Phase::Prompting))
            }
            Input::DismissRequested => {
                self.visible = false;
                Some(Output::Dismissed)
            }
        }
    }

    fn commit_prompt(&self) -> Option<String> {
        (!self.prompting.prompt_value().is_empty())
            .then(|| self.prompting.prompt_value().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use ruru_provider::{ModelSelection, ProviderId};
    use ruru_types::{Target, TextCapture, TextCaptureMethod};

    use super::*;

    fn text_target(text: &str) -> Target {
        Target::Text(TextCapture {
            text: text.into(),
            method: TextCaptureMethod::Accessibility,
        })
    }

    #[test]
    fn empty_prompt_is_the_default_action() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Show);
        overlay.update(Input::SetTarget(text_target("epistemic uncertainty")));
        overlay.update(Input::CaptureCompleted);

        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::SubmitRequested)),
            Some(Output::Submitted {
                prompt: None,
                model: None,
            })
        );
        assert!(overlay.visible);
    }

    #[test]
    fn showing_again_clears_the_previous_target() {
        let mut overlay = Overlay::new();
        overlay.update(Input::SetTarget(text_target("previous selection")));

        overlay.update(Input::Show);

        assert!(overlay.target.is_none());
        assert!(overlay.visible);
    }

    #[test]
    fn submission_waits_for_capture_completion() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Show);

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::SubmitRequested))
                .is_none()
        );
        assert_eq!(overlay.phase, Phase::Prompting);

        overlay.update(Input::CaptureCompleted);

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::SubmitRequested))
                .is_some()
        );
        assert_eq!(overlay.phase, Phase::Answering);
    }

    #[test]
    fn removing_target_clears_it_and_notifies_the_host() {
        let mut overlay = Overlay::new();
        overlay.update(Input::SetTarget(text_target("selected text")));

        assert_eq!(
            overlay.update(Input::RemoveTargetRequested),
            Some(Output::TargetRemoved)
        );
        assert!(overlay.target.is_none());
        assert!(overlay.update(Input::RemoveTargetRequested).is_none());
    }

    #[test]
    fn target_cannot_be_removed_while_answering() {
        let mut overlay = Overlay::new();
        overlay.update(Input::SetTarget(text_target("selected text")));
        overlay.phase = Phase::Answering;

        assert!(overlay.update(Input::RemoveTargetRequested).is_none());
        assert!(overlay.target.is_some());
    }

    #[test]
    fn showing_again_returns_to_prompting_with_the_new_target() {
        let mut overlay = Overlay::new();
        overlay.phase = Phase::Answering;
        overlay.update(Input::Prompting(prompting::Input::InputChanged(
            "old prompt".into(),
        )));
        overlay.update(Input::SetTarget(text_target("previous selection")));

        overlay.update(Input::Show);
        overlay.update(Input::SetTarget(text_target("new selection")));

        assert_eq!(overlay.phase, Phase::Prompting);
        assert!(overlay.prompting.prompt_value().is_empty());
        assert!(matches!(
            overlay.target,
            Some(Target::Text(TextCapture { ref text, .. })) if text == "new selection"
        ));
    }

    #[test]
    fn typed_text_becomes_the_ask() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Prompting(prompting::Input::InputChanged(
            "in one sentence".into(),
        )));

        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::SubmitRequested)),
            Some(Output::Submitted {
                prompt: Some("in one sentence".into()),
                model: None,
            })
        );
    }

    #[test]
    fn model_selection_is_stored_and_included_in_submission() {
        let mut overlay = Overlay::new();
        let selection = ModelSelection::new(ProviderId::new("openai"), "gpt-test");

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::ModelSelected(
                    selection.clone()
                )))
                .is_none()
        );
        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::SubmitRequested)),
            Some(Output::Submitted {
                prompt: None,
                model: Some(selection),
            })
        );
    }

    #[test]
    fn region_capture_request_keeps_the_prompt() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Prompting(prompting::Input::InputChanged(
            "what is shown?".into(),
        )));

        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::CaptureRegionRequested)),
            Some(Output::CaptureRegionRequested)
        );
        assert_eq!(overlay.phase, Phase::Prompting);
        assert_eq!(overlay.prompting.prompt_value(), "what is shown?");
    }

    #[test]
    fn submit_moves_from_prompting_to_answering() {
        let mut overlay = Overlay::new();
        assert_eq!(overlay.phase, Phase::Prompting);

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::SubmitRequested))
                .is_some()
        );

        assert_eq!(overlay.phase, Phase::Answering);
    }

    #[test]
    fn prompting_messages_stay_inside_the_overlay() {
        let mut overlay = Overlay::new();

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::InputChanged(
                    "why?".into()
                )))
                .is_none()
        );
        assert_eq!(overlay.phase, Phase::Prompting);
    }

    #[test]
    fn markdown_link_clicks_bubble_up_to_the_host() {
        let mut overlay = Overlay::new();

        assert_eq!(
            overlay.update(Input::Answering(answering::Input::LinkClicked(
                "https://example.com".into()
            ))),
            Some(Output::LinkClicked("https://example.com".into()))
        );
    }

    #[test]
    fn tokens_accumulate_into_the_answer() {
        let mut overlay = Overlay::new();
        overlay.update(Input::AppendAnswer("un".into()));
        overlay.update(Input::AppendAnswer("certainty".into()));
        overlay.update(Input::FinishAnswer);

        assert_eq!(overlay.answering.answer(), "uncertainty");
        assert!(overlay.answering.is_done());
    }

    #[test]
    fn aborting_finishes_the_partial_answer_and_notifies_the_host() {
        let mut overlay = Overlay::new();
        overlay.phase = Phase::Answering;
        overlay.update(Input::AppendAnswer("partial".into()));

        assert_eq!(
            overlay.update(Input::ProviderRequestAbortRequested),
            Some(Output::ProviderRequestAbortRequested)
        );
        assert_eq!(overlay.answering.answer(), "partial");
        assert!(overlay.answering.is_done());
        assert!(
            overlay
                .update(Input::ProviderRequestAbortRequested)
                .is_none()
        );
    }

    #[test]
    fn retrying_clears_the_previous_answer_and_notifies_the_host() {
        let mut overlay = Overlay::new();
        overlay.phase = Phase::Answering;
        overlay.update(Input::AppendAnswer("previous answer".into()));
        overlay.update(Input::FinishAnswer);

        assert_eq!(
            overlay.update(Input::ProviderRequestRetryRequested),
            Some(Output::ProviderRequestRetryRequested)
        );
        assert!(overlay.answering.answer().is_empty());
        assert!(!overlay.answering.is_done());
        assert!(
            overlay
                .update(Input::ProviderRequestRetryRequested)
                .is_none()
        );
    }

    #[test]
    fn back_returns_to_prompting() {
        let mut overlay = Overlay::new();
        overlay.phase = Phase::Answering;

        assert_eq!(
            overlay.update(Input::BackRequested),
            Some(Output::PhaseChanged(Phase::Prompting))
        );
        assert_eq!(overlay.phase, Phase::Prompting);
    }

    #[test]
    fn close_hides_the_overlay() {
        let mut overlay = Overlay::new();
        overlay.visible = true;

        assert_eq!(
            overlay.update(Input::DismissRequested),
            Some(Output::Dismissed)
        );
        assert!(!overlay.visible);
    }

    #[test]
    fn submitting_clears_the_previous_response() {
        let mut overlay = Overlay::new();
        overlay.answering.push_token("old answer");
        overlay.answering.finish();
        overlay.answering.fail("old error".into());

        overlay.update(Input::Prompting(prompting::Input::SubmitRequested));

        assert!(overlay.answering.answer().is_empty());
        assert!(!overlay.answering.is_done());
        assert!(overlay.answering.error().is_none());
    }
}
