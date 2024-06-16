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

    devices::ega::io.rs

    Implementation of the IoDevice interface trait for the IBM EGA card.

*/
use super::*;
use crate::bus::{BusInterface, DeviceRunTimeUnit, IoDevice};

impl IoDevice for EGACard {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port {
            INPUT_STATUS_REGISTER_0 => self.read_input_status_register_0(),
            INPUT_STATUS_REGISTER_1 => {
                // Don't answer this port if we are in MDA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => 0xFF,
                    IoAddressSelect::CompatCGA => self.read_input_status_register_1(),
                }
            }
            INPUT_STATUS_REGISTER_1_MDA => {
                // Don't respond on this port if we are in CGA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => self.read_input_status_register_1(),
                    IoAddressSelect::CompatCGA => 0xFF,
                }
            }
            //MODE_CONTROL_REGISTER => {
            //    log::error!("Read from write-only mode control register");
            //    0
            //}
            CRTC_REGISTER => {
                // Don't answer this port if we are in MDA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => 0xFF,
                    IoAddressSelect::CompatCGA => self.read_input_status_register_1(),
                }
            }
            CRTC_REGISTER_MDA => {
                // Don't respond on this port if we are in CGA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => self.crtc.read_crtc_register(),
                    IoAddressSelect::CompatCGA => 0xFF,
                }
            }
            _ => {
                0xFF // Open bus
            }
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        match port {
            MISC_OUTPUT_REGISTER => {
                self.write_external_misc_output_register(data);
            }
            //MODE_CONTROL_REGISTER => {
            //    self.handle_mode_register(data);
            //}
            CRTC_REGISTER_ADDRESS | CRTC_REGISTER_ADDRESS_MDA => {
                self.crtc.write_crtc_register_address(data);
            }
            CRTC_REGISTER | CRTC_REGISTER_MDA => {
                let (recalc, clear_intr) = self.crtc.write_crtc_register_data(data);
                if recalc {
                    self.recalculate_mode();
                }
                if clear_intr {
                    self.intr = false;
                }
            }
            EGA_GRAPHICS_1_POSITION => self.gc.write_graphics_position(1, data),
            EGA_GRAPHICS_2_POSITION => self.gc.write_graphics_position(2, data),
            EGA_GRAPHICS_ADDRESS => self.gc.write_graphics_address(data),
            EGA_GRAPHICS_DATA => self.gc.write_graphics_data(data),
            SEQUENCER_ADDRESS_REGISTER => self.sequencer.write_address(data),
            SEQUENCER_DATA_REGISTER => {
                self.sequencer.write_data(data);
                self.recalculate_mode();
            }
            ATTRIBUTE_REGISTER | ATTRIBUTE_REGISTER_ALT => {
                self.ac.write_attribute_register(data);
                self.recalculate_mode();
            }
            FEATURE_CONTROL_REGISTER => {
                self.feature_bits = data & 0x03;
            }
            //COLOR_CONTROL_REGISTER => {
            //    self.handle_cc_register_write(data);
            //}
            _ => {}
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        if self.dip_sw.get_physical_state() == EGA_DIP_SWITCH_MDA {
            vec![
                (String::from("EGA Attribute Register"), ATTRIBUTE_REGISTER),
                (String::from("EGA Attribute Register"), ATTRIBUTE_REGISTER_ALT),
                (String::from("EGA Misc Output Register"), MISC_OUTPUT_REGISTER),
                (String::from("EGA Input Status Register 0"), INPUT_STATUS_REGISTER_0),
                (
                    String::from("EGA Input Status Register 1 (MDA)"),
                    INPUT_STATUS_REGISTER_1_MDA,
                ),
                (
                    String::from("EGA Sequencer Address Register"),
                    SEQUENCER_ADDRESS_REGISTER,
                ),
                (String::from("EGA Sequencer Data Register"), SEQUENCER_DATA_REGISTER),
                (String::from("EGA CRTC Address (MDA)"), CRTC_REGISTER_ADDRESS_MDA),
                (String::from("EGA CRTC Data (MDA)"), CRTC_REGISTER_MDA),
                (String::from("EGA Graphics 1 Position"), EGA_GRAPHICS_1_POSITION),
                (String::from("EGA Graphics 2 Position"), EGA_GRAPHICS_2_POSITION),
                (String::from("EGA Graphics Address"), EGA_GRAPHICS_ADDRESS),
                (String::from("EGA Graphics Data"), EGA_GRAPHICS_DATA),
            ]
        }
        else {
            vec![
                (String::from("EGA Attribute Register"), ATTRIBUTE_REGISTER),
                (String::from("EGA Attribute Register"), ATTRIBUTE_REGISTER_ALT),
                (String::from("EGA Misc Output Register"), MISC_OUTPUT_REGISTER),
                (String::from("EGA Input Status Register 0"), INPUT_STATUS_REGISTER_0),
                (String::from("EGA Input Status Register 1"), INPUT_STATUS_REGISTER_1),
                (
                    String::from("EGA Input Status Register 1 (MDA)"),
                    INPUT_STATUS_REGISTER_1_MDA,
                ),
                (
                    String::from("EGA Sequencer Address Register"),
                    SEQUENCER_ADDRESS_REGISTER,
                ),
                (String::from("EGA Sequencer Data Register"), SEQUENCER_DATA_REGISTER),
                (String::from("EGA CRTC Address"), CRTC_REGISTER_ADDRESS),
                (String::from("EGA CRTC Data"), CRTC_REGISTER),
                (String::from("EGA CRTC Address (MDA)"), CRTC_REGISTER_ADDRESS_MDA),
                (String::from("EGA CRTC Data (MDA)"), CRTC_REGISTER_MDA),
                (String::from("EGA Graphics 1 Position"), EGA_GRAPHICS_1_POSITION),
                (String::from("EGA Graphics 2 Position"), EGA_GRAPHICS_2_POSITION),
                (String::from("EGA Graphics Address"), EGA_GRAPHICS_ADDRESS),
                (String::from("EGA Graphics Data"), EGA_GRAPHICS_DATA),
            ]
        }
    }
}
