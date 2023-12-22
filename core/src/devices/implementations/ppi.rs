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

    devices::ppi.rs

    Implement the 8255 PPI (Programmable Peripheral Interface)

    Other than reporting DIP switch status and other system information the
    PPI acts as the interface for the PC/XT keyboard. We emulate the keyboard
    through the PPI.
*/
#![allow(dead_code)]

use std::cell::Cell;

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice, NO_IO_BYTE},
    devices::{implementations::pic, traits::videocard::VideoType},
    machine_types::MachineType,
};

pub const PPI_PORT_A: u16 = 0x60;
pub const PPI_PORT_B: u16 = 0x61;
pub const PPI_PORT_C: u16 = 0x62;
pub const PPI_COMMAND_PORT: u16 = 0x63;

pub const KB_RESET_US: f64 = 10_000.0; // Time with clock line pulled low before kb is reset - 10ms
pub const KB_RESET_DELAY_US: f64 = 1000.0; // Delay period between detecting reset and sending reset byte - 1ms

// Dipswitch information from
// http://www.minuszerodegrees.net/5150/misc/5150_motherboard_switch_settings.htm

// BIT values read from PPI are INVERTED of dipswitch settings
// (DIP SWITCH OFF = Bit ON)

// SW1 ON:  No floppy
// SW1 OFF: One or more
pub const SW1_HAS_FLOPPIES: u8 = 0b0000_0000;

// SW2 ON:  8087 NOT installed
// SW2 OFF: 8087 installed
pub const SW1_HAVE_8087: u8 = 0b0000_0000;

// SW4_3: ON,ON: Only bank 0 populated
// SW4_3: ON, OFF: Only banks 0/1 populated
// SW4_3: OFF, ON: Only banks 0/1/2 populated
// SW4_3: OFF, OFF: Banks 0/1/2/3 populated
pub const SW1_RAM_BANKS_1: u8 = 0b0001_1000;
pub const SW1_RAM_BANKS_2: u8 = 0b0001_0000;
pub const SW1_RAM_BANKS_3: u8 = 0b0000_1000;
pub const SW1_RAM_BANKS_4: u8 = 0b0000_0000;

// SW6_5: OFF, OFF: MDA card
// SW6_5: ON, OFF: CGA 40 Cols
// SW6_5: OFF, ON: CGA 80 Cols
// SW6_5: ON, ON: EGA or VGA card (Requires '82 BIOS)
pub const SW1_HAVE_MDA: u8 = 0b0000_0000;
pub const SW1_HAVE_CGA_LORES: u8 = 0b0010_0000;
pub const SW1_HAVE_CGA_HIRES: u8 = 0b0001_0000;
pub const SW1_HAVE_EXPANSION: u8 = 0b0011_0000;

// SW8_7: ON, ON: One floppy
// SW8_7: ON, OFF: Two floppies
// SW8_7: OFF, ON: Three floppies
// SW8_7: OFF, OFF: Four floppies
pub const SW1_ONE_FLOPPY: u8 = 0b1100_0000;
pub const SW1_TWO_FLOPPIES: u8 = 0b1000_0000;
pub const SW1_THREE_FLOPPIES: u8 = 0b0100_0000;
pub const SW1_FOUR_FLOPPIES: u8 = 0b0000_0000;

// DIP SWITCH BLOCK #2

// 5150 64-256K motherboard
pub const SW2_V1_RAM_16K: u8 = 0b0001_1111;
pub const SW2_V1_RAM_32K: u8 = 0b0001_1111;
pub const SW2_V1_RAM_48K: u8 = 0b0001_1111;
pub const SW2_V1_RAM_64K: u8 = 0b0001_1111;
pub const SW2_V1_RAM_96K: u8 = 0b0001_1110;
pub const SW2_V1_RAM_128K: u8 = 0b0001_1101;
pub const SW2_V1_RAM_160K: u8 = 0b0001_1100;
pub const SW2_V1_RAM_192K: u8 = 0b0001_1011;
pub const SW2_V1_RAM_224K: u8 = 0b0001_1010;
pub const SW2_V1_RAM_256K: u8 = 0b0001_1001;
pub const SW2_V1_RAM_288K: u8 = 0b0001_1000;
pub const SW2_V1_RAM_320K: u8 = 0b0001_0111;
pub const SW2_V1_RAM_352K: u8 = 0b0001_0110;
pub const SW2_V1_RAM_384K: u8 = 0b0001_0101;
pub const SW2_V1_RAM_416K: u8 = 0b0001_0100;
pub const SW2_V1_RAM_448K: u8 = 0b0001_0011;
pub const SW2_V1_RAM_480K: u8 = 0b0001_0010;
pub const SW2_V1_RAM_512K: u8 = 0b0001_0001;
pub const SW2_V1_RAM_544K: u8 = 0b0001_0000;
pub const SW2_V1_RAM_576K: u8 = 0b0000_1111;
pub const SW2_V1_RAM_608K: u8 = 0b0000_1110;
pub const SW2_V1_RAM_640K: u8 = 0b0000_1101;

