/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
*/

use std::path::Path;

use display_manager_eframe::{DisplayManager, DisplayManagerEvent, EFrameDisplayManager};
use marty_egui::FileDialogFilter;
use marty_frontend_common::{constants::LONG_NOTIFICATION_TIME, thread_events::FileSaveContext};
use marty_videocard_renderer::RendererEvent;
use web_time::Duration;

use crate::emulator::Emulator;

#[cfg(not(target_arch = "wasm32"))]
fn save_png(path: &Path, png_data: &[u8]) -> Result<(), String> {
    std::fs::write(path, png_data).map_err(|err| err.to_string())
}

#[cfg(target_arch = "wasm32")]
fn save_png(path: &Path, png_data: &[u8]) -> Result<(), String> {
    crate::wasm::file_save::save_file_dialog(&path.to_string_lossy(), png_data).map_err(|err| err.to_string())
}

fn open_save_dialog(emu: &mut Emulator, suggested_filename: String, png_data: Vec<u8>, shader_output: bool) {
    let initial_directory = emu.rm.resource_path("screenshot");
    let title = if shader_output {
        "Save Shader-output Screenshot"
    }
    else {
        "Save Screenshot"
    };
    emu.gui.save_file_dialog(
        FileSaveContext::screenshot(suggested_filename, png_data),
        title,
        vec![FileDialogFilter::new("PNG Images", vec!["png"])],
        initial_directory.as_deref(),
    );
}

pub(crate) fn process_events(emu: &mut Emulator, display_manager: &mut EFrameDisplayManager) {
    while let Some(event) = display_manager.get_event() {
        match event {
            DisplayManagerEvent::ShaderScreenshotReady { path, png_data } => match save_png(&path, &png_data) {
                Ok(()) => {
                    log::info!("Saved shader-output screenshot: {}", path.display());
                    emu.gui
                        .toasts()
                        .info(format!("Screenshot saved!\n{}", path.display()))
                        .duration(Some(Duration::from_secs(5)));
                }
                Err(err) => {
                    log::error!("Failed to save shader-output screenshot {}: {}", path.display(), err);
                    emu.gui
                        .toasts()
                        .error(format!("Failed to save shader-output screenshot: {err}"))
                        .duration(Some(LONG_NOTIFICATION_TIME));
                }
            },
            DisplayManagerEvent::ShaderScreenshotCaptured {
                suggested_filename,
                png_data,
            } => {
                open_save_dialog(emu, suggested_filename, png_data, true);
            }
            DisplayManagerEvent::ShaderScreenshotFailed { target, error } => {
                log::error!("Failed to capture shader-output screenshot {}: {}", target, error);
                emu.gui.toasts().error(error).duration(Some(LONG_NOTIFICATION_TIME));
            }
        }
    }

    display_manager.for_each_renderer(|renderer, _vid, _backend_buf| {
        while let Some(event) = renderer.get_event() {
            match event {
                RendererEvent::ScreenshotSaved { path } => {
                    emu.gui
                        .toasts()
                        .info(format!("Screenshot saved!\n{}", path.display()))
                        .duration(Some(Duration::from_secs(5)));
                }
                RendererEvent::ScreenshotCaptured {
                    suggested_filename,
                    png_data,
                } => {
                    open_save_dialog(emu, suggested_filename, png_data, false);
                }
                RendererEvent::ScreenshotFailed(error) => {
                    log::error!("Failed to capture screenshot: {error}");
                    emu.gui.toasts().error(error).duration(Some(LONG_NOTIFICATION_TIME));
                }
            }
        }
    });
}
