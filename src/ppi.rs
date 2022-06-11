/*
    ppi.rc
    Implement the 8255 PPI (Programmable Peripheral Interface)

    Other than reporting DIP switch status and other system information the PPI
    acts as the interface for the PC/XT keyboard. We emulate the keyboard through 
    the PPI.
*/
#![allow(dead_code)]

use crate::io::{IoDevice};
use crate::machine::{MachineType, VideoType};
use crate::pic;

pub const PPI_PORT_A: u16 = 0x60;
pub const PPI_PORT_B: u16 = 0x61;
pub const PPI_PORT_C: u16 = 0x62;
pub const PPI_COMMAND_PORT: u16 = 0x63;

pub const KB_RESET_CYCLES: u32 = 47700;
pub const KB_RESET_CYCLE_DELAY: u32 = 1000; // Cycles until reset byte is sent after reset

// Dipswitch information from
// http://www.minuszerodegrees.net/5150/misc/5150_motherboard_switch_settings.htm

// BIT values read from PPI are INVERTED of dipswitch settings
// (DIP SWITCH OFF = Bit ON)

// SW1 ON:  No floppy
// SW1 OFF: One or more
pub const SW1_HAS_FLOPPIES: u8   = 0b0000_0001;

// SW2 ON:  8087 NOT installed
// SW2 OFF: 8087 installed
pub const SW1_HAVE_8087: u8      = 0b0000_0010;

// SW4_3: ON,ON: Only bank 0 populated
// SW4_3: ON, OFF: Only banks 0/1 populated
// SW4_3: OFF, ON: Only banks 0/1/2 populated
// SW4_3: OFF, OFF: Banks 0/1/2/3 populated
pub const SW1_RAM_BANKS: u8    = 0b0000_1100;

// SW6_5: OFF, OFF: MDA card
// SW6_5: ON, OFF: CGA 40 Cols
// SW6_5: OFF, ON: CGA 80 Cols
// SW6_5: ON, ON: EGA or VGA card (Requires '82 BIOS)
pub const SW1_HAVE_MDA: u8       = 0b0011_0000;
pub const SW1_HAVE_CGA_LORES: u8 = 0b0010_0000;
pub const SW1_HAVE_CGA_HIRES: u8 = 0b0001_0000;
pub const SW1_HAVE_EGA: u8       = 0b0000_0000;

// SW8_7: ON, ON: One floppy
// SW8_7: ON, OFF: Two floppies
// SW8_7: OFF, ON: Three floppies??
// SW8_7: OFF, OFF: Four floppies!!
pub const SW1_ONE_FLOPPY: u8     = 0b0000_0000;
pub const SW1_TWO_FLOPPIES: u8   = 0b0100_0000;
pub const SW1_THREE_FLOPPIES: u8 = 0b1000_0000;
pub const SW1_FOUR_FLOPPIES: u8  = 0b1100_0000;

// DIP SWITCH BLOCK #2
pub const SW2_RAM_64K: u8        = 0b0001_1111;
pub const SW2_RAM_96K: u8        = 0b0001_1110;
pub const SW2_RAM_128K: u8       = 0b0001_1101;
pub const SW2_RAM_160K: u8       = 0b0001_1100;
pub const SW2_RAM_192K: u8       = 0b0001_1011;
pub const SW2_RAM_224K: u8       = 0b0001_1010;
pub const SW2_RAM_256K: u8       = 0b0001_1001;
pub const SW2_RAM_288K: u8       = 0b0001_1000;
pub const SW2_RAM_320K: u8       = 0b0001_0111;
pub const SW2_RAM_384K: u8       = 0b0001_0110;
pub const SW2_RAM_416K: u8       = 0b0001_0100;
pub const SW2_RAM_448K: u8       = 0b0001_0011;
pub const SW2_RAM_480K: u8       = 0b0001_0010;
pub const SW2_RAM_512K: u8       = 0b0001_0001;
pub const SW2_RAM_544K: u8       = 0b0001_0000;
// need mb revision?
pub const SW2_RAM_576K: u8       = 0b0000_1111;
pub const SW2_RAM_608K: u8       = 0b0000_1110;
pub const SW2_RAM_640K: u8       = 0b0000_1101;
pub const SW2_5: u8              = 0b0001_0000;

pub const SW2_RAM_TEST: u8       = 0b1110_1111;

// PORT B INPUTS
pub const PORTB_TIMER2_GATE: u8  = 0b0000_0001;
pub const PORTB_SPEAKER_DATA: u8 = 0b0000_0010;
pub const PORTB_SW2_SELECT: u8   = 0b0000_0100;
pub const PORTB_CASSETTE: u8     = 0b0000_1000;
pub const PORTB_PARITY_MB_EN: u8 = 0b0001_0000;
pub const PORTB_PARITY_EX_EN: u8 = 0b0010_0000;
pub const PORTB_PULL_KB_LOW: u8  = 0b0100_0000;
pub const PORTB_PRESENT_SW1_PORTA: u8  = 0b1000_0000;

