use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCaptureMethod {
    Accessibility,
    Clipboard,
}

// `display` is the platform display id (e.g., a CGDirectDisplayID on macOS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub display: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowBounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone)]
pub struct TextCapture {
    pub text: String,
    pub method: TextCaptureMethod,
}

#[derive(Debug, Clone)]
pub struct ImageCapture {
    pub png: Vec<u8>,
    /// Interactive selection supplies PNG bytes without a display rectangle.
    pub region: Option<ScreenRect>,
}

#[derive(Debug, Clone)]
pub struct ContextCapture {
    pub image: ImageCapture,
    pub accessibility_text: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Target {
    Text(TextCapture),
    Image(ImageCapture),
}

#[derive(Debug, Clone)]
pub struct Provenance {
    pub app_name: String,
    pub window_title: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModelSelection {
    pub provider: ProviderId,
    pub model: String,
}

impl ModelSelection {
    pub fn new(provider: ProviderId, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }
}

impl fmt::Display for ModelSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} · {}", self.model, self.provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_selection_has_a_dropdown_label() {
        let selection = ModelSelection::new(ProviderId::new("openai"), "gpt-test");
        assert_eq!(selection.to_string(), "gpt-test · openai");
    }
}
