use iced::Theme;
use iced::widget::container;

use crate::style::{BACKGROUND_COLOR, TEXT_COLOR};

pub(super) fn window(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(BACKGROUND_COLOR.into()),
        text_color: Some(TEXT_COLOR),
        ..Default::default()
    }
}
