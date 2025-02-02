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
use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    cpu_common::LogicAnalyzer,
};

impl IoDevice for VGACard {
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
            VGA_GRAPHICS_ADDRESS => self.gc.read_address(),
            VGA_GRAPHICS_DATA => self.gc.read_data(),
            VGA_SEQUENCER_ADDRESS_REGISTER => self.sequencer.read_address(),
            VGA_SEQUENCER_DATA_REGISTER => self.sequencer.read_data(),
            VGA_ATTRIBUTE_REGISTER | VGA_ATTRIBUTE_REGISTER_ALT => self.ac.read_attribute_register(),
            VGA_CRTC_REGISTER_ADDRESS => self.crtc.read_crtc_register_address(),
            VGA_CRTC_REGISTER => {
                // Don't answer this port if we are in MDA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => 0xFF,
                    IoAddressSelect::CompatCGA => self.crtc.read_crtc_register_data(),
                }
            }
            MDA_CRTC_REGISTER => {
                // Don't respond on this port if we are in CGA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => self.crtc.read_crtc_register_data(),
                    IoAddressSelect::CompatCGA => 0xFF,
                }
            }
            PEL_ADDRESS_WRITE_MODE => self.ac.read_pel_address_write_mode(),
            PEL_DATA => self.ac.read_pel_data(),
            DAC_STATE_REGISTER => {
                // Read only register
                self.ac.read_color_dac_state()
            }
            PEL_MASK => self.ac.read_pel_mask(),
            _ => {
                0xFF // Open bus
            }
        }
    }

    fn write_u8(
        &mut self,
        port: u16,
        data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        match port {
            MISC_OUTPUT_REGISTER => {
                self.write_external_misc_output_register(data);
            }
            //MODE_CONTROL_REGISTER => {
            //    self.handle_mode_register(data);
            //}
            VGA_CRTC_REGISTER_ADDRESS | MDA_CRTC_REGISTER_ADDRESS => {
                self.crtc.write_crtc_register_address(data);
            }
            VGA_CRTC_REGISTER | MDA_CRTC_REGISTER => {
                let (recalc, clear_intr) = self.crtc.write_crtc_register_data(data);
                if recalc {
                    self.recalculate_mode();
                }
                if clear_intr {
                    self.intr = false;
                }
            }
            VGA_GRAPHICS_ADDRESS => self.gc.write_address(data),
            VGA_GRAPHICS_DATA => self.gc.write_data(data),
            VGA_SEQUENCER_ADDRESS_REGISTER => self.sequencer.write_address(data),
            VGA_SEQUENCER_DATA_REGISTER => {
                self.sequencer.write_data(data);
                self.recalculate_mode();
            }
            VGA_ATTRIBUTE_REGISTER | VGA_ATTRIBUTE_REGISTER_ALT => {
                self.ac.write_attribute_register(data);
                self.recalculate_mode();
            }
            FEATURE_CONTROL_REGISTER => {
                self.feature_bits = data & 0x03;
            }
            PEL_ADDRESS_WRITE_MODE => self.ac.write_pel_address_write_mode(data),
            PEL_ADDRESS_READ_MODE => self.ac.write_pel_address_read_mode(data),
            PEL_DATA => self.ac.write_pel_data(data),
            PEL_MASK => self.ac.write_pel_mask(data),
            _ => {}
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        if self.dip_sw.get_physical_state() == EGA_DIP_SWITCH_MDA {
            vec![
                (String::from("EGA Attribute Register"), VGA_ATTRIBUTE_REGISTER),
                (String::from("EGA Attribute Register"), VGA_ATTRIBUTE_REGISTER_ALT),
                (String::from("EGA Misc Output Register"), MISC_OUTPUT_REGISTER),
                (String::from("EGA Input Status Register 0"), INPUT_STATUS_REGISTER_0),
                (
                    String::from("EGA Input Status Register 1 (MDA)"),
                    INPUT_STATUS_REGISTER_1_MDA,
                ),
                (
                    String::from("EGA Sequencer Address Register"),
                    VGA_SEQUENCER_ADDRESS_REGISTER,
                ),
                (String::from("EGA Sequencer Data Register"), VGA_SEQUENCER_DATA_REGISTER),
                (String::from("EGA CRTC Address (MDA)"), MDA_CRTC_REGISTER_ADDRESS),
                (String::from("EGA CRTC Data (MDA)"), MDA_CRTC_REGISTER),
                (String::from("EGA Graphics Address"), VGA_GRAPHICS_ADDRESS),
                (String::from("EGA Graphics Data"), VGA_GRAPHICS_DATA),
            ]
        }
        else {
            vec![
                (String::from("VGA Attribute Register"), VGA_ATTRIBUTE_REGISTER),
                (String::from("VGA Attribute Register"), VGA_ATTRIBUTE_REGISTER_ALT),
                (String::from("VGA Misc Output Register"), MISC_OUTPUT_REGISTER),
                (String::from("VGA Input Status Register 0"), INPUT_STATUS_REGISTER_0),
                (String::from("VGA Input Status Register 1"), INPUT_STATUS_REGISTER_1),
                (
                    String::from("VGA Input Status Register 1 (MDA)"),
                    INPUT_STATUS_REGISTER_1_MDA,
                ),
                (
                    String::from("VGA Sequencer Address Register"),
                    VGA_SEQUENCER_ADDRESS_REGISTER,
                ),
                (String::from("VGA Sequencer Data Register"), VGA_SEQUENCER_DATA_REGISTER),
                (String::from("VGA CRTC Address"), VGA_CRTC_REGISTER_ADDRESS),
                (String::from("VGA CRTC Data"), VGA_CRTC_REGISTER),
                (String::from("MDA CRTC Address (VGA)"), MDA_CRTC_REGISTER_ADDRESS),
                (String::from("MDA CRTC Data (VGA)"), MDA_CRTC_REGISTER),
                (String::from("VGA Graphics Address"), VGA_GRAPHICS_ADDRESS),
                (String::from("VGA Graphics Data"), VGA_GRAPHICS_DATA),
                (String::from("VGA Pel Address Read"), PEL_ADDRESS_READ_MODE),
                (String::from("VGA Pel Address Write"), PEL_ADDRESS_WRITE_MODE),
                (String::from("VGA Pel Data"), PEL_DATA),
                (String::from("VGA Pel Mask"), PEL_MASK),
                (String::from("VGA DAC State"), DAC_STATE_REGISTER),
            ]
        }
    }
}
