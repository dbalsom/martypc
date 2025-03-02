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

use crate::{emulator::Emulator, floppy::load_floppy::load_floppy_image};
use std::{path::PathBuf, sync::Arc};

use fluxfox::DiskImage;
use marty_egui::{modal::ModalContext, state::FloppyDriveSelection};
use marty_frontend_common::{
    constants::{LONG_NOTIFICATION_TIME, NORMAL_NOTIFICATION_TIME},
    thread_events::{FileOpenContext, FileSaveContext, FileSelectionContext, FrontendThreadEvent},
};

pub fn handle_thread_event(emu: &mut Emulator) {
    while let Ok(event) = emu.receiver.try_recv() {
        match event {
            FrontendThreadEvent::FileDialogCancelled => {
                emu.gui.modal.close();
            }
            FrontendThreadEvent::FileOpenError(context, error) => {
                log::error!("File open error: {}", error);
                emu.gui
                    .toasts()
                    .error(format!("File open error: {}", error))
                    .duration(Some(LONG_NOTIFICATION_TIME));
                emu.gui.modal.close();
            }
            FrontendThreadEvent::FileSaveError(error) => {
                log::error!("File save error: {}", error);
                emu.gui
                    .toasts()
                    .error(format!("File save error: {}", error))
                    .duration(Some(LONG_NOTIFICATION_TIME));
                emu.gui.modal.close();
            }
            FrontendThreadEvent::FileOpenDialogComplete {
                context,
                path,
                contents,
            } => {
                emu.gui
                    .toasts()
                    .info(format!(
                        "File opened: {:?} ({}) bytes",
                        path.clone().unwrap_or(PathBuf::from("None")),
                        contents.len()
                    ))
                    .duration(Some(NORMAL_NOTIFICATION_TIME));

                match context {
                    FileOpenContext::FloppyDiskImage { drive_select, fsc } => {
                        let mut floppy_path = None;

                        if let FileSelectionContext::Path(path) = &fsc {
                            floppy_path = Some(path.clone());
                        }

                        emu.gui
                            .toasts()
                            .info("Loading disk image...")
                            .duration(Some(NORMAL_NOTIFICATION_TIME));

                        load_floppy_image(emu, drive_select, fsc, contents, floppy_path.as_deref());
                    }
                    FileOpenContext::CartridgeImage { .. } => {}
                }
            }
            FrontendThreadEvent::FileSaveDialogComplete(save_context) => {
                let (drive_select, format, fsc) = match save_context {
                    FileSaveContext::FloppyDiskImage {
                        drive_select,
                        format,
                        fsc,
                    } => (drive_select, format, fsc),
                };

                let path_buf = if let FileSelectionContext::Path(path) = fsc {
                    path
                }
                else {
                    log::error!("Failed to get file path from FileSaveDialogComplete event");
                    emu.gui
                        .toasts()
                        .error("Failed to get file path!".to_string())
                        .duration(Some(LONG_NOTIFICATION_TIME));
                    return;
                };

                if let Some(fdc) = emu.machine.fdc() {
                    let (disk_image_opt, _) = fdc.get_image(drive_select);
                    if let Some(floppy_image) = disk_image_opt {
                        let mut image = floppy_image.write().unwrap();
                        match fluxfox::ImageWriter::new(&mut image)
                            .with_format(format)
                            .with_path(path_buf.clone())
                            .write()
                        {
                            Ok(_) => {
                                log::info!("Floppy image successfully saved: {:?}", path_buf);

                                // emu.gui.set_floppy_selection(
                                //     *drive_select,
                                //     None,
                                //     FloppyDriveSelection::Image(path_buf.clone()),
                                //     Some(*format),
                                //     image.compatible_formats(true),
                                //     None,
                                // );

                                emu.gui
                                    .toasts()
                                    .info(format!("Floppy saved: {:?}", path_buf.file_name().unwrap_or_default()))
                                    .duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                            Err(err) => {
                                log::error!("Floppy image failed to save: {}", err);

                                emu.gui
                                    .toasts()
                                    .error(format!("Failed to save: {}", err))
                                    .duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                        }
                    }
                }
                emu.gui.modal.close();
            }
            FrontendThreadEvent::FloppyImageLoadError(err) => {
                log::error!("Failed to load floppy image! Error: {}", err);
                emu.gui
                    .toasts()
                    .error(format!("Floppy load failed: {}", err))
                    .duration(Some(LONG_NOTIFICATION_TIME));

                emu.gui.modal.close();
            }
            FrontendThreadEvent::FloppyImageBeginLongLoad => {
                emu.gui
                    .modal
                    .open(ModalContext::ProgressBar("Loading floppy image...".into(), 0.0));
            }
            FrontendThreadEvent::FloppyImageLoadProgress(title, progress) => {
                emu.gui
                    .modal
                    .open(ModalContext::ProgressBar(title.into(), progress as f32));
            }
            FrontendThreadEvent::FloppyImageLoadComplete {
                drive_select,
                item,
                image,
                path,
            } => {
                // emu.gui
                //     .toasts()
                //     .info("Got FloppyImageLoadComplete event")
                //     .duration(Some(NORMAL_NOTIFICATION_TIME));

                if let Some(fdc) = emu.machine.fdc() {
                    match fdc.attach_image(
                        drive_select,
                        Arc::<DiskImage>::into_inner(image).unwrap(),
                        path.clone(),
                        emu.config.emulator.media.write_protect_default,
                    ) {
                        Ok(image_lock) => {
                            let item_idx = if let FileSelectionContext::Index(idx) = item {
                                Some(idx)
                            }
                            else {
                                None
                            };

                            log::info!("Floppy image successfully loaded into virtual drive.");
                            emu.gui.floppy_viewer.set_disk(drive_select, image_lock.clone());
                            let image = image_lock.read().unwrap();
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
                                .info(format!(
                                    "Floppy loaded: {}",
                                    path.clone().unwrap_or(PathBuf::from("None")).display()
                                ))
                                .duration(Some(NORMAL_NOTIFICATION_TIME));

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