// 5150 64-256K motherboard
pub const SW2_V2_RAM_64K: u8 = 0b0001_1111;
pub const SW2_V2_RAM_128K: u8 = 0b0001_1101;
pub const SW2_V2_RAM_192K: u8 = 0b0001_1011;
pub const SW2_V2_RAM_256K: u8 = 0b0001_1001;
pub const SW2_V2_RAM_288K: u8 = 0b0001_1000;
pub const SW2_V2_RAM_320K: u8 = 0b0001_0111;
pub const SW2_V2_RAM_352K: u8 = 0b0001_0110;
pub const SW2_V2_RAM_384K: u8 = 0b0001_0101;
pub const SW2_V2_RAM_416K: u8 = 0b0001_0100;
pub const SW2_V2_RAM_448K: u8 = 0b0001_0011;
pub const SW2_V2_RAM_480K: u8 = 0b0001_0010;
pub const SW2_V2_RAM_512K: u8 = 0b0001_0001;
pub const SW2_V2_RAM_544K: u8 = 0b0001_0000;
pub const SW2_V2_RAM_576K: u8 = 0b0000_1111;
pub const SW2_V2_RAM_608K: u8 = 0b0000_1110;
pub const SW2_V2_RAM_640K: u8 = 0b0000_1101;

// PORT B INPUTS
pub const PORTB_TIMER2_GATE: u8 = 0b0000_0001;
pub const PORTB_SPEAKER_DATA: u8 = 0b0000_0010;
pub const PORTB_SW2_SELECT: u8 = 0b0000_0100;

// This bit is cassette motor control on 5150, SW1 select on 5160
pub const PORTB_CASSETTE: u8 = 0b0000_1000;
pub const PORTB_SW1_SELECT: u8 = 0b0000_1000;

pub const PORTB_PARITY_MB_EN: u8 = 0b0001_0000;
pub const PORTB_PARITY_EX_EN: u8 = 0b0010_0000;
pub const PORTB_PULL_KB_LOW: u8 = 0b0100_0000;

pub const PORTB_KB_CLEAR: u8 = 0b1000_0000;
pub const PORTB_PRESENT_SW1_PORTA: u8 = 0b1000_0000;

#[derive(Debug)]
pub enum PortAMode {
    SwitchBlock1,
    KeyboardByte,
}
#[derive(Debug)]
pub enum PortCMode {
    Switch2OneToFour,
    Switch2Five,
    Switch1OneToFour,
    Switch1FiveToEight,
}
pub struct Ppi {
    machine_type: MachineType,
    port_a_mode: PortAMode,
    port_c_mode: PortCMode,
    kb_clock_low: bool,
    kb_counting_low: bool,
    kb_low_count: f64,
    kb_do_reset: bool,
    kb_count_until_reset_byte: f64,
    kb_resets_counter: u32,
    pb_byte: u8,
    kb_byte: u8,
    keyboard_clear_scheduled: bool,
    ksr_cleared: bool,
    kb_enabled: bool,
    dip_sw1: u8,
    dip_sw2: u8,
    timer_in: bool,
    speaker_in: bool,
}

// This structure implements an interface for wires connected to the PPI from
// other components. Components connected to the PPI will receive a reference
// to this structure on creation, and can read or modify the wire state via
// Cell's internal mutability.
// (unimplemented)
pub struct PpiWires {
    timer_monitor: Cell<bool>,
    timer_gate2: Cell<bool>,
    speaker_monitor: Cell<bool>,
}

#[derive(Default)]
pub struct PpiStringState {
    pub port_a_mode: String,
    pub port_a_value_bin: String,
    pub port_a_value_hex: String,
    pub port_b_value_bin: String,
    pub kb_byte_value_hex: String,
    pub kb_resets_counter: String,
    pub port_c_mode: String,
    pub port_c_value: String,
}

impl Ppi {
    pub fn new(
        machine_type: MachineType,
        conventional_mem: u32,
        video_types: Vec<VideoType>,
        num_floppies: u32,
    ) -> Self {
        // Creation of the PPI is primarily concerned with setting up the DIP switches.
        let (sw1_bank_bits, sw2_ram_dip_bits) = Ppi::get_ram_dip(machine_type, conventional_mem);

        let (sw1_floppy_ct_bits, sw1_master_floppy_bit) = match num_floppies {
            1 => (SW1_ONE_FLOPPY, SW1_HAS_FLOPPIES),
            2 => (SW1_TWO_FLOPPIES, SW1_HAS_FLOPPIES),
            3 => (SW1_THREE_FLOPPIES, SW1_HAS_FLOPPIES),
            4 => (SW1_FOUR_FLOPPIES, SW1_HAS_FLOPPIES),
            _ => (0, 0),
        };

        let sw1_video_bits = if video_types.contains(&VideoType::VGA) || video_types.contains(&VideoType::EGA) {
            // We have a card that requires an expansion BIOs.
            SW1_HAVE_EXPANSION
        }
        else if video_types.contains(&VideoType::CGA) {
            // We have a CGA card.
            SW1_HAVE_CGA_HIRES
        }
        else {
            // MDA or no card.
            SW1_HAVE_MDA
        };

        Self {
            machine_type,
            port_a_mode: match machine_type {
                MachineType::Ibm5150v64K | MachineType::Ibm5150v256K => PortAMode::SwitchBlock1,
                MachineType::Ibm5160 => PortAMode::KeyboardByte,
                _ => {
                    panic!("Machine type: {:?} has no PPI", machine_type);
                }
            },
            port_c_mode: match machine_type {
                MachineType::Ibm5150v64K | MachineType::Ibm5150v256K => PortCMode::Switch2OneToFour,
                MachineType::Ibm5160 => PortCMode::Switch1FiveToEight,
                _ => {
                    panic!("Machine type: {:?} has no PPI", machine_type);
                }
            },
            kb_clock_low: false,
            kb_counting_low: false,
            kb_low_count: 0.0,
            kb_do_reset: false,
            kb_count_until_reset_byte: 0.0,
            kb_resets_counter: 0,
            pb_byte: 0,
            kb_byte: 0,
            keyboard_clear_scheduled: false,
            ksr_cleared: true,
            kb_enabled: true,
            dip_sw1: match machine_type {
                MachineType::Ibm5150v64K | MachineType::Ibm5150v256K => {
                    let dip_sw1 = sw1_bank_bits | sw1_floppy_ct_bits | sw1_video_bits | sw1_master_floppy_bit;
                    log::debug!("DIP SW1: {:08b}", dip_sw1);
                    !dip_sw1
                }
                MachineType::Ibm5160 => {
                    let dip_sw1 = sw1_bank_bits | sw1_floppy_ct_bits | sw1_video_bits | sw1_master_floppy_bit;
                    log::debug!("DIP SW1: {:08b}", dip_sw1);
                    !dip_sw1
                }
                _ => {
                    log::error!("Machine type: {:?} has no PPI", machine_type);
                    0
                }
            },
            dip_sw2: sw2_ram_dip_bits,
            timer_in: false,
            speaker_in: false,
        }
    }

