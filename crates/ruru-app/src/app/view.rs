use iced::widget::mouse_area;
use iced::{Element, Subscription, window as iced_window};

use super::{App, Input};

pub(super) fn view(state: &App, id: iced_window::Id) -> Element<'_, Input> {
    if id == state.overlay.id {
        mouse_area(ruru_ui::view(&state.overlay.ui).map(Input::Overlay))
            .on_press(Input::DragWindow)
            .into()
    } else if id == state.main_window.id {
        state.main_window.ui.view().map(Input::MainWindow)
    } else {
        iced::widget::space().into()
    }
}

pub(super) fn subscription(state: &App) -> Subscription<Input> {
    Subscription::batch([
        iced::event::listen_with(|event, _status, window| match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }) => {
                Some(Input::CheckPromptInputFocus(window))
            }
            iced::Event::Window(iced_window::Event::CloseRequested) => {
                Some(Input::WindowCloseRequested(window))
            }
            _ => None,
        }),
        state.overlay.ui.subscription().map(Input::Overlay),
    ])
}
