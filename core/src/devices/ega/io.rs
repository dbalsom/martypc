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
            CRTC_REGISTER_ADDRESS => {
                self.crtc.write_crtc_register_address(data);
            }
            CRTC_REGISTER => {
                self.crtc.write_crtc_register_data(data);
                self.recalculate_mode();
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
            //COLOR_CONTROL_REGISTER => {
            //    self.handle_cc_register_write(data);
            //}
            _ => {}
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![
            ATTRIBUTE_REGISTER,
            ATTRIBUTE_REGISTER_ALT,
            MISC_OUTPUT_REGISTER,
            INPUT_STATUS_REGISTER_0,
            INPUT_STATUS_REGISTER_1,
            INPUT_STATUS_REGISTER_1_MDA,
            SEQUENCER_ADDRESS_REGISTER,
            SEQUENCER_DATA_REGISTER,
            CRTC_REGISTER_ADDRESS,
            CRTC_REGISTER,
            CRTC_REGISTER_ADDRESS_MDA,
            CRTC_REGISTER_MDA,
            EGA_GRAPHICS_1_POSITION,
            EGA_GRAPHICS_2_POSITION,
            EGA_GRAPHICS_ADDRESS,
            EGA_GRAPHICS_DATA,
        ]
    }
}
