/*
    ppi.rc
    Implement the 8255 PPI (Programmable Peripheral Interface)

    Doesn't do a whole lot other than reading DIP switches and reporing on some 
    system status (Parity errors)
*/
use crate::io::{IoBusInterface, IoDevice};

pub const PPI_PORT_A: u16 = 0x60;
pub const PPI_PORT_B: u16 = 0x61;
pub const PPI_PORT_C: u16 = 0x62;

// Dipswitch information from
// http://www.minuszerodegrees.net/5150/ram/5150_ram_16_64_SW2.jpg

pub const SW1_NO_FLOPPIES: u8    = 0b0000_0001;
pub const SW1_NO_8087: u8        = 0b0000_0010;
// MDA = 5=OFF, 6=OFF
pub const SW1_HAVE_MDA: u8       = 0b0000_0000;
// CGA LoRes = 5=OFF, 6=ON
pub const SW1_HAVE_CGA_LORES: u8 = 0b0010_0000;
// CGA HiRes = 5=ON, 6=OFF
pub const SW1_HAVE_CGA_HIRES: u8 = 0b0001_0000;
// EGA/VGA = 5=ON, 6=ON
pub const SW1_HAVE_EGA: u8       = 0b0011_0000;
// Floppy drive count
pub const SW1_ONE_FLOPPY: u8     = 0b1100_0000;
pub const SW1_TWO_FLOPPIES: u8   = 0b1000_0000;
pub const SW1_THREE_FLOPPIES: u8 = 0b0100_0000;
pub const SW1_FOUR_FLOPPIES: u8  = 0b0000_0000;

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
pub const SW2_RAM_576K: u8       = 0b0000_1111;
pub const SW2_RAM_608K: u8       = 0b0000_1110;
pub const SW2_RAM_640K: u8       = 0b0000_1101;
pub const SW2_5: u8              = 0b0001_0000;

// PORT B INPUTS
pub const PORTB_TIMER2_GATE: u8  = 0b0000_0001;
pub const PORTB_SPEAKER_DATA: u8 = 0b0000_0010;
pub const PORTB_SW2_SELECT: u8   = 0b0000_0100;
pub const PORTB_CASSETTE: u8     = 0b0000_1000;
pub const PORTB_PARITY_MB_EN: u8 = 0b0001_0000;
pub const PORTB_PARITY_EX_EN: u8 = 0b0010_0000;
pub const PORTB_PULL_KB_LOW: u8  = 0b0100_0000;
pub const PRESENT_SW1_PORTA: u8  = 0b1000_0000;

pub struct Ppi {
    parity_error: bool,
    have_kb_byte: bool,
    present_sw1: bool,
    present_sw2_04: bool,
    kb_byte: u8,
    dip_sw1: u8,
    dip_sw2: u8
}

impl Ppi {

    pub fn new() -> Self {
        Self {
            parity_error: false,
            have_kb_byte: false,
            present_sw1: true,
            present_sw2_04: true,
            kb_byte: 0,
            dip_sw1: SW1_NO_8087 | SW1_HAVE_CGA_HIRES | SW1_TWO_FLOPPIES,
            dip_sw2: SW2_RAM_544K
        }
    }
}

impl IoDevice for Ppi {
    fn read_u8(&mut self, port: u16) -> u8 {
        //println!("PPI Read from port: {:04X}", port);
        match port {
            PPI_PORT_A => {
                if self.present_sw1 {
                    // If no KB byte pending, Port A represents SW1 Dipswitches
                    self.dip_sw1
                }
                else {
                    self.kb_byte
                }
            },
            PPI_PORT_B => {0},
            PPI_PORT_C => {

                let mut byte = 0;
                if self.present_sw2_04 {
                    byte |= self.dip_sw1 & 0x0F;
                }
                else {
                    // Present status of SW2 dip 5 on PC01
                    if self.dip_sw2 & SW2_5 != 0 {
                        byte |= 0x01;
                    }
                    // When presenting bit 5, PC1-3 read high
                    byte |= 0x0E;
                }

                // Emulate parity failure
                if self.parity_error {
                    byte |= 0xC0;
                }

                byte
            },
            _ => panic!("PPI: Bad port #")
        }
    }
    fn write_u8(&mut self, port: u16, data: u8) {
        match port {
            PPI_PORT_A => {},
            PPI_PORT_B => {
                log::debug!("PPI: Write to Port B: {:02X}", data);

                if data & PORTB_SW2_SELECT != 0 {
                    // If Bit 2 is ON, PC0-PC3 represent SW2 S1-S4
                    // If Bit 2 is OFF, PC0 is SW2 S5, and PC01, PC02, PC03 will read ON
                    self.present_sw2_04 = true;

                }
                if data & PRESENT_SW1_PORTA != 0 {
                    self.present_sw1 = true;
                }
            },
            PPI_PORT_C => {},
            _ => panic!("PPI: Bad port #")
        }
    }
    fn read_u16(&mut self, port: u16) -> u16 {
        log::error!("Invalid 16-bit read from PPI");
        0   
    }
    fn write_u16(&mut self, port: u16, data: u16) {
        log::error!("Invalid 16-bit write to PPI");
    }
}