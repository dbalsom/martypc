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

    machine_config.rs

    This module manages machine configuration definitions.

*/

use crate::machine_types::{
    EmsType,
    FdcType,
    FloppyDriveType,
    HardDiskControllerType,
    HardDriveFormat,
    MachineType,
    SerialControllerType,
    SerialMouseType,
};
use anyhow::{anyhow, Error};
use lazy_static::lazy_static;
use std::collections::HashMap;

use crate::{
    bus::ClockFactor,
    cpu_common::CpuType,
    device_traits::videocard::VideoType,
    devices::{keyboard::KeyboardType, pit::PitType},
    tracelogger::TraceLogger,
};

use crate::{device_traits::videocard::VideoCardSubType, devices::a0::A0Type};
use serde_derive::Deserialize;

// Clock derivation from reenigne
// See https://www.vogons.org/viewtopic.php?t=55049
pub const IBM_PC_SYSTEM_CLOCK: f64 = 157.5 / 11.0;
pub const PIT_DIVISOR: u32 = 12;

pub const GAME_PORT_DEFAULT_IO: u16 = 0x201;

/// This enum is intended to represent any specific add-on device type
/// that the bus needs to know about.
pub enum DeviceType {
    Keyboard(KeyboardType),
    VideoCard(VideoType),
}

// placeholder for future feature
#[allow(dead_code)]
pub struct MmioSpec {
    base_addr: u32,
    size: u32,
}

// placeholder for future feature
#[allow(dead_code)]
pub struct DeviceSpec {
    dtype: DeviceType,      // Type of device.
    debug: bool,            // Whether device should enable debug functionality.
    tracelog: TraceLogger,  // Tracelogger for device to use.
    mmio: Option<MmioSpec>, // Whether device has a mmio mapping.
    io: bool,               // Whether device registers IO ports / requires IO dispatch.
    hotplug: bool,          // Whether device can be added/removed while machine is running.
}

#[derive(Copy, Clone, Debug)]
pub enum KbControllerType {
    Ppi,
    PCJr,
    At,
}

#[derive(Copy, Clone, Debug)]
pub enum PicType {
    Single,
    Chained,
}

#[derive(Copy, Clone, Debug)]
pub enum DmaType {
    Single,
    Chained,
}

#[derive(Copy, Clone, Debug)]
pub enum BusType {
    Isa8,
    Isa16,
}

// Machine Configuration file types

#[derive(Clone, Debug, Deserialize)]
pub struct CpuConfig {
    pub upgrade_type: Option<CpuType>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MemoryConfig {
    pub conventional: ConventionalMemoryConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EmsMemoryConfig {
    pub ems_type: EmsType,
    pub window: u32,
    pub io_base: u16,
    pub size: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConventionalMemoryConfig {
    pub size: u32,
    pub wait_states: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KeyboardConfig {
    #[serde(rename = "type")]
    pub kb_type: KeyboardType,
    pub layout: String,
    #[serde(default)]
    pub typematic: bool,
    pub typematic_delay: Option<f64>,
    pub typematic_rate: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SerialMouseConfig {
    #[serde(rename = "type")]
    pub mouse_type: SerialMouseType,
    pub port: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GamePortConfig {
    pub io_base: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VideoCardConfig {
    #[serde(rename = "type")]
    pub video_type:    VideoType,
    #[serde(rename = "subtype")]
    pub video_subtype: Option<VideoCardSubType>,
    pub dip_switch:    Option<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SerialPortConfig {
    pub io_base: u32,
    pub irq: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SerialControllerConfig {
    #[serde(rename = "type")]
    pub sc_type: SerialControllerType,
    pub port:    Vec<SerialPortConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FloppyControllerConfig {
    #[serde(rename = "type")]
    pub fdc_type: FdcType,
    pub drive:    Vec<FloppyDriveConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FloppyDriveConfig {
    #[serde(rename = "type")]
    pub fd_type: FloppyDriveType,
    pub image:   Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HardDriveControllerConfig {
    #[serde(rename = "type")]
    pub hdc_type: HardDiskControllerType,
    pub drive:    Option<Vec<HardDriveConfig>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HardDriveConfig {
    #[serde(rename = "type")]
    pub hd_type: Option<u32>,
    pub format: Option<HardDriveFormat>,
    pub vhd: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FloppyImage {
    pub image: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HardDriveImage {
    pub image: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MediaConfig {
    pub floppy: Option<Vec<FloppyImage>>,
    pub hdd:    Option<Vec<HardDriveImage>>,
}

#[derive(Clone, Debug)]
pub struct MachineConfiguration {
    pub speaker: bool,
    pub ppi_turbo: Option<bool>,
    pub machine_type: MachineType,
    pub cpu: Option<CpuConfig>,
    pub memory: MemoryConfig,
    pub ems: Option<EmsMemoryConfig>,
    pub keyboard: Option<KeyboardConfig>,
    pub serial_mouse: Option<SerialMouseConfig>,
    pub video: Vec<VideoCardConfig>,
    pub serial: Vec<SerialControllerConfig>,
    pub game_port: Option<GamePortConfig>,
    pub fdc: Option<FloppyControllerConfig>,
    pub hdc: Option<HardDriveControllerConfig>,
    pub media: Option<MediaConfig>,
}

lazy_static! {
    /// This hashmap defines ROM feature requirements for the base machine types.
    /// The key is the machine type, and the value is a vector of ROM features.
    static ref BASE_ROM_FEATURES: HashMap<MachineType, Vec<&'static str>> = {
        let mut m = HashMap::new();
        m.insert(MachineType::Default, vec![]);
        m.insert(MachineType::Ibm5150v64K, vec!["ibm5150v64k"]);
        m.insert(MachineType::Ibm5150v256K, vec!["ibm5150v256k"]);
        m.insert(MachineType::Ibm5160, vec!["ibm5160"]);
        m.insert(MachineType::IbmPCJr, vec!["ibm_pcjr"]);
        m.insert(MachineType::Tandy1000, vec!["tandy1000"]);
        m
    };

    /// This hashmap defines optional ROM feature requirements for the base machine types.
    /// Missing optional features will not stop machine creation.
    static ref OPTIONAL_ROM_FEATURES: HashMap<MachineType, Vec<&'static str>> = {
        let mut m = HashMap::new();
        m.insert(MachineType::Default, vec![]);
        m.insert(MachineType::Ibm5150v64K, vec!["ibm_basic"]);
        m.insert(MachineType::Ibm5150v256K, vec!["ibm_basic"]);
        m.insert(MachineType::Ibm5160, vec!["ibm_basic"]);
        m.insert(MachineType::IbmPCJr, vec!["pcjr_cartridge"]);
        m.insert(MachineType::Tandy1000, vec![]);
        m
    };

    /// This hashmap is used to permit certain CPUs to be swapped out in place of others.
    static ref COMPATIBLE_CPUS: HashMap<CpuType, Vec<CpuType>> = {
        let mut m = HashMap::new();
        m.insert(CpuType::Intel8088, vec![CpuType::NecV20]);
        m.insert(CpuType::NecV20, vec![CpuType::Intel8088]);
        m
    };
}

pub fn get_base_rom_features(machine_type: MachineType) -> Option<&'static Vec<&'static str>> {
    BASE_ROM_FEATURES.get(&machine_type)
}

pub fn get_optional_rom_features(machine_type: MachineType) -> Option<&'static Vec<&'static str>> {
    OPTIONAL_ROM_FEATURES.get(&machine_type)
}

/// Defines the basic architecture of a machine. These are the fixed components on a machine's motherboard or otherwise
/// non-optional components common to all machines of its type. Optional components are defined in a machine
/// configuration file.
#[derive(Copy, Clone, Debug)]
pub struct MachineDescriptor {
    pub machine_type: MachineType,
    pub system_crystal: f64,        // The main system crystal speed in MHz.
    pub timer_crystal: Option<f64>, // The main timer crystal speed in MHz. On PC/AT, there is a separate timer crystal to run the PIT at the same speed as PC/XT.
    pub bus_crystal: f64,
    pub open_bus_byte: u8,
    pub cpu_type: CpuType,
    pub cpu_factor: ClockFactor, // Specifies the CPU speed in either a divisor or multiplier of system crystal.
    pub cpu_turbo_factor: ClockFactor, // Same as above, but when turbo button is active
    pub bus_type: BusType,
    pub bus_factor: ClockFactor, // Specifies the ISA bus speed in either a divisor or multiplier of bus crystal.
    pub timer_divisor: u32,      // Specifies the PIT timer speed in a divisor of timer clock speed.
    pub have_ppi: bool,
    pub a0: Option<A0Type>, // Whether the machine has an A0 register to control NMI, and what type it is.
    pub kb_controller: KbControllerType,
    pub pit_type: PitType,
    pub pic_type: PicType,
    pub dma_type: Option<DmaType>,     // Not all machines have DMA (PCJr)
    pub onboard_serial: Option<u16>,   // Whether the machine has an onboard serial port - and if so, the port base.
    pub onboard_parallel: Option<u16>, // Whether the machine has an onboard parallel port - and if so, the port base.
    pub allow_expansion_video: bool,   // Whether the machine allows for expansion video cards.
    pub pcjr_cart_slot: bool,          // Does the system have PCJr cartridge slots?
    pub game_port: Option<u16>,        // Does the system have an onboard game port, and if so, at what address?
}

impl Default for MachineDescriptor {
    /// The default MachineDescriptor represents the IBM 5150.
    fn default() -> Self {
        MachineDescriptor {
            machine_type: MachineType::Default,
            system_crystal: IBM_PC_SYSTEM_CLOCK,
            timer_crystal: None,
            bus_crystal: IBM_PC_SYSTEM_CLOCK,
            open_bus_byte: 0xFF,
            cpu_type: CpuType::Intel8088,
            cpu_factor: ClockFactor::Divisor(3),
            cpu_turbo_factor: ClockFactor::Divisor(2),
            bus_type: BusType::Isa8,
            bus_factor: ClockFactor::Divisor(1),
            timer_divisor: PIT_DIVISOR,
            have_ppi: true,
            a0: Some(A0Type::PCXT),
            kb_controller: KbControllerType::Ppi,
            pit_type: PitType::Model8253,
            pic_type: PicType::Single,
            dma_type: Some(DmaType::Single),
            onboard_serial: None,
            onboard_parallel: None,
            allow_expansion_video: true,
            pcjr_cart_slot: false,
            game_port: None,
        }
    }
}

impl MachineDescriptor {
    pub fn is_compatible_configuration(&self, config: &MachineConfiguration) -> bool {
        // Check CPU compatibility
        if let Some(cpu_opt) = &config.cpu {
            if let Some(upgrade_type) = cpu_opt.upgrade_type {
                if let Some(compatible_cpus) = COMPATIBLE_CPUS.get(&self.cpu_type) {
                    if !compatible_cpus.contains(&upgrade_type) {
                        log::warn!(
                            "Incompatible CPU upgrade specified: {:?} not compatible with base type {:?}",
                            upgrade_type,
                            self.cpu_type
                        );
                        return false;
                    }
                }
            }
        }
        true
    }
}

lazy_static! {
    /// Eventually we will want to move these machine definitions into a config file
    /// so that people can define custom architectures.
    pub static ref MACHINE_DESCS: HashMap<MachineType, MachineDescriptor> = {
        let map = HashMap::from([
            (
                MachineType::Ibm5150v64K,
                MachineDescriptor {
                    machine_type: MachineType::Ibm5150v64K,
                    ..Default::default()
                },
            ),
            (
                MachineType::Ibm5150v256K,
                MachineDescriptor {
                    machine_type: MachineType::Ibm5150v256K,
                    ..Default::default()
                },
            ),
            (
                MachineType::Ibm5160,
                MachineDescriptor {
                    machine_type: MachineType::Ibm5160,
                    ..Default::default()
                },
            ),
            (
                MachineType::IbmPCJr,
                MachineDescriptor {
                    machine_type: MachineType::IbmPCJr,
                    system_crystal: IBM_PC_SYSTEM_CLOCK,
                    timer_crystal: None,
                    bus_crystal: IBM_PC_SYSTEM_CLOCK,
                    cpu_type: CpuType::Intel8088,
                    cpu_factor: ClockFactor::Divisor(3),
                    cpu_turbo_factor: ClockFactor::Divisor(2),
                    bus_type: BusType::Isa8,
                    bus_factor: ClockFactor::Divisor(1),
                    timer_divisor: PIT_DIVISOR,
                    have_ppi: true,
                    a0: Some(A0Type::PCJr),
                    kb_controller: KbControllerType::Ppi,
                    pit_type: PitType::Model8253,
                    pic_type: PicType::Single,
                    dma_type: None,
                    pcjr_cart_slot: true, // PCJr has cartridge slots!
                    game_port: Some(GAME_PORT_DEFAULT_IO),
                    ..Default::default()
                },
            ),
            (
                MachineType::Tandy1000,
                MachineDescriptor {
                    machine_type: MachineType::Tandy1000,
                    system_crystal: IBM_PC_SYSTEM_CLOCK,
                    timer_crystal: None,
                    bus_crystal: IBM_PC_SYSTEM_CLOCK,
                    open_bus_byte: 0xE8,
                    cpu_type: CpuType::Intel8088,
                    cpu_factor: ClockFactor::Divisor(3),
                    cpu_turbo_factor: ClockFactor::Divisor(2),
                    bus_type: BusType::Isa8,
                    bus_factor: ClockFactor::Divisor(1),
                    timer_divisor: PIT_DIVISOR,
                    have_ppi: true,
                    a0: Some(A0Type::Tandy1000),
                    kb_controller: KbControllerType::Ppi,
                    pit_type: PitType::Model8253,
                    pic_type: PicType::Single,
                    dma_type: Some(DmaType::Single),
                    onboard_serial: None,
                    onboard_parallel: Some(0x378),
                    allow_expansion_video: false,
                    pcjr_cart_slot: false,
                    game_port: Some(GAME_PORT_DEFAULT_IO),
                    ..Default::default()
                },
            )
        ]);
        map
    };
}

pub fn get_machine_descriptor(machine_type: MachineType) -> Option<&'static MachineDescriptor> {
    MACHINE_DESCS.get(&machine_type)
}

pub fn normalize_conventional_memory(config: &MachineConfiguration) -> Result<u32, Error> {
    let mut conventional_memory = config.memory.conventional.size;
    conventional_memory = conventional_memory & 0xfffff000; // Normalize to 4K boundary

    // For 5150 machines we set conventional memory to the next largest valid DIP value
    let new_conventional_memory = match config.machine_type {
        MachineType::Ibm5150v64K => match conventional_memory {
            0x00000..=0x04000 => 0x04000,
            0x04001..=0x08000 => 0x08000,
            0x08001..=0x0C000 => 0x0C000,
            0x0C001..=0x10000 => 0x10000,
            0x10001..=0x18000 => 0x18000,
            0x18001..=0x20000 => 0x20000,
            0x20001..=0x28000 => 0x28000,
            0x28001..=0x30000 => 0x30000,
            0x30001..=0x38000 => 0x38000,
            0x38001..=0x40000 => 0x40000,
            0x40001..=0x48000 => 0x48000,
            0x48001..=0x50000 => 0x50000,
            0x50001..=0x58000 => 0x58000,
            0x58001..=0x60000 => 0x60000,
            0x60001..=0x68000 => 0x68000,
            0x68001..=0x70000 => 0x70000,
            0x70001..=0x78000 => 0x78000,
            0x78001..=0x80000 => 0x80000,
            0x80001..=0x88000 => 0x88000,
            0x88001..=0x90000 => 0x90000,
            0x90001..=0x98000 => 0x98000,
            0x98001..=0xA0000 => 0xA0000,
            0xA0001.. => conventional_memory,
        },
        MachineType::Ibm5150v256K => match conventional_memory {
            0x00000..=0x10000 => 0x10000,
            0x10001..=0x20000 => 0x20000,
            0x20001..=0x30000 => 0x30000,
            0x30001..=0x40000 => 0x40000,
            0x40001..=0x48000 => 0x48000,
            0x48001..=0x50000 => 0x50000,
            0x50001..=0x58000 => 0x58000,
            0x58001..=0x60000 => 0x60000,
            0x60001..=0x68000 => 0x68000,
            0x68001..=0x70000 => 0x70000,
            0x70001..=0x78000 => 0x78000,
            0x78001..=0x80000 => 0x80000,
            0x80001..=0x88000 => 0x88000,
            0x88001..=0x90000 => 0x90000,
            0x90001..=0x98000 => 0x98000,
            0x98001..=0xA0000 => 0xA0000,
            0xA0001.. => conventional_memory,
        },
        _ => conventional_memory,
    };

    if new_conventional_memory == 0 {
        Err(anyhow!(
            "Invalid conventional memory size specified: {}",
            conventional_memory
        ))
    }
    else {
        Ok(new_conventional_memory)
    }
}
