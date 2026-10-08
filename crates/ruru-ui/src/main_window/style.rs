use iced::widget::{button, container};
use iced::{Border, Theme};

use crate::style::{BACKGROUND_COLOR, INPUT_BACKGROUND, MUTED_COLOR, SELECTION_COLOR, TEXT_COLOR};

pub(super) const WINDOW_PADDING: f32 = 24.0;
pub(super) const LOGO_SIZE: f32 = 120.0;
pub(super) const HEADER_ACTION_SIZE: f32 = 36.0;
pub(super) const HEADER_ACTION_ICON_SIZE: f32 = 22.0;
pub(super) const SETTINGS_ICON_SIZE: f32 = 25.0;

pub(super) fn window(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(BACKGROUND_COLOR.into()),
        text_color: Some(TEXT_COLOR),
        ..Default::default()
    }
}

pub(super) fn header_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(INPUT_BACKGROUND.into()),
        button::Status::Pressed => Some(SELECTION_COLOR.into()),
        _ => None,
    };

    button::Style {
        background,
        text_color: match status {
            button::Status::Hovered | button::Status::Pressed => TEXT_COLOR,
            _ => MUTED_COLOR,
        },
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
