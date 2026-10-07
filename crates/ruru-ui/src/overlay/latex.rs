use std::borrow::Cow;
use std::ops::Range;

use iced::widget::{container, markdown, rich_text, row, svg, text};
use iced::{Center, Element, Fill, Length, Pixels};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parse as parse_latex;
use ratex_svg::SvgOptions;
use ratex_types::{color::Color, math_style::MathStyle};

use super::answering::Input;
use super::style;

const MATH_START: char = '\u{e000}';
const MATH_END: char = '\u{e001}';

#[derive(Debug, Clone, PartialEq, Eq)]
enum Fragment<'a> {
    Text(&'a str),
    Math { source: String, display: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplicitDelimiter {
    Parentheses,
    Brackets,
    Dollars,
}

impl ExplicitDelimiter {
    fn opening(self) -> &'static str {
        match self {
            Self::Parentheses => r"\(",
            Self::Brackets => r"\[",
            Self::Dollars => "$$",
        }
    }

    fn closing(self) -> &'static str {
        match self {
            Self::Parentheses => r"\)",
            Self::Brackets => r"\]",
            Self::Dollars => "$$",
        }
    }

    fn display(self) -> bool {
        !matches!(self, Self::Parentheses)
    }
}

pub(super) fn parse(source: &str) -> markdown::Content {
    markdown::Content::parse(&mark_math(source))
}

fn mark_math(source: &str) -> String {
    let source = mark_explicit_math(source);
    let parser = Parser::new_ext(&source, Options::ENABLE_MATH).into_offset_iter();
    let mut marked = String::with_capacity(source.len());
    let mut cursor = 0;

    for (event, range) in parser {
        let (math, display) = match event {
            Event::InlineMath(math) => (math, false),
            Event::DisplayMath(math) => (math, true),
            _ => continue,
        };

        marked.push_str(&source[cursor..range.start]);
        push_marker(&mut marked, &math, display);
        cursor = range.end;
    }

    marked.push_str(&source[cursor..]);
    marked
}

fn mark_explicit_math(source: &str) -> Cow<'_, str> {
    let protected = code_ranges(source);
    let mut output = None;
    let mut copied = 0;
    let mut search = 0;

    while let Some((opening, delimiter)) = find_opening(source, search, &protected) {
        let content_start = opening + delimiter.opening().len();
        let Some(closing) = find_delimiter(source, content_start, delimiter.closing(), &protected)
        else {
            search = content_start;
            continue;
        };

        let marked = output.get_or_insert_with(|| String::with_capacity(source.len()));
        marked.push_str(&source[copied..opening]);
        push_marker(
            marked,
            source[content_start..closing].trim(),
            delimiter.display(),
        );

        copied = closing + delimiter.closing().len();
        search = copied;
    }

    match output {
        Some(mut marked) => {
            marked.push_str(&source[copied..]);
            Cow::Owned(marked)
        }
        None => Cow::Borrowed(source),
    }
}

fn code_ranges(source: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut block_start = None;

    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = block_start.take() {
                    ranges.push(start..range.end);
                }
            }
            Event::Code(_) if block_start.is_none() => ranges.push(range),
            _ => {}
        }
    }

    ranges
}

fn find_opening(
    source: &str,
    start: usize,
    protected: &[Range<usize>],
) -> Option<(usize, ExplicitDelimiter)> {
    [
        ExplicitDelimiter::Parentheses,
        ExplicitDelimiter::Brackets,
        ExplicitDelimiter::Dollars,
    ]
    .into_iter()
    .filter_map(|delimiter| {
        find_delimiter(source, start, delimiter.opening(), protected)
            .map(|index| (index, delimiter))
    })
    .min_by_key(|(index, _)| *index)
}

fn find_delimiter(
    source: &str,
    start: usize,
    delimiter: &str,
    protected: &[Range<usize>],
) -> Option<usize> {
    source[start..]
        .match_indices(delimiter)
        .map(|(relative, _)| start + relative)
        .find(|index| {
            !protected.iter().any(|range| range.contains(index))
                && source[..*index]
                    .bytes()
                    .rev()
                    .take_while(|byte| *byte == b'\\')
                    .count()
                    % 2
                    == 0
        })
}

