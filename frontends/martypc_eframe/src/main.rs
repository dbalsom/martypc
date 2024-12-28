#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::{AppCreator, NativeOptions, Renderer};
#[cfg(not(target_arch = "wasm32"))]
use martypc_eframe::native::startup;
use martypc_eframe::{app::MartyApp, MARTY_ICON};
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::ControlFlow,
    keyboard::KeyCode,
    platform::windows::EventLoopBuilderExtWindows,
};

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    // Set up the default window size and icon

    // We should probably split up the 'startup' function.
    // If we create an EmulatorBuilder, we can have a pre_init and post_init function,
    // the latter can be called after the eframe instance is created, to instantiate the
    // DisplayManager.
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&MARTY_ICON[..]).expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "MartyPC",
        native_options,
        Box::new(|cc| Ok(Box::new(MartyApp::new(cc)))),
    )
}

// #[cfg(not(target_arch = "wasm32"))]
// #[cfg(any(feature = "glow", feature = "wgpu"))]
// #[allow(clippy::needless_pass_by_value)]
// pub fn run_custom(
//     app_name: &str,
//     mut native_options: NativeOptions,
//     app_creator: AppCreator<'_>,
// ) -> eframe::Result {
//     #[cfg(not(feature = "__screenshot"))]
//     assert!(
//         std::env::var("EFRAME_SCREENSHOT_TO").is_err(),
//         "EFRAME_SCREENSHOT_TO found without compiling with the '__screenshot' feature"
//     );
//
//     if native_options.viewport.title.is_none() {
//         native_options.viewport.title = Some(app_name.to_owned());
//     }
//
//     let renderer = native_options.renderer;
//
//     #[cfg(all(feature = "glow", feature = "wgpu"))]
//     {
//         match renderer {
//             Renderer::Glow => "glow",
//             Renderer::Wgpu => "wgpu",
//         };
//         log::info!("Both the glow and wgpu renderers are available. Using {renderer}.");
//     }
//
//     use eframe::native::run;
//     match renderer {
//         #[cfg(feature = "glow")]
//         Renderer::Glow => {
//             log::debug!("Using the glow renderer");
//             native::run::run_glow(app_name, native_options, app_creator)
//         }
//
//         #[cfg(feature = "wgpu")]
//         Renderer::Wgpu => {
//             log::debug!("Using the wgpu renderer");
//             native::run::run_wgpu(app_name, native_options, app_creator)
//         }
//     }
// }

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().expect("No window").document().expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(|cc| Ok(Box::new(MartyApp::new(cc)))))
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html("<p> The app has crashed. See the developer console for details. </p>");
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
