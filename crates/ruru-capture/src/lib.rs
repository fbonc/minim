use thiserror::Error;

pub use ruru_types::{
    ContextCapture, ImageCapture, Provenance, ScreenRect, Target, TextCapture, TextCaptureMethod,
    WindowBounds,
};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Accessibility,
    ScreenRecording,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("no capture method succeeded")]
    AllMethodsFailed,

    #[error("{permission:?} permission not granted")]
    PermissionDenied { permission: Permission },

    #[error("{0}")]
    Platform(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct PendingContextCapture {
    complete: Box<dyn FnOnce() -> Result<ContextCapture> + Send>,
}

impl PendingContextCapture {
    pub fn new(complete: impl FnOnce() -> Result<ContextCapture> + Send + 'static) -> Self {
        Self {
            complete: Box::new(complete),
        }
    }

    pub fn completed(context: ContextCapture) -> Self {
        Self::new(|| Ok(context))
    }

    pub fn complete(self) -> Result<ContextCapture> {
        (self.complete)()
    }
}

pub trait Capturer: Send + Sync {
    fn capture_text(&self) -> Result<TextCapture>;

    fn begin_context_capture(&self) -> Result<PendingContextCapture>;

    fn select_region(&self) -> Result<Option<ImageCapture>>;

    fn capture_provenance(&self) -> Result<Provenance>;

    fn capture_window_bounds(&self) -> Result<WindowBounds>;
}

pub fn new_capturer() -> Box<dyn Capturer> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacosCapturer::new());
}