fn push_marker(output: &mut String, source: &str, display: bool) {
    output.push(MATH_START);
    output.push(if display { 'd' } else { 'i' });
    output.push(':');

    for byte in source.as_bytes() {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }

    output.push(MATH_END);
}

pub(super) struct Viewer;

impl<'a> markdown::Viewer<'a, Input> for Viewer {
    fn on_link_click(url: markdown::Uri) -> Input {
        Input::LinkClicked(url)
    }

    fn paragraph(
        &self,
        settings: markdown::Settings,
        content: &markdown::Text,
    ) -> Element<'a, Input> {
        if let Some(source) = display_math(content, settings.style) {
            return math(&source, true, settings.text_size);
        }

        if has_math(content, settings.style) {
            inline_content(content, settings, settings.text_size)
        } else {
            markdown::paragraph(settings, content, Self::on_link_click)
        }
    }

    fn heading(
        &self,
        settings: markdown::Settings,
        level: &'a markdown::HeadingLevel,
        content: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, Input> {
        let size = heading_size(settings, *level);

        if has_math(content, settings.style) {
            container(inline_content(content, settings, size))
                .padding(iced::padding::top(if index > 0 {
                    settings.text_size / 2.0
                } else {
                    Pixels::ZERO
                }))
                .into()
        } else {
            markdown::heading(settings, level, content, index, Self::on_link_click)
        }
    }
}

fn inline_content<'a>(
    content: &markdown::Text,
    settings: markdown::Settings,
    size: Pixels,
) -> Element<'a, Input> {
    let mut elements = Vec::new();

    for span in content.spans(settings.style).iter() {
        for fragment in fragments(&span.text) {
            match fragment {
                Fragment::Text(source) => {
                    for text_fragment in split_text(source) {
                        let mut styled = span.clone();
                        styled.text = Cow::Owned(text_fragment.to_owned());

                        elements.push(
                            rich_text(vec![styled])
                                .on_link_click(Input::LinkClicked)
                                .size(size)
                                .into(),
                        );
                    }
                }
                Fragment::Math { source, display } => {
                    elements.push(math(&source, display, size));
                }
            }
        }
    }

    row(elements)
        .width(Fill)
        .align_y(Center)
        .wrap()
        .vertical_spacing(0)
        .into()
}

fn math<'a>(source: &str, display: bool, size: Pixels) -> Element<'a, Input> {
    match render_math_svg(source, display, size) {
        Ok(bytes) => {
            let equation: Element<'a, Input> = svg::Svg::new(svg::Handle::from_memory(bytes))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .into();

            if display {
                container(equation)
                    .center_x(Fill)
                    .padding(style::MATH_BLOCK_PADDING)
                    .into()
            } else {
                equation
            }
        }
        Err(_) => text(source.to_owned())
            .font(iced::Font::MONOSPACE)
            .size(size)
            .color(style::DANGER_COLOR)
            .into(),
    }
}

fn render_math_svg(source: &str, display: bool, size: Pixels) -> Result<Vec<u8>, String> {
    let source = sanitize_math(source);
    let nodes = parse_latex(&source).map_err(|error| error.to_string())?;
    let color = style::TEXT_COLOR;
    let options = LayoutOptions {
        style: if display {
            MathStyle::Display
        } else {
            MathStyle::Text
        },
        color: Color::new(color.r, color.g, color.b, color.a),
        ..LayoutOptions::default()
    };
    let layout = layout(&nodes, &options);
    let display_list = to_display_list(&layout);
    let svg = ratex_svg::render_to_svg(
        &display_list,
        &SvgOptions {
            font_size: size.0 as f64,
            padding: 1.0,
            embed_glyphs: true,
            ..SvgOptions::default()
        },
    );

    Ok(svg.into_bytes())
}

fn sanitize_math(source: &str) -> Cow<'_, str> {
    if !source.contains("&nbsp;")
        && !source.contains("&lt;")
        && !source.contains("&gt;")
        && !source.contains("&amp;")
        && !source.contains('\u{a0}')
    {
        return Cow::Borrowed(source);
    }

    Cow::Owned(
        source
            .replace("&nbsp;", r"\;")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", r"\&")
            .replace('\u{a0}', " "),
    )
}

fn display_math(content: &markdown::Text, style: markdown::Style) -> Option<String> {
    let source = content
        .spans(style)
        .iter()
        .map(|span| span.text.as_ref())
        .collect::<String>();
    let mut fragments = fragments(source.trim());

    match (fragments.next(), fragments.next()) {
        (
            Some(Fragment::Math {
                source,
                display: true,
            }),
            None,
        ) => Some(source),
        _ => None,
    }
}

fn has_math(content: &markdown::Text, style: markdown::Style) -> bool {
    content
        .spans(style)
        .iter()
        .any(|span| fragments(&span.text).any(|fragment| matches!(fragment, Fragment::Math { .. })))
}

fn fragments(source: &str) -> impl Iterator<Item = Fragment<'_>> {
    let mut fragments = Vec::new();
    let mut cursor = 0;
    let mut search = 0;

    while let Some(relative_start) = source[search..].find(MATH_START) {
        let start = search + relative_start;
        let payload_start = start + MATH_START.len_utf8();
        let Some(relative_end) = source[payload_start..].find(MATH_END) else {
            break;
        };
        let end = payload_start + relative_end;
        let Some((display, math)) = decode_marker(&source[payload_start..end]) else {
            search = payload_start;
            continue;
        };

        if cursor < start {
            fragments.push(Fragment::Text(&source[cursor..start]));
        }
        fragments.push(Fragment::Math {
            source: math,
            display,
        });
        cursor = end + MATH_END.len_utf8();
        search = cursor;
    }

    if cursor < source.len() {
        fragments.push(Fragment::Text(&source[cursor..]));
    }

    fragments.into_iter()
}

fn decode_marker(payload: &str) -> Option<(bool, String)> {
    let (kind, encoded) = payload.split_once(':')?;
    let display = match kind {
        "i" => false,
        "d" => true,
        _ => return None,
    };

    if encoded.len() % 2 != 0 {
        return None;
    }

    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect::<Option<Vec<_>>>()?;

    String::from_utf8(bytes)
        .ok()
        .map(|source| (display, source))
}

fn split_text(source: &str) -> impl Iterator<Item = &str> {
    let mut start = 0;
    let mut fragments = Vec::new();

    for (index, character) in source.char_indices() {
        if character.is_whitespace() {
            let end = index + character.len_utf8();
            fragments.push(&source[start..end]);
            start = end;
        }
    }

    if start < source.len() {
        fragments.push(&source[start..]);
    }

    fragments
        .into_iter()
        .filter(|fragment| !fragment.is_empty())
}

