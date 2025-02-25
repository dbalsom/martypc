/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------
*/
#![warn(clippy::all, rust_2018_idioms)]
// hide console window on Windows in release, unless devmode feature is enabled
#![cfg_attr(all(not(debug_assertions), not(feature = "devmode")), windows_subsystem = "windows")]

use martypc_eframe::{app::MartyApp, MARTY_ICON};

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
#[async_std::main]
async fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    // Set up the default window size and icon

    // We should probably split up the 'startup' function.
    // If we create an EmulatorBuilder, we can have a pre_init and post_init function,
    // the latter can be called after the eframe instance is created, to instantiate the
    // DisplayManager.
    let mut native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&MARTY_ICON[..]).expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    let app = MartyApp::new(&mut native_options).await;

    eframe::run_native("MartyPC", native_options, Box::new(|cc| Ok(Box::new(app.init(cc)))))
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

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    use wasm_bindgen_futures::spawn_local;

    // Redirect `log` messages to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    // Closure to start the application after user interaction
    let start_application = || {
        spawn_local(async {
            let document = web_sys::window().expect("No window").document().expect("No document");

            // Hide the "Click to start" screen
            if let Some(start_screen) = document.get_element_by_id("start_screen") {
                start_screen.set_attribute("style", "display:none;").unwrap();
            }

            // Show the loading spinner
            if let Some(loading_text) = document.get_element_by_id("loading_text") {
                loading_text.set_attribute("style", "display:block;").unwrap();
            }

            // Locate and show the canvas
            let canvas = document
                .get_element_by_id("the_canvas_id")
                .expect("Failed to find the_canvas_id")
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("the_canvas_id was not a HtmlCanvasElement");

            canvas.set_attribute("style", "display:block;").unwrap();

            // Initialize the app
            let web_options = eframe::WebOptions::default();
            let app = MartyApp::new().await;
            log::debug!("App created, emu is Some?: {}", app.emu.is_some());

            if app.emu.is_none() {
                log::error!("Failed to create emulator, exiting.");
                if let Some(loading_text) = document.get_element_by_id("loading_text") {
                    loading_text.set_inner_html(
                        "<p> MartyPC failed to initialize. See the developer console for details (Hit f12). </p>",
                    );
                }
                panic!("Failed to create emulator");
            }

            let start_result = eframe::WebRunner::new()
                .start(canvas, web_options, Box::new(move |cc| Ok(Box::new(app.init(cc)))))
                .await;

            // Remove the loading text and spinner
            if let Some(loading_text) = document.get_element_by_id("loading_text") {
                match start_result {
                    Ok(_) => {
                        loading_text.remove();
                    }
                    Err(e) => {
                        loading_text
                            .set_inner_html("<p> The app has crashed. See the developer console for details. </p>");
                        panic!("Failed to start eframe: {e:?}");
                    }
                }
            }
        });
    };

    // Wait for user interaction
    let document = web_sys::window().expect("No window").document().expect("No document");

    let start_logo = document
        .get_element_by_id("start_logo")
        .expect("Failed to find start_logo");

    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
        start_application(); // Start the application after user interaction
    }) as Box<dyn FnMut(_)>);

    start_logo
        .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
        .expect("Failed to add event listener");
    closure.forget(); // Prevent the closure from being dropped
}
