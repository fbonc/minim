use iced::widget::scrollable;
use iced::{Border, Color, Theme};

pub const TEXT_COLOR: Color = Color::from_rgb8(0xEE, 0xEB, 0xFF);

pub const MUTED_COLOR: Color = Color::from_rgb8(0xB4, 0xAF, 0xC9);

pub const DANGER_COLOR: Color = Color::from_rgb8(0xFF, 0x7A, 0x8A);

pub const ACCENT_COLOR: Color = Color::from_rgb8(0xFF, 0x7A, 0x99);

pub const BACKGROUND_COLOR: Color = Color::from_rgb8(0x17, 0x15, 0x1F);

pub const INPUT_BACKGROUND: Color = Color::from_rgb8(0x2A, 0x27, 0x38);

pub const INPUT_BORDER_COLOR: Color = Color::from_rgb8(0x48, 0x43, 0x5F);

pub const SELECTION_COLOR: Color = Color::from_rgb8(0x2C, 0x27, 0x50);

pub const SCROLLER_COLOR: Color = Color::from_rgb8(0x8C, 0x87, 0xA3);

pub const BORDER_COLOR: Color = Color::from_rgb8(0x34, 0x30, 0x4A);

pub const SCROLLBAR_WIDTH: f32 = 6.0;

pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut base = scrollable::default(theme, status);
    base.vertical_rail.background = None;
    base.vertical_rail.border = Border::default();
    base.vertical_rail.scroller.background = SCROLLER_COLOR.into();
    base
}