    fn get_ram_dip(machine_type: MachineType, conventional_mem: u32) -> (u8, u8) {
        match machine_type {
            MachineType::Ibm5150v64K => match conventional_mem {
                0x04000 => (SW2_V1_RAM_16K, SW1_RAM_BANKS_1),
                0x08000 => (SW2_V1_RAM_32K, SW1_RAM_BANKS_2),
                0x0C000 => (SW2_V1_RAM_48K, SW1_RAM_BANKS_3),
                0x10000 => (SW2_V1_RAM_64K, SW1_RAM_BANKS_4),
                0x18000 => (SW2_V1_RAM_96K, SW1_RAM_BANKS_4),
                0x20000 => (SW2_V1_RAM_128K, SW1_RAM_BANKS_4),
                0x28000 => (SW2_V1_RAM_160K, SW1_RAM_BANKS_4),
                0x30000 => (SW2_V1_RAM_192K, SW1_RAM_BANKS_4),
                0x38000 => (SW2_V1_RAM_224K, SW1_RAM_BANKS_4),
                0x40000 => (SW2_V1_RAM_256K, SW1_RAM_BANKS_4),
                0x48000 => (SW2_V1_RAM_288K, SW1_RAM_BANKS_4),
                0x50000 => (SW2_V1_RAM_320K, SW1_RAM_BANKS_4),
                0x58000 => (SW2_V1_RAM_352K, SW1_RAM_BANKS_4),
                0x60000 => (SW2_V1_RAM_384K, SW1_RAM_BANKS_4),
                0x68000 => (SW2_V1_RAM_416K, SW1_RAM_BANKS_4),
                0x70000 => (SW2_V1_RAM_448K, SW1_RAM_BANKS_4),
                0x78000 => (SW2_V1_RAM_480K, SW1_RAM_BANKS_4),
                0x80000 => (SW2_V1_RAM_512K, SW1_RAM_BANKS_4),
                0x88000 => (SW2_V1_RAM_544K, SW1_RAM_BANKS_4),
                0x90000 => (SW2_V1_RAM_576K, SW1_RAM_BANKS_4),
                0x98000 => (SW2_V1_RAM_608K, SW1_RAM_BANKS_4),
                0xA0000 => (SW2_V1_RAM_640K, SW1_RAM_BANKS_4),
                _ => {
                    log::error!("Invalid conventional memory size: {}", conventional_mem);
                    (SW2_V1_RAM_16K, SW1_RAM_BANKS_1)
                }
            },
            MachineType::Ibm5150v256K => match conventional_mem {
                0x10000 => (SW2_V2_RAM_64K, SW1_RAM_BANKS_4),
                0x20000 => (SW2_V2_RAM_128K, SW1_RAM_BANKS_4),
                0x30000 => (SW2_V2_RAM_192K, SW1_RAM_BANKS_4),
                0x40000 => (SW2_V2_RAM_256K, SW1_RAM_BANKS_4),
                0x48000 => (SW2_V2_RAM_288K, SW1_RAM_BANKS_4),
                0x50000 => (SW2_V2_RAM_320K, SW1_RAM_BANKS_4),
                0x58000 => (SW2_V2_RAM_352K, SW1_RAM_BANKS_4),
                0x60000 => (SW2_V2_RAM_384K, SW1_RAM_BANKS_4),
                0x68000 => (SW2_V2_RAM_416K, SW1_RAM_BANKS_4),
                0x70000 => (SW2_V2_RAM_448K, SW1_RAM_BANKS_4),
                0x78000 => (SW2_V2_RAM_480K, SW1_RAM_BANKS_4),
                0x80000 => (SW2_V2_RAM_512K, SW1_RAM_BANKS_4),
                0x88000 => (SW2_V2_RAM_544K, SW1_RAM_BANKS_4),
                0x90000 => (SW2_V2_RAM_576K, SW1_RAM_BANKS_4),
                0x98000 => (SW2_V2_RAM_608K, SW1_RAM_BANKS_4),
                0xA0000 => (SW2_V2_RAM_640K, SW1_RAM_BANKS_4),
                _ => {
                    log::error!("Invalid conventional memory size: {}", conventional_mem);
                    (SW2_V2_RAM_64K, SW1_RAM_BANKS_1)
                }
            },
            _ => (0, 0),
        }
    }
}

impl IoDevice for Ppi {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        //log::trace!("PPI Read from port: {:04X}", port);
        match port {
            PPI_PORT_A => {
                // Return dip switch block 1 or kb_byte depending on port mode
                // 5160 will always return kb_byte.
                // PPI PB7 supresses keyboard shift register output.
                match self.port_a_mode {
                    PortAMode::SwitchBlock1 => self.dip_sw1,
                    PortAMode::KeyboardByte => {
                        if self.kb_enabled {
                            self.kb_byte
                        }
                        else {
                            0
                        }
                    }
                }
            }
            PPI_PORT_B => self.handle_portb_read(),
            PPI_PORT_C => self.calc_port_c_value(),
            PPI_COMMAND_PORT => NO_IO_BYTE,
            _ => panic!("PPI: Bad port #"),
        }
    }

    fn write_u8(&mut self, port: u16, byte: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        match port {
            PPI_PORT_A => {
                // Read-only port
            }
            PPI_PORT_B => {
                //log::trace!("PPI: Write to Port B: {:02X}", byte);
                self.handle_portb_write(byte);
            }
            PPI_PORT_C => {
                // Read-only port
            }
            PPI_COMMAND_PORT => {
                self.handle_command_port_write(byte);
            }
            _ => panic!("PPI: Bad port #"),
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![PPI_PORT_A, PPI_PORT_B, PPI_PORT_C, PPI_COMMAND_PORT]
    }
}

impl Ppi {
    pub fn handle_command_port_write(&mut self, byte: u8) {
        log::trace!("PPI: Write to command port: {:02X}", byte);
    }

    pub fn handle_portb_read(&self) -> u8 {
        self.pb_byte
    }

    pub fn handle_portb_write(&mut self, byte: u8) {
        self.pb_byte = byte;

        match self.machine_type {
            MachineType::Ibm5150v64K | MachineType::Ibm5150v256K => {
                // 5150 Behavior Only
                if byte & PORTB_SW2_SELECT != 0 {
                    // If Bit 2 is ON, PC0-PC3 represent SW2 S1-S4
                    self.port_c_mode = PortCMode::Switch2OneToFour;
                }
                else {
                    // If Bit 2 is OFF, PC0 is SW2 S5, and PC01, PC02, PC03 will read ON
                    self.port_c_mode = PortCMode::Switch2Five;
                }

                // Besides controlling the state of port A, this bit also suppresses IRQ1
                if byte & PORTB_PRESENT_SW1_PORTA != 0 {
                    self.keyboard_clear_scheduled = true;
                    self.kb_enabled = false;
                    self.port_a_mode = PortAMode::SwitchBlock1
                }
                else {
                    self.kb_enabled = true;
                    self.port_a_mode = PortAMode::KeyboardByte
                }
            }
            MachineType::Ibm5160 => {
                // 5160 Behavior only
                if byte & PORTB_SW1_SELECT == 0 {
                    // If Bit 3 is OFF, PC0-PC3 represent SW1 S1-S4
                    self.port_c_mode = PortCMode::Switch1OneToFour;
                }
                else {
                    self.port_c_mode = PortCMode::Switch1FiveToEight;
                }

                // On the 5160, this bit clears the keyboard and suppresses IRQ1.
                if byte & PORTB_KB_CLEAR != 0 {
                    self.keyboard_clear_scheduled = true;
                    self.kb_enabled = false;
                }
                else {
                    self.kb_enabled = true;
                }
                self.port_a_mode = PortAMode::KeyboardByte;
            }
            _ => {
                panic!("Invalid model type for PPI");
            }
        }

        // Handle keyboard clock line bit for either 5150 or 5160
        if self.pb_byte & PORTB_PULL_KB_LOW == 0 {
            //log::trace!("PPI: Pulling keyboard clock LOW");
            self.kb_clock_low = true;
            self.kb_counting_low = true;
        }
        else if self.kb_clock_low {
            //log::trace!("PPI: Keyboard clock resume HIGH");
            self.kb_clock_low = false;

            if self.kb_low_count > KB_RESET_DELAY_US {
                // Clock line was low long enough to trigger reset
                // Start timer until reset byte is sent
                self.kb_low_count = 0.0;
                self.kb_do_reset = true;
                self.kb_count_until_reset_byte = 0.0;
            }
        }
    }

    /// Send a byte to the keyboard shift register.
    pub fn send_keyboard(&mut self, byte: u8) {
        // Only send a scancode if the keyboard is not actively being reset.
        if self.kb_enabled && self.ksr_cleared && !self.kb_clock_low {
            self.ksr_cleared = false;
            self.kb_byte = byte;
        }
    }

    /// Return whether the keyboard enable line (PB7) is set and the keyboard clock line is not held low.
    pub fn kb_enabled(&self) -> bool {
        self.kb_enabled && !self.kb_clock_low
    }

