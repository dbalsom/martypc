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
pub mod joystick_state;
pub mod keyboard_state;
pub mod mouse_state;

//use display_manager_wgpu::DisplayManager;
//use display_manager_wgpu::WgpuDisplayManager;

use display_manager_eframe::EFrameDisplayManager;
use std::{cell::RefCell, ffi::OsString, rc::Rc};

use anyhow::Error;
use config_toml_bpaf::ConfigFileParams;

use crate::{
    counter::Counter,
    emulator::{joystick_state::JoystickData, keyboard_state::KeyboardData, mouse_state::MouseData},
    event_loop::thread_events::FrontendThreadEvent,
    input::HotkeyManager,
    sound::sound_player::SoundInterface,
};
use frontend_common::{
    cartridge_manager::CartridgeManager,
    display_scaler::SCALER_MODES,
    floppy_manager::FloppyManager,
    resource_manager::ResourceManager,
    rom_manager::RomManager,
    timestep_manager::PerfSnapshot,
    vhd_manager::VhdManager,
};
use marty_core::{
    cpu_common::{Cpu, CpuOption},
    machine::{ExecutionControl, Machine, MachineEvent, MachineState},
    vhd::VirtualHardDisk,
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
    pub romm: RomManager,
    pub romsets: Vec<String>,
    pub config: ConfigFileParams,
    pub machine: Machine,
    pub machine_events: Vec<MachineEvent>,
    pub exec_control: Rc<RefCell<ExecutionControl>>,
    pub mouse_data: MouseData,
    pub joy_data: JoystickData,
    pub kb_data: KeyboardData,
    pub stat_counter: Counter,
    pub gui: GuiState,
    pub floppy_manager: FloppyManager,
    pub vhd_manager: VhdManager,
    pub cart_manager: CartridgeManager,
    pub flags: EmuFlags,
    pub perf: PerfSnapshot,
    pub hkm: HotkeyManager,
    pub si: Option<SoundInterface>,
    pub receiver: crossbeam_channel::Receiver<FrontendThreadEvent>,
    pub sender: crossbeam_channel::Sender<FrontendThreadEvent>,
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

        // Set the initial power-on state.
        if self.config.emulator.auto_poweron {
            self.machine.change_state(MachineState::On);
        }
        else {
            self.machine.change_state(MachineState::Off);
        }

        self.flags.debug_keyboard = self.config.emulator.input.debug_keyboard;

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

        // TODO: Re-enable these
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
                    if let Some(vreset_seg) = self.config.emulator.vreset_bin_seg {
                        if let Some(vreset_ofs) = self.config.emulator.vreset_bin_ofs {
                            let prog_vec = match std::fs::read(prog_bin.clone()) {
                                Ok(vec) => vec,
                                Err(e) => {
                                    eprintln!("Error opening filename {:?}: {}", prog_bin, e);
                                    std::process::exit(1);
                                }
                            };

                            if let Err(_) = self
                                .machine
                                .load_program(&prog_vec, prog_seg, prog_ofs, vreset_seg, vreset_ofs)
                            {
                                eprintln!(
                                    "Error loading program into memory at {:04X}:{:04X}.",
                                    prog_seg, prog_ofs
                                );
                                std::process::exit(1);
                            };
                        }
                        else {
                            eprintln!("Must specify program start offset.");
                            std::process::exit(1);
                        }
                    }
                    else {
                        eprintln!("Must specify program start segment.");
                        std::process::exit(1);
                    }
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

        // // Populate the list of display targets for each display.
        // self.dm.for_each_target(|dtc, dt_idx| {
        //     if let Some(card_id) = &dtc.get_card_id() {
        //         if let Some(video_card) = self.machine.bus().video(card_id) {
        //             self.gui
        //                 .set_display_apertures(dt_idx, video_card.list_display_apertures());
        //         }
        //     }
        // });

        // Populate the list of scaler modes, defined by display_scaler trait module
        self.gui.set_scaler_modes(SCALER_MODES.to_vec());

        // Disable warpspeed feature if 'devtools' flag not on.
        #[cfg(not(feature = "devtools"))]
        {
            self.config.emulator.warpspeed = false;
        }

        // Set up cycle trace viewer
        self.gui
            .cycle_trace_viewer
            .set_mode(self.config.machine.cpu.trace_mode.unwrap_or_default());
        self.gui
            .cycle_trace_viewer
            .set_header(self.machine.cpu().cycle_table_header());

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

    /// Get a list of VHD images specified in the machine configuration.
    /// Returns a vector of Option<String> where Some(String) is the filename of the VHD image, and None is an empty
    /// hard drive slot.
    pub fn get_vhds_from_machine(&self) -> Vec<Option<String>> {
        let mut vhd_names: Vec<Option<String>> = Vec::new();

        let machine_config = self.machine.config();

        if let Some(controller) = machine_config.hdc.as_ref() {
            for drive in controller.drive.as_ref().unwrap_or(&Vec::new()) {
                if let Some(vhd) = drive.vhd.as_ref() {
                    vhd_names.push(Some(vhd.clone()));
                }
                else {
                    vhd_names.push(None);
                }
            }
        }

        vhd_names
    }

    /// Mount VHD images into hard drive devices.
    /// VHD images can be specified either in the machine configuration, or in the main configuration.
    /// Images specified in the main configuration will override images specified in a machine configuration.
    /// Images are mounted in the order they are specified, starting with the first hard disk controller, and first
    /// hard disk, and continuing until all images are mounted, or there are no more hard disks.
    pub fn mount_vhds(&mut self) -> Result<(), Error> {
        // First, retrieve the list of VHD images specified in the machine configuration.
        let mut vhd_names: Vec<Option<String>> = self.get_vhds_from_machine();
        let machine_max = vhd_names.len();

        for (drive_i, vhd) in self
            .config
            .emulator
            .media
            .vhd
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .enumerate()
        {
            if drive_i >= machine_max {
                // Add new drive
                vhd_names.push(Some(vhd.filename.clone()));
            }
            else {
                // Replace existing drive
                vhd_names[drive_i] = Some(vhd.filename.clone());
            }
        }

        let mut config_drive_idx: usize = 0;
        for vhd_name in vhd_names.into_iter().filter_map(|x| x) {
            let vhd_os_name: OsString = vhd_name.into();
            match self.vhd_manager.load_vhd_file_by_name(config_drive_idx, &vhd_os_name) {
                Ok((vhd_file, vhd_idx)) => match VirtualHardDisk::from_file(vhd_file) {
                    Ok(vhd) => {
                        if let Some(hdc) = self.machine.hdc() {
                            match hdc.set_vhd(config_drive_idx, vhd) {
                                Ok(_) => {
                                    log::info!(
                                        "VHD image {:?} successfully loaded into virtual drive: {}",
                                        vhd_os_name,
                                        config_drive_idx
                                    );

                                    if let Some(selection) = self.vhd_manager.get_vhd_path(vhd_idx) {
                                        self.gui
                                            .set_hdd_selection(config_drive_idx, Some(vhd_idx), Some(selection));
                                    }
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
            config_drive_idx += 1;
        }
        Ok(())
    }

    pub fn post_dm_build_init(&mut self) {
        // // Set all DisplayTargets to hardware aspect correction
        // self.dm.for_each_target(|dtc, _idx| {
        //     dtc.set_aspect_mode(AspectCorrectionMode::Hardware);
        // });
        //
        // let mut vid_list = Vec::new();
        // // Get a list of all cards as we can't nest dm closures
        // self.dm.for_each_card(|vid| {
        //     vid_list.push(vid.clone());
        // });
        //
        // for vid in vid_list.iter() {
        //     if let Some(card) = self.machine.bus().video(vid) {
        //         let extents = card.get_display_extents();
        //
        //         //assert_eq!(extents.double_scan, true);
        //         if let Err(_e) = self.dm.on_card_resized(vid, extents) {
        //             log::error!("Failed to resize videocard!");
        //         }
        //     }
        // }

        // // Sort vid_list by index
        // vid_list.sort_by(|a, b| a.idx.cmp(&b.idx));
        //
        // // Build list of cards to set in UI.
        // let mut card_strs = Vec::new();
        // for vid in vid_list.iter() {
        //     let card_str = format!("Card: {} ({:?})", vid.idx, vid.vtype);
        //     card_strs.push(card_str);
        // }

        // Set list of video cards
        //self.gui.set_card_list(card_strs);

        // Set list of virtual serial ports
        self.gui.set_serial_ports(self.machine.bus().enumerate_serial_ports());

        // Set floppy drives.
        let drive_ct = self.machine.bus().floppy_drive_ct();
        let mut drive_types = Vec::new();
        for i in 0..drive_ct {
            if let Some(fdc) = self.machine.bus().fdc() {
                drive_types.push(fdc.drive(i).get_type());
            }
        }
        self.gui.set_floppy_drives(drive_types);

        // Set default floppy path. This is used to set the default path for Save As dialogs.
        self.gui.set_paths(self.rm.get_resource_path("floppy").unwrap());

        // Set hard drives.
        self.gui.set_hdds(self.machine.bus().hdd_ct());

        // Set cartridge slots
        self.gui.set_cart_slots(self.machine.bus().cart_ct());

        // Set autofloppy paths
        self.gui
            .set_autofloppy_paths(self.floppy_manager.get_autofloppy_paths());

        // Request initial events from GUI.
        self.gui.initialize();
    }

    pub fn start(&mut self) {
        //self.machine.play_sound_buffer();
    }
}
