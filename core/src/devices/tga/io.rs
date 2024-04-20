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

    devices::tga::io.rs

    Implementation of the IoDevice interface trait for the TGA card.

*/
use super::*;
use crate::bus::IoDevice;

// CRTC registers are mirrored from 0x3D0 - 0x3D5 due to incomplete
// address decoding.
pub const CRTC_REGISTER_SELECT0: u16 = 0x3D0;
pub const CRTC_REGISTER0: u16 = 0x3D1;
pub const CRTC_REGISTER_SELECT1: u16 = 0x3D2;
pub const CRTC_REGISTER1: u16 = 0x3D3;
pub const CRTC_REGISTER_SELECT2: u16 = 0x3D4;
pub const CRTC_REGISTER2: u16 = 0x3D5;

pub const CRTC_REGISTER_BASE: u16 = 0x3D0;
pub const CRTC_REGISTER_MASK: u16 = 0x007;

pub const CGA_MODE_CONTROL_REGISTER: u16 = 0x3D8;
pub const CGA_COLOR_CONTROL_REGISTER: u16 = 0x3D9;
pub const CGA_STATUS_REGISTER: u16 = 0x3DA;
pub const CGA_LIGHTPEN_LATCH_RESET: u16 = 0x3DB;
pub const CGA_LIGHTPEN_LATCH_SET: u16 = 0x3DC;

pub const TGA_VIDEO_ARRAY_ADDRESS: u16 = 0x3DA;
pub const TGA_VIDEO_ARRAY_DATA: u16 = 0x3DE;
pub const TGA_PAGE_REGISTER: u16 = 0x3DF;

impl IoDevice for TGACard {
    fn read_u8(&mut self, port: u16, delta: DeviceRunTimeUnit) -> u8 {
        // Catch up to CPU state.
        //let _ticks = self.catch_up(delta, false);

        //self.rw_op(ticks, 0, port as u32, RwSlotType::Io);

        if (port & !CRTC_REGISTER_MASK) == CRTC_REGISTER_BASE {
            // Read is from CRTC register.
            if port & 0x01 != 0 {
                self.handle_crtc_register_read()
            }
            else {
                0
            }
        }
        else {
            match port {
                CGA_MODE_CONTROL_REGISTER => {
                    log::error!("CGA: Read from Mode control register!");
                    0
                }
                CGA_STATUS_REGISTER => self.handle_status_register_read(),
                CGA_LIGHTPEN_LATCH_RESET => {
                    self.clear_lp_latch();
                    0
                }
                CGA_LIGHTPEN_LATCH_SET => {
                    self.set_lp_latch();
                    0
                }
                _ => 0,
            }
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, delta: DeviceRunTimeUnit) {
        let debug_port = (if port == 0x3D5 { true } else { false }) && self.debug;

        // Catch up to CPU state.
        //let _ticks = self.catch_up(delta, debug_port);

        //self.rw_op(ticks, data, port as u32, RwSlotType::Io);

        if (port & !CRTC_REGISTER_MASK) == CRTC_REGISTER_BASE {
            // Write is to CRTC register.
            if port & 0x01 == 0 {
                self.handle_crtc_register_select(data);
            }
            else {
                self.handle_crtc_register_write(data);
                self.recalc_extents();
            }
        }
        else {
            match port {
                CGA_MODE_CONTROL_REGISTER => {
                    self.handle_mode_register(data);
                }
                CGA_COLOR_CONTROL_REGISTER => {
                    self.handle_cc_register_write(data);
                }
                CGA_LIGHTPEN_LATCH_RESET => self.clear_lp_latch(),
                CGA_LIGHTPEN_LATCH_SET => {
                    log::debug!("wrote latch set register");
                    self.set_lp_latch()
                }
                TGA_VIDEO_ARRAY_ADDRESS => {
                    // One of the minor differences between Tandy and PCJr:
                    // PCJr uses a flip/flop for both address and data via 3DA.
                    // Tandy uses 3DE as a dedicated data port.
                    match self.subtype {
                        VideoCardSubType::Tandy1000 => {
                            self.video_array_select(data);
                        }
                        VideoCardSubType::IbmPCJr => {
                            match self.address_flipflop {
                                true => self.video_array_select(data),
                                false => self.video_array_write(data),
                            }
                            self.address_flipflop = !self.address_flipflop;
                        }
                        _ => {
                            unreachable!("TGA: Invalid subtype for TGA");
                        }
                    }
                }
                TGA_VIDEO_ARRAY_DATA => {
                    self.video_array_write(data);
                }
                TGA_PAGE_REGISTER => {
                    self.page_register_write(data);
                }
                _ => {}
            }
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        let mut ports = vec![
            (String::from("TGA CRTC Address"), CRTC_REGISTER_SELECT0),
            (String::from("TGA CRTC Data"), CRTC_REGISTER0),
            (String::from("TGA CRTC Address"), CRTC_REGISTER_SELECT1),
            (String::from("TGA CRTC Data"), CRTC_REGISTER1),
            (String::from("TGA CRTC Address"), CRTC_REGISTER_SELECT2),
            (String::from("TGA CRTC Data"), CRTC_REGISTER2),
            (String::from("TGA Lightpen Latch Reset"), CGA_LIGHTPEN_LATCH_RESET),
            (String::from("TGA Lightpen Latch Set"), CGA_LIGHTPEN_LATCH_SET),
            (String::from("TGA Status Register"), CGA_STATUS_REGISTER),
            (String::from("TGA Page Register"), TGA_PAGE_REGISTER),
        ];

        // One of the minor differences between Tandy and PCJr:
        // PCJr uses a flip/flop for both address and data via 3DA.
        // Tandy uses 3DE as a dedicated data port.
        if matches!(self.subtype, VideoCardSubType::Tandy1000) {
            ports.push((String::from("TGA Mode Control Register"), CGA_MODE_CONTROL_REGISTER));
            ports.push((String::from("TGA Color Control Register"), CGA_COLOR_CONTROL_REGISTER));
            ports.push((String::from("TGA Video Array Data"), TGA_VIDEO_ARRAY_DATA));
        }

        ports
    }
}
