use std::time::Duration;

use iced::widget::{container, markdown, scrollable, svg, text};
use iced::{Element, Fill, Radians, Subscription};

use super::{latex, style};

#[derive(Debug, Clone)]
pub enum Input {
    LinkClicked(markdown::Uri),
    LoadingTick,
}

#[derive(Debug, Default)]
pub(super) struct Answering {
    answer: String,
    markdown: markdown::Content,
    done: bool,
    error: Option<String>,
    loading_frame: u8,
}

impl Answering {
    pub fn update(&mut self, input: Input) {
        match input {
            Input::LoadingTick => self.loading_frame = self.loading_frame.wrapping_add(1) % 24,
            Input::LinkClicked(_) => {}
        }
    }

    pub fn push_token(&mut self, token: &str) {
        self.answer.push_str(token);
        self.markdown = latex::parse(&self.answer);
    }

    pub fn finish(&mut self) {
        self.done = true;
    }

    pub fn fail(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(super) fn is_streaming(&self) -> bool {
        !self.done && self.error.is_none()
    }

    pub fn subscription(&self) -> Subscription<Input> {
        if self.is_waiting() {
            iced::time::every(Duration::from_millis(80)).map(|_| Input::LoadingTick)
        } else {
            Subscription::none()
        }
    }

    pub fn view(&self) -> Element<'_, Input> {
        if let Some(error) = self.error() {
            return text(error)
                .size(style::ERROR_SIZE)
                .color(style::DANGER_COLOR)
                .into();
        }

        if self.is_waiting() {
            let angle = Radians(f32::from(self.loading_frame) * std::f32::consts::TAU / 24.0);
            let loader = svg(svg::Handle::from_memory(
                include_bytes!("../../../../assets/loading.svg").as_slice(),
            ))
            .width(style::LOADING_ICON_SIZE)
            .height(style::LOADING_ICON_SIZE)
            .rotation(angle);

            return container(loader).center(Fill).into();
        }

        let answer = container(markdown::view_with(
            self.markdown.items(),
            markdown::Settings::with_text_size(style::ANSWER_SIZE, style::answer_markdown()),
            &latex::Viewer,
        ))
        .width(Fill)
        .padding(iced::Padding {
            top: 0.0,
            right: style::SCROLL_GUTTER,
            bottom: 0.0,
            left: 0.0,
        });

        scrollable(answer)
            .width(Fill)
            .height(Fill)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(style::SCROLLBAR_WIDTH)
                    .scroller_width(style::SCROLLBAR_WIDTH)
                    .margin(2.0),
            ))
            .style(style::scroll)
            .into()
    }

    fn is_waiting(&self) -> bool {
        self.answer.is_empty() && self.is_streaming()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    impl Answering {
                #[cfg(test)]
        pub fn answer(&self) -> &str {
            &self.answer
        }

        #[cfg(test)]
        pub fn is_done(&self) -> bool {
            self.done
        }
    }

    #[test]
    fn streamed_tokens_update_the_source_and_markdown() {
        let mut answering = Answering::default();

        answering.push_token("# Head");
        answering.push_token("ing\n\nBody");

        assert_eq!(answering.answer(), "# Heading\n\nBody");
        assert!(matches!(
            answering.markdown.items().first(),
            Some(markdown::Item::Heading(..))
        ));
        assert!(matches!(
            answering.markdown.items().get(1),
            Some(markdown::Item::Paragraph(..))
        ));
    }

    #[test]
    fn reset_clears_the_previous_response() {
        let mut answering = Answering::default();
        answering.push_token("old answer");
        answering.finish();
        answering.fail("old error".into());

        answering.reset();

        assert!(answering.answer().is_empty());
        assert!(answering.markdown.items().is_empty());
        assert!(!answering.is_done());
        assert!(answering.error().is_none());
        assert!(answering.is_waiting());
    }

    #[test]
    fn waiting_ends_when_the_first_token_arrives() {
        let mut answering = Answering::default();
        assert!(answering.is_waiting());

        answering.push_token("first");

        assert!(!answering.is_waiting());
    }
}
