use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use axuielement as ax;
use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
};
use xcap::image::codecs::png::{CompressionType, FilterType, PngEncoder};
use xcap::image::{ExtendedColorType, ImageEncoder, RgbaImage};
use xcap::{Monitor, Window};

use crate::{
    Capturer, ContextCapture, Error, ImageCapture, PendingContextCapture, Permission, Provenance,
    Result, ScreenRect, TextCapture, TextCaptureMethod, WindowBounds,
};

const CLIPBOARD_TIMEOUT: Duration = Duration::from_millis(150);
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_millis(15);

pub struct MacosCapturer {}

impl MacosCapturer {
    pub fn new() -> Self {
        Self {}
    }
}

// Target capture
impl MacosCapturer {
    fn ax_selected_text(&self) -> Option<String> {
        let text = focused_element()?
            .string_attribute(ax::ax_attribute::AX_SELECTED_TEXT_ATTRIBUTE)
            .ok()??;
        non_empty(text)
    }

    // text fallback: clipboard
    fn clipboard_text(&self) -> Option<String> {
        let mut clipboard = Clipboard::new().ok()?;
        let original = clipboard.get_text().ok();

        if !send_copy_shortcut() {
            return None;
        }

        let captured = poll_for_copied_text(&mut clipboard, original.as_deref());

        if captured.is_some() {
            match &original {
                Some(text) => {
                    let _ = clipboard.set_text(text.clone());
                }
                None => {
                    let _ = clipboard.clear();
                }
            }
        }

        captured
    }

    #[cfg(test)]
    fn capture_image(&self, region: ScreenRect) -> Result<ImageCapture> {
        let png = screenshot_png(region)?;
        Ok(ImageCapture {
            png,
            region: Some(region),
        })
    }
}

// Provenance capture
impl MacosCapturer {
    fn focused_app_name(&self) -> Option<String> {
        Some(
            NSWorkspace::sharedWorkspace()
                .frontmostApplication()?
                .localizedName()?
                .to_string(),
        )
    }

    fn focused_window_title(&self) -> Option<String> {
        focused_window()?
            .string_attribute(ax::ax_attribute::AX_TITLE_ATTRIBUTE)
            .ok()?
    }

    fn document_path(&self) -> Option<String> {
        let doc = focused_window()?
            .string_attribute(ax::ax_attribute::AX_DOCUMENT_ATTRIBUTE)
            .ok()??;
        doc_url_to_path(&doc)
    }

    fn focused_window_bounds(&self) -> Option<WindowBounds> {
        let window = focused_window()?;
        let position = window
            .point_attribute(ax::ax_attribute::AX_POSITION_ATTRIBUTE)
            .ok()??;
        let size = window
            .size_attribute(ax::ax_attribute::AX_SIZE_ATTRIBUTE)
            .ok()??;

        Some(WindowBounds {
            x: position.x,
            y: position.y,
            w: size.width,
            h: size.height,
        })
    }
}

impl Capturer for MacosCapturer {
    fn capture_text(&self) -> Result<TextCapture> {
        if !ax::is_process_trusted_with_prompt() {
            return Err(Error::PermissionDenied {
                permission: Permission::Accessibility,
            });
        }
        if let Some(text) = self.ax_selected_text() {
            return Ok(TextCapture {
                text,
                method: TextCaptureMethod::Accessibility,
            });
        }
        if let Some(text) = self.clipboard_text() {
            return Ok(TextCapture {
                text,
                method: TextCaptureMethod::Clipboard,
            });
        }
        Err(Error::AllMethodsFailed)
    }

    fn begin_context_capture(&self) -> Result<PendingContextCapture> {
        let frontmost_app = NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .ok_or(Error::AllMethodsFailed)?;
        let pid = frontmost_app.processIdentifier();
        let app = ax::AXUIElement::from_pid(pid).ok_or(Error::AllMethodsFailed)?;
        let _ = app.set_bool_attribute("AXManualAccessibility", true);
        let window = app
            .element_attribute(ax::ax_attribute::AX_FOCUSED_WINDOW_ATTRIBUTE)
            .ok()
            .flatten()
            .ok_or(Error::AllMethodsFailed)?;
        let pos = window
            .point_attribute(ax::ax_attribute::AX_POSITION_ATTRIBUTE)
            .ok()
            .flatten()
            .ok_or(Error::AllMethodsFailed)?;
        let size = window
            .size_attribute(ax::ax_attribute::AX_SIZE_ATTRIBUTE)
            .ok()
            .flatten()
            .ok_or(Error::AllMethodsFailed)?;
        let region = window_screen_rect(pos, size).ok_or(Error::AllMethodsFailed)?;
        let window_bounds = WindowBounds {
            x: pos.x,
            y: pos.y,
            w: size.width,
            h: size.height,
        };
        let capture_window = matching_window(pid as u32, window_bounds);
        let fallback_image = if capture_window.is_none() {
            Some(screenshot(region)?)
        } else {
            None
        };
        let accessibility_text = app
            .element_attribute(ax::ax_attribute::AX_FOCUSED_UI_ELEMENT_ATTRIBUTE)
            .ok()
            .flatten()
            .and_then(|element| {
                element
                    .string_attribute(ax::ax_attribute::AX_VALUE_ATTRIBUTE)
                    .ok()
                    .flatten()
            })
            .and_then(non_empty);

        Ok(PendingContextCapture::new(move || {
            let image = match capture_window {
                Some(window) => window.capture_image().map_err(xcap_err)?,
                None => fallback_image.expect("fallback context image"),
            };
            Ok(ContextCapture {
                image: ImageCapture {
                    png: encode_png(&image)?,
                    region: Some(region),
                },
                accessibility_text,
            })
        }))
    }

    fn select_region(&self) -> Result<Option<ImageCapture>> {
        let directory = tempfile::tempdir()
            .map_err(|e| Error::Platform(format!("failed to create capture file: {e}")))?;
        let path = directory.path().join("selection.png");
        let output = Command::new("/usr/sbin/screencapture")
            .args(["-i", "-s", "-x", "-d", "-t", "png"])
            .arg(&path)
            .output()
            .map_err(|e| Error::Platform(format!("failed to start region selection: {e}")))?;

        if !path.exists() {
            if output.stderr.is_empty() {
                return Ok(None);
            }
            return Err(Error::Platform(format!(
                "region selection failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        if !output.status.success() {
            return Err(Error::Platform(format!(
                "region selection failed: {}",
                output.status
            )));
        }

        let png = std::fs::read(&path)
            .map_err(|e| Error::Platform(format!("failed to read captured region: {e}")))?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(Error::Platform(
                "region selection did not produce a PNG".into(),
            ));
        }
        Ok(Some(ImageCapture { png, region: None }))
    }

    fn capture_provenance(&self) -> Result<Provenance> {
        Ok(Provenance {
            app_name: self.focused_app_name().unwrap_or_default(),
            window_title: self.focused_window_title().unwrap_or_default(),
            path: self.document_path(),
        })
    }

    fn capture_window_bounds(&self) -> Result<WindowBounds> {
        self.focused_window_bounds().ok_or(Error::AllMethodsFailed)
    }
}

fn matching_window(pid: u32, bounds: WindowBounds) -> Option<Window> {
    Window::all()
        .ok()?
        .into_iter()
        .filter(|window| window.pid().ok() == Some(pid))
        .min_by_key(|window| {
            let values = [
                (window.x().ok().map(i64::from), bounds.x),
                (window.y().ok().map(i64::from), bounds.y),
                (window.width().ok().map(i64::from), bounds.w),
                (window.height().ok().map(i64::from), bounds.h),
            ];

            values
                .into_iter()
                .try_fold(0_u64, |score, (actual, expected)| {
                    Some(score + (actual? - expected.round() as i64).unsigned_abs())
                })
                .unwrap_or(u64::MAX)
        })
}

fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s) }
}

fn frontmost_app_element() -> Option<ax::AXUIElement> {
    let pid = NSWorkspace::sharedWorkspace()
        .frontmostApplication()?
        .processIdentifier();
    let app = ax::AXUIElement::from_pid(pid)?;
    // Ask Chromium/Electron apps to build their AX tree (off by default).
    let _ = app.set_bool_attribute("AXManualAccessibility", true);
    Some(app)
}

fn focused_element() -> Option<ax::AXUIElement> {
    frontmost_app_element()?
        .element_attribute(ax::ax_attribute::AX_FOCUSED_UI_ELEMENT_ATTRIBUTE)
        .ok()?
}

fn focused_window() -> Option<ax::AXUIElement> {
    frontmost_app_element()?
        .element_attribute(ax::ax_attribute::AX_FOCUSED_WINDOW_ATTRIBUTE)
        .ok()?
}

fn screenshot(region: ScreenRect) -> Result<RgbaImage> {
    let monitor = monitor_for_display(region.display)?;
    let (mon_w, mon_h) = (
        monitor.width().map_err(xcap_err)?,
        monitor.height().map_err(xcap_err)?,
    );
    let (x, y, w, h) = clamp_region(region, mon_w, mon_h)
        .ok_or_else(|| Error::Platform("capture region is empty or off-screen".to_string()))?;
    monitor.capture_region(x, y, w, h).map_err(xcap_err)
}

#[cfg(test)]
fn screenshot_png(region: ScreenRect) -> Result<Vec<u8>> {
    encode_png(&screenshot(region)?)
}

fn monitor_for_display(display: u32) -> Result<Monitor> {
    Monitor::all()
        .map_err(xcap_err)?
        .into_iter()
        .find(|m| m.id().map(|id| id == display).unwrap_or(false))
        .ok_or_else(|| Error::Platform(format!("no monitor with display id {display}")))
}

// Clip a display-relative region to the monitor and snap to whole pixels.
// Returns None if nothing on-screen remains.
fn clamp_region(region: ScreenRect, mon_w: u32, mon_h: u32) -> Option<(u32, u32, u32, u32)> {
    let (mon_w, mon_h) = (mon_w as f64, mon_h as f64);
    let left = region.x.max(0.0);
    let top = region.y.max(0.0);
    let right = (region.x + region.w).min(mon_w);
    let bottom = (region.y + region.h).min(mon_h);
    let w = right - left;
    let h = bottom - top;
    if w < 1.0 || h < 1.0 {
        return None;
    }
    Some((left as u32, top as u32, w as u32, h as u32))
}

// Map an AX window rect (global top-left points) to a display-relative ScreenRect.
fn window_screen_rect(pos: ax::AXPoint, size: ax::AXSize) -> Option<ScreenRect> {
    let center_x = (pos.x + size.width / 2.0) as i32;
    let center_y = (pos.y + size.height / 2.0) as i32;
    let monitor = Monitor::from_point(center_x, center_y).ok()?;
    Some(ScreenRect {
        x: pos.x - monitor.x().ok()? as f64,
        y: pos.y - monitor.y().ok()? as f64,
        w: size.width,
        h: size.height,
        display: monitor.id().ok()?,
    })
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    PngEncoder::new_with_quality(&mut buf, CompressionType::Fast, FilterType::Sub)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| Error::Platform(format!("PNG encode failed: {e}")))?;
    Ok(buf)
}

fn xcap_err(e: xcap::XCapError) -> Error {
    Error::Platform(format!("screen capture failed: {e}"))
}

fn send_copy_shortcut() -> bool {
    const KEY_C: u16 = 8;

    let Some(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return false;
    };
    let Some(key_down) = CGEvent::new_keyboard_event(Some(&source), KEY_C, true) else {
        return false;
    };
    let Some(key_up) = CGEvent::new_keyboard_event(Some(&source), KEY_C, false) else {
        return false;
    };

    CGEvent::set_flags(Some(&key_down), CGEventFlags::MaskCommand);
    CGEvent::set_flags(Some(&key_up), CGEventFlags::MaskCommand);
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&key_down));
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&key_up));
    true
}

fn poll_for_copied_text(clipboard: &mut Clipboard, original: Option<&str>) -> Option<String> {
    let deadline = Instant::now() + CLIPBOARD_TIMEOUT;
    loop {
        if let Ok(current) = clipboard.get_text()
            && let Some(text) = clipboard_changed(original, &current)
        {
            return Some(text);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(CLIPBOARD_POLL_INTERVAL);
    }
}

fn clipboard_changed(original: Option<&str>, current: &str) -> Option<String> {
    if current.trim().is_empty() {
        return None;
    }
    if Some(current) == original {
        return None;
    }
    Some(current.to_string())
}

// Convert a `kAXDocument` value into a filesystem path.
// normally a `file://` URL but some apps hand back a bare POSIX path.
fn doc_url_to_path(doc: &str) -> Option<String> {
    let trimmed = doc.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("file://") {
        let path = rest.strip_prefix("localhost").unwrap_or(rest);
        if !path.starts_with('/') {
            return None;
        }
        Some(percent_decode(path))
    } else if trimmed.starts_with('/') {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Decode `%XX` escapes in a URL path back into raw bytes, interpreting the
/// result as UTF-8. Invalid escapes are left verbatim.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_rejects_blank() {
        assert_eq!(non_empty("hello".to_string()), Some("hello".to_string()));
        assert_eq!(non_empty("  x ".to_string()), Some("  x ".to_string()));
        assert_eq!(non_empty(String::new()), None);
        assert_eq!(non_empty("   \n\t".to_string()), None);
    }

    #[test]
    fn clipboard_change_detection() {
        // Unchanged: same value that was already on the clipboard.
        assert_eq!(clipboard_changed(Some("old"), "old"), None);
        // Changed: a new value replaced the old one.
        assert_eq!(
            clipboard_changed(Some("old"), "new"),
            Some("new".to_string())
        );
        // Changed from an empty/non-text clipboard.
        assert_eq!(clipboard_changed(None, "new"), Some("new".to_string()));
        // Whitespace-only copies are treated as no capture.
        assert_eq!(clipboard_changed(Some("old"), "   "), None);
        assert_eq!(clipboard_changed(None, ""), None);
    }

    #[test]
    fn doc_url_plain_and_encoded() {
        assert_eq!(
            doc_url_to_path("file:///Users/a/notes.txt"),
            Some("/Users/a/notes.txt".to_string())
        );
        assert_eq!(
            doc_url_to_path("file:///Users/a/my%20file.pdf"),
            Some("/Users/a/my file.pdf".to_string())
        );
        // Optional localhost host component.
        assert_eq!(
            doc_url_to_path("file://localhost/Users/a/b.txt"),
            Some("/Users/a/b.txt".to_string())
        );
        // Non-ASCII via UTF-8 percent bytes: %C3%A9 -> é.
        assert_eq!(
            doc_url_to_path("file:///Users/caf%C3%A9.txt"),
            Some("/Users/café.txt".to_string())
        );
    }

    #[test]
    fn doc_url_bare_path_and_rejects() {
        // Some apps return a bare POSIX path.
        assert_eq!(
            doc_url_to_path("/Users/a/b.txt"),
            Some("/Users/a/b.txt".to_string())
        );
        // Not a file: URL and not a path.
        assert_eq!(doc_url_to_path("https://example.com"), None);
        assert_eq!(doc_url_to_path(""), None);
        assert_eq!(doc_url_to_path("   "), None);
    }

    #[test]
    fn percent_decode_edge_cases() {
        assert_eq!(percent_decode("abc"), "abc");
        assert_eq!(percent_decode("a%20b"), "a b");
        // Malformed escape (not two hex digits) is left verbatim.
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%zz"), "a%zz");
        // Trailing percent with no digits.
        assert_eq!(percent_decode("a%"), "a%");
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> ScreenRect {
        ScreenRect {
            x,
            y,
            w,
            h,
            display: 0,
        }
    }

    #[test]
    fn clamp_region_within_bounds() {
        assert_eq!(
            clamp_region(rect(10.0, 20.0, 100.0, 50.0), 1920, 1080),
            Some((10, 20, 100, 50))
        );
    }

    #[test]
    fn clamp_region_clips_to_monitor() {
        // Overhangs the right/bottom edges: width/height are trimmed.
        assert_eq!(
            clamp_region(rect(1900.0, 1060.0, 100.0, 100.0), 1920, 1080),
            Some((1900, 1060, 20, 20))
        );
        // Negative origin (off the top-left) is clipped back to zero.
        assert_eq!(
            clamp_region(rect(-30.0, -10.0, 100.0, 60.0), 1920, 1080),
            Some((0, 0, 70, 50))
        );
    }

    #[test]
    fn clamp_region_rejects_empty() {
        // Zero area.
        assert_eq!(clamp_region(rect(0.0, 0.0, 0.0, 100.0), 1920, 1080), None);
        // Fully off-screen to the right.
        assert_eq!(
            clamp_region(rect(3000.0, 0.0, 100.0, 100.0), 1920, 1080),
            None
        );
        // Sub-pixel sliver.
        assert_eq!(clamp_region(rect(0.0, 0.0, 0.4, 0.4), 1920, 1080), None);
    }

    // Integration tests.
    // Need the Accessibility permissions granted to the test runner.
    //
    // Assert only that the calls return without panicking,
    // since the actual text depends on what is focused when they run.

    #[test]
    #[ignore = "requires Accessibility permission and a focused app"]
    fn live_capture_provenance() {
        let cap = MacosCapturer::new();
        let prov = cap.capture_provenance().expect("provenance is best-effort");
        println!(
            "app={:?} window={:?} path={:?}",
            prov.app_name, prov.window_title, prov.path
        );
    }

    #[test]
    #[ignore = "requires Accessibility permission; select text before running"]
    fn live_capture_text() {
        let cap = MacosCapturer::new();
        match cap.capture_text() {
            Ok(t) => println!("captured {:?} via {:?}", t.text, t.method),
            Err(e) => println!("no text captured: {e}"),
        }
    }

    #[test]
    #[ignore = "requires Screen Recording permission"]
    fn live_capture_region() {
        let display = Monitor::all()
            .expect("monitors")
            .into_iter()
            .find_map(|m| m.is_primary().ok().filter(|&p| p).and(m.id().ok()))
            .expect("a primary monitor");
        let cap = MacosCapturer::new();
        let region = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 200.0,
            display,
        };
        let img = cap.capture_image(region).expect("capture");
        assert!(!img.png.is_empty());
        assert_eq!(&img.png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    #[ignore = "requires Accessibility and Screen Recording permission"]
    fn live_capture_context() {
        let cap = MacosCapturer::new();
        match cap
            .begin_context_capture()
            .and_then(PendingContextCapture::complete)
        {
            Ok(context) => println!(
                "context image {} bytes, AX text: {}",
                context.image.png.len(),
                context.accessibility_text.as_deref().unwrap_or("<none>")
            ),
            Err(e) => println!("no context: {e}"),
        }
    }
}
