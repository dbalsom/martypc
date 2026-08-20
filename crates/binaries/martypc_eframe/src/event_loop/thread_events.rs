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

    event_loop/thread_events.rs

    Handle events received from background threads spawned by the frontend.
*/

#[cfg(not(target_arch = "wasm32"))]
use crate::event_loop::egui_events::handle_load_vhd;
use crate::{emulator::Emulator, floppy::load_floppy::load_floppy_image};
use egui::ViewportCommand;
use fluxfox::DiskImage;
use marty_egui::state::FloppyDriveSelection;
#[cfg(not(target_arch = "wasm32"))]
use marty_frontend_common::thread_events::DirectoryOpenContext;
use marty_frontend_common::{
    cartridge_manager::read_jripcart_image,
    constants::{LONG_NOTIFICATION_TIME, NORMAL_NOTIFICATION_TIME},
    thread_events::{
        FileOpenContext,
        FileSaveContext,
        FileSelectionContext,
        FloppyImageLoadSource,
        FrontendThreadEvent,
    },
};
use std::{path::PathBuf, sync::Arc};

pub fn handle_thread_event(emu: &mut Emulator, ctx: &egui::Context) {
    while let Ok(event) = emu.receiver.try_recv() {
        match event {
            FrontendThreadEvent::FileOpenDialogCancelled(context) => {
                if let FileOpenContext::ServiceHostFile { .. } = context {
                    emu.machine.abort_service_host_file_request();
                }
                emu.gui.modal.close_file_dialog();
            }
            FrontendThreadEvent::FileDialogCancelled => {
                emu.gui.modal.close_file_dialog();
            }
            #[cfg(not(target_arch = "wasm32"))]
            FrontendThreadEvent::DirectoryOpenDialogCancelled(_) => {
                emu.gui.modal.close_file_dialog();
            }
            FrontendThreadEvent::FileOpenError(context, error) => {
                let error_prefix = if let FileOpenContext::ServiceHostFile { .. } = context {
                    emu.machine.abort_service_host_file_request();
                    "Host file open error"
                }
                else {
                    "File open error"
                };
                log::error!("{}: {}", error_prefix, error);
                emu.gui
                    .toasts()
                    .error(format!("{}: {}", error_prefix, error))
                    .duration(Some(LONG_NOTIFICATION_TIME));
                emu.gui.modal.close_file_dialog();
            }
            FrontendThreadEvent::FileSaveError(error) => {
                log::error!("File save error: {}", error);
                emu.gui
                    .toasts()
                    .error(format!("File save error: {}", error))
                    .duration(Some(LONG_NOTIFICATION_TIME));
                emu.gui.modal.close_file_dialog();
            }
            #[cfg(not(target_arch = "wasm32"))]
            FrontendThreadEvent::DirectoryOpenError(_, error) => {
                log::error!("Directory open error: {}", error);
                emu.gui
                    .toasts()
                    .error(format!("Directory open error: {}", error))
                    .duration(Some(LONG_NOTIFICATION_TIME));
                emu.gui.modal.close_file_dialog();
            }
            FrontendThreadEvent::FileOpenDialogComplete {
                context,
                path,
                contents,
            } => {
                emu.gui.modal.close_file_dialog();
                if let FileOpenContext::ServiceHostFile { fsc } = &context {
                    let filename = match fsc {
                        FileSelectionContext::Path(path) => path
                            .file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy()
                            .into_owned(),
                        _ => {
                            log::error!("Host file dialog returned without a filename");
                            emu.machine.abort_service_host_file_request();
                            continue;
                        }
                    };
                    log::debug!(
                        "Selected host file for guest transfer: '{}' ({} bytes)",
                        filename,
                        contents.len()
                    );
                    emu.machine.stage_service_host_file(filename, contents);
                    continue;
                }

                emu.gui
                    .toasts()
                    .info(format!(
                        "File opened: {:?} ({}) bytes",
                        path.clone().unwrap_or(PathBuf::from("None")),
                        contents.len()
                    ))
                    .duration(Some(NORMAL_NOTIFICATION_TIME));

                match context {
                    FileOpenContext::ServiceHostFile { .. } => unreachable!(),
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
                    FileOpenContext::CartridgeImage { slot_select, fsc } => {
                        let cartridge_path = match fsc {
                            FileSelectionContext::Path(path) => Some(path),
                            _ => path,
                        };
                        load_cartridge_image(emu, slot_select, contents, cartridge_path);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    FileOpenContext::VhdDiskImage { .. } => {
                        log::error!("VHD path selection was returned with file contents");
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            FrontendThreadEvent::FileOpenPathDialogComplete { context, path } => {
                emu.gui.modal.close_file_dialog();
                match context {
                    FileOpenContext::VhdDiskImage { drive_select } => {
                        handle_load_vhd(emu, drive_select, FileSelectionContext::Path(path));
                    }
                    _ => {
                        let error = "Unsupported path-only file dialog context";
                        log::error!("{}", error);
                        emu.gui
                            .toasts()
                            .error(error.to_string())
                            .duration(Some(LONG_NOTIFICATION_TIME));
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            FrontendThreadEvent::DirectoryOpenDialogComplete { context, path } => {
                emu.gui.modal.close_file_dialog();
                match context {
                    DirectoryOpenContext::VhdSource => {
                        emu.gui.vhd_creator.set_source_path(path);
                    }
                }
            }
            FrontendThreadEvent::FileSaveDialogComplete(save_context) => {
                emu.gui.modal.close_file_dialog();
                let (drive_select, format, fsc) = match save_context {
                    FileSaveContext::Screenshot { filename, fsc, .. } => {
                        let saved_as = match fsc {
                            FileSelectionContext::Path(path) => path.display().to_string(),
                            _ => filename,
                        };
                        log::info!("Screenshot saved: {}", saved_as);
                        emu.gui
                            .toasts()
                            .info(format!("Screenshot saved!\n{}", saved_as))
                            .duration(Some(NORMAL_NOTIFICATION_TIME));
                        continue;
                    }
                    FileSaveContext::GuestFile { filename, .. } => {
                        log::info!("Guest file saved: {}", filename);
                        emu.gui
                            .toasts()
                            .info(format!("Guest file saved: {}", filename))
                            .duration(Some(NORMAL_NOTIFICATION_TIME));
                        continue;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    FileSaveContext::VhdDiskImage { fsc } => {
                        if let FileSelectionContext::Path(path) = fsc {
                            emu.gui.vhd_creator.set_output_path(path);
                        }
                        else {
                            log::error!("Failed to get VHD path from FileSaveDialogComplete event");
                            emu.gui
                                .toasts()
                                .error("Failed to get VHD output path!".to_string())
                                .duration(Some(LONG_NOTIFICATION_TIME));
                        }
                        continue;
                    }
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
                        match fluxfox::ImageWriter::<std::fs::File>::new(&mut image)
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
            }
            FrontendThreadEvent::FloppyImageLoadError(err) => {
                log::error!("Failed to load floppy image! Error: {}", err);
                emu.gui
                    .toasts()
                    .error(format!("Floppy load failed: {}", err))
                    .duration(Some(LONG_NOTIFICATION_TIME));

                emu.gui.modal.close_progress();
            }
            FrontendThreadEvent::FloppyImageBeginLongLoad => {
                emu.gui.modal.open_progress("Loading floppy image...", 0.0);
            }
            FrontendThreadEvent::FloppyImageLoadProgress(title, progress) => {
                emu.gui.modal.open_progress(title, progress as f32);
            }
            FrontendThreadEvent::FloppyImageLoadComplete {
                drive_select,
                item,
                image,
                path,
                source,
            } => {
                emu.gui.modal.close_progress();
                // emu.gui
                //     .toasts()
                //     .info("Got FloppyImageLoadComplete event")
                //     .duration(Some(NORMAL_NOTIFICATION_TIME));

                if let Some(fdc) = emu.machine.fdc() {
                    let write_protected =
                        source == FloppyImageLoadSource::ZipArchive || emu.config.emulator.media.write_protect_default;
                    match fdc.attach_image(
                        drive_select,
                        Arc::<DiskImage>::into_inner(image).unwrap(),
                        path.clone(),
                        write_protected,
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
                            let selection = match source {
                                FloppyImageLoadSource::DiskImage => {
                                    FloppyDriveSelection::Image(path.clone().unwrap_or_default())
                                }
                                FloppyImageLoadSource::ZipArchive => FloppyDriveSelection::ZipArchive(
                                    path.as_ref()
                                        .and_then(|path| path.file_name())
                                        .map(PathBuf::from)
                                        .unwrap_or_default(),
                                ),
                            };
                            emu.gui.set_floppy_selection(
                                drive_select,
                                item_idx,
                                selection,
                                image.source_format(),
                                image.compatible_formats(true),
                                Some(write_protected),
                            );

                            let path = path.unwrap_or(PathBuf::from("None"));
                            let notification = match source {
                                FloppyImageLoadSource::DiskImage => format!("Floppy loaded: {}", path.display()),
                                FloppyImageLoadSource::ZipArchive => format!("Mounted ZIP: {}", path.display()),
                            };
                            emu.gui
                                .toasts()
                                .info(notification)
                                .duration(Some(NORMAL_NOTIFICATION_TIME));
                        }
                        Err(err) => {
                            log::warn!("Floppy image failed to load: {}", err);
                        }
                    }
                }
            }
            FrontendThreadEvent::FloppyImageSaveError(err) => {
                log::error!("Floppy image save error: {}", err);
                emu.gui.modal.close_progress();
            }
            FrontendThreadEvent::FloppyImageSaveComplete(path) => {
                emu.gui.modal.close_progress();
                log::info!("Floppy image saved: {:?}", path);
            }
            FrontendThreadEvent::QuitRequested => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            FrontendThreadEvent::ToggleFullscreen => {
                let mut fullscreen_state = false;
                ctx.input(|i| {
                    fullscreen_state = i.viewport().fullscreen.unwrap_or(false);
                });
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen_state));
            }
        }
    }
}

fn load_cartridge_image(emu: &mut Emulator, slot_idx: usize, contents: Vec<u8>, path: Option<PathBuf>) {
    let cartridge = match read_jripcart_image(&contents) {
        Ok(cartridge) => cartridge,
        Err(error) => {
            log::error!("Failed to parse cartridge image: {}", error);
            emu.gui
                .toasts()
                .error(format!("Cartridge load failed: {}", error))
                .duration(Some(LONG_NOTIFICATION_TIME));
            return;
        }
    };

    let Some(cartridge_slot) = emu.machine.cart_slot()
    else {
        log::error!("Cartridge load failed: No cartridge slots are present");
        emu.gui
            .toasts()
            .error("Cartridge load failed: No cartridge slots are present".to_string())
            .duration(Some(LONG_NOTIFICATION_TIME));
        return;
    };

    if let Err(error) = cartridge_slot.insert_cart(slot_idx, cartridge) {
        log::error!("Failed to insert cartridge into slot {}: {}", slot_idx, error);
        emu.gui
            .toasts()
            .error(format!("Cartridge load failed: {}", error))
            .duration(Some(LONG_NOTIFICATION_TIME));
        return;
    }

    let display_path = path.unwrap_or_else(|| PathBuf::from(format!("Cartridge Slot {slot_idx}")));
    log::info!(
        "Cartridge image {:?} successfully loaded into slot {}",
        display_path,
        slot_idx
    );
    emu.gui.set_cart_selection(slot_idx, None, Some(display_path.clone()));
    emu.gui
        .toasts()
        .info(format!("Cartridge inserted: {}", display_path.display()))
        .duration(Some(NORMAL_NOTIFICATION_TIME));

    // Inserting a cartridge toggles the physical slot switch and reboots the machine.
    emu.machine.reboot();
}
