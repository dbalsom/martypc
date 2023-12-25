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

    emulator::mod.rs

    MartyPC Desktop front-end Emulator struct and implementation.
*/

use display_manager_wgpu::DisplayManager;
use std::{cell::RefCell, ffi::OsString, path::PathBuf, rc::Rc};

use crate::{Counter, KeyboardData, MouseData};
use anyhow::Error;
use config_toml_bpaf::ConfigFileParams;
use display_manager_wgpu::WgpuDisplayManager;
use frontend_common::{
    display_scaler::SCALER_MODES,
    floppy_manager::FloppyManager,
    resource_manager::ResourceManager,
    rom_manager::RomManager,
};
use marty_core::{
    cpu_common::CpuOption,
    machine::{ExecutionControl, Machine, MachineState},
    vhd::VirtualHardDisk,
    vhd_manager::VHDManager,
};
use marty_egui::{state::GuiState, GuiBoolean, GuiWindow};
use videocard_renderer::AspectCorrectionMode;

/// Define flags to be used by emulator.
pub struct EmuFlags {
    pub render_gui: bool,
    pub debug_keyboard: bool,
}

/// Define the main Emulator struct for this frontend.
/// All the items that the winit event loop closure needs should be set here so that
/// we can call an event handler in a different file.
/// All members are public so that a reference to this struct can be passed around as 'god' state.
pub struct Emulator {
    pub rm: ResourceManager,
    pub dm: WgpuDisplayManager,
    pub romm: RomManager,
    pub config: ConfigFileParams,
    pub machine: Machine,
    pub exec_control: Rc<RefCell<ExecutionControl>>,
    pub mouse_data: MouseData,
    pub kb_data: KeyboardData,
    pub stat_counter: Counter,
    pub gui: GuiState,
    //context: &'a mut GuiRenderContext,
    pub floppy_manager: FloppyManager,
    pub vhd_manager: VHDManager,
    pub hdd_path: PathBuf,
    //pub floppy_path: PathBuf,
    pub flags: EmuFlags,
}

impl Emulator {
    #[allow(dead_code)]
    pub fn validate_config(&self) -> Result<(), Error> {
        Ok(())
    }

