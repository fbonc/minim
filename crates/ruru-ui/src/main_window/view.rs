use iced::widget::{container, space};
use iced::{Element, Fill};

use super::{Input, MainWindow, style};

impl MainWindow {
    pub fn view(&self) -> Element<'_, Input> {
        container(space())
            .width(Fill)
            .height(Fill)
            .style(style::window)
            .into()
    }
}
