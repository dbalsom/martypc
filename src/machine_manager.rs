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

use crate::pit::PitType;
use crate::config::MachineType;
use crate::cpu_common::CpuType;


#[derive (Debug)]
pub enum KbControllerType {
    Ppi,
    At
}

#[derive (Debug)]
pub enum PicType {
    Single,
    Chained
}


#[derive (Debug)]
pub enum DmaType {
    Single,
    Chained
}


#[derive (Debug)]
pub enum BusType {
    Isa8,
    Isa16
}

#[derive (Debug)]
pub struct MachineDescriptor {
    pub machine_type: MachineType,
    pub cpu_type: CpuType,
    pub cpu_freq: f64,
    pub bus_type: BusType,
    pub bus_freq: f64,
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
                        cpu_type: CpuType::Intel8088,
                        cpu_freq: 4.77272666,
                        bus_type: BusType::Isa8,
                        bus_freq: 4.77272666,
                        have_ppi: true,
                        kb_controller: KbControllerType::Ppi,
                        pit_type: PitType::Model8253,
                        pic_type: PicType::Single,
                        dma_type: DmaType::Single,
                        conventional_ram: 0x1000000,
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
                        cpu_type: CpuType::Intel8088,
                        cpu_freq: 4.77272666,
                        bus_type: BusType::Isa8,
                        bus_freq: 4.77272666,
                        have_ppi: true,
                        kb_controller: KbControllerType::Ppi,
                        pit_type: PitType::Model8253,
                        pic_type: PicType::Single,
                        dma_type: DmaType::Single,
                        conventional_ram: 0x1000000,
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