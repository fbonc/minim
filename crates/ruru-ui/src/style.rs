use iced::widget::scrollable;
use iced::{Border, Color, Theme};

pub const TEXT_COLOR: Color = Color::from_rgb8(0xED, 0xEB, 0xE6);

pub const MUTED_COLOR: Color = Color::from_rgb8(0xB3, 0xB0, 0xA8);

pub const DANGER_COLOR: Color = Color::from_rgb8(0xF0, 0x8A, 0x5D);

pub const ACCENT_COLOR: Color = Color::from_rgb8(0xFF, 0x8A, 0x9E);

pub const BACKGROUND_COLOR: Color = Color::from_rgb8(0x15, 0x15, 0x14);

pub const INPUT_BACKGROUND: Color = Color::from_rgb8(0x28, 0x28, 0x26);

pub const INPUT_BORDER_COLOR: Color = Color::from_rgb8(0x47, 0x45, 0x3F);

pub const SELECTION_COLOR: Color = Color::from_rgb8(0x3D, 0x18, 0x20);

pub const SCROLLER_COLOR: Color = Color::from_rgb8(0x8E, 0x8B, 0x83);

pub const BORDER_COLOR: Color = Color::from_rgb8(0x34, 0x33, 0x2F);

pub const SCROLLBAR_WIDTH: f32 = 6.0;

pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut base = scrollable::default(theme, status);
    base.vertical_rail.background = None;
    base.vertical_rail.border = Border::default();
    base.vertical_rail.scroller.background = SCROLLER_COLOR.into();
    base
}