    pub fn calc_port_c_value(&self) -> u8 {
        let mut speaker_bit = 0;
        if let MachineType::Ibm5160 = self.machine_type {
            speaker_bit = (self.speaker_in as u8) << 4;
        }
        let timer_bit = (self.timer_in as u8) << 5;

        match (&self.machine_type, &self.port_c_mode) {
            (MachineType::Ibm5150v64K | MachineType::Ibm5150v256K, PortCMode::Switch2OneToFour) => {
                // We aren't implementing the cassette on 5150, and we'll never have parity errors
                (self.dip_sw2 & 0x0F) | timer_bit
            }
            (MachineType::Ibm5150v64K | MachineType::Ibm5150v256K, PortCMode::Switch2Five) => {
                // On 5150, only Switch Block 2, Switch #5 is actually passed through
                // If Port C is in Switch Block 2 mode, switches 6, 7, 8 and will read high (off)
                (self.dip_sw2 >> 4 & 0x01) | timer_bit
            }
            (MachineType::Ibm5160, PortCMode::Switch1OneToFour) => {
                // Cassette data line has been replaced with a speaker monitor line.
                (self.dip_sw1 & 0x0F) | speaker_bit | timer_bit
            }
            (MachineType::Ibm5160, PortCMode::Switch1FiveToEight) => {
                // Cassette data line has been replaced with a speaker monitor line.
                // On 5160, all four switches 5-8 are readable
                (self.dip_sw1 >> 4 & 0x0F) | speaker_bit | timer_bit
            }
            _ => {
                panic!("Invalid PPI state");
            }
        }
    }

    pub fn get_string_state(&self) -> PpiStringState {
        let port_a_value = match self.port_a_mode {
            PortAMode::SwitchBlock1 => self.dip_sw1,
            PortAMode::KeyboardByte => self.kb_byte,
        };
        let port_b_value = self.pb_byte;
        let port_c_value = self.calc_port_c_value();

        PpiStringState {
            port_a_mode: format!("{:?}", self.port_a_mode),
            port_a_value_bin: format!("{:08b}", port_a_value),
            port_a_value_hex: format!("{:02X}", port_a_value),
            port_b_value_bin: format!("{:08b}", port_b_value),
            kb_byte_value_hex: format!("{:02X}", self.kb_byte),
            kb_resets_counter: format!("{}", self.kb_resets_counter),
            port_c_mode: format!("{:?}", self.port_c_mode),
            port_c_value: format!("{:08b}", port_c_value),
        }
    }

    pub fn get_pb0_state(&self) -> bool {
        self.pb_byte & PORTB_TIMER2_GATE != 0
    }

    pub fn get_pb1_state(&self) -> bool {
        self.pb_byte & PORTB_SPEAKER_DATA != 0
    }

    pub fn get_pit_channel2_gate(&mut self) -> bool {
        self.pb_byte & PORTB_TIMER2_GATE != 0
    }

    pub fn set_pit_output_bit(&mut self, state: bool) {
        self.timer_in = state;
    }

    pub fn set_speaker_bit(&mut self, state: bool) {
        self.speaker_in = state;
    }

    /// Return whether NMI generation is enabled
    pub fn nmi_enabled(&self) -> bool {
        self.pb_byte & PORTB_PARITY_MB_EN == 0 || self.pb_byte & PORTB_PARITY_EX_EN == 0
    }

    pub fn run(&mut self, pic: &mut pic::Pic, us: f64) {
        // Our keyboard byte was read, so clear the interrupt request line and reset the byte
        // read at the keyboard IO port to 0
        if self.keyboard_clear_scheduled {
            self.keyboard_clear_scheduled = false;
            self.ksr_cleared = true;
            self.kb_byte = 0;
            pic.clear_interrupt(1);
            //log::trace!("PPI: Clearing keyboard");
        }

        // Keyboard should send a 'aa' byte when clock line is held low (for how long?)
        // BIOS waits 20ms.
        // Clock line must go high again
        if self.kb_counting_low && self.kb_low_count < KB_RESET_US {
            self.kb_low_count += us;
        }

        // Send reset byte after delay elapsed. The delay gives the BIOS POST routines
        // time to check for interrupts as they do not do it immediately
        if self.kb_do_reset {
            self.kb_count_until_reset_byte += us;

            if self.kb_count_until_reset_byte > KB_RESET_DELAY_US {
                self.kb_do_reset = false;
                self.kb_count_until_reset_byte = 0.0;
                self.kb_resets_counter += 1;

                log::trace!("PPI: Sending keyboard reset byte");
                self.kb_byte = 0xAA;

                if self.kb_enabled {
                    pic.request_interrupt(1);
                }
            }
        }
    }
}
