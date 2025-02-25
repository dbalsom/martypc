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

use std::{ffi::OsString, io::Cursor, path::Path, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use std::thread::spawn;

#[cfg(target_arch = "wasm32")]
use crate::app::FileOpenContext;
#[cfg(target_arch = "wasm32")]
use crate::wasm::{file_open::open_file, worker::spawn};

use crate::{
    emulator::Emulator,
    event_loop::{egui_events::FileSelectionContext, thread_events::FrontendThreadEvent},
};
use fluxfox::{DiskImage, LoadingStatus};
use marty_core::device_types::fdc::FloppyImageType;
use marty_egui::state::FloppyDriveSelection;
use marty_frontend_common::{
    constants::NORMAL_NOTIFICATION_TIME,
    floppy_manager::FloppyError,
    types::floppy::FloppyImageSource,
};

/// Load a floppy image into the emulator, given a file selection context which will either
/// reference a path or the index of the image in the floppy manager (for quick-access menu).
pub fn handle_load_floppy(emu: &mut Emulator, drive_select: usize, context: FileSelectionContext) {
    if let Some(fdc) = emu.machine.fdc() {
        let mut floppy_result: Option<Result<FloppyImageSource, FloppyError>> = None;
        let mut floppy_name = None;
        match context.clone() {
            FileSelectionContext::Index(item_idx) => {
                let name = emu.floppy_manager.get_floppy_name(item_idx);

                if let Some(name) = name {
                    floppy_name = Some(name.clone());
                    log::info!(
                        "Loading floppy image by index: {}->{:?} into drive: {}",
                        item_idx,
                        name,
                        drive_select
                    );

                    let floppy_path = match emu.floppy_manager.get_floppy_path(item_idx) {
                        Some(path) => path,
                        None => {
                            log::error!("Failed to resolve index to floppy path");
                            emu.gui
                                .toasts()
                                .error("Failed to resolve index to floppy path".to_string())
                                .duration(Some(NORMAL_NOTIFICATION_TIME));
                            return;
                        }
                    };

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        // On native target, we can use blocking native file io.
                        floppy_result = Some(emu.floppy_manager.load_floppy_by_path(floppy_path, &mut emu.rm));
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        // On web, we must use our open_file utility to spawn a file open dialog, and
                        // user the provided sender to send the result back to the main thread once
                        // fetched.

                        let new_fsc_context = FileSelectionContext::Path(floppy_path.clone());
                        let new_context = FileOpenContext::FloppyDiskImage {
                            drive_select,
                            fsc: new_fsc_context,
                        };
                        open_file(new_context, emu.sender.clone());
                        return;
                    }
                };
            }
            FileSelectionContext::Path(path) => {
                if let Some(file_name) = path.file_name() {
                    floppy_name = Some(file_name.to_os_string());
                }

                log::info!("Loading floppy image by path: {:?} into drive: {}", path, drive_select);
                //floppy_result = Some(emu.floppy_manager.load_floppy_by_path(path, &emu.rm).await);
            }
        }

        if let Some(floppy_result) = floppy_result {
            match floppy_result {
                Ok(FloppyImageSource::ZipArchive(zip_vec, path)) => {
                    let mut image_type = None;
                    image_type = Some(fdc.drive(drive_select).get_largest_supported_image_format());
                    match emu.floppy_manager.build_autofloppy_image_from_zip(
                        zip_vec,
                        Some(FloppyImageType::Image360K),
                        &mut emu.rm,
                    ) {
                        Ok(vec) => {
                            if let Some(fdc) = emu.machine.fdc() {
                                match fdc.load_image_from(drive_select, vec, None, true) {
                                    Ok(image_lock) => {
                                        log::info!("Floppy image successfully loaded into virtual drive.");

                                        let image = image_lock.read().unwrap();
                                        let compat_formats = image.compatible_formats(true);

                                        let name = floppy_name.unwrap_or_else(|| OsString::from("Unknown"));

                                        emu.gui.set_floppy_selection(
                                            drive_select,
                                            None,
                                            FloppyDriveSelection::ZipArchive(name.into()),
                                            image.source_format(),
                                            compat_formats,
                                            None,
                                        );

                                        emu.gui.set_floppy_write_protected(drive_select, true);

                                        emu.gui
                                            .toasts()
                                            .info("Directory successfully mounted!".to_string())
                                            .duration(Some(NORMAL_NOTIFICATION_TIME));
                                    }
                                    Err(err) => {
                                        log::warn!("Floppy image failed to load: {}", err);
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("Failed to build autofloppy image. Error: {}", err);
                            emu.gui
                                .toasts()
                                .error(format!("Directory mount failed: {}", err))
                                .duration(Some(NORMAL_NOTIFICATION_TIME));
                        }
                    }
                }
                Ok(FloppyImageSource::KryoFluxSet(floppy_image, floppy_path))
                | Ok(FloppyImageSource::DiskImage(floppy_image, floppy_path)) => {
                    let sender = emu.sender.clone();
                    spawn(move || {
                        let mut image_buffer = Cursor::new(floppy_image);
                        let inner_sender = sender.clone();
                        let loading_callback = Arc::new(Box::new(move |status| match status {
                            LoadingStatus::Progress(progress) => {
                                _ = inner_sender.send(FrontendThreadEvent::FloppyImageLoadProgress(
                                    "Loading floppy image...".to_string(),
                                    progress,
                                ));
                            }
                            LoadingStatus::ProgressSupport => {
                                _ = inner_sender.send(FrontendThreadEvent::FloppyImageBeginLongLoad);
                            }
                            _ => {}
                        }));

                        match DiskImage::load(&mut image_buffer, Some(&floppy_path), None, Some(loading_callback)) {
                            Ok(disk_image) => {
                                _ = sender.send(FrontendThreadEvent::FloppyImageLoadComplete {
                                    drive_select,
                                    image: disk_image,
                                    item: context,
                                    path: Some(floppy_path),
                                });
                            }
                            Err(err) => {
                                _ = sender.send(FrontendThreadEvent::FloppyImageLoadError(err.to_string()));
                            }
                        }
                    });
                }
                Err(e) => {
                    log::error!("Failed to load floppy image: {}", e);
                    emu.gui
                        .toasts()
                        .error(format!("Failed to load floppy image: {}", e))
                        .duration(Some(NORMAL_NOTIFICATION_TIME));
                }
            }
        }
        else {
            log::error!("Failed to load floppy image: No result returned.");
            emu.gui
                .toasts()
                .error("Failed to load floppy image: No result returned.")
                .duration(Some(NORMAL_NOTIFICATION_TIME));
        }
    }
}

/// Load a floppy image asynchronously, sending the result back to the frontend thread as a
/// `FloppyImageLoadComplete` event.
pub fn load_floppy_image(
    emu: &mut Emulator,
    drive_select: usize,
    context: FileSelectionContext,
    image_buffer: Vec<u8>,
    image_path: Option<&Path>,
) {
    let inner_sender = emu.sender.clone();
    let inner_progress_sender = emu.sender.clone();
    let inner_path = image_path.map(|p| p.to_path_buf());
    spawn(move || {
        log::debug!("In load_floppy_image worker...");
        let mut image_buffer = Cursor::new(image_buffer);
        let loading_callback = Arc::new(Box::new(move |status| match status {
            LoadingStatus::Progress(progress) => {
                _ = inner_progress_sender.send(FrontendThreadEvent::FloppyImageLoadProgress(
                    "Loading floppy image...".to_string(),
                    progress,
                ));
            }
            LoadingStatus::ProgressSupport => {
                _ = inner_progress_sender.send(FrontendThreadEvent::FloppyImageBeginLongLoad);
            }
            _ => {}
        }));

        match DiskImage::load(&mut image_buffer, inner_path.as_deref(), None, Some(loading_callback)) {
            Ok(disk_image) => {
                _ = inner_sender.send(FrontendThreadEvent::FloppyImageLoadComplete {
                    drive_select,
                    image: disk_image,
                    item: context,
                    path: inner_path,
                });
            }
            Err(err) => {
                _ = inner_sender.send(FrontendThreadEvent::FloppyImageLoadError(err.to_string()));
            }
        }
    });
}
