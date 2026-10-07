use iced::Element;
use minim_provider::ModelSelection;

pub mod main_window;
pub mod overlay;
pub(crate) mod style;

pub use minim_types::Target;
pub use overlay::Output as OverlayOutput;
pub use overlay::{Overlay, Phase};

pub fn view(
    overlay: &Overlay,
    available_models: Vec<ModelSelection>,
) -> Element<'_, overlay::Input> {
    overlay.view(available_models)
}