    /// Apply settings from configuration to machine, gui, and display manager state.
    /// Should only be called after such are constructed.
    pub fn apply_config(&mut self) -> Result<(), Error> {
        log::debug!("Applying configuration to emulator state...");

        // Set the inital power-on state.
        if self.config.emulator.auto_poweron {
            self.machine.change_state(MachineState::On);
        }
        else {
            self.machine.change_state(MachineState::Off);
        }

        self.flags.debug_keyboard = self.config.emulator.debug_keyboard;

        // Do PIT phase offset option
        self.machine
            .pit_adjust(self.config.machine.pit_phase.unwrap_or(0) & 0x03);

        // Set options from config. We do this now so that we can set the same state for both GUI and machine

        // TODO: Add GUI for these two options?
        self.machine.set_cpu_option(CpuOption::OffRailsDetection(
            self.config.machine.cpu.off_rails_detection.unwrap_or(false),
        ));
        self.machine.set_cpu_option(CpuOption::EnableServiceInterrupt(
            self.config.machine.cpu.service_interrupt.unwrap_or(false),
        ));

        // TODO: Reenable these
        //gui.set_option(GuiBoolean::EnableSnow, config.machine.cga_snow.unwrap_or(false));
        //machine.set_video_option(VideoOption::EnableSnow(config.machine.cga_snow.unwrap_or(false)));
        //gui.set_option(GuiBoolean::CorrectAspect, config.emulator.scaler_aspect_correction);

        //if config.emulator.scaler_aspect_correction {
        // Default to hardware aspect correction.
        //video.set_aspect_mode(AspectCorrectionMode::Hardware);

        // Load program binary if one was specified in config options
        if let Some(prog_bin) = self.config.emulator.run_bin.clone() {
            if let Some(prog_seg) = self.config.emulator.run_bin_seg {
                if let Some(prog_ofs) = self.config.emulator.run_bin_ofs {
                    let prog_vec = match std::fs::read(prog_bin.clone()) {
                        Ok(vec) => vec,
                        Err(e) => {
                            eprintln!("Error opening filename {:?}: {}", prog_bin, e);
                            std::process::exit(1);
                        }
                    };

                    if let Err(_) = self.machine.load_program(&prog_vec, prog_seg, prog_ofs) {
                        eprintln!(
                            "Error loading program into memory at {:04X}:{:04X}.",
                            prog_seg, prog_ofs
                        );
                        std::process::exit(1);
                    };
                }
                else {
                    eprintln!("Must specify program load offset.");
                    std::process::exit(1);
                }
            }
            else {
                eprintln!("Must specify program load segment.");
                std::process::exit(1);
            }
        }

        self.gui.set_option(
            GuiBoolean::CpuEnableWaitStates,
            self.config.machine.cpu.wait_states.unwrap_or(true),
        );
        self.machine.set_cpu_option(CpuOption::EnableWaitStates(
            self.config.machine.cpu.wait_states.unwrap_or(true),
        ));

        self.gui.set_option(
            GuiBoolean::CpuInstructionHistory,
            self.config.machine.cpu.instruction_history.unwrap_or(false),
        );

        self.machine.set_cpu_option(CpuOption::InstructionHistory(
            self.config.machine.cpu.instruction_history.unwrap_or(false),
        ));

        self.gui
            .set_option(GuiBoolean::CpuTraceLoggingEnabled, self.config.machine.cpu.trace_on);
        self.machine
            .set_cpu_option(CpuOption::TraceLoggingEnabled(self.config.machine.cpu.trace_on));

        self.gui.set_option(GuiBoolean::TurboButton, self.config.machine.turbo);

        self.gui.set_scaler_presets(&self.config.emulator.scaler_preset);

        // Populate the list of display targets for each display.
        self.dm.for_each_target(|dtc, dt_idx| {
            if let Some(card_id) = &dtc.get_card_id() {
                if let Some(video_card) = self.machine.bus().video(card_id) {
                    self.gui
                        .set_display_apertures(dt_idx, video_card.list_display_apertures());
                }
            }
        });

        // Populate the list of scaler modes, defined by display_scaler trait module
        self.gui.set_scaler_modes(SCALER_MODES.to_vec());

        // Disable warpspeed feature if 'devtools' flag not on.
        #[cfg(not(feature = "devtools"))]
        {
            self.config.emulator.warpspeed = false;
        }

        // Debug mode on?
        if self.config.emulator.debug_mode {
            // Open default debug windows
            self.gui.set_window_open(GuiWindow::CpuControl, true);
            self.gui.set_window_open(GuiWindow::DisassemblyViewer, true);
            self.gui.set_window_open(GuiWindow::CpuStateViewer, true);

            // Override CpuInstructionHistory
            self.gui.set_option(GuiBoolean::CpuInstructionHistory, true);
            self.machine.set_cpu_option(CpuOption::InstructionHistory(true));

            // Disable autostart
            self.config.emulator.cpu_autostart = false;
        }

        #[cfg(debug_assertions)]
        if self.config.emulator.debug_warn {
            // User compiled MartyPC in debug mode, let them know...
            self.gui.show_warning(
                &"MartyPC has been compiled in debug mode and will be extremely slow.\n \
                    To compile in release mode, use 'cargo build -r'\n \
                    To disable this error, set debug_warn=false in martypc.toml."
                    .to_string(),
            );
        }

        Ok(())
    }

    pub fn mount_vhds(&mut self) -> Result<(), Error> {
        let mut vhd_names: Vec<Option<String>> = Vec::new();

        for vhd in self.config.emulator.media.vhd.as_ref().unwrap_or(&Vec::new()) {
            vhd_names.push(Some(vhd.filename.clone()));
        }

        let mut vhd_idx: usize = 0;
        for vhd_name in vhd_names.into_iter().filter_map(|x| x) {
            let vhd_os_name: OsString = vhd_name.into();
            match self.vhd_manager.load_vhd_file(vhd_idx, &vhd_os_name) {
                Ok(vhd_file) => match VirtualHardDisk::from_file(vhd_file) {
                    Ok(vhd) => {
                        if let Some(hdc) = self.machine.hdc() {
                            match hdc.set_vhd(vhd_idx, vhd) {
                                Ok(_) => {
                                    log::info!(
                                        "VHD image {:?} successfully loaded into virtual drive: {}",
                                        vhd_os_name,
                                        vhd_idx
                                    );
                                }
                                Err(err) => {
                                    log::error!("Error mounting VHD: {}", err);
                                }
                            }
                        }
                        else {
                            log::error!("Couldn't load VHD: No Hard Disk Controller present!");
                        }
                    }
                    Err(err) => {
                        log::error!("Error loading VHD: {}", err);
                    }
                },
                Err(err) => {
                    log::error!("Failed to load VHD image {:?}: {}", vhd_os_name, err);
                }
            }
            vhd_idx += 1;
        }
        Ok(())
    }

