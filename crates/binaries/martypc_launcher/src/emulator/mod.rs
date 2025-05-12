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

    emulator::mod.rs

    MartyPC Desktop front-end Emulator struct and implementation.
*/
pub mod joystick_state;
pub mod keyboard_state;
pub mod mouse_state;

use crate::gui::GuiState;
use anyhow::Error;

#[cfg(target_arch = "wasm32")]
use crate::wasm::file_open;
use crate::{
    counter::Counter,
    emulator::{joystick_state::JoystickData, keyboard_state::KeyboardData, mouse_state::MouseData},
    //floppy::load_floppy::handle_load_floppy,
    input::HotkeyManager,
    sound::SoundInterface,
};
use fluxfox::DiskImage;
use marty_config::ConfigFileParams;
use marty_core::{
    cpu_common::{Cpu, CpuOption},
    machine::{ExecutionControl, Machine, MachineEvent, MachineState},
    vhd::{VhdIO, VirtualHardDisk},
};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    rc::Rc,
    sync::Arc,
};

use marty_frontend_common::{
    cartridge_manager::CartridgeManager,
    floppy_manager::FloppyManager,
    marty_common::types::ui::MouseCaptureMode,
    resource_manager::ResourceManager,
    rom_manager::RomManager,
    thread_events::{FileOpenContext, FileSelectionContext, FrontendThreadEvent},
    timestep_manager::PerfSnapshot,
    types::floppy::FloppyImageSource,
    vhd_manager::VhdManager,
};

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
    pub receiver: crossbeam_channel::Receiver<FrontendThreadEvent<Arc<DiskImage>>>,
    pub sender: crossbeam_channel::Sender<FrontendThreadEvent<Arc<DiskImage>>>,
}

impl Emulator {
    #[allow(dead_code)]
    pub fn validate_config(&self) -> Result<(), Error> {
        Ok(())
    }

    /// Apply settings from configuration to machine, gui, and display manager state.
    /// Should only be called after such are constructed.
    pub fn apply_config(&mut self) -> Result<(), Error> {
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

    // /// Insert floppy disks into floppy drives.
    // pub fn insert_floppies(
    //     &mut self,
    //     sender: crossbeam_channel::Sender<FrontendThreadEvent<Arc<DiskImage>>>,
    // ) -> Result<(), Error> {
    //     let floppy_max = self.machine.bus().floppy_drive_ct();
    //     let mut image_names: Vec<Option<String>> = vec![None; floppy_max];
    //
    //     for (drive_i, vhd) in self
    //         .config
    //         .emulator
    //         .media
    //         .floppy
    //         .as_ref()
    //         .unwrap_or(&Vec::new())
    //         .iter()
    //         .enumerate()
    //     {
    //         if drive_i < floppy_max {
    //             image_names[drive_i] = Some(vhd.filename.clone());
    //         }
    //     }
    //
    //     #[cfg(target_arch = "wasm32")]
    //     for (idx, image_name) in image_names.into_iter().filter_map(|x| x).enumerate() {
    //         let floppy_name: OsString = image_name.into();
    //         let floppy_path = self
    //             .rm
    //             .resolve_path_from_filename("floppy", std::path::Path::new(&floppy_name))?;
    //
    //         let fsc = FileSelectionContext::Path(floppy_path);
    //         let context = FileOpenContext::FloppyDiskImage { drive_select: idx, fsc };
    //         file_open::open_file(context, sender.clone());
    //     }
    //     #[cfg(not(target_arch = "wasm32"))]
    //     for (idx, image_name) in image_names.into_iter().filter_map(|x| x).enumerate() {
    //         use std::path::PathBuf;
    //         let floppy_path = PathBuf::from(image_name);
    //         //handle_load_floppy(self, idx, FileSelectionContext::Path(floppy_path.clone()));
    //         match self
    //             .floppy_manager
    //             .load_floppy_by_path(floppy_path.clone(), &mut self.rm)
    //         {
    //             Ok(fis) => match fis {
    //                 FloppyImageSource::DiskImage(floppy_file, path) => {
    //                     if let Some(fdc) = &mut self.machine.bus_mut().fdc_mut() {
    //                         match fdc.load_image_from(idx, floppy_file, Some(&path.clone()), false) {
    //                             Ok(_) => {
    //                                 log::info!(
    //                                     "Floppy disk image {:?} successfully loaded into drive: {}",
    //                                     path.display(),
    //                                     idx
    //                                 );
    //                             }
    //                             Err(err) => {
    //                                 log::error!(
    //                                     "Error inserting floppy disk image {:?} into drive {}: {}",
    //                                     path.display(),
    //                                     idx,
    //                                     err
    //                                 );
    //                             }
    //                         }
    //                     }
    //                     else {
    //                         log::error!("Couldn't load floppy disk: No Floppy Disk Controller present!");
    //                     }
    //                 }
    //                 _ => {
    //                     log::error!(
    //                         "Unsupported image source for auto-loading floppy disk: {:?}",
    //                         floppy_path.display()
    //                     );
    //                 }
    //             },
    //             Err(err) => {
    //                 log::error!("Failed to load floppy disk image {}: {}", floppy_path.display(), err);
    //             }
    //         }
    //     }
    //     Ok(())
    // }

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

        let mut drive_idx: usize = 0;
        for vhd_name in vhd_names.into_iter().filter_map(|x| x) {
            let vhd_os_name: OsString = vhd_name.into();

            #[cfg(not(target_arch = "wasm32"))]
            match self.vhd_manager.load_vhd_file_by_name(drive_idx, &vhd_os_name) {
                Ok((vhd_file, vhd_idx)) => {
                    self.load_vhd(Box::new(vhd_file), drive_idx, &vhd_os_name, Some(vhd_idx))?;
                }
                Err(err) => {
                    log::error!("Failed to load VHD image {:?}: {}", vhd_os_name, err);
                }
            }
            #[cfg(target_arch = "wasm32")]
            match self
                .vhd_manager
                .load_vhd_file_by_name(&mut self.rm, drive_idx, &vhd_os_name)
            {
                Ok(vhd_data) => {
                    self.load_vhd(Box::new(std::io::Cursor::new(vhd_data)), drive_idx, &vhd_os_name, None)?;
                }
                Err(err) => {
                    log::error!("Failed to load VHD image {:?}: {}", vhd_os_name, err);
                }
            }
            drive_idx += 1;
        }
        Ok(())
    }

    pub fn load_vhd(
        &mut self,
        vhd_file: Box<dyn VhdIO>,
        drive_idx: usize,
        vhd_os_name: &OsStr,
        vhd_idx: Option<usize>,
    ) -> Result<(), Error> {
        Ok(())
    }

    pub fn post_dm_build_init(&mut self) {}

    pub fn start(&mut self) {}
}
