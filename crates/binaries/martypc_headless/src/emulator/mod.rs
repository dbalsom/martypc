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

/// Definition of [Emulator] struct and related types.
use crate::{JoystickData, MouseData};
use std::ffi::OsString;

use crate::{Counter, KeyboardData};
use anyhow::Error;
use marty_config::ConfigFileParams;
use marty_core::{
    cpu_common::CpuOption,
    machine::{Machine, MachineEvent, MachineState},
    vhd::VirtualHardDisk,
};
use marty_frontend_common::{
    cartridge_manager::CartridgeManager,
    floppy_manager::FloppyManager,
    resource_manager::ResourceManager,
    rom_manager::RomManager,
    timestep_manager::PerfSnapshot,
    vhd_manager::VhdManager,
};

/// Define flags to be used by emulator.
#[derive(Default)]
pub struct EmuFlags {
    pub render_gui: bool,
    pub debug_keyboard: bool,
}

/// Define the main [Emulator] struct for this frontend.
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
    //pub exec_control: Rc<RefCell<ExecutionControl>>,
    pub mouse_data: MouseData,
    pub joy_data: JoystickData,
    pub kb_data: KeyboardData,
    pub stat_counter: Counter,
    pub floppy_manager: FloppyManager,
    pub vhd_manager: VhdManager,
    pub cart_manager: CartridgeManager,
    pub flags: EmuFlags,
    pub perf: PerfSnapshot,
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

        // Do PIT phase offset option
        self.machine
            .pit_adjust(self.config.machine.pit_phase.unwrap_or(0) & 0x03);

        self.machine.set_cpu_option(CpuOption::OffRailsDetection(
            self.config.machine.cpu.off_rails_detection.unwrap_or(false),
        ));
        self.machine.set_cpu_option(CpuOption::EnableServiceInterrupt(
            self.config.machine.cpu.service_interrupt.unwrap_or(false),
        ));

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

        self.machine.set_cpu_option(CpuOption::EnableWaitStates(
            self.config.machine.cpu.wait_states.unwrap_or(true),
        ));

        self.machine.set_cpu_option(CpuOption::InstructionHistory(
            self.config.machine.cpu.instruction_history.unwrap_or(false),
        ));

        // Debug mode on?
        if self.config.emulator.debug_mode {
            self.machine.set_cpu_option(CpuOption::InstructionHistory(true));
            // Disable autostart
            self.config.emulator.cpu_autostart = false;
        }

        #[cfg(debug_assertions)]
        if self.config.emulator.debug_warn {
            log::warn!("Debug build. Performance may be affected.");
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
                Ok((vhd_file, _vhd_idx)) => match VirtualHardDisk::parse(Box::new(vhd_file), false) {
                    Ok(vhd) => {
                        if let Some(hdc) = self.machine.hdc_mut() {
                            match hdc.set_vhd(config_drive_idx, vhd) {
                                Ok(_) => {
                                    log::info!(
                                        "VHD image {:?} successfully loaded into virtual drive: {}",
                                        vhd_os_name,
                                        config_drive_idx
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
            config_drive_idx += 1;
        }
        Ok(())
    }

    pub fn start(&mut self) {
        //self.machine.play_sound_buffer();
    }
}
