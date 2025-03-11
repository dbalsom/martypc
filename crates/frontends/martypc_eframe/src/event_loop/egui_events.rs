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

//! Process events received from the emulator GUI.
//! Typically, the GUI is implemented by the `marty_egui` crate.

use std::{
    ffi::OsString,
    io::Cursor,
    mem::discriminant,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::{emulator, emulator::Emulator, floppy::load_floppy::handle_load_floppy};
use display_manager_eframe::EFrameDisplayManager;

use marty_frontend_common::{
    constants::{LONG_NOTIFICATION_TIME, NORMAL_NOTIFICATION_TIME, SHORT_NOTIFICATION_TIME},
    floppy_manager::FloppyError,
    thread_events::{FileSelectionContext, FrontendThreadEvent},
    types::floppy::FloppyImageSource,
};

use marty_core::{
    breakpoints::BreakPointType,
    cpu_common,
    cpu_common::{Cpu, CpuOption, Register16},
    device_traits::videocard::ClockingMode,
    device_types::fdc::FloppyImageType,
    machine::{MachineOption, MachineState},
    vhd,
    vhd::VirtualHardDisk,
};
use marty_egui::{
    modal::ModalContext,
    state::FloppyDriveSelection,
    DeviceSelection,
    GuiBoolean,
    GuiEnum,
    GuiEvent,
    GuiFloat,
    GuiVariable,
    GuiVariableContext,
    InputFieldChangeSource,
};
use marty_videocard_renderer::AspectCorrectionMode;

use fluxfox::{DiskImage, LoadingStatus};

use anyhow::Error;
use winit::event_loop::ActiveEventLoop;

#[cfg(target_arch = "wasm32")]
use crate::wasm::file_open;
#[cfg(target_arch = "wasm32")]
use crate::wasm::worker::spawn_closure_worker as spawn;
use marty_frontend_common::{
    display_manager::{DisplayManager, DtHandle},
    timestep_manager::{TimestepManager, TimestepUpdate},
};
#[cfg(not(target_arch = "wasm32"))]
use std::thread::spawn;

//noinspection RsBorrowChecker
pub fn handle_egui_event(
    emu: &mut Emulator,
    dm: &mut EFrameDisplayManager,
    tm: &TimestepManager,
    tmu: &mut TimestepUpdate,
    gui_event: &GuiEvent,
) {
    match gui_event {
        GuiEvent::Exit => {
            // User chose exit option from menu. Shut down.
            // TODO: Add a timeout from last VHD write for safety?
            let _ = emu.sender.send(FrontendThreadEvent::QuitRequested);
        }
        GuiEvent::SetNMI(state) => {
            // User wants to crash the computer. Sure, why not.
            emu.machine.set_nmi(*state);
        }
        // Gui variables have a context, which is sort of like a namespace so that multiple versions
        // of a single GuiEnum can be stored - for example we have a Context per configured Display
        // target. A Global context is used if only a single instance of any GuiEnum is required.
        GuiEvent::VariableChanged(ctx, eopt) => match eopt {
            GuiVariable::Bool(op, val) => match (op, *val) {
                (GuiBoolean::CpuEnableWaitStates, state) => {
                    emu.machine.set_cpu_option(CpuOption::EnableWaitStates(state));
                }
                (GuiBoolean::CpuInstructionHistory, state) => {
                    emu.machine.set_cpu_option(CpuOption::InstructionHistory(state));
                }
                (GuiBoolean::CpuTraceLoggingEnabled, state) => {
                    emu.machine.set_cpu_option(CpuOption::TraceLoggingEnabled(state));
                }
                (GuiBoolean::TurboButton, state) => {
                    emu.machine.set_turbo_mode(state);
                }
                _ => {}
            },
            GuiVariable::Float(op, val) => match op {
                GuiFloat::EmulationSpeed => {
                    log::debug!("Got emulation speed factor: {}", val);
                    tmu.new_throttle_factor = Some(*val as f64);

                    if let Some(si) = &mut emu.si {
                        si.set_master_speed(*val);
                    }
                }
                GuiFloat::MouseSpeed => {
                    if let Some(mouse) = emu.machine.bus_mut().mouse_mut() {
                        log::debug!("Setting mouse speed factor: {:?}", val);
                        mouse.set_speed(*val);
                    }
                }
            },
            GuiVariable::Enum(op) => match ctx {
                GuiVariableContext::SoundSource(s_idx) => match op {
                    GuiEnum::AudioMuted(state) => {
                        if let Some(si) = &mut emu.si {
                            si.set_volume(*s_idx, None, Some(*state));
                        }
                    }
                    GuiEnum::AudioVolume(vol) => {
                        if let Some(si) = &mut emu.si {
                            si.set_volume(*s_idx, Some(*vol), None);
                        }
                    }
                    _ => {}
                },
                GuiVariableContext::Display(dth) => match op {
                    GuiEnum::DisplayType(display_type) => {
                        log::debug!("Got display type update event: {:?}", display_type);
                        if let Err(e) = dm.set_display_type(*dth, *display_type) {
                            log::error!("Failed to set display type for display target: {:?}", e);
                        }
                    }
                    GuiEnum::DisplayAperture(aperture) => {
                        if let Some(vid) = dm.set_display_aperture(*dth, *aperture).ok().flatten() {
                            if let Some(video_card) = emu.machine.bus().video(&vid) {
                                if let Err(e) = dm.on_card_resized(&vid, video_card.get_display_extents()) {
                                    log::error!("Failed to set display aperture for display target: {:?}", e);
                                }
                            }
                        }
                    }
                    GuiEnum::DisplayScalerMode(new_mode) => {
                        log::debug!("Got scaler mode update event: {:?}", new_mode);
                        if let Err(_e) = dm.set_scaler_mode(*dth, *new_mode) {
                            log::error!("Failed to set scaler mode for display target!");
                        }
                    }
                    GuiEnum::DisplayScalerPreset(new_preset) => {
                        log::debug!("Got scaler preset update event: {:?}", new_preset);
                        if let Err(_e) = dm.apply_scaler_preset(*dth, new_preset.clone()) {
                            log::error!("Failed to set scaler preset for display target!");
                        }

                        // Update the scaler adjustment window with the parameters from the preset,
                        // so we can adjust them from that base.
                        if let Some(scaler_params) = dm.scaler_params(*dth) {
                            emu.gui.scaler_adjust.set_params(*dth, scaler_params);
                        }

                        dm.with_renderer_mut(*dth, |renderer| {
                            // Update composite checkbox state
                            let composite_enable = renderer.get_composite();
                            emu.gui.set_option_enum(
                                GuiEnum::DisplayComposite(composite_enable),
                                Some(GuiVariableContext::Display(*dth)),
                            );

                            // Update aspect correction checkbox state
                            let aspect_correct = renderer.get_params().aspect_correction;
                            let aspect_correct_on = !matches!(aspect_correct, AspectCorrectionMode::None);
                            emu.gui.set_option_enum(
                                GuiEnum::DisplayAspectCorrect(aspect_correct_on),
                                Some(GuiVariableContext::Display(*dth)),
                            );
                        });
                    }
                    GuiEnum::DisplayComposite(state) => {
                        log::debug!("Got composite state update event: {}", state);

                        dm.with_renderer_mut(*dth, |renderer| {
                            renderer.set_composite(*state);
                        });
                    }
                    GuiEnum::DisplayAspectCorrect(state) => {
                        if let Err(_e) = dm.set_aspect_correction(*dth, *state) {
                            log::error!("Failed to set aspect correction state for display target!");
                        }
                    }
                    _ => {}
                },
                #[cfg(feature = "use_serialport")]
                GuiVariableContext::SerialPort(serial_id) => match op {
                    GuiEnum::SerialPortBridge(host_id) => {
                        match emu
                            .machine
                            .bridge_serial_port(*serial_id, "DUMMY".to_string(), host_id.clone())
                        {
                            Ok(_) => {
                                emu.gui
                                    .toasts()
                                    .info(format!("Serial port bridged to: {}", host_id))
                                    .duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                            Err(e) => {
                                emu.gui
                                    .toasts()
                                    .error(format!("Failed to bridge serial port: {}", e))
                                    .duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                        }
                    }
                    _ => {}
                },
                GuiVariableContext::Global => {}
                _ => {
                    log::warn!("Unhandled enum context: {:?}", ctx);
                }
            },
        },
        GuiEvent::LoadVHD(drive_idx, image_idx) => {
            log::debug!("Releasing VHD slot: {}", drive_idx);
            emu.vhd_manager.release_vhd(*drive_idx);

            let mut error_str = None;

            match emu.vhd_manager.load_vhd_file(*drive_idx, *image_idx) {
                Ok(vhd_file) => match VirtualHardDisk::parse(Box::new(vhd_file), false) {
                    Ok(vhd) => {
                        if let Some(hdc) = emu.machine.hdc_mut() {
                            match hdc.set_vhd(*drive_idx, vhd) {
                                Ok(_) => {
                                    let vhd_name = emu.vhd_manager.get_vhd_name(*image_idx).unwrap();
                                    log::info!(
                                        "VHD image {:?} successfully loaded into virtual drive: {}",
                                        vhd_name,
                                        *drive_idx
                                    );

                                    emu.gui
                                        .toasts()
                                        .info(format!("VHD loaded: {:?}", vhd_name))
                                        .duration(Some(NORMAL_NOTIFICATION_TIME));
                                }
                                Err(err) => {
                                    error_str = Some(format!("Error mounting VHD: {}", err));
                                }
                            }
                        }
                        else if let Some(hdc) = emu.machine.xtide_mut() {
                            match hdc.set_vhd(*drive_idx, vhd) {
                                Ok(_) => {
                                    let vhd_name = emu.vhd_manager.get_vhd_name(*image_idx).unwrap();
                                    log::info!(
                                        "VHD image {:?} successfully loaded into virtual drive: {}",
                                        vhd_name,
                                        *drive_idx
                                    );

                                    emu.gui
                                        .toasts()
                                        .info(format!("VHD loaded: {:?}", vhd_name))
                                        .duration(Some(NORMAL_NOTIFICATION_TIME));
                                }
                                Err(err) => {
                                    error_str = Some(format!("Error mounting VHD: {}", err));
                                }
                            }
                        }
                        else {
                            error_str = Some("No Hard Disk Controller present!".to_string());
                        }
                    }
                    Err(err) => {
                        error_str = Some(format!("Error loading VHD: {}", err));
                    }
                },
                Err(err) => {
                    error_str = Some(format!("Failed to load VHD image index {}: {}", *image_idx, err));
                }
            }

            // Handle errors.
            if let Some(err_str) = error_str {
                log::error!("{}", err_str);
                emu.gui.toasts().error(err_str).duration(Some(LONG_NOTIFICATION_TIME));
            }
        }
        GuiEvent::CreateVHD(filename, fmt) => {
            // The user requested that a new VHD be created, with the given filename and format.
            log::info!("Got CreateVHD event: {:?}, {:?}", filename, fmt);

            let mut vhd_path = emu.rm.resource_path("hdd").unwrap();
            vhd_path.push(filename);

            // TODO: Factor out VHD support into a separate library.
            //       The emulator core should not be writing files.
            match vhd::create_vhd(
                vhd_path.into_os_string(),
                fmt.geometry.c(),
                fmt.geometry.h(),
                fmt.geometry.s(),
            ) {
                Ok(_) => {
                    // We don't actually do anything with the newly created file
                    // But show a toast notification.
                    emu.gui
                        .toasts()
                        .info(format!("Created VHD: {}", filename.to_string_lossy()))
                        .duration(Some(Duration::from_secs(5)));

                    // Rescan resource paths to show new file in list
                    if let Err(e) = emu.vhd_manager.scan_resource(&mut emu.rm) {
                        log::error!("Error scanning hdd directory: {}", e);
                    };
                }
                Err(err) => {
                    log::error!("Error creating VHD: {}", err);
                    emu.gui
                        .toasts()
                        .error(format!("{}", err))
                        .duration(Some(LONG_NOTIFICATION_TIME));
                }
            }
        }
        GuiEvent::RescanMediaFolders => {
            // User requested to rescan media folders (ie, when a new disk image was copied into
            // the /media resource directory)
            if let Err(e) = emu.floppy_manager.scan_resource(&mut emu.rm) {
                log::error!("Error scanning floppy directory: {}", e);
            }
            if let Err(e) = emu.floppy_manager.scan_autofloppy(&mut emu.rm) {
                log::error!("Error scanning autofloppy directory: {}", e);
            }
            if let Err(e) = emu.vhd_manager.scan_resource(&mut emu.rm) {
                log::error!("Error scanning hdd directory: {}", e);
            }
            if let Err(e) = emu.cart_manager.scan_resource(&mut emu.rm) {
                log::error!("Error scanning cartridge directory: {}", e);
            }
            // Update Floppy Disk Image tree
            match emu.floppy_manager.make_tree(&mut emu.rm) {
                Ok(floppy_tree) => {
                    //log::debug!("Built tree {:?}, setting tree in GUI...", floppy_tree);
                    emu.gui.set_floppy_tree(floppy_tree)
                }
                Err(e) => {
                    emu.gui
                        .toasts()
                        .error(format!("Failed to build floppy tree: {}", e))
                        .duration(Some(SHORT_NOTIFICATION_TIME));
                }
            }

            emu.gui.set_autofloppy_paths(emu.floppy_manager.get_autofloppy_paths());
            // Update VHD Image tree
            if let Ok(hdd_tree) = emu.vhd_manager.make_tree(&mut emu.rm) {
                emu.gui.set_hdd_tree(hdd_tree);
            }
            // Update Cartridge Image tree
            if let Ok(cart_tree) = emu.cart_manager.make_tree(&mut emu.rm) {
                emu.gui.set_cart_tree(cart_tree);
            }
        }
        GuiEvent::InsertCartridge(slot_select, item_idx) => {
            // User requested to insert a PCjr cartridge into the indicated slot, from the quick access menu.
            // This will reboot the machine.
            log::debug!("Insert Cart image: {:?} into drive: {}", item_idx, slot_select);

            let mut reboot = false;
            if let Some(cart_slot) = emu.machine.cart_slot() {
                match emu.cart_manager.get_cart_name(*item_idx) {
                    Some(name) => {
                        log::info!("Loading cart image: {:?} into slot: {}", name, slot_select);

                        match emu.cart_manager.load_cart_data(*item_idx, &mut emu.rm) {
                            Ok(cart_image) => match cart_slot.insert_cart(*slot_select, cart_image) {
                                Ok(()) => {
                                    log::info!("Cart image successfully loaded into slot: {}", slot_select);

                                    emu.gui.set_cart_selection(
                                        *slot_select,
                                        Some(*item_idx),
                                        Some(name.clone().into()),
                                    );

                                    emu.gui
                                        .toasts()
                                        .info(format!("Cartridge inserted: {:?}", name.clone()))
                                        .duration(Some(NORMAL_NOTIFICATION_TIME));

                                    // Inserting a cartridge reboots the machine due to a switch in the cartridge slot.
                                    reboot = true;
                                }
                                Err(err) => {
                                    log::error!("Cart image failed to load into slot {}: {}", slot_select, err);
                                    emu.gui
                                        .toasts()
                                        .error(format!("Cartridge load failed: {}", err))
                                        .duration(Some(NORMAL_NOTIFICATION_TIME));
                                }
                            },
                            Err(err) => {
                                log::error!("Failed to load cart image: {:?} Error: {}", item_idx, err);
                                emu.gui
                                    .toasts()
                                    .error(format!("Cartridge load failed: {}", err))
                                    .duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                        }
                    }
                    None => {
                        emu.gui
                            .toasts()
                            .error("Cartridge load failed: Invalid name!".to_string())
                            .duration(Some(NORMAL_NOTIFICATION_TIME));
                    }
                }
            }

            if reboot {
                emu.machine.change_state(MachineState::Rebooting);
            }
        }
        GuiEvent::RemoveCartridge(slot_select) => {
            // User requested to remove a PCjr cartridge from the indicated slot. This will reboot the machine.
            log::info!("Removing cartridge from slot: {}", slot_select);

            let mut reboot = false;
            if let Some(cart_slot) = emu.machine.cart_slot() {
                cart_slot.remove_cart(*slot_select);
                emu.gui.set_cart_selection(*slot_select, None, None);
                emu.gui
                    .toasts()
                    .info("Cartridge removed!".to_string())
                    .duration(Some(SHORT_NOTIFICATION_TIME));

                reboot = true;
            }
            if reboot {
                emu.machine.change_state(MachineState::Rebooting);
            }
        }
        GuiEvent::RequestLoadFloppyDialog(drive_select) => {
            // User requested a file dialog to load a floppy image into the indicated drive slot.
            log::debug!("Requesting floppy load dialog for drive: {}", drive_select);
            #[cfg(target_arch = "wasm32")]
            {
                use marty_frontend_common::thread_events::FileOpenContext;
                let context = FileOpenContext::FloppyDiskImage {
                    drive_select: *drive_select,
                    fsc: FileSelectionContext::Path(PathBuf::new()),
                };
                file_open::open_file_dialog(context, emu.sender.clone());
            }
        }
        GuiEvent::RequestSaveFloppyDialog(drive_select, format) => {
            // User requested a file dialog to load a floppy image into the indicated drive slot.
            log::debug!(
                "Requesting floppy save dialog for drive: {}, format: {:?}",
                drive_select,
                format
            );
            // TODO: Implement save floppy image on web
            //       ImageBuilder needs to be able to accept a Writer (`with_writer` perhaps?)
            #[cfg(target_arch = "wasm32")]
            {
                // if let Some(fdc) = emu.machine.fdc() {
                //     let (disk_image_opt, _) = fdc.get_image(*drive_select);
                //     if let Some(floppy_image) = disk_image_opt {
                //         let mut image = floppy_image.write().unwrap();
                //         match fluxfox::ImageWriter::new(&mut image)
                //             .with_format(*format)
                //             .with_path(filepath.clone())
                //             .write()
                //         {
                //             Ok(_) => {
                //                 log::info!("Floppy image successfully saved: {:?}", filepath);
                //
                //                 emu.gui.set_floppy_selection(
                //                     *drive_select,
                //                     None,
                //                     FloppyDriveSelection::Image(filepath.clone()),
                //                     Some(*format),
                //                     image.compatible_formats(true),
                //                     None,
                //                 );
                //
                //                 emu.gui
                //                     .toasts()
                //                     .info(format!("Floppy saved: {:?}", filepath.file_name().unwrap_or_default()))
                //                     .duration(Some(NORMAL_NOTIFICATION_TIME));
                //             }
                //             Err(err) => {
                //                 log::error!("Floppy image failed to save: {}", err);
                //
                //                 emu.gui
                //                     .toasts()
                //                     .error(format!("Failed to save: {}", err))
                //                     .duration(Some(NORMAL_NOTIFICATION_TIME));
                //             }
                //         }
                //     }
                // }

                //file_save::save_file_dialog(context, emu.sender.clone());
            }
        }
        GuiEvent::LoadQuickFloppy(drive_select, item_idx) => {
            // User selected a floppy image from the quick access menu.
            log::debug!("Load floppy quick image: {:?} into drive: {}", item_idx, drive_select);
            handle_load_floppy(emu, *drive_select, FileSelectionContext::Index(*item_idx));
        }
        GuiEvent::LoadFloppyAs(drive_select, path) => {
            // User selected a floppy image by path
            // TODO: This should be a thread event as file dialog is asynchronous
            log::debug!(
                "Load floppy image: {} into drive: {}",
                path.to_string_lossy(),
                drive_select
            );
            handle_load_floppy(emu, *drive_select, FileSelectionContext::Path(path.clone()));
        }
        GuiEvent::LoadAutoFloppy(drive_select, path) => {
            log::debug!(
                "Mounting directory path: {:?} into drive: {}",
                path.to_string_lossy(),
                drive_select
            );
            /*
            // Query the indicated floppy drive for the largest supported image format.
            // An autofloppy will always be built to the largest supported capacity.
            let mut image_type = None;
            if let Some(fdc) = emu.machine.fdc() {
                image_type = Some(fdc.drive(*drive_select).get_largest_supported_image_format());
            }

            match emu
                .floppy_manager
                .build_autofloppy_image_from_dir(path, image_type, &emu.rm)
                .await
            {
                Ok(vec) => {
                    if let Some(fdc) = emu.machine.fdc() {
                        let mut load_success = false;
                        match fdc.load_image_from(*drive_select, vec, None, true) {
                            Ok(image) => {
                                log::info!("Floppy image successfully loaded into virtual drive.");
                                load_success = true;

                                emu.gui.set_floppy_selection(
                                    *drive_select,
                                    None,
                                    FloppyDriveSelection::Directory(path.clone()),
                                    image.source_format(),
                                    image.compatible_formats(true),
                                    Some(true),
                                );

                                emu.gui.set_floppy_write_protected(*drive_select, true);
                            }
                            Err(err) => {
                                log::warn!("Floppy image failed to load: {}", err);
                            }
                        }

                        let mut patch_success = false;
                        // Patch the floppy image with the correct BPB for the selected format type.
                        match fdc.patch_image_bpb(*drive_select, image_type) {
                            Ok(()) => {
                                log::info!("Floppy image patched with correct BPB.");
                                patch_success = true;
                            }
                            Err(err) => {
                                log::warn!("Failed to patch floppy image with correct BPB: {}", err);
                            }
                        }

                        if load_success & patch_success {
                            emu.gui
                                .toasts()
                                .info("Floppy image successfully mounted!".to_string())
                                .duration(Some(NORMAL_NOTIFICATION_TIME));
                        }
                        else {
                            emu.gui
                                .toasts()
                                .error("Failed to mount floppy image!".to_string())
                                .duration(Some(NORMAL_NOTIFICATION_TIME));
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
            }*/
        }
        GuiEvent::SaveFloppy(drive_select, image_idx) => {
            log::debug!(
                "Received SaveFloppy event image index: {}, drive: {}",
                image_idx,
                drive_select
            );

            if let Some(fdc) = emu.machine.fdc() {
                let floppy = fdc.get_image(*drive_select);
                if let Some(floppy_image) = floppy.0 {
                    // match emu.floppy_manager.save_floppy_data(floppy_image, *image_idx, &emu.rm) {
                    //     Ok(path) => {
                    //         log::info!("Floppy image successfully saved: {:?}", path);
                    //
                    //         emu.gui
                    //             .toasts()
                    //             .info(format!("Floppy saved: {:?}", path.file_name()))
                    //             .set_duration(Some(SHORT_NOTIFICATION_TIME));
                    //     }
                    //     Err(err) => {
                    //         log::warn!("Floppy image failed to save: {}", err);
                    //     }
                    // }
                }
            }
        }
        GuiEvent::SaveFloppyAs(drive_select, format, filepath) => {
            log::debug!(
                "Received SaveFloppyAs event drive: {} format: {:?} filename: {:?}",
                drive_select,
                format,
                filepath,
            );

            if let Some(fdc) = emu.machine.fdc() {
                let (disk_image_opt, _) = fdc.get_image(*drive_select);
                if let Some(floppy_image) = disk_image_opt {
                    let mut image = floppy_image.write().unwrap();
                    match fluxfox::ImageWriter::<std::fs::File>::new(&mut image)
                        .with_format(*format)
                        .with_path(filepath.clone())
                        .write()
                    {
                        Ok(_) => {
                            log::info!("Floppy image successfully saved: {:?}", filepath);

                            emu.gui.set_floppy_selection(
                                *drive_select,
                                None,
                                FloppyDriveSelection::Image(filepath.clone()),
                                Some(*format),
                                image.compatible_formats(true),
                                None,
                            );

                            emu.gui
                                .toasts()
                                .info(format!("Floppy saved: {:?}", filepath.file_name().unwrap_or_default()))
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
        GuiEvent::EjectFloppy(drive_select) => {
            // User ejected the floppy from the drive slot 'drive_select'
            log::info!("Ejecting floppy in drive: {}", drive_select);
            if let Some(fdc) = emu.machine.fdc() {
                fdc.unload_image(*drive_select);
                emu.gui.set_floppy_selection(
                    *drive_select,
                    None,
                    FloppyDriveSelection::None,
                    None,
                    Vec::new(),
                    Some(false),
                );
                emu.gui
                    .toasts()
                    .info("Floppy ejected!".to_string())
                    .duration(Some(SHORT_NOTIFICATION_TIME));
            }
        }
        GuiEvent::CreateNewFloppy(drive_select, format, formatted) => {
            // User requested to create a new floppy image in of 'format' in the drive slot 'drive_select'
            log::info!(
                "Creating new floppy image in drive: {} of format {:?}, formatted: {}",
                drive_select,
                format,
                formatted
            );
            if let Some(fdc) = emu.machine.fdc() {
                fdc.unload_image(*drive_select);
                emu.gui.set_floppy_selection(
                    *drive_select,
                    None,
                    FloppyDriveSelection::None,
                    None,
                    Vec::new(),
                    Some(false),
                );

                match fdc.create_new_image(*drive_select, *format, *formatted) {
                    Ok(image_lock) => {
                        let image = image_lock.read().unwrap();
                        emu.gui.set_floppy_selection(
                            *drive_select,
                            None,
                            FloppyDriveSelection::NewImage(*format),
                            image.source_format(),
                            image.compatible_formats(true),
                            Some(false),
                        );

                        emu.gui
                            .toasts()
                            .info("New floppy created!".to_string())
                            .duration(Some(SHORT_NOTIFICATION_TIME));
                    }
                    Err(e) => {
                        log::error!("Failed to create new floppy image: {}", e);
                        emu.gui
                            .toasts()
                            .error(format!("Failed to create new floppy: {}", e))
                            .duration(Some(NORMAL_NOTIFICATION_TIME));
                    }
                }
            }
        }
        GuiEvent::QueryCompatibleFloppyFormats(drive_select) => {
            if let Some(fdc) = emu.machine.fdc() {
                if let Some(image_lock) = fdc.get_image(*drive_select).0 {
                    let image = image_lock.read().unwrap();
                    let compat_formats = image.compatible_formats(true);
                    emu.gui.set_floppy_supported_formats(*drive_select, compat_formats);
                }
            }
        }
        GuiEvent::SetFloppyWriteProtect(drive_select, state) => {
            log::info!("Setting floppy write protect: {}", state);
            if let Some(fdc) = emu.machine.fdc() {
                fdc.write_protect(*drive_select, *state);
            }
        }
        #[cfg(feature = "use_serialport")]
        GuiEvent::BridgeSerialPort(guest_port_id, host_port_name, host_port_id) => {
            log::info!("Bridging serial port: {}, id: {}", host_port_name, host_port_id);
            if let Err(err) = emu
                .machine
                .bridge_serial_port(*guest_port_id, host_port_name.clone(), *host_port_id)
            {
                emu.gui
                    .toasts()
                    .error(err.to_string())
                    .duration(Some(NORMAL_NOTIFICATION_TIME));
            }
            else {
                emu.gui
                    .toasts()
                    .info(format!("Serial port successfully bridged to {}", host_port_name))
                    .duration(Some(NORMAL_NOTIFICATION_TIME));

                // Update the serial port enum to show the bridged port
                emu.gui.set_option_enum(
                    GuiEnum::SerialPortBridge(*host_port_id),
                    Some(GuiVariableContext::SerialPort(*guest_port_id)),
                );
                log::debug!(
                    "updating SerialPortBridge, host_port: {} context (guest_port): {}",
                    host_port_id,
                    guest_port_id
                );
            }
        }
        GuiEvent::DumpVRAM => {
            if let Some(video_card) = emu.machine.primary_videocard() {
                let dump_path = emu.rm.resource_path("dump").unwrap();
                video_card.dump_mem(&dump_path);
            }

            // TODO: A video card dump may be multiple files (one file per plane). We can't create
            //       a single unique filename in this case.
            // if let Some(video_card) = emu.machine.primary_videocard() {
            //     let base_name = format!("{:?}_mem", video_card.get_video_type());
            //
            //     emu.rm
            //         .get_available_filename("dump", &base_name, Some("bin"))
            //         .ok()
            //         .map(|path| video_card.dump_mem(&path))
            //         .or_else(|| {
            //             log::error!("Failed to get available filename for memory dump!");
            //             None
            //         });
            // }
        }
        GuiEvent::DumpSegment(register) => {
            let base_segment = match register {
                Register16::CS | Register16::DS | Register16::ES | Register16::SS => {
                    emu.machine.cpu().get_register16(*register)
                }
                _ => {
                    log::error!("Invalid segment register for dump: {:?}", register);
                    return;
                }
            };

            let flat_addr = cpu_common::calc_linear_address(base_segment, 0);
            log::info!("Dumping {:?}: {:04X} ({:08X})", register, base_segment, flat_addr);

            let end = flat_addr + 0x10000;
            let base_name = format!("{:?}_dump", register).to_ascii_lowercase();
            match emu.rm.get_available_filename("dump", &base_name, Some("bin")) {
                Ok(path) => {
                    emu.machine.bus().dump_mem_range(flat_addr, end, &path);
                    emu.gui
                        .toasts()
                        .info(format!("Segment dumped: {:?}", path))
                        .duration(Some(NORMAL_NOTIFICATION_TIME));
                }
                Err(e) => {
                    log::error!("Failed to get available filename for memory dump!");
                    emu.gui
                        .toasts()
                        .error(format!("Failed to dump segment: {e}"))
                        .duration(Some(LONG_NOTIFICATION_TIME));
                }
            }
        }
        GuiEvent::DumpAllMem => {
            emu.rm
                .get_available_filename("dump", "memdump", Some("bin"))
                .ok()
                .map(|path| emu.machine.bus().dump_mem(&path))
                .or_else(|| {
                    log::error!("Failed to get available filename for memory dump!");
                    None
                });
        }
        GuiEvent::EditBreakpoint => {
            // Get breakpoints from GUI
            let bp_set = emu.gui.get_breakpoints();

            let mut breakpoints = Vec::new();

            // Push exec breakpoint to list if valid expression
            if let Some(addr) = emu.machine.cpu().eval_address(bp_set.breakpoint) {
                let flat_addr = u32::from(addr);
                if flat_addr > 0 && flat_addr < 0x100000 {
                    breakpoints.push(BreakPointType::ExecuteFlat(flat_addr));
                }
            };

            // Push mem breakpoint to list if valid expression
            if let Some(addr) = emu.machine.cpu().eval_address(bp_set.mem_breakpoint) {
                let flat_addr = u32::from(addr);
                if flat_addr > 0 && flat_addr < 0x100000 {
                    breakpoints.push(BreakPointType::MemAccessFlat(flat_addr));
                }
            }

            // Push int breakpoint to list
            if let Ok(iv) = u32::from_str_radix(bp_set.int_breakpoint, 10) {
                if iv < 256 {
                    breakpoints.push(BreakPointType::Interrupt(iv as u8));
                }
            }

            // Push io breakpoint to list
            if let Ok(addr) = u32::from_str_radix(bp_set.io_breakpoint, 16) {
                let port = addr as u16;
                log::debug!("Adding I/O breakpoint: {:04X}", port);
                breakpoints.push(BreakPointType::IoAccess(port));
            }

            // Push stopwatches to list
            if let Some(addr) = emu.machine.cpu().eval_address(bp_set.sw_start) {
                let start_flat_addr = u32::from(addr);
                if start_flat_addr > 0 && start_flat_addr < 0x100000 {
                    if let Some(addr) = emu.machine.cpu().eval_address(bp_set.sw_stop) {
                        let stop_flat_addr = u32::from(addr);
                        if stop_flat_addr > 0 && stop_flat_addr < 0x100000 {
                            breakpoints.push(BreakPointType::StartWatch(start_flat_addr));
                            breakpoints.push(BreakPointType::StopWatch(stop_flat_addr));
                            emu.machine.set_stopwatch(0, start_flat_addr, stop_flat_addr);
                        }
                    }
                }
            }

            emu.machine.set_breakpoints(breakpoints);
        }
        GuiEvent::MemoryUpdate => {
            // The address bar for the memory viewer was updated. We need to
            // evaluate the expression and set a new row value for the control.
            // The memory contents will be updated in the normal frame update.
            let (mem_dump_addr_str, source) = emu.gui.memory_viewer.get_address();

            if let InputFieldChangeSource::UserInput = source {
                // Only evaluate expression if the address box was changed by user input.
                let mem_dump_addr: u32 = match emu.machine.cpu().eval_address(&mem_dump_addr_str) {
                    Some(i) => {
                        let addr: u32 = i.into();
                        addr & !0x0F
                    }
                    None => {
                        // Show address 0 if expression eval fails
                        0
                    }
                };
                emu.gui.memory_viewer.set_address(mem_dump_addr as usize);
            }
        }
        GuiEvent::MemoryByteUpdate(addr, val) => {
            // The user has changed a memory value in the memory viewer.
            // We need to update the memory contents in the emulator.
            _ = emu.machine.bus_mut().write_u8(*addr, *val, 0);
        }
        GuiEvent::Register16Update(reg, val) => {
            // The user has changed a 16-bit register value in the register viewer.
            // We need to update the register contents in the emulator.
            emu.machine.cpu_mut().set_register16(*reg, *val);
        }
        GuiEvent::CpuFlagsUpdate(flags) => {
            // The user has changed the CPU flags in the register viewer.
            // We need to update the flags in the emulator.
            emu.machine.cpu_mut().set_flags(*flags);
        }
        GuiEvent::CpuFlushQueue => {
            // The user has requested to clear the CPU instruction queue.
            emu.machine.cpu_mut().flush_piq();
        }
        GuiEvent::TokenHover(addr) => {
            // Hovered over a token in a TokenListView.
            let cpu_type = emu.machine.cpu().get_type();
            let debug = emu.machine.bus_mut().get_memory_debug(cpu_type, *addr);
            emu.gui.memory_viewer.set_hover_text(format!("{}", debug));
        }
        // Request to flush trac
        GuiEvent::FlushLogs => {
            emu.machine.flush_trace_logs();
        }
        GuiEvent::DelayAdjust => {
            let delay_params = emu.gui.delay_adjust.get_params();

            emu.machine
                .set_cpu_option(CpuOption::DramRefreshAdjust(delay_params.dram_delay));
            emu.machine
                .set_cpu_option(CpuOption::HaltResumeDelay(delay_params.halt_resume_delay));
        }
        GuiEvent::TickDevice(dev, ticks) => {
            match dev {
                DeviceSelection::Timer(_t) => {}
                DeviceSelection::VideoCard => {
                    if let Some(video_card) = emu.machine.primary_videocard() {
                        // Playing around with the clock forces the adapter into
                        // cycle mode, if supported.
                        video_card.set_clocking_mode(ClockingMode::Cycle);
                        video_card.debug_tick(*ticks, None);
                    }
                }
            }
        }
        // User changed the machine's operational state.
        GuiEvent::MachineStateChange(state) => {
            match state {
                MachineState::Off | MachineState::Rebooting => {
                    // Clear the screen if rebooting or turning off

                    // TODO: Fix this (2024)

                    // emu.dm.for_each_renderer(|renderer, _card_id, buf| {
                    //     renderer.clear();
                    //     buf.fill(0);
                    // });

                    if emu.config.machine.reload_roms {
                        // Tell the Machine to wait on execution until ROMs are reloaded
                        emu.machine.set_reload_pending(true);
                    }
                }
                _ => {
                    emu.machine.set_reload_pending(false);
                }
            }
            emu.machine.change_state(*state);
        }
        GuiEvent::TakeScreenshot(dt_idx) => {
            // User requested to take a screenshot
            let screenshot_path = emu.rm.resource_path("screenshot").unwrap();

            // TODO: Fix this (2024)

            if let Err(err) = dm.save_screenshot(DtHandle::from(*dt_idx), screenshot_path) {
                log::error!("Failed to save screenshot: {}", err);
                emu.gui
                    .toasts()
                    .error(format!("{}", err))
                    .duration(Some(LONG_NOTIFICATION_TIME));
            }
        }
        GuiEvent::ToggleFullscreen(_dt_idx) => {
            // User requested to toggle fullscreen mode
            let _ = emu.sender.send(FrontendThreadEvent::ToggleFullscreen);
        }
        GuiEvent::CtrlAltDel => {
            // User requested to send CTRL + ALT + DEL keyboard combination
            emu.machine.emit_ctrl_alt_del();
        }
        GuiEvent::CompositeAdjust(dt, params) => {
            // User adjusted the composite video parameters
            dm.with_renderer(*dt, |renderer| {
                renderer.cga_direct_param_update(params);
            });
        }
        GuiEvent::ScalerAdjust(dt_idx, params) => {
            // User adjusted the scaler parameters
            if let Err(err) = dm.apply_scaler_params(DtHandle::from(*dt_idx), params) {
                log::error!("Failed to apply scaler params: {}", err);
            }
        }
        GuiEvent::ZoomChanged(zoom) => {
            // User changed the global zoom level

            // emu.dm.for_each_gui(|gui, _window| {
            //     gui.set_zoom_factor(*zoom);
            // });
        }
        GuiEvent::ResetIOStats => {
            // User reset the IO monitor statistics
            emu.machine.bus_mut().reset_io_stats();
        }
        GuiEvent::StartRecordingDisassembly => {
            // User started recording disassembly
            emu.machine.set_option(MachineOption::RecordListing(true));
        }
        GuiEvent::StopRecordingDisassembly => {
            // User stopped recording disassembly
            emu.machine.set_option(MachineOption::RecordListing(false));
        }
        GuiEvent::ClearKeyboard => {
            if let Some(kb) = emu.machine.bus_mut().keyboard_mut() {
                log::debug!("Clearing keyboard.");
                kb.clear(true);

                emu.gui
                    .toasts()
                    .info("Keyboard reset!".to_string())
                    .duration(Some(SHORT_NOTIFICATION_TIME));
            }
        }
        _ => {
            log::warn!("Unhandled GUI event: {:?}", discriminant(gui_event));
        }
    }
}
