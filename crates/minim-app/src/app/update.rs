use iced::{Task, window as iced_window};
use minim_core::Output as CoreOutput;
use minim_types::Target;
use minim_ui::main_window;
use minim_ui::overlay;
use minim_ui::{OverlayOutput, Phase};

use super::windows::{configure_window_for_active_space, show_overlay};
use super::{ANSWERING_HEIGHT, App, Input, PROMPTING_HEIGHT, WINDOW_WIDTH};

pub(super) fn update(state: &mut App, input: Input) -> Task<Input> {
    match input {
        Input::WindowOpened(id) if id == state.overlay.id => configure_window_for_active_space(id),
        #[cfg(target_os = "macos")]
        Input::WindowOpened(id) if id == state.main_window.id => {
            // AppKit installs its Apple event handlers during launch.
            Task::run(super::reopen::install(), |_| Input::ShowMainWindow)
        }
        Input::WindowOpened(_) => Task::none(),
        Input::WindowCloseRequested(id) if id == state.main_window.id => update(
            state,
            Input::MainWindow(main_window::Input::DismissRequested),
        ),
        Input::WindowCloseRequested(id) if id == state.overlay.id => {
            update(state, Input::Overlay(overlay::Input::DismissRequested))
        }
        Input::WindowCloseRequested(_) => Task::none(),
        #[cfg(target_os = "macos")]
        Input::ShowMainWindow => {
            let was_visible = state.main_window.ui.visible;
            let _ = state.main_window.ui.update(main_window::Input::Show);
            if was_visible {
                iced_window::minimize(state.main_window.id, false)
                    .chain(iced_window::gain_focus(state.main_window.id))
            } else {
                iced_window::set_mode(state.main_window.id, iced_window::Mode::Windowed)
                    .chain(iced_window::gain_focus(state.main_window.id))
            }
        }
        Input::DragWindow => iced_window::drag(state.overlay.id),
        Input::MainWindow(input) => match state.main_window.ui.update(input) {
            Some(main_window::Output::Dismissed) => {
                iced_window::set_mode(state.main_window.id, iced_window::Mode::Hidden)
            }
            None => Task::none(),
        },
        Input::Core(output) => match output {
            CoreOutput::ShowRequested { focused_window } => {
                let _ = state.overlay.ui.update(overlay::Input::Show);
                show_overlay(state.overlay.id, focused_window)
            }
            CoreOutput::CaptureCompleted => {
                let _ = state.overlay.ui.update(overlay::Input::CaptureCompleted);
                Task::none()
            }
            CoreOutput::TargetCaptured(target) => {
                let _ = state.overlay.ui.update(overlay::Input::SetTarget(target));
                Task::none()
            }
            CoreOutput::RegionSelectionFinished(result) => {
                state.selecting_region = false;
                match result {
                    Ok(Some(image)) => {
                        let _ = state
                            .overlay
                            .ui
                            .update(overlay::Input::SetTarget(Target::Image(image)));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("region selection failed: {error}");
                        let _ = state.overlay.ui.update(overlay::Input::CaptureFailed);
                    }
                }
                iced_window::set_mode(state.overlay.id, iced_window::Mode::Windowed)
                    .chain(iced_window::gain_focus(state.overlay.id))
            }
            CoreOutput::AnswerChunk(chunk) => {
                let _ = state.overlay.ui.update(overlay::Input::AppendAnswer(chunk));
                Task::none()
            }
            CoreOutput::AnswerCompleted => {
                let _ = state.overlay.ui.update(overlay::Input::FinishAnswer);
                Task::none()
            }
            CoreOutput::RequestFailed(error) => {
                eprintln!("core request failed: {error}");
                let _ = state.overlay.ui.update(overlay::Input::FailAnswer(error));
                Task::none()
            }
        },
        Input::Overlay(input) => match state.overlay.ui.update(input) {
            Some(OverlayOutput::Submitted { prompt, model }) => {
                let available_models = state.providers.available_models();
                let Some(model) = model.filter(|selected| available_models.contains(selected))
                else {
                    let _ = state.overlay.ui.update(overlay::Input::FailAnswer(
                        "no available model selected".into(),
                    ));
                    return Task::none();
                };
                let _ = state
                    .to_core
                    .try_send(minim_core::Input::Submit { prompt, model });
                iced_window::resize(
                    state.overlay.id,
                    iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT),
                )
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
                state.selecting_region = true;
                iced_window::set_mode(state.overlay.id, iced_window::Mode::Hidden)
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
            Some(OverlayOutput::PhaseChanged(phase)) => match phase {
                Phase::Prompting => iced_window::resize(
                    state.overlay.id,
                    iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT),
                ),
                Phase::Answering => iced_window::resize(
                    state.overlay.id,
                    iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT),
                ),
            },
            Some(OverlayOutput::Dismissed) => {
                iced_window::set_mode(state.overlay.id, iced_window::Mode::Hidden)
            }
            None => Task::none(),
        },
        Input::CheckPromptInputFocus(id) => {
            if id == state.overlay.id
                && state.overlay.ui.visible
                && state.overlay.ui.phase == Phase::Prompting
            {
                state
                    .overlay
                    .ui
                    .check_prompt_input_focus()
                    .map(Input::Overlay)
            } else {
                Task::none()
            }
        }
        Input::BeginRegionSelection => {
            if let Err(error) = state.to_core.try_send(minim_core::Input::SelectRegion) {
                state.selecting_region = false;
                eprintln!("region selection failed to start: {error}");
                let _ = state.overlay.ui.update(overlay::Input::CaptureFailed);
                iced_window::set_mode(state.overlay.id, iced_window::Mode::Windowed)
                    .chain(iced_window::gain_focus(state.overlay.id))
            } else {
                Task::none()
            }
        }
    }
}
