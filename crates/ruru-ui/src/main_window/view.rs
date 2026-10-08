use std::sync::LazyLock;

use iced::widget::{button, column, container, image, row, space, svg, rule};
use iced::{Center, Element, Fill};

use super::{Input, MainWindow, WindowMode, style};

static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../../../assets/banner.png").as_slice())
});

impl MainWindow {
    pub fn view(&self) -> Element<'_, Input> {
        container(column![header(self), space()].width(Fill).height(Fill))
            .padding(style::WINDOW_PADDING)
            .width(Fill)
            .height(Fill)
            .style(style::window)
            .into()
    }
}

fn header(window: &MainWindow) -> Element<'_, Input> {
    let action: Element<'_, Input> = match window.mode {
        WindowMode::Home => settings_button(),
        WindowMode::Settings => space().width(style::HEADER_ACTION_SIZE).into(),
    };

    column![
    row![
        image(LOGO.clone()).width(style::LOGO_SIZE),
        space().width(Fill),
        action
    ]
    .align_y(Center)
    .width(Fill),
    rule::horizontal(1)
    ].spacing(15)
    .into()
}

fn settings_button() -> Element<'static, Input> {
    let icon = svg(svg::Handle::from_memory(
        include_bytes!("../../../../assets/settings.svg").as_slice(),
    ))
    .width(style::SETTINGS_ICON_SIZE)
    .height(style::SETTINGS_ICON_SIZE);

    button(container(icon).center(Fill))
        .on_press(Input::SettingsRequested)
        .width(style::HEADER_ACTION_SIZE)
        .height(style::HEADER_ACTION_SIZE)
        .padding(0)
        .style(style::header_button)
        .into()
}
