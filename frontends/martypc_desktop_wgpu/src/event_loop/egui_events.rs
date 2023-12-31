/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    event_loop/update.rs

    Process received egui events.
*/

use crate::Emulator;
use display_manager_wgpu::DisplayManager;
use marty_core::{
    breakpoints::BreakPointType,
    cpu_common::CpuOption,
    device_traits::videocard::ClockingMode,
    machine::MachineState,
    vhd,
};
use marty_egui::{DeviceSelection, GuiBoolean, GuiEnum, GuiEvent, GuiVariable, GuiVariableContext};
use std::{mem::discriminant, time::Duration};

use frontend_common::constants::{LONG_NOTIFICATION_TIME, NORMAL_NOTIFICATION_TIME, SHORT_NOTIFICATION_TIME};
use marty_core::vhd::VirtualHardDisk;
use winit::event_loop::EventLoopWindowTarget;

pub fn handle_egui_event(emu: &mut Emulator, elwt: &EventLoopWindowTarget<()>, gui_event: &GuiEvent) {
    match gui_event {
        GuiEvent::Exit => {
            // User chose exit option from menu. Shut down.
            // TODO: Add a timeout from last VHD write for safety?
            println!("Thank you for using MartyPC!");
            elwt.exit();
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
            },
            GuiVariable::Enum(op) => match ctx {
                GuiVariableContext::Display(d_idx) => match op {
                    GuiEnum::DisplayAperture(aperture) => {
                        if let Some(vid) = emu.dm.set_display_aperture(*d_idx, *aperture).ok().flatten() {
                            if let Some(video_card) = emu.machine.bus().video(&vid) {
                                if let Err(e) = emu.dm.on_card_resized(&vid, video_card.get_display_extents()) {
                                    log::error!("Failed to set display aperture for display target: {:?}", e);
                                }
                            }
                        }
                    }
                    GuiEnum::DisplayScalerMode(new_mode) => {
                        log::debug!("Got scaler mode update event: {:?}", new_mode);
                        if let Err(_e) = emu.dm.set_scaler_mode(*d_idx, *new_mode) {
                            log::error!("Failed to set scaler mode for display target!");
                        }
                    }
                    GuiEnum::DisplayScalerPreset(new_preset) => {
                        log::debug!("Got scaler preset update event: {:?}", new_preset);
                        if let Err(_e) = emu.dm.apply_scaler_preset(*d_idx, new_preset.clone()) {
                            log::error!("Failed to set scaler preset for display target!");
                        }
                    }
                    GuiEnum::DisplayComposite(state) => {
                        log::debug!("Got composite state update event: {}", state);
                        if let Some(renderer) = emu.dm.get_renderer(*d_idx) {
                            renderer.set_composite(*state);
                        }
                    }
                    GuiEnum::DisplayAspectCorrect(state) => {
                        if let Err(_e) = emu.dm.set_aspect_correction(*d_idx, *state) {
                            log::error!("Failed to set aspect correction state for display target!");
                        }
                    }
                },
                GuiVariableContext::Global => {}
            },
        },
        GuiEvent::LoadVHD(drive_idx, image_idx) => {
            log::debug!("Releasing VHD slot: {}", drive_idx);
            emu.vhd_manager.release_vhd(*drive_idx);

            let mut error_str = None;

            match emu.vhd_manager.load_vhd_file(*drive_idx, *image_idx) {
                Ok(vhd_file) => match VirtualHardDisk::from_file(vhd_file) {
                    Ok(vhd) => {
                        if let Some(hdc) = emu.machine.hdc() {
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
                                        .set_duration(Some(NORMAL_NOTIFICATION_TIME));
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
                emu.gui
                    .toasts()
                    .error(err_str)
                    .set_duration(Some(LONG_NOTIFICATION_TIME));
            }
        }
        GuiEvent::CreateVHD(filename, fmt) => {
            log::info!("Got CreateVHD event: {:?}, {:?}", filename, fmt);

            let vhd_path = emu.hdd_path.join(filename);

            match vhd::create_vhd(
                vhd_path.into_os_string(),
                fmt.max_cylinders,
                fmt.max_heads,
                fmt.max_sectors,
            ) {
                Ok(_) => {
                    // We don't actually do anything with the newly created file
                    // But show a toast notification.
                    emu.gui
                        .toasts()
                        .info(format!("Created VHD: {}", filename.to_string_lossy()))
                        .set_duration(Some(Duration::from_secs(5)));

                    // Rescan resource paths to show new file in list
                    if let Err(e) = emu.vhd_manager.scan_resource(&emu.rm) {
                        log::error!("Error scanning hdd directory: {}", e);
                    };
                }
                Err(err) => {
                    log::error!("Error creating VHD: {}", err);
                    emu.gui
                        .toasts()
                        .error(format!("{}", err))
                        .set_duration(Some(LONG_NOTIFICATION_TIME));
                }
            }
        }
        GuiEvent::RescanMediaFolders => {
            if let Err(e) = emu.floppy_manager.scan_resource(&emu.rm) {
                log::error!("Error scanning floppy directory: {}", e);
            }
            if let Err(e) = emu.vhd_manager.scan_resource(&emu.rm) {
                log::error!("Error scanning hdd directory: {}", e);
            };
        }
        GuiEvent::LoadFloppy(drive_select, item_idx) => {
            log::debug!("Load floppy image: {:?} into drive: {}", item_idx, drive_select);

            if let Some(fdc) = emu.machine.fdc() {
                emu.floppy_manager.get_floppy_name(*item_idx).map(|name| {
                    log::info!("Loading floppy image: {:?} into drive: {}", name, drive_select);

                    match emu.floppy_manager.load_floppy_data(*item_idx, &emu.rm) {
                        Ok(floppy_image) => match fdc.load_image_from(*drive_select, floppy_image) {
                            Ok(()) => {
                                log::info!("Floppy image successfully loaded into virtual drive.");
                                emu.gui
                                    .set_floppy_selection(*drive_select, Some(*item_idx), Some(name.clone().into()));

                                emu.gui
                                    .toasts()
                                    .info(format!("Floppy loaded: {:?}", name.clone()))
                                    .set_duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                            Err(err) => {
                                log::error!("Floppy image failed to load into virtual drive: {}", err);
                                emu.gui
                                    .toasts()
                                    .error(format!("Floppy load failed: {}", err))
                                    .set_duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                        },
                        Err(err) => {
                            log::error!("Failed to load floppy image: {:?} Error: {}", item_idx, err);
                            emu.gui
                                .toasts()
                                .error(format!("Floppy load failed: {}", err))
                                .set_duration(Some(NORMAL_NOTIFICATION_TIME));
                        }
                    }
                });
            }
        }
        /*
        GuiEvent::LoadFloppy(drive_select, filename) => {
            log::debug!("Load floppy image: {:?} into drive: {}", filename, drive_select);

            match emu.floppy_manager.load_floppy_data(&filename) {
                Ok(vec) => {
                    if let Some(fdc) = emu.machine.fdc() {
                        match fdc.load_image_from(*drive_select, vec) {
                            Ok(()) => {
                                log::info!("Floppy image successfully loaded into virtual drive.");
                            }
                            Err(err) => {
                                log::warn!("Floppy image failed to load: {}", err);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to load floppy image: {:?} Error: {}", filename, e);
                    // TODO: Some sort of GUI indication of failure
                    eprintln!("Failed to read floppy image file: {:?} Error: {}", filename, e);
                }
            }
        }
        GuiEvent::SaveFloppy(drive_select, filename) => {
            log::debug!("Save floppy image: {:?} into drive: {}", filename, drive_select);

            if let Some(fdc) = emu.machine.fdc() {
                let floppy = fdc.get_image_data(*drive_select);
                if let Some(floppy_image) = floppy {
                    match emu.floppy_manager.save_floppy_data(floppy_image, &filename) {
                        Ok(()) => {
                            log::info!("Floppy image successfully saved: {:?}", filename);
                        }
                        Err(err) => {
                            log::warn!("Floppy image failed to save: {}", err);
                        }
                    }
                }
            }
        }
        */
        GuiEvent::EjectFloppy(drive_select) => {
            log::info!("Ejecting floppy in drive: {}", drive_select);
            if let Some(fdc) = emu.machine.fdc() {
                fdc.unload_image(*drive_select);
                emu.gui.set_floppy_selection(*drive_select, None, None);
                emu.gui
                    .toasts()
                    .info("Floppy ejected!".to_string())
                    .set_duration(Some(SHORT_NOTIFICATION_TIME));
            }
        }
        GuiEvent::BridgeSerialPort(port_name) => {
            log::info!("Bridging serial port: {}", port_name);
            emu.machine.bridge_serial_port(1, port_name.clone());
        }
        GuiEvent::DumpVRAM => {
            if let Some(video_card) = emu.machine.primary_videocard() {
                let dump_path = emu.rm.get_resource_path("dump").unwrap();
                video_card.dump_mem(&dump_path);
            }
        }
        GuiEvent::DumpCS => {
            emu.rm
                .get_available_filename("dump", "cs_dump", Some("bin"))
                .ok()
                .map(|path| emu.machine.cpu().dump_cs(&path));
        }
        GuiEvent::DumpAllMem => {
            emu.rm
                .get_available_filename("dump", "memdump", Some("bin"))
                .ok()
                .map(|path| emu.machine.bus().dump_mem(&path));
        }
        GuiEvent::EditBreakpoint => {
            // Get breakpoints from GUI
            let (bp_str, bp_mem_str, bp_int_str) = emu.gui.get_breakpoints();

            let mut breakpoints = Vec::new();

            // Push exec breakpoint to list if valid expression
            if let Some(addr) = emu.machine.cpu().eval_address(&bp_str) {
                let flat_addr = u32::from(addr);
                if flat_addr > 0 && flat_addr < 0x100000 {
                    breakpoints.push(BreakPointType::ExecuteFlat(flat_addr));
                }
            };

            // Push mem breakpoint to list if valid expression
            if let Some(addr) = emu.machine.cpu().eval_address(&bp_mem_str) {
                let flat_addr = u32::from(addr);
                if flat_addr > 0 && flat_addr < 0x100000 {
                    breakpoints.push(BreakPointType::MemAccessFlat(flat_addr));
                }
            }

            // Push int breakpoint to list
            if let Ok(iv) = u32::from_str_radix(bp_int_str, 10) {
                if iv < 256 {
                    breakpoints.push(BreakPointType::Interrupt(iv as u8));
                }
            }

            emu.machine.set_breakpoints(breakpoints);
        }
        GuiEvent::MemoryUpdate => {
            // The address bar for the memory viewer was updated. We need to
            // evaluate the expression and set a new row value for the control.
            // The memory contents will be updated in the normal frame update.
            let mem_dump_addr_str = emu.gui.memory_viewer.get_address();
            // Show address 0 if expression evail fails
            let mem_dump_addr: u32 = match emu.machine.cpu().eval_address(&mem_dump_addr_str) {
                Some(i) => {
                    let addr: u32 = i.into();
                    addr & !0x0F
                }
                None => 0,
            };
            emu.gui.memory_viewer.set_row(mem_dump_addr as usize);
        }
        GuiEvent::TokenHover(addr) => {
            // Hovered over a token in a TokenListView.
            let debug = emu.machine.bus_mut().get_memory_debug(*addr);
            emu.gui.memory_viewer.set_hover_text(format!("{}", debug));
        }
        GuiEvent::FlushLogs => {
            // Request to flush trace logs.
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
                        video_card.debug_tick(*ticks);
                    }
                }
            }
        }
        GuiEvent::MachineStateChange(state) => {
            match state {
                MachineState::Off | MachineState::Rebooting => {
                    // Clear the screen if rebooting or turning off

                    emu.dm.for_each_renderer(|renderer, _card_id, buf| {
                        renderer.clear();
                        buf.fill(0);
                    });
                }
                _ => {}
            }
            emu.machine.change_state(*state);
        }
        GuiEvent::TakeScreenshot(dt_idx) => {
            let screenshot_path = emu.rm.get_resource_path("screenshot").unwrap();

            if let Err(err) = emu.dm.save_screenshot(*dt_idx, screenshot_path) {
                log::error!("Failed to save screenshot: {}", err);
                emu.gui
                    .toasts()
                    .error(format!("{}", err))
                    .set_duration(Some(LONG_NOTIFICATION_TIME));
            }
        }
        GuiEvent::CtrlAltDel => {
            emu.machine.ctrl_alt_del();
        }
        GuiEvent::CompositeAdjust(dt_idx, params) => {
            //log::warn!("got composite params: {:?}", params);
            emu.dm.with_renderer(*dt_idx, |renderer| {
                renderer.cga_direct_param_update(params);
            });
        }
        GuiEvent::ScalerAdjust(dt_idx, params) => {
            //log::warn!("Received ScalerAdjust event: {:?}", params);
            if let Err(err) = emu.dm.apply_scaler_params(*dt_idx, params) {
                log::error!("Failed to apply scaler params: {}", err);
            }
        }
        GuiEvent::ZoomChanged(zoom) => {
            emu.dm.for_each_gui(|gui, _window| {
                gui.set_zoom_factor(*zoom);
            });
        }
        _ => {
            log::warn!("Unhandled GUI event: {:?}", discriminant(gui_event));
        }
    }
}