    pub fn post_dm_build_init(&mut self) {
        // Set all DisplayTargets to hardware aspect correction
        self.dm.for_each_target(|dtc, _idx| {
            dtc.set_aspect_mode(AspectCorrectionMode::Hardware);
        });

        let mut vid_list = Vec::new();
        // Get a list of all cards as we can't nest dm closures
        self.dm.for_each_card(|vid| {
            vid_list.push(vid.clone());
        });

        if vid_list.len() != self.config.machine.videocard.as_ref().unwrap_or(&Vec::new()).len() {
            log::error!("Number of videocards installed does not match number of cards in config!");
        }

        for vid in vid_list.iter() {
            if let Some(card) = self.machine.bus().video(vid) {
                let extents = card.get_display_extents();

                //assert_eq!(extents.double_scan, true);
                if let Err(_e) = self.dm.on_card_resized(vid, extents) {
                    log::error!("Failed to resize videocard!");
                }
            }
        }

        // Build list of cards to set in UI.
        let mut card_strs = Vec::new();
        for vid in vid_list.iter() {
            let card_str = format!("Card: {} ({:?})", vid.idx, vid.vtype);
            card_strs.push(card_str);
        }
        self.gui.set_card_list(card_strs);

        /*
            if let Some(card) = machine.videocard() {
                if let RenderMode::Direct = card.get_render_mode() {
                    if let Some(render_window) = window_manager.get_render_window(card.get_video_type()) {
                        let extents = card.get_display_extents();
                        let (aper_x, mut aper_y) = card.get_display_aperture();
                        assert!(aper_x != 0 && aper_y != 0);

                        if extents.double_scan {
                            video.set_double_scan(true);
                            aper_y *= 2;
                        }
                        else {
                            video.set_double_scan(false);
                        }

                        let aspect_ratio = if config.emulator.scaler_aspect_correction {
                            Some(marty_render::AspectRatio { h: 4, v: 3 })
                        }
                        else {
                            None
                        };

                        video.set_aspect_ratio(aspect_ratio);

                        let (aper_correct_x, aper_correct_y) = {
                            let dim = video.get_display_dimensions();
                            (dim.w, dim.h)
                        };

                        let mut double_res = false;

                        // Get the current monitor resolution.
                        if let Some(monitor) = render_window.window.current_monitor() {
                            let monitor_size = monitor.size();
                            let dip_scale = monitor.scale_factor();

                            log::debug!(
                                "Current monitor resolution: {}x{} scale factor: {}",
                                monitor_size.width,
                                monitor_size.height,
                                dip_scale
                            );

                            // Take into account DPI scaling for window-fit.
                            let scaled_width = ((aper_correct_x * 2) as f64 * dip_scale) as u32;
                            let scaled_height = ((aper_correct_y * 2) as f64 * dip_scale) as u32;
                            log::debug!(
                                "Target resolution after aspect correction and DPI scaling: {}x{}",
                                scaled_width,
                                scaled_height
                            );

                            if (scaled_width <= monitor_size.width) && (scaled_height <= monitor_size.height) {
                                // Monitor is large enough to double the display window
                                double_res = true;
                            }
                        }

                        let window_resize_w = if double_res { aper_correct_x * 2 } else { aper_correct_x };
                        let window_resize_h = if double_res { aper_correct_y * 2 } else { aper_correct_y };

                        log::debug!("Resizing window to {}x{}", window_resize_w, window_resize_h);
                        //resize_h = if card.get_scanline_double() { resize_h * 2 } else { resize_h };

                        render_window
                            .window
                            .set_inner_size(winit::dpi::LogicalSize::new(window_resize_w, window_resize_h));

                        log::debug!("Resizing marty_render buffer to {}x{}", aper_x, aper_y);

                        video.resize((aper_x, aper_y).into());

                        /*
                        let pixel_res = video.get_display_dimensions();

                        if (pixel_res.w > 0) && (pixel_res.h > 0) {
                            log::debug!("Resizing pixel buffer to {}x{}", pixel_res.w, pixel_res.h);
                            pixels.resize_buffer(pixel_res.w, pixel_res.h).expect("Failed to resize Pixels buffer.");
                        }
                        */

                        //VideoRenderer::set_alpha(pixels.frame_mut(), pixel_res.w, pixel_res.h, 255);

                        // Recalculate sampling parameters.
                        //resample_context.precalc(aper_x, aper_y, aper_correct_x, aper_correct_y);

                        // Update internal state and request a redraw
                        render_window.window.request_redraw();
                    }
                }
            }
        }

         */

        // Set floppy drives.
        self.gui.set_floppy_drives(self.machine.bus().floppy_drive_ct());
    }
    pub fn start(&mut self) {
        self.machine.play_sound_buffer();
    }
}
