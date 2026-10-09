use iced::widget::{button, checkbox, container, overlay::menu, pick_list, rule, text_input};
use iced::{Background, Border, Color, Shadow, Theme};

use crate::style::{
    BACKGROUND_COLOR, BORDER_COLOR, INPUT_BACKGROUND, INPUT_BORDER_COLOR, SCROLLER_COLOR,
    SELECTION_COLOR, TEXT_COLOR,
};
pub(super) use crate::style::{DANGER_COLOR, MUTED_COLOR};

pub(super) const WINDOW_PADDING: f32 = 24.0;
pub(super) const CONTENT_SPACING: f32 = 20.0;
pub(super) const LOGO_SIZE: f32 = 120.0;
pub(super) const HEADER_ACTION_SIZE: f32 = 36.0;
pub(super) const HEADER_ACTION_ICON_SIZE: f32 = 22.0;
pub(super) const SETTINGS_ICON_SIZE: f32 = 25.0;
pub(super) const SIDEBAR_WIDTH: f32 = 160.0;
pub(super) const SETTINGS_COLUMN_SPACING: f32 = 24.0;
pub(super) const SCROLL_CONTENT_GAP: f32 = 38.0;
pub(super) const SECTION_PADDING: f32 = 18.0;
pub(super) const SECTION_SPACING: f32 = 16.0;
pub(super) const MODEL_LIST_WIDTH: f32 = 360.0;
pub(super) const MODEL_LIST_HEIGHT: f32 = 280.0;
pub(super) const SECTION_TEXT_SIZE: f32 = 13.5;
pub(super) const PAGE_HEADING_SIZE: f32 = 24.0;
pub(super) const SUBHEADING_SIZE: f32 = 17.0;
pub(super) const BODY_TEXT_SIZE: f32 = 12.5;

pub(super) fn window(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(BACKGROUND_COLOR.into()),
        text_color: Some(TEXT_COLOR),
        ..Default::default()
    }
}

pub(super) fn sidebar_rule(theme: &Theme) -> rule::Style {
    let mut style = rule::default(theme);
    style.color = BORDER_COLOR;
    style
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

pub(super) fn provider_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(0x1E, 0x1E, 0x1C).into()),
        border: Border {
            color: BORDER_COLOR,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

pub(super) fn model_list(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(INPUT_BACKGROUND.into()),
        border: Border {
            color: BORDER_COLOR,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

pub(super) fn selected_section(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(SELECTION_COLOR.into()),
        text_color: TEXT_COLOR,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(super) fn action_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgb8(0xB0, 0x1C, 0x38),
        button::Status::Active => Color::from_rgb8(0xC8, 0x20, 0x3F),
        button::Status::Disabled => BORDER_COLOR,
    };

    button::Style {
        background: Some(background.into()),
        text_color: if status == button::Status::Disabled {
            MUTED_COLOR
        } else {
            Color::WHITE
        },
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(super) fn add_provider_picker(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: TEXT_COLOR,
        placeholder_color: MUTED_COLOR,
        handle_color: MUTED_COLOR,
        background: match status {
            pick_list::Status::Hovered | pick_list::Status::Opened { .. } => {
                INPUT_BORDER_COLOR.into()
            }
            pick_list::Status::Active => BORDER_COLOR.into(),
        },
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
    }
}

pub(super) fn add_provider_menu(_theme: &Theme) -> menu::Style {
    menu::Style {
        background: INPUT_BACKGROUND.into(),
        border: Border {
            color: INPUT_BORDER_COLOR,
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: TEXT_COLOR,
        selected_text_color: TEXT_COLOR,
        selected_background: SELECTION_COLOR.into(),
        shadow: Shadow::default(),
    }
}

pub(super) fn subtle_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(SELECTION_COLOR.into()),
            _ => None,
        },
        text_color: if status == button::Status::Disabled {
            MUTED_COLOR
        } else {
            TEXT_COLOR
        },
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(super) fn key_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(INPUT_BACKGROUND),
        border: Border {
            color: if matches!(status, text_input::Status::Focused { .. }) {
                Color::from_rgba8(0xFF, 0x8A, 0x9E, 0.45)
            } else {
                BORDER_COLOR
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        icon: MUTED_COLOR,
        placeholder: SCROLLER_COLOR,
        value: TEXT_COLOR,
        selection: SELECTION_COLOR,
    }
}

pub(super) fn provider_checkbox(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let (checked, hovered) = match status {
        checkbox::Status::Active { is_checked } => (is_checked, false),
        checkbox::Status::Hovered { is_checked } => (is_checked, true),
        checkbox::Status::Disabled { is_checked } => (is_checked, false),
    };

    checkbox::Style {
        background: if checked {
            Color::from_rgb8(0xC8, 0x20, 0x3F).into()
        } else {
            INPUT_BACKGROUND.into()
        },
        icon_color: Color::WHITE,
        border: Border {
            color: if checked || hovered {
                Color::from_rgb8(0xC8, 0x20, 0x3F)
            } else {
                BORDER_COLOR
            },
            width: 1.0,
            radius: 4.0.into(),
        },
        text_color: Some(TEXT_COLOR),
    }
}
