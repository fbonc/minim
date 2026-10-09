use iced::{Task, window as iced_window};
use ruru_core::{Input as CoreInput, Output as CoreOutput, SettingsChange};
use ruru_types::{ProviderId, Target};
use ruru_ui::main_window;
use ruru_ui::overlay;
use ruru_ui::{OverlayMode, OverlayOutput};

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
            Some(output) => handle_main_window_output(state, output),
            None => Task::none(),
        },
        Input::Core(output) => match output {
            CoreOutput::ShowRequested { focused_window } => {
                advance_provider_request_id(state);
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
            CoreOutput::AnswerChunk { request_id, chunk } => {
                if state.current_provider_request_id == request_id {
                    let _ = state.overlay.ui.update(overlay::Input::AppendAnswer(chunk));
                }
                Task::none()
            }
            CoreOutput::AnswerCompleted { request_id } => {
                if state.current_provider_request_id == request_id {
                    advance_provider_request_id(state);
                    let _ = state.overlay.ui.update(overlay::Input::FinishAnswer);
                }
                Task::none()
            }
            CoreOutput::ProviderRequestFailed { request_id, error } => {
                if state.current_provider_request_id == request_id {
                    advance_provider_request_id(state);
                    eprintln!("provider request failed: {error}");
                    let _ = state.overlay.ui.update(overlay::Input::FailAnswer(error));
                }
                Task::none()
            }
            CoreOutput::RequestFailed(error) => {
                eprintln!("core request failed: {error}");
                let _ = state.overlay.ui.update(overlay::Input::FailAnswer(error));
                Task::none()
            }
            CoreOutput::SettingsUpdated(update) => {
                advance_provider_request_id(state);
                let _ = state
                    .overlay
                    .ui
                    .update(overlay::Input::ProviderRequestAbortRequested);
                match update.change {
                    SettingsChange::KeySaved => state.main_window.ui.key_saved(&update.provider),
                    SettingsChange::KeyRemoved => {
                        state.main_window.ui.key_removed(&update.provider)
                    }
                    SettingsChange::ModelsChanged => {}
                }
                state
                    .main_window
                    .ui
                    .set_models(&update.provider, update.models);
                state.main_window.ui.clear_settings_error();
                state.overlay.ui.reconcile_models(&update.available_models);
                Task::none()
            }
            CoreOutput::ModelsDiscovered { provider, result } => {
                match result {
                    Ok(models) => state
                        .main_window
                        .ui
                        .finish_model_discovery(&provider, models),
                    Err(error) => state.main_window.ui.provider_error(&provider, error),
                }
                Task::none()
            }
            CoreOutput::SettingsFailed { provider, error } => {
                if let Some(provider) = provider {
                    state.main_window.ui.provider_error(&provider, error);
                } else {
                    state.main_window.ui.settings_error(error);
                }
                Task::none()
            }
        },
        Input::Overlay(input) => match state.overlay.ui.update(input) {
            Some(OverlayOutput::Submitted { prompt, model }) => {
                let request_id = advance_provider_request_id(state);
                if let Err(error) = state.to_core.unbounded_send(CoreInput::Submit {
                    request_id,
                    prompt,
                    model,
                }) {
                    advance_provider_request_id(state);
                    let _ = state.overlay.ui.update(overlay::Input::FailAnswer(format!(
                        "failed to start provider request: {error}"
                    )));
                    return Task::none();
                }
                iced_window::resize(
                    state.overlay.id,
                    iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT),
                )
            }
            Some(OverlayOutput::ModelSelected(model)) => {
                if let Err(error) = state.to_core.unbounded_send(CoreInput::SelectModel(model)) {
                    eprintln!("failed to select model: {error}");
                }
                Task::none()
            }
            Some(OverlayOutput::TargetRemoved) => {
                if let Err(error) = state.to_core.unbounded_send(CoreInput::RemoveTarget) {
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
                abort_provider_request(state);
                Task::none()
            }
            Some(OverlayOutput::ProviderRequestRetryRequested) => {
                let request_id = advance_provider_request_id(state);
                if let Err(error) = state
                    .to_core
                    .unbounded_send(CoreInput::RetryProviderRequest { request_id })
                {
                    advance_provider_request_id(state);
                    eprintln!("failed to retry provider request: {error}");
                    let _ = state.overlay.ui.update(overlay::Input::FailAnswer(format!(
                        "failed to retry provider request: {error}"
                    )));
                }
                Task::none()
            }
            Some(OverlayOutput::CopyAnswerRequested(answer)) => iced::clipboard::write(answer),
            Some(OverlayOutput::LinkClicked(uri)) => {
                println!("link clicked: {uri}");
                Task::none()
            }
            Some(OverlayOutput::ModeChanged(mode)) => match mode {
                OverlayMode::Prompting => {
                    abort_provider_request(state);
                    iced_window::resize(
                        state.overlay.id,
                        iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT),
                    )
                }
                OverlayMode::Answering => iced_window::resize(
                    state.overlay.id,
                    iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT),
                ),
            },
            Some(OverlayOutput::Dismissed) => {
                abort_provider_request(state);
                iced_window::set_mode(state.overlay.id, iced_window::Mode::Hidden)
            }
            None => Task::none(),
        },
        Input::CheckPromptInputFocus(id) => {
            if id == state.overlay.id
                && state.overlay.ui.visible
                && state.overlay.ui.mode == OverlayMode::Prompting
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
            if let Err(error) = state.to_core.unbounded_send(CoreInput::SelectRegion) {
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

fn abort_provider_request(state: &mut App) {
    advance_provider_request_id(state);
    if let Err(error) = state
        .to_core
        .unbounded_send(CoreInput::AbortProviderRequest)
    {
        eprintln!("failed to abort provider request: {error}");
    }
}

fn advance_provider_request_id(state: &mut App) -> ruru_core::ProviderRequestId {
    state.current_provider_request_id = state.current_provider_request_id.wrapping_add(1);
    state.current_provider_request_id
}

fn handle_main_window_output(state: &mut App, output: main_window::Output) -> Task<Input> {
    match output {
        main_window::Output::Dismissed => {
            iced_window::set_mode(state.main_window.id, iced_window::Mode::Hidden)
        }
        main_window::Output::JumpToSection(section) => {
            state.main_window.ui.jump_to_section(section)
        }
        main_window::Output::DiscoverModels(id) => {
            state.main_window.ui.start_model_discovery(&id);
            send_settings_command(state, &id, CoreInput::DiscoverModels(id.clone()))
        }
        main_window::Output::SaveKey(id, key) => {
            state.main_window.ui.start_model_discovery(&id);
            send_settings_command(state, &id, CoreInput::SaveKey(id.clone(), key))
        }
        main_window::Output::RemoveKey(id) => {
            send_settings_command(state, &id, CoreInput::RemoveKey(id.clone()))
        }
        main_window::Output::SetModel {
            provider,
            model,
            enabled,
        } => send_settings_command(
            state,
            &provider,
            CoreInput::SetModel {
                provider: provider.clone(),
                model,
                enabled,
            },
        ),
    }
}

fn send_settings_command(state: &mut App, provider: &ProviderId, input: CoreInput) -> Task<Input> {
    if let Err(error) = state.to_core.unbounded_send(input) {
        state
            .main_window
            .ui
            .provider_error(provider, error.to_string());
    }
    Task::none()
}
