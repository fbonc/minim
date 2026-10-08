use std::sync::LazyLock;

use iced::advanced::text::Wrapping;
use iced::widget::{button, column, container, image, row, space, svg, text};
use iced::{Center, Element, Fill};

use ruru_provider::ModelSelection;
use ruru_types::Target;

use super::style;
use super::{Input, Overlay, Phase};

static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../../../assets/banner.png").as_slice())
});

impl Overlay {
    pub fn view(&self, available_models: Vec<ModelSelection>) -> Element<'_, Input> {
        let selected_model = self
            .selected_model
            .clone()
            .filter(|selected| available_models.contains(selected));
        let content = match self.phase {
            Phase::Prompting => self
                .prompting
                .view(available_models, selected_model, self.capture_ready)
                .map(Input::Prompting),
            Phase::Answering => self.answering.view().map(Input::Answering),
        };

        container(
            column![header(self), content]
                .spacing(style::CARD_SPACING)
                .width(Fill)
                .height(Fill),
        )
        .padding(style::CARD_PADDING)
        .width(Fill)
        .height(Fill)
        .style(style::card)
        .into()
    }
}

fn header(overlay: &Overlay) -> Element<'_, Input> {
    let target: Element<'_, Input> = if let Some(target) = &overlay.target {
        let label = match target {
            Target::Text(text) => text
                .text
                .as_str()
                .trim()
                .replace("\n", "")
                .replace("\r", "")
                .replace("\t", ""),
            Target::Image(_) => style::IMAGE_TARGET_LABEL.to_string(),
        };

        let mut target = row![
            container(
                text(label)
                    .size(style::TARGET_SIZE)
                    .color(style::MUTED_COLOR)
                    .width(Fill)
                    .wrapping(Wrapping::None),
            )
            .width(Fill)
            .clip(true)
        ];

        if overlay.phase == Phase::Prompting {
            target = target.push(target_clear_button());
        }

        target
            .spacing(style::HEADER_ACTION_SPACING)
            .align_y(Center)
            .width(Fill)
            .into()
    } else {
        space().width(Fill).into()
    };

    let mut actions = row![].spacing(style::HEADER_ACTION_SPACING);

    if overlay.phase == Phase::Answering {
        if overlay.answering.is_streaming() {
            actions = actions.push(abort_button());
        } else {
            if overlay.answering.can_copy() {
                actions = actions.push(copy_button());
            }
            actions = actions.push(retry_button());
        }
        actions = actions.push(header_button("←", Input::BackRequested));
    }

    actions = actions.push(header_button("×", Input::DismissRequested));

    row![image(LOGO.clone()).width(style::LOGO_SIZE), target, actions,]
        .spacing(style::HEADER_SPACING)
        .align_y(Center)
        .into()
}

fn copy_button() -> Element<'static, Input> {
    let icon = svg(svg::Handle::from_memory(
        include_bytes!("../../../../assets/copy.svg").as_slice(),
    ))
    .width(style::COPY_ICON_SIZE)
    .height(style::COPY_ICON_SIZE);

    button(container(icon).center(Fill))
        .on_press(Input::CopyAnswerRequested)
        .width(style::HEADER_ACTION_SIZE)
        .height(style::HEADER_ACTION_SIZE)
        .padding(0)
        .style(style::header_button)
        .into()
}

fn retry_button() -> Element<'static, Input> {
    let icon = svg(svg::Handle::from_memory(
        include_bytes!("../../../../assets/retry.svg").as_slice(),
    ))
    .width(style::RETRY_ICON_SIZE)
    .height(style::RETRY_ICON_SIZE);

    button(container(icon).center(Fill))
        .on_press(Input::ProviderRequestRetryRequested)
        .width(style::HEADER_ACTION_SIZE)
        .height(style::HEADER_ACTION_SIZE)
        .padding(0)
        .style(style::header_button)
        .into()
}

fn abort_button() -> Element<'static, Input> {
    let icon = svg(svg::Handle::from_memory(
        include_bytes!("../../../../assets/stop.svg").as_slice(),
    ))
    .width(style::ABORT_ICON_SIZE)
    .height(style::ABORT_ICON_SIZE);

    button(container(icon).center(Fill))
        .on_press(Input::ProviderRequestAbortRequested)
        .width(style::HEADER_ACTION_SIZE)
        .height(style::HEADER_ACTION_SIZE)
        .padding(0)
        .style(style::header_button)
        .into()
}

fn header_button(label: &'static str, input: Input) -> Element<'static, Input> {
    let icon = container(text(label).size(style::HEADER_ACTION_ICON_SIZE)).center(Fill);

    button(icon)
        .on_press(input)
        .width(style::HEADER_ACTION_SIZE)
        .height(style::HEADER_ACTION_SIZE)
        .padding(0)
        .style(style::header_button)
        .into()
}

fn target_clear_button() -> Element<'static, Input> {
    let icon = svg(svg::Handle::from_memory(
        include_bytes!("../../../../assets/clear-target.svg").as_slice(),
    ))
    .width(style::TARGET_CLEAR_ICON_SIZE)
    .height(style::TARGET_CLEAR_ICON_SIZE);

    button(container(icon).center(Fill))
        .on_press(Input::RemoveTargetRequested)
        .width(style::HEADER_ACTION_SIZE)
        .height(style::HEADER_ACTION_SIZE)
        .padding(0)
        .style(style::header_button)
        .into()
}