#[derive(Debug)]
pub enum PortAMode {
    SwitchBlock1,
    KeyboardByte
}
#[derive(Debug)]
pub enum PortCMode {
    Switch2OneToFour,
    Switch2FiveToEight
}
pub struct Ppi {
    machine_type: MachineType,
    port_a_mode: PortAMode,
    port_c_mode: PortCMode,
    kb_clock_low: bool,
    kb_counting_low: bool,
    kb_low_count: u32,
    kb_been_reset: bool,
    kb_count_until_reset_byte: u32,
    pb_byte: u8,
    kb_byte: u8,
    clear_keyboard: bool,
    dip_sw1: u8,
    dip_sw2: u8,
    timer_in: bool,
    speaker_in: bool,
}

#[derive(Default)]
pub struct PpiStringState {
    pub port_a_mode: String,
    pub port_a_value_bin: String,
    pub port_a_value_hex: String,
    pub kb_byte_value_hex: String,
    pub port_c_mode: String,
    pub port_c_value: String,
}

impl Ppi {

    pub fn new(machine_type: MachineType, video_type: VideoType ) -> Self {

        Self {
            machine_type,
            port_a_mode: PortAMode::SwitchBlock1,
            port_c_mode: PortCMode::Switch2OneToFour,
            kb_clock_low: false,
            kb_counting_low: false,
            kb_low_count: 0,
            kb_been_reset: false,
            kb_count_until_reset_byte: 0,
            pb_byte: 0,
            kb_byte: 0,
            clear_keyboard: false,
            dip_sw1: match machine_type {
                MachineType::IBM_PC_5150 => SW1_HAVE_CGA_LORES | SW1_HAS_FLOPPIES | SW1_TWO_FLOPPIES | SW1_RAM_BANKS,
                MachineType::IBM_XT_5160 => SW1_HAVE_CGA_LORES | SW1_HAS_FLOPPIES | SW1_TWO_FLOPPIES | SW1_RAM_BANKS
            },
            dip_sw2: SW2_RAM_TEST,
            timer_in: false,
            speaker_in: false,
        }
    }
}

impl IoDevice for Ppi {
    fn read_u8(&mut self, port: u16) -> u8 {
        //log::trace!("PPI Read from port: {:04X}", port);
        match port {
            PPI_PORT_A => {
                // Return dip switch block 1 or kb_byte depending on port mode
                // 5160 will always return kb_byte
                match self.port_a_mode {
                    PortAMode::SwitchBlock1 => self.dip_sw1,
                    PortAMode::KeyboardByte => self.kb_byte
                }
            },
            PPI_PORT_B => {
                // Write-only port
                0
            },
            PPI_PORT_C => {
                self.calc_port_c_value()
            },
            _ => panic!("PPI: Bad port #")
        }
    }
    fn write_u8(&mut self, port: u16, byte: u8) {
        match port {
            PPI_PORT_A => {
                // Read-only port
            },
            PPI_PORT_B => {
                //log::trace!("PPI: Write to Port B: {:02X}", byte);

                self.pb_byte = byte;
                if self.pb_byte & PORTB_SW2_SELECT != 0 {
                    // If Bit 2 is ON, PC0-PC3 represent SW2 S1-S4
                    self.port_c_mode = PortCMode::Switch2OneToFour;
                }
                else {
                    // If Bit 2 is OFF, PC0 is SW2 S5, and PC01, PC02, PC03 will read ON
                    self.port_c_mode = PortCMode::Switch2FiveToEight;
                }

                if self.pb_byte & PORTB_PRESENT_SW1_PORTA != 0 {
                    // Besides controlling the state of port A, this bit also triggers a 
                    // clear of the keyboard
                    // Set flag to clear interrupt & byte read on next run()
                    self.clear_keyboard = true;

                    self.port_a_mode = match self.machine_type {
                        MachineType::IBM_PC_5150 => {
                            //log::trace!("PPI: Presenting SW1 on Port A");
                            PortAMode::SwitchBlock1
                        }
                        MachineType::IBM_XT_5160 => {
                            // On 5160, port is always kb byte
                            PortAMode::KeyboardByte
                        }
                    };

                }
                else {
                    //log::trace!("PPI: Presenting keyboard byte on Port A");
                    self.port_a_mode = PortAMode::KeyboardByte;
                }

                if self.pb_byte & PORTB_PULL_KB_LOW == 0 {
                    log::trace!("PPI: Pulling keyboard clock LOW");
                    self.kb_clock_low = true;
                    self.kb_counting_low = true;
                }
                else if self.kb_clock_low {
                    log::trace!("PPI: Keyboard clock resume HIGH");
                    self.kb_clock_low = false;

                    if self.kb_low_count > KB_RESET_CYCLES {
                        // Clock line was low long enough to trigger reset
                        // Start timer until reset byte is sent
                        self.kb_low_count;
                        self.kb_been_reset = true;
                        self.kb_count_until_reset_byte = 0;
                    }
                }
            },
            PPI_PORT_C => {
                // Read-only port
            },
            PPI_COMMAND_PORT => {
                self.handle_command_port_write(byte);
            }
            _ => panic!("PPI: Bad port #")
        }
    }
    fn read_u16(&mut self, _port: u16) -> u16 {
        log::error!("Invalid 16-bit read from PPI");
        0   
    }
    fn write_u16(&mut self, _port: u16, _data: u16) {
        log::error!("Invalid 16-bit write to PPI");
    }
}

