use std::sync::Arc;
use std::thread;

use futures_channel::mpsc;
use futures_util::Stream;

use crate::Output;
use crate::core::Core;

const HOTKEY_ACCELERATOR: &str = "Cmd+Shift+KeyE";

pub(crate) fn outputs(core: Arc<Core>) -> impl Stream<Item = Output> {
    let (outputs, receiver) = mpsc::unbounded();

    match ruru_platform::new_hotkey(HOTKEY_ACCELERATOR) {
        Ok(hotkey) => {
            thread::spawn(move || {
                loop {
                    match hotkey.recv() {
                        Ok(()) => {
                            let mut overlay_requested = true;
                            let result = core.capture(|focused_window, target| {
                                overlay_requested = outputs
                                    .unbounded_send(Output::ShowRequested { focused_window })
                                    .is_ok();

                                if overlay_requested && let Some(target) = target {
                                    overlay_requested = outputs
                                        .unbounded_send(Output::TargetCaptured(target))
                                        .is_ok();
                                }
                            });

                            if !overlay_requested {
                                break;
                            }

                            if let Some(error) = result.failure {
                                let _ = outputs.unbounded_send(Output::RequestFailed(error));
                            }

                            if outputs.unbounded_send(Output::CaptureCompleted).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = outputs.unbounded_send(Output::RequestFailed(format!(
                                "failed to receive hotkey: {error:?}"
                            )));
                            break;
                        }
                    };
                }
            });
        }
        Err(error) => {
            let _ = outputs.unbounded_send(Output::RequestFailed(format!(
                "failed to register {HOTKEY_ACCELERATOR}: {error:?}"
            )));
        }
    }

    receiver
}
