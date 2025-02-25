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

    event_loop/thread_events.rs

    Handle events received from background threads spawned by the frontend.
*/
use crate::{
    emulator::Emulator,
    event_loop::{egui_events::FileSelectionContext, thread_events::FrontendThreadEvent::FloppyImageLoadProgress},
};
use fluxfox::DiskImage;
use marty_egui::{modal::ModalContext, state::FloppyDriveSelection};
use marty_frontend_common::constants::NORMAL_NOTIFICATION_TIME;
use std::path::PathBuf;

pub enum FrontendThreadEvent {
    FloppyImageLoadError(String),
    FloppyImageBeginLongLoad,
    FloppyImageLoadProgress(String, f64),
    FloppyImageLoadComplete {
        drive_select: usize,
        item: FileSelectionContext,
        image: DiskImage,
        path: Option<PathBuf>,
    },
    FloppyImageSaveError(String),
    FloppyImageSaveComplete(PathBuf),
}

pub fn handle_thread_event(emu: &mut Emulator) {
    while let Ok(event) = emu.receiver.try_recv() {
        match event {
            FrontendThreadEvent::FloppyImageLoadError(err) => {
                log::error!("Failed to load floppy image! Error: {}", err);
                emu.gui
                    .toasts()
                    .error(format!("Floppy load failed: {}", err))
                    .set_duration(Some(NORMAL_NOTIFICATION_TIME));

                emu.gui.modal.close();
            }
            FrontendThreadEvent::FloppyImageBeginLongLoad => {
                emu.gui
                    .modal
                    .open(ModalContext::ProgressBar("Loading floppy image...".into(), 0.0), None);
            }
            FrontendThreadEvent::FloppyImageLoadProgress(title, progress) => {
                emu.gui
                    .modal
                    .open(ModalContext::ProgressBar(title.into(), progress as f32), None);
            }
            FrontendThreadEvent::FloppyImageLoadComplete {
                drive_select,
                item,
                image,
                path,
            } => {
                if let Some(fdc) = emu.machine.fdc() {
                    match fdc.attach_image(
                        drive_select,
                        image,
                        path.clone(),
                        emu.config.emulator.media.write_protect_default,
                    ) {
                        Ok(image) => {
                            let item_idx = if let FileSelectionContext::Index(idx) = item {
                                Some(idx)
                            }
                            else {
                                None
                            };

                            log::info!("Floppy image successfully loaded into virtual drive.");
                            emu.gui.set_floppy_selection(
                                drive_select,
                                item_idx,
                                FloppyDriveSelection::Image(path.clone().unwrap_or_default().into()),
                                image.source_format(),
                                image.compatible_formats(true),
                                Some(emu.config.emulator.media.write_protect_default),
                            );

                            emu.gui
                                .toasts()
                                .info(format!("Floppy loaded: {:?}", path.clone()))
                                .set_duration(Some(NORMAL_NOTIFICATION_TIME));

                            emu.gui.modal.close();
                        }
                        Err(err) => {
                            log::warn!("Floppy image failed to load: {}", err);
                        }
                    }
                }
            }
            FrontendThreadEvent::FloppyImageSaveError(err) => {
                log::error!("Floppy image save error: {}", err);
            }
            FrontendThreadEvent::FloppyImageSaveComplete(path) => {
                emu.gui.modal.close();
                log::info!("Floppy image saved: {:?}", path);
            }
        }
    }
}