fn heading_size(settings: markdown::Settings, level: markdown::HeadingLevel) -> Pixels {
    match level {
        markdown::HeadingLevel::H1 => settings.h1_size,
        markdown::HeadingLevel::H2 => settings.h2_size,
        markdown::HeadingLevel::H3 => settings.h3_size,
        markdown::HeadingLevel::H4 => settings.h4_size,
        markdown::HeadingLevel::H5 => settings.h5_size,
        markdown::HeadingLevel::H6 => settings.h6_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::font;

    #[test]
    fn pulldown_cmark_identifies_inline_and_display_math() {
        let marked = mark_math("Euler: $e^{i\\pi} + 1 = 0$.\n\n$$\\sum_i x_i$$");
        let parsed = fragments(&marked).collect::<Vec<_>>();

        assert_eq!(
            parsed,
            vec![
                Fragment::Text("Euler: "),
                Fragment::Math {
                    source: "e^{i\\pi} + 1 = 0".into(),
                    display: false,
                },
                Fragment::Text(".\n\n"),
                Fragment::Math {
                    source: "\\sum_i x_i".into(),
                    display: true,
                },
            ]
        );
    }

    #[test]
    fn standard_tex_delimiters_become_math() {
        let marked = mark_math(r"Inline \(x^2\). Display: \[\sum_i x_i\]");

        assert_eq!(
            fragments(&marked).collect::<Vec<_>>(),
            vec![
                Fragment::Text("Inline "),
                Fragment::Math {
                    source: "x^2".into(),
                    display: false,
                },
                Fragment::Text(". Display: "),
                Fragment::Math {
                    source: "\\sum_i x_i".into(),
                    display: true,
                },
            ]
        );
    }

    #[test]
    fn multiline_display_math_with_delimiter_whitespace_is_recognized() {
        let source = r"$$ \frac{\partial}{\partial t}u(x,t)
\alpha \frac{\partial^2}{\partial x^2}u(x,t) $$

$$ \nabla\cdot\mathbf{E}=\frac{\rho}{\varepsilon_0}, \qquad
\nabla\times\mathbf{B}=\mu_0\mathbf{J}+\mu_0\varepsilon_0\frac{\partial\mathbf{E}}{\partial t} $$";
        let marked = mark_math(source);
        let parsed = fragments(&marked).collect::<Vec<_>>();
        let formulas = parsed
            .iter()
            .filter_map(|fragment| match fragment {
                Fragment::Math { source, display } => Some((source, display)),
                Fragment::Text(_) => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(formulas.len(), 2);
        for (formula, display) in formulas {
            assert!(*display);
            render_math_svg(formula, true, Pixels(16.0))
                .expect("multiline display formula should render");
        }
    }

    #[test]
    fn math_is_not_recognized_inside_code() {
        let source = "`$inline$ \\(alternate\\)`\n\n```text\n$$display$$\n\\[alternate\\]\n```";
        assert_eq!(mark_math(source), source);
    }

    #[test]
    fn unmatched_and_escaped_alternate_delimiters_remain_text() {
        for source in [r"unmatched \[x", r"escaped \\[x\\]"] {
            assert_eq!(mark_math(source), source);
        }
    }

    #[test]
    fn prices_remain_text() {
        let source = "It costs $5 or $10.";
        assert_eq!(mark_math(source), source);
    }

    #[test]
    fn markers_round_trip_arbitrary_formula_text() {
        let mut marker = String::new();
        push_marker(&mut marker, r"x_{\text{owl}} = \$5", true);

        assert_eq!(
            fragments(&marker).collect::<Vec<_>>(),
            vec![Fragment::Math {
                source: r"x_{\text{owl}} = \$5".into(),
                display: true,
            }]
        );
    }

    #[test]
    fn iced_markdown_preserves_math_markers() {
        let content = parse("Before $x^2$ after");
        let Some(markdown::Item::Paragraph(text)) = content.items().first() else {
            panic!("expected a paragraph");
        };
        let source = text
            .spans(markdown::Style::from(iced::Theme::Dark))
            .iter()
            .map(|span| span.text.as_ref())
            .collect::<String>();

        assert_eq!(
            fragments(&source).collect::<Vec<_>>(),
            vec![
                Fragment::Text("Before "),
                Fragment::Math {
                    source: "x^2".into(),
                    display: false,
                },
                Fragment::Text(" after"),
            ]
        );
    }

    #[test]
    fn invalid_markers_remain_literal_text() {
        let source = "before \u{e000}not-a-marker\u{e001} after";
        assert_eq!(
            fragments(source).collect::<Vec<_>>(),
            vec![Fragment::Text(source)]
        );
    }

    #[test]
    fn common_latex_renders_to_svg() {
        let formulas = [
            ("scripts", r"x^2 + y_1 = 10"),
            ("fraction and root", r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}"),
            ("greek", r"\alpha + \beta = \Gamma"),
            ("sum", r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}"),
            ("integral", r"\int_0^1 x^2 \, dx = \frac{1}{3}"),
            (
                "limit and functions",
                r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
            ),
            ("delimiters", r"\left(\frac{a}{b}\right)"),
            ("matrix", r"\begin{bmatrix}a & b \\ c & d\end{bmatrix}"),
            (
                "cases",
                r"f(x)=\begin{cases}x^2 & x\ge 0 \\ -x & x<0\end{cases}",
            ),
            ("text", r"P(\text{heads}) = 0.5"),
            ("accents", r"\hat{x}, \bar{x}, \vec{x}"),
            ("math alphabets", r"\mathbb{R}, \mathbf{x}, \mathcal{F}"),
            ("set notation", r"A=\{3x:x\in 2\mathbb Z\}"),
            ("divisibility", r"6\mid(x-12)"),
        ];

        for (name, formula) in formulas {
            let svg = render_math_svg(formula, true, Pixels(16.0))
                .unwrap_or_else(|error| panic!("{name} failed to render: {error}"));
            let svg = String::from_utf8(svg).expect("SVG is UTF-8");

            assert!(svg.starts_with("<svg"), "{name} has no SVG root");
            assert!(svg.contains("viewBox="), "{name} has no viewport");
            assert!(svg.contains("<path"), "{name} has no rendered glyphs");
            assert!(svg.ends_with("</svg>"), "{name} has no closing SVG tag");
        }
    }

    #[test]
    fn common_markdown_blocks_are_preserved() {
        let content = parse(
            "# Heading\n\nParagraph\n\n> Quote\n\n- bullet\n\n1. ordered\n\n- [x] task\n\n---\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```rust\nlet x = 1;\n```\n\n![owl](https://example.com/owl.png)",
        );
        let items = content.items();

        assert!(matches!(items.first(), Some(markdown::Item::Heading(..))));
        assert!(
            items
                .iter()
                .any(|item| matches!(item, markdown::Item::Paragraph(..)))
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item, markdown::Item::Quote(..)))
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item, markdown::Item::List { start: None, .. }))
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item, markdown::Item::List { start: Some(1), .. }))
        );
        assert!(items.iter().any(|item| matches!(
            item,
            markdown::Item::List { bullets, .. }
                if bullets.iter().any(|bullet| matches!(
                    bullet,
                    markdown::Bullet::Task { done: true, .. }
                ))
        )));
        assert!(
            items
                .iter()
                .any(|item| matches!(item, markdown::Item::Rule))
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item, markdown::Item::Table { .. }))
        );
        assert!(items.iter().any(|item| matches!(
            item,
            markdown::Item::CodeBlock { language, code, .. }
                if language.as_deref() == Some("rust") && code.contains("let x = 1;")
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            markdown::Item::Image { url, .. }
                if url == "https://example.com/owl.png"
        )));
    }

    #[test]
    fn common_inline_markdown_styles_are_preserved() {
        let content =
            parse("plain **bold** *italic* ~~strike~~ `code` [link](https://example.com)");
        let Some(markdown::Item::Paragraph(text)) = content.items().first() else {
            panic!("expected a paragraph");
        };
        let style = style::answer_markdown();
        let spans = text.spans(style);
        let find = |needle: &str| {
            spans
                .iter()
                .find(|span| span.text.as_ref() == needle)
                .unwrap_or_else(|| panic!("missing {needle:?} span"))
        };

        assert_eq!(
            find("bold").font.expect("bold font").weight,
            font::Weight::Bold
        );
        assert_eq!(
            find("italic").font.expect("italic font").style,
            font::Style::Italic
        );
        assert!(find("strike").strikethrough);
        assert!(find("code").highlight.is_some());
        assert_eq!(find("link").link.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn math_survives_common_markdown_contexts() {
        let content = parse(
            r"# Energy $E=mc^2$

**Result:** $x=\frac{-b}{2a}$

$$\sum_{i=1}^n i$$",
        );
        let style = style::answer_markdown();

        for item in content.items() {
            let text = match item {
                markdown::Item::Heading(_, text) | markdown::Item::Paragraph(text) => text,
                _ => continue,
            };
            let source = text
                .spans(style)
                .iter()
                .map(|span| span.text.as_ref())
                .collect::<String>();

            for fragment in fragments(&source) {
                if let Fragment::Math { source, display } = fragment {
                    render_math_svg(&source, display, Pixels(16.0))
                        .unwrap_or_else(|error| panic!("integrated math failed: {error}"));
                }
            }
        }
    }

    #[test]
    fn fraction_shorthand_and_stretchy_braces_render() {
        render_math_svg(
            r"\left\{\frac12, \frac32, \frac52, \frac72\right\}",
            true,
            Pixels(16.0),
        )
        .expect("common TeX shorthand should render");
    }

    #[test]
    fn html_spacing_artifacts_are_removed_from_math() {
        let source = r"\left\{\frac{1}{2^n-1}: n \in \mathbb{Z},&nbsp;n \ge 1\right\}";
        let sanitized = sanitize_math(source);

        assert!(!sanitized.contains("&nbsp;"));
        render_math_svg(source, true, Pixels(16.0))
            .expect("HTML spacing artifacts should not break otherwise valid math");
    }
}
