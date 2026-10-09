use iced::Element;

pub mod main_window;
pub mod overlay;
pub(crate) mod style;

pub use overlay::Output as OverlayOutput;
pub use overlay::{Overlay, OverlayMode};
pub use ruru_types::Target;

pub fn view(overlay: &Overlay) -> Element<'_, overlay::Input> {
    overlay.view()
}
