/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    machine_manager.rs

    This module manages machine configuration defintions.
  
*/

use std::collections::HashMap;
use lazy_static::lazy_static;

use crate::devices::pit::PitType;
use crate::config::{MachineType, VideoType, KeyboardType};
use crate::cpu_common::CpuType;
use crate::bus::ClockFactor;
use crate::tracelogger::TraceLogger;

// Clock derivision from reenigne
// See https://www.vogons.org/viewtopic.php?t=55049
pub const IBM_PC_SYSTEM_CLOCK: f64 = 157.5/11.0;
pub const PIT_DIVISOR: u32 = 12;

/// This enum is intended to represent any specific add-on device type
/// that the bus needs to know about.
pub enum DeviceType {
    Keyboard(KeyboardType),
    VideoCard(VideoType),
}

pub struct MmioSpec {
    base_addr: u32,
    size: u32
}

pub struct DeviceSpec {
    dtype: DeviceType,          // Type of device.
    debug: bool,                // Whether or not device should enable debug functionality. 
    tracelog: TraceLogger,      // Tracelogger for device to use.
    mmio: Option<MmioSpec>,     // Whether or not device has a mmio mapping.
    io: bool,                   // Whether or not device registers IO ports / requires IO dispatch.
    hotplug: bool,              // Whether or not a device can be added/removed while machine is running.
}

#[derive (Copy, Clone, Debug)]
pub enum KbControllerType {
    Ppi,
    At
}

#[derive (Copy, Clone, Debug)]
pub enum PicType {
    Single,
    Chained
}

#[derive (Copy, Clone, Debug)]
pub enum DmaType {
    Single,
    Chained
}


#[derive (Copy, Clone, Debug)]
pub enum BusType {
    Isa8,
    Isa16
}

#[derive (Copy, Clone, Debug)]
pub struct MachineDescriptor {
    pub machine_type: MachineType,
    pub system_crystal: f64,            // The main system crystal speed in MHz. 
    pub timer_crystal: Option<f64>,     // The main timer crystal speed in MHz. On PC/AT, there is a separate timer
                                        // crystal to run the PIT at the same speed as PC/XT. 
    pub bus_crystal: f64,
    pub cpu_type: CpuType,
    pub cpu_factor: ClockFactor,        // Specifies the CPU speed in either a divisor or multiplier of system crystal.
    pub cpu_turbo_factor: ClockFactor,  // Same as above, but when turbo button is active
    pub bus_type: BusType,
    pub bus_factor: ClockFactor,        // Specifies the ISA bus speed in either a divisor or multiplier of bus crystal.
    pub timer_divisor: u32,             // Specifies the PIT timer speed in a divisor of timer clock speed.
    pub have_ppi: bool,
    pub kb_controller: KbControllerType,
    pub pit_type: PitType,
    pub pic_type: PicType,
    pub dma_type: DmaType,
    pub conventional_ram: u32,
    pub conventional_ram_speed: f64,
    pub num_floppies: u32,
    pub serial_ports: bool, // TODO: Eventually add a way to specify number of ports and base IO
    pub serial_mouse: bool, // TODO: Allow specifying which port mouse is connected to?
}

lazy_static! {
    pub static ref MACHINE_DESCS: HashMap<MachineType, MachineDescriptor> = {

        let map = HashMap::from(
            [
                ( 
                    MachineType::IBM_PC_5150,
                    MachineDescriptor {
                        machine_type: MachineType::IBM_PC_5150,
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
                        kb_controller: KbControllerType::Ppi,
                        pit_type: PitType::Model8253,
                        pic_type: PicType::Single,
                        dma_type: DmaType::Single,
                        conventional_ram: 0x100000,
                        conventional_ram_speed: 200.0,
                        num_floppies: 2,
                        serial_ports: true,
                        serial_mouse: true,
                    }
                ),
                ( 
                    MachineType::IBM_XT_5160,
                    MachineDescriptor {
                        machine_type: MachineType::IBM_XT_5160,
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
                        kb_controller: KbControllerType::Ppi,
                        pit_type: PitType::Model8253,
                        pic_type: PicType::Single,
                        dma_type: DmaType::Single,
                        conventional_ram: 0x100000,
                        conventional_ram_speed: 200.0,
                        num_floppies: 2,
                        serial_ports: true,
                        serial_mouse: true
                    }
                ),        
            ]
        );
        map
    };
}