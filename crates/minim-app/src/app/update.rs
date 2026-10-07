use iced::{Task, window as iced_window};
use minim_core::Output as CoreOutput;
use minim_types::Target;
use minim_ui::overlay;
use minim_ui::{OverlayOutput, Phase};

use super::window::{configure_window_for_active_space, show_overlay};
use super::{ANSWERING_HEIGHT, App, Input, PROMPTING_HEIGHT, WINDOW_WIDTH};

pub(super) fn update(state: &mut App, input: Input) -> Task<Input> {
    match input {
        Input::WindowOpened(id) => {
            state.window = id;
            match id {
                Some(id) => configure_window_for_active_space(id),
                None => Task::none(),
            }
        }
        Input::DragWindow => match state.window {
            Some(id) => iced_window::drag(id),
            None => Task::none(),
        },
        Input::Core(output) => match output {
            CoreOutput::ShowRequested { focused_window } => {
                let _ = state.overlay.update(overlay::Input::Show);
                match state.window {
                    Some(id) => show_overlay(id, focused_window),
                    None => Task::none(),
                }
            }
            CoreOutput::CaptureCompleted => {
                let _ = state.overlay.update(overlay::Input::CaptureCompleted);
                Task::none()
            }
            CoreOutput::TargetCaptured(target) => {
                let _ = state.overlay.update(overlay::Input::SetTarget(target));
                Task::none()
            }
            CoreOutput::RegionSelectionFinished(result) => {
                state.selecting_region = false;
                match result {
                    Ok(Some(image)) => {
                        let _ = state
                            .overlay
                            .update(overlay::Input::SetTarget(Target::Image(image)));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("region selection failed: {error}");
                        let _ = state.overlay.update(overlay::Input::CaptureFailed);
                    }
                }
                match state.window {
                    Some(id) => iced_window::set_mode(id, iced_window::Mode::Windowed)
                        .chain(iced_window::gain_focus(id)),
                    None => Task::none(),
                }
            }
            CoreOutput::AnswerChunk(chunk) => {
                let _ = state.overlay.update(overlay::Input::AppendAnswer(chunk));
                Task::none()
            }
            CoreOutput::AnswerCompleted => {
                let _ = state.overlay.update(overlay::Input::FinishAnswer);
                Task::none()
            }
            CoreOutput::RequestFailed(error) => {
                eprintln!("core request failed: {error}");
                let _ = state.overlay.update(overlay::Input::FailAnswer(error));
                Task::none()
            }
        },
        Input::Overlay(input) => match state.overlay.update(input) {
            Some(OverlayOutput::Submitted { prompt, model }) => {
                let available_models = state.providers.available_models();
                let Some(model) = model.filter(|selected| available_models.contains(selected))
                else {
                    let _ = state.overlay.update(overlay::Input::FailAnswer(
                        "no available model selected".into(),
                    ));
                    return Task::none();
                };
                let _ = state
                    .to_core
                    .try_send(minim_core::Input::Submit { prompt, model });
                match state.window {
                    Some(id) => {
                        iced_window::resize(id, iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT))
                    }
                    None => Task::none(),
                }
            }
            Some(OverlayOutput::TargetRemoved) => {
                if let Err(error) = state.to_core.try_send(minim_core::Input::RemoveTarget) {
                    eprintln!("failed to remove target from core: {error}");
                }
                Task::none()
            }
            Some(OverlayOutput::CaptureRegionRequested) => {
                if state.selecting_region {
                    return Task::none();
                }
                let Some(id) = state.window else {
                    return Task::none();
                };
                state.selecting_region = true;
                iced_window::set_mode(id, iced_window::Mode::Hidden)
                    .chain(Task::done(Input::BeginRegionSelection))
            }
            Some(OverlayOutput::ProviderRequestAbortRequested) => {
                if let Err(error) = state
                    .to_core
                    .try_send(minim_core::Input::AbortProviderRequest)
                {
                    eprintln!("failed to abort provider request: {error}");
                }
                Task::none()
            }
            Some(OverlayOutput::ProviderRequestRetryRequested) => {
                if let Err(error) = state
                    .to_core
                    .try_send(minim_core::Input::RetryProviderRequest)
                {
                    eprintln!("failed to retry provider request: {error}");
                }
                Task::none()
            }
            Some(OverlayOutput::LinkClicked(uri)) => {
                println!("link clicked: {uri}");
                Task::none()
            }
            Some(OverlayOutput::PhaseChanged(phase)) => match (phase, state.window) {
                (Phase::Prompting, Some(id)) => {
                    iced_window::resize(id, iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT))
                }
                (Phase::Answering, Some(id)) => {
                    iced_window::resize(id, iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT))
                }
                (_, None) => Task::none(),
            },
            Some(OverlayOutput::Dismissed) => match state.window {
                Some(id) => iced_window::set_mode(id, iced_window::Mode::Hidden),
                None => Task::none(),
            },
            None => Task::none(),
        },
        Input::CheckPromptInputFocus => {
            if state.overlay.visible && state.overlay.phase == Phase::Prompting {
                state.overlay.check_prompt_input_focus().map(Input::Overlay)
            } else {
                Task::none()
            }
        }
        Input::BeginRegionSelection => {
            if let Err(error) = state.to_core.try_send(minim_core::Input::SelectRegion) {
                state.selecting_region = false;
                eprintln!("region selection failed to start: {error}");
                let _ = state.overlay.update(overlay::Input::CaptureFailed);
                match state.window {
                    Some(id) => iced_window::set_mode(id, iced_window::Mode::Windowed)
                        .chain(iced_window::gain_focus(id)),
                    None => Task::none(),
                }
            } else {
                Task::none()
            }
        }
    }
}
