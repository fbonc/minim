use iced::{Task, Theme, window as iced_window};
use ruru_core::{Output as CoreOutput, Sender as CoreSender};
use ruru_ui::Overlay as OverlayUi;
use ruru_ui::main_window::{self, MainWindow as MainWindowUi};
use ruru_ui::overlay;

#[cfg(target_os = "macos")]
mod reopen;
mod update;
mod view;
mod windows;

const WINDOW_WIDTH: f32 = 500.0;
const MIN_WINDOW_WIDTH: f32 = 225.0;
const PROMPTING_HEIGHT: f32 = 180.0;
const MIN_WINDOW_HEIGHT: f32 = PROMPTING_HEIGHT;
const ANSWERING_HEIGHT: f32 = 360.0;

struct App {
    main_window: WindowHost<MainWindowUi>,
    overlay: WindowHost<OverlayUi>,
    to_core: CoreSender,
    current_provider_request_id: ruru_core::ProviderRequestId,
    selecting_region: bool,
}

struct WindowHost<T> {
    id: iced_window::Id,
    ui: T,
}

#[derive(Debug, Clone)]
enum Input {
    WindowOpened(iced_window::Id),
    WindowCloseRequested(iced_window::Id),
    #[cfg(target_os = "macos")]
    ShowMainWindow,
    DragWindow,
    Core(CoreOutput),
    MainWindow(main_window::Input),
    Overlay(overlay::Input),
    CheckPromptInputFocus(iced_window::Id),
    BeginRegionSelection,
}

pub(super) fn run() -> iced::Result {
    iced::daemon(boot, update::update, view::view)
        .subscription(view::subscription)
        .theme(Theme::Dark)
        .title("ruru")
        .style(|_state: &App, _theme: &iced::Theme| iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: iced::Color::from_rgb8(0xED, 0xEB, 0xE6),
        })
        .run()
}

fn boot() -> (App, Task<Input>) {
    let (to_core, core_outputs, settings) = ruru_core::start();
    let provider_views = settings
        .providers
        .iter()
        .map(|provider| {
            main_window::ProviderView::new(
                provider.id.clone(),
                provider.name,
                provider.has_key,
                provider.models.clone(),
            )
        })
        .collect();
    let mut main_ui = MainWindowUi::new().with_providers(provider_views);
    if let Some(error) = settings.load_error {
        main_ui.settings_error(format!("Cannot edit settings: {error}"));
    }
    let overlay_ui = OverlayUi::new()
        .with_selected_model(settings.selected_model)
        .with_models(settings.available_models);
    let (main, open_main) = iced_window::open(windows::main_settings());
    let (overlay, open_overlay) = iced_window::open(windows::overlay_settings());

    let tasks = vec![
        open_main.map(Input::WindowOpened),
        open_overlay.map(Input::WindowOpened),
        Task::run(core_outputs, Input::Core),
    ];

    (
        App {
            main_window: WindowHost {
                id: main,
                ui: main_ui,
            },
            overlay: WindowHost {
                id: overlay,
                ui: overlay_ui,
            },
            to_core,
            current_provider_request_id: 0,
            selecting_region: false,
        },
        Task::batch(tasks),
    )
}