impl Ppi {

    pub fn handle_command_port_write(&mut self, byte: u8) {
        log::trace!("PPI: Write to command port: {:02X}", byte);
    }

    pub fn send_keyboard(&mut self, byte: u8 ) {
        self.kb_byte = byte;
    }

    pub fn calc_port_c_value(&self) -> u8 {
        let timer_bit = (self.timer_in as u8) << 4;
        let speaker_bit = (self.speaker_in as u8) << 5;

        match (&self.machine_type, &self.port_c_mode) {
            (MachineType::IBM_PC_5150, PortCMode::Switch2OneToFour) => {
                // We aren't implementing the cassette on 5150, and we'll never have parity errors
                (self.dip_sw2 & 0x0F) | timer_bit
            }
            (MachineType::IBM_PC_5150, PortCMode::Switch2FiveToEight) => {
                // On 5150, only Switch Block 2, Switch #5 is accurately passed through
                // If Port C is in Switch Block 2 mode, 6, 7, 8 and will read high (off)
                (self.dip_sw2 >> 5 & 0x01) | timer_bit
            }
            (MachineType::IBM_XT_5160, PortCMode::Switch2OneToFour) => {
                // Cassette data line has been replaced with a speaker monitor line.
                (self.dip_sw2 & 0x0F) | speaker_bit | timer_bit             
            }
            (MachineType::IBM_XT_5160, PortCMode::Switch2FiveToEight) => {
                // Cassette data line has been replaced with a speaker monitor line.
                // On 5160, all four switches 5-8 are readable
                (self.dip_sw2 >> 5 & 0x0F) | speaker_bit | timer_bit             
            }
        }
    }

    pub fn get_string_state(&self) -> PpiStringState {
        
        let port_a_value = match self.port_a_mode {
            PortAMode::SwitchBlock1 => {
                self.dip_sw1
            }
            PortAMode::KeyboardByte => {
                self.kb_byte
            }
        };
        let port_c_value = self.calc_port_c_value();

        PpiStringState {
            port_a_mode: format!("{:?}", self.port_a_mode),
            port_a_value_bin: format!("{:08b}", port_a_value),
            port_a_value_hex: format!("{:02X}", port_a_value),
            kb_byte_value_hex: format!("{:02X}", self.kb_byte),
            port_c_mode: format!("{:?}", self.port_c_mode),
            port_c_value: format!("{:08b}", port_c_value )
        }
    }

    pub fn run(&mut self, pic: &mut pic::Pic, cycles: u32 ) {

        // Our keyboard byte was read, so clear the interrupt request line and reset the byte
        // read at the keyboard IO port to 0
        if self.clear_keyboard {
            self.clear_keyboard = false;
            self.kb_byte = 0;
            pic.clear_interrupt(1);
            log::trace!("PPI: Clearing keyboard");
        }

        // Keyboard should send a 'aa' byte when clock line is held low (for how long?)
        // BIOS waits 20ms. We consider ourselves reset after 10ms
        // Clock line must go high again
        if self.kb_counting_low && self.kb_low_count < KB_RESET_CYCLES {
            self.kb_low_count += cycles;
        }

        // Send reset byte after delay elapsed. The delay gives the BIOS POST routines
        // time to check for interrupts as they do not do it immediately 
        if self.kb_been_reset {
            self.kb_count_until_reset_byte += cycles;

            if self.kb_count_until_reset_byte > KB_RESET_CYCLE_DELAY {
                self.kb_been_reset = false;
                self.kb_count_until_reset_byte = 0;

                log::trace!("PPI: Sending keyboard reset byte");
                self.kb_byte = 0xAA;
                // Bios KB check expects a reset byte to generate a KB interrupt
                pic.request_interrupt(1);
            }
        }
    }
}