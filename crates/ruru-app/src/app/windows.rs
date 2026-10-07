use iced::{Task, window as iced_window};
use ruru_types::WindowBounds;

use super::{Input, PROMPTING_HEIGHT, WINDOW_WIDTH};

pub(super) fn main_settings() -> iced_window::Settings {
    iced_window::Settings {
        size: iced::Size::new(900.0, 620.0),
        min_size: Some(iced::Size::new(600.0, 400.0)),
        position: iced_window::Position::Centered,
        exit_on_close_request: false,
        ..Default::default()
    }
}

pub(super) fn overlay_settings() -> iced_window::Settings {
    iced_window::Settings {
        size: iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT),
        min_size: Some(iced::Size::new(
            super::MIN_WINDOW_WIDTH,
            super::MIN_WINDOW_HEIGHT,
        )),
        position: iced_window::Position::Centered,
        visible: false,
        decorations: false,
        transparent: true,
        resizable: true,
        level: iced_window::Level::AlwaysOnTop,
        exit_on_close_request: false,
        ..Default::default()
    }
}

pub(super) fn show_overlay(
    id: iced_window::Id,
    focused_window: Option<WindowBounds>,
) -> Task<Input> {
    let task = iced_window::set_mode(id, iced_window::Mode::Hidden).chain(iced_window::resize(
        id,
        iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT),
    ));
    let task = match focused_window {
        Some(bounds) => task.chain(iced_window::move_to(id, overlay_position(bounds))),
        None => task,
    };

    task.chain(iced_window::set_mode(id, iced_window::Mode::Windowed))
        .chain(iced_window::gain_focus(id))
}

fn overlay_position(bounds: WindowBounds) -> iced::Point {
    iced::Point::new(
        (bounds.x + (bounds.w - f64::from(WINDOW_WIDTH)) / 2.0) as f32,
        (bounds.y + (bounds.h - f64::from(PROMPTING_HEIGHT)) / 2.0) as f32,
    )
}

#[cfg(target_os = "macos")]
pub(super) fn configure_window_for_active_space(id: iced_window::Id) -> Task<Input> {
    iced_window::run(id, |window| {
        use iced::window::raw_window_handle::RawWindowHandle;
        use objc2::rc::Retained;
        use objc2_app_kit::{NSView, NSWindowCollectionBehavior};

        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // The AppKit raw handle owns a valid NSView for the duration of this callback.
        let Some(view): Option<Retained<NSView>> =
            (unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) })
        else {
            return;
        };
        let Some(window) = view.window() else {
            return;
        };
        window.setCollectionBehavior(
            window.collectionBehavior() | NSWindowCollectionBehavior::MoveToActiveSpace,
        );
    })
    .discard()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn configure_window_for_active_space(_id: iced_window::Id) -> Task<Input> {
    Task::none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_is_centered_on_the_focused_window() {
        assert_eq!(
            overlay_position(WindowBounds {
                x: 100.0,
                y: 200.0,
                w: 1_200.0,
                h: 800.0,
            }),
            iced::Point::new(450.0, 510.0)
        );
    }
}
