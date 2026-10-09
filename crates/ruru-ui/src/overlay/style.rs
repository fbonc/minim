use iced::border::Radius;
use iced::widget::{button, container, markdown, overlay::menu, pick_list, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme};

pub use crate::style::{
    ACCENT_COLOR, BORDER_COLOR, DANGER_COLOR, INPUT_BACKGROUND, INPUT_BORDER_COLOR, MUTED_COLOR,
    SCROLLBAR_WIDTH, SELECTION_COLOR, TEXT_COLOR, scroll,
};

pub const MODEL_PICKER_BACKGROUND: Color = Color::from_rgb8(0x21, 0x1E, 0x2C);
pub const MODEL_MENU_HIGHLIGHT: Color = Color::from_rgb8(0x2C, 0x27, 0x50);
pub const BORDER_STYLE: Border = Border {
    color: BORDER_COLOR,
    width: 1.0,
    radius: Radius {
        top_left: 16.0,
        top_right: 16.0,
        bottom_right: 16.0,
        bottom_left: 16.0,
    },
};

pub const CARD_SPACING: f32 = 14.0;
pub const CARD_PADDING: f32 = 22.0;
pub const INPUT_PADDING: f32 = 12.0;
pub const INPUT_RADIUS: f32 = 10.0;
pub const LOGO_SIZE: f32 = 80.0;
pub const HEADER_SPACING: f32 = 10.0;
pub const HEADER_ACTION_SPACING: f32 = 4.0;
pub const HEADER_ACTION_SIZE: f32 = 28.0;
pub const HEADER_ACTION_ICON_SIZE: f32 = 18.0;
pub const SCROLL_GUTTER: f32 = 14.0;
pub const MATH_BLOCK_PADDING: f32 = 8.0;
pub const INPUT_ACTION_SPACING: f32 = 0.0;

pub const TARGET_SIZE: f32 = 16.0;
pub const TARGET_CLEAR_ICON_SIZE: f32 = 17.0;
pub const ABORT_ICON_SIZE: f32 = 13.0;
pub const COPY_ICON_SIZE: f32 = 17.0;
pub const RETRY_ICON_SIZE: f32 = 17.0;
pub const LOADING_ICON_SIZE: f32 = 20.0;
pub const INPUT_SIZE: f32 = 16.0;
pub const ANSWER_SIZE: f32 = 15.0;
pub const ERROR_SIZE: f32 = 14.0;
pub const SEND_SIZE: f32 = 24.0;
pub const SEND_BUTTON_SIZE: f32 = 36.0;
pub const SEND_BUTTON_INSET: f32 = 4.0;
pub const CAPTURE_BUTTON_TEXT_SIZE: f32 = 13.0;
pub const CAPTURE_BUTTON_SPACING: f32 = 6.0;
pub const MODEL_PICKER_TEXT_SIZE: f32 = 13.0;

pub const PLACEHOLDERS: [&str; 9] = [
    "Ask Ruru…",
    "What’s on your mind?",
    "Ribbit me a question…",
    "Hop to it…",
    "Croak away…",
    "Toss Ruru a fly…",
    "Ponder away…",
    "Leap in…",
    "What are we wondering?",
];
pub const IMAGE_TARGET_LABEL: &str = "[image capture]";

#[cfg(target_os = "macos")]
// Cosmic Text resolves `.SF NS` bold to Menlo instead of the system bold face.
const ANSWER_FONT: Font = Font::with_name("Helvetica Neue");
#[cfg(not(target_os = "macos"))]
const ANSWER_FONT: Font = Font::DEFAULT;

pub const HINT_TEXT: &str = "Or just press Enter — I’ll figure it out";
pub const HINT_SIZE: f32 = 12.5;
pub const HINT_SPACING: f32 = 8.0;
pub const HINT_COLOR: Color = Color::from_rgb8(0x8C, 0x87, 0xA3);

pub fn input_box(_theme: &Theme, focused: bool) -> container::Style {
    let border_color = if focused {
        Color::from_rgba8(0xA9, 0x9F, 0xFF, 0.45)
    } else {
        INPUT_BORDER_COLOR
    };

    container::Style {
        background: Some(INPUT_BACKGROUND.into()),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: INPUT_RADIUS.into(),
        },
        text_color: Some(Color::TRANSPARENT),
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn input(_theme: &Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        icon: MUTED_COLOR,
        placeholder: MUTED_COLOR,
        value: TEXT_COLOR,
        selection: SELECTION_COLOR,
    }
}

pub fn send_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(BORDER_COLOR.into()),
        button::Status::Pressed => Some(INPUT_BORDER_COLOR.into()),
        button::Status::Active | button::Status::Disabled => None,
    };

    button::Style {
        background,
        text_color: match status {
            button::Status::Disabled => MUTED_COLOR,
            _ => Color::from_rgb8(0xC9, 0xC2, 0xFF),
        },
        border: Border {
            radius: INPUT_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn header_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered => (Some(INPUT_BACKGROUND.into()), TEXT_COLOR),
        button::Status::Pressed => (Some(MODEL_MENU_HIGHLIGHT.into()), TEXT_COLOR),
        _ => (None, MUTED_COLOR),
    };

    button::Style {
        background,
        text_color,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn capture_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered => (Some(INPUT_BACKGROUND.into()), ACCENT_COLOR),
        button::Status::Pressed => (Some(MODEL_MENU_HIGHLIGHT.into()), TEXT_COLOR),
        _ => (None, MUTED_COLOR),
    };

    button::Style {
        background,
        text_color,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn model_picker(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let border_color = match status {
        pick_list::Status::Active => INPUT_BORDER_COLOR,
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => {
            Color::from_rgb8(0x6C, 0x5C, 0xFF)
        }
    };

    pick_list::Style {
        text_color: TEXT_COLOR,
        placeholder_color: MUTED_COLOR,
        handle_color: MUTED_COLOR,
        background: MODEL_PICKER_BACKGROUND.into(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
    }
}

pub fn model_menu(_theme: &Theme) -> menu::Style {
    menu::Style {
        background: MODEL_PICKER_BACKGROUND.into(),
        border: Border {
            color: INPUT_BORDER_COLOR,
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: TEXT_COLOR,
        selected_text_color: TEXT_COLOR,
        selected_background: MODEL_MENU_HIGHLIGHT.into(),
        shadow: Shadow::default(),
    }
}

pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(MODEL_PICKER_BACKGROUND.into()),
        border: BORDER_STYLE,
        text_color: Some(TEXT_COLOR),
        ..Default::default()
    }
}

pub fn answer_markdown() -> markdown::Style {
    let mut style = markdown::Style::from(Theme::Dark);
    style.font = ANSWER_FONT;
    style.inline_code_highlight.background = INPUT_BACKGROUND.into();
    style.inline_code_color = TEXT_COLOR;
    style.link_color = Color::from_rgb8(0xA9, 0x9F, 0xFF);
    style
}
