/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    machine_manager.rs

    This module manages machine configuration defintions.
  
*/

use std::collections::HashMap;
use lazy_static::lazy_static;

use crate::devices::pit::PitType;
use crate::config::MachineType;
use crate::cpu_common::CpuType;
use crate::bus::ClockFactor;

// Clock derivision from reenigne
// See https://www.vogons.org/viewtopic.php?t=55049
pub const IBM_PC_SYSTEM_CLOCK: f64 = 157.5/11.0;
pub const PIT_DIVISOR: u32 = 12;

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