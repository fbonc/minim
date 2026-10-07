use iced::{Task, Theme, window as iced_window};
use ruru_core::{Output as CoreOutput, Sender as CoreSender};
use ruru_provider::ProviderRegistry;
use ruru_ui::Overlay as OverlayUi;
use ruru_ui::main_window::{self, MainWindow as MainWindowUi};
use ruru_ui::overlay;

mod providers;
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
    providers: ProviderRegistry,
    to_core: CoreSender,
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
            text_color: iced::Color::BLACK,
        })
        .run()
}

fn boot() -> (App, Task<Input>) {
    let (providers, selected_model) = providers::load_providers();
    let (to_core, core_outputs) = ruru_core::start(providers.clone());
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
                ui: MainWindowUi::new(),
            },
            overlay: WindowHost {
                id: overlay,
                ui: OverlayUi::new().with_selected_model(selected_model),
            },
            providers,
            to_core,
            selecting_region: false,
        },
        Task::batch(tasks),
    )
}
