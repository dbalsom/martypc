/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

use std::{io::Cursor, path::Path, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use std::thread::spawn;

#[cfg(target_arch = "wasm32")]
use crate::wasm::{file_open::open_file, worker::spawn};
#[cfg(target_arch = "wasm32")]
use marty_frontend_common::thread_events::FileOpenContext;

use crate::emulator::Emulator;
use fluxfox::{
    file_system::FileSystemType,
    types::TrackDataResolution,
    DiskImage,
    ImageBuilder,
    LoadingStatus,
    StandardFormat,
};
use marty_frontend_common::{
    constants::NORMAL_NOTIFICATION_TIME,
    floppy_manager::FloppyError,
    thread_events::{FileSelectionContext, FloppyImageLoadSource, FrontendThreadEvent},
    types::floppy::FloppyImageSource,
};

/// Load a floppy image into the emulator, given a file selection context which will either
/// reference a path or the index of the image in the floppy manager (for quick-access menu).
pub fn handle_load_floppy(emu: &mut Emulator, drive_select: usize, context: FileSelectionContext) {
    if emu.machine.fdc().is_some() {
        let mut floppy_result: Option<Result<FloppyImageSource, FloppyError>> = None;
        match context.clone() {
            FileSelectionContext::Index(item_idx) => {
                let name = emu.floppy_manager.get_floppy_name(item_idx);

                if let Some(name) = name {
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
                log::info!("Loading floppy image by path: {:?} into drive: {}", path, drive_select);
                //floppy_result = Some(emu.floppy_manager.load_floppy_by_path(path, &emu.rm).await);
            }
            _ => {
                log::warn!("Invalid file selection context for floppy image load.");
            }
        }

        if let Some(floppy_result) = floppy_result {
            match floppy_result {
                Ok(FloppyImageSource::ZipArchive(zip_vec, floppy_path)) => {
                    load_zip_archive(emu, drive_select, context, zip_vec, floppy_path);
                }
                Ok(FloppyImageSource::KryoFluxSet(floppy_image, floppy_path))
                | Ok(FloppyImageSource::DiskImage(floppy_image, floppy_path)) => {
                    load_disk_image(emu, drive_select, context, floppy_image, floppy_path);
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
    let Some(image_path) = image_path
    else {
        let error = "Floppy image has no filename";
        log::error!("{}", error);
        emu.gui.toasts().error(error).duration(Some(NORMAL_NOTIFICATION_TIME));
        return;
    };

    match emu
        .floppy_manager
        .load_floppy_from_bytes(image_buffer, image_path.to_path_buf())
    {
        Ok(FloppyImageSource::ZipArchive(zip_vec, floppy_path)) => {
            load_zip_archive(emu, drive_select, context, zip_vec, floppy_path);
        }
        Ok(FloppyImageSource::KryoFluxSet(floppy_image, floppy_path))
        | Ok(FloppyImageSource::DiskImage(floppy_image, floppy_path)) => {
            load_disk_image(emu, drive_select, context, floppy_image, floppy_path);
        }
        Err(error) => {
            log::error!("Failed to classify floppy image: {}", error);
            emu.gui
                .toasts()
                .error(format!("Failed to load floppy image: {}", error))
                .duration(Some(NORMAL_NOTIFICATION_TIME));
        }
    }
}

fn load_disk_image(
    emu: &Emulator,
    drive_select: usize,
    context: FileSelectionContext,
    floppy_image: Vec<u8>,
    floppy_path: std::path::PathBuf,
) {
    let inner_sender = emu.sender.clone();
    let inner_progress_sender = emu.sender.clone();
    spawn(move || {
        log::debug!("In load_floppy_image worker...");
        let mut image_buffer = Cursor::new(floppy_image);
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

        match DiskImage::load(&mut image_buffer, Some(&floppy_path), None, Some(loading_callback)) {
            Ok(disk_image) => {
                _ = inner_sender.send(FrontendThreadEvent::FloppyImageLoadComplete {
                    drive_select,
                    image: Arc::new(disk_image),
                    item: context,
                    path: Some(floppy_path),
                    source: FloppyImageLoadSource::DiskImage,
                });
            }
            Err(err) => {
                _ = inner_sender.send(FrontendThreadEvent::FloppyImageLoadError(err.to_string()));
            }
        }
    });
}

fn load_zip_archive(
    emu: &Emulator,
    drive_select: usize,
    context: FileSelectionContext,
    archive: Vec<u8>,
    archive_path: std::path::PathBuf,
) {
    let sender = emu.sender.clone();
    spawn(move || {
        log::debug!("Building floppy image from ZIP archive...");
        let image_result = ImageBuilder::new()
            .with_resolution(TrackDataResolution::BitStream)
            .with_standard_format(StandardFormat::PcFloppy360)
            .with_filesystem_from_archive(&archive, FileSystemType::Fat12, true, false)
            .with_creator_tag(b"MartyPC")
            .build();

        match image_result {
            Ok(disk_image) => {
                _ = sender.send(FrontendThreadEvent::FloppyImageLoadComplete {
                    drive_select,
                    image: Arc::new(disk_image),
                    item: context,
                    path: Some(archive_path),
                    source: FloppyImageLoadSource::ZipArchive,
                });
            }
            Err(error) => {
                _ = sender.send(FrontendThreadEvent::FloppyImageLoadError(format!(
                    "Failed to build floppy image from ZIP archive: {}",
                    error
                )));
            }
        }
    });
}
