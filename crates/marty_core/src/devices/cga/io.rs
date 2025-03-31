/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    devices::cga::io.rs

    Implementation of the IoDevice interface trait for the IBM CGA card.

*/
use super::*;
use crate::{bus::IoDevice, cpu_common::LogicAnalyzer};

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

impl IoDevice for CGACard {
    fn read_u8(&mut self, port: u16, delta: DeviceRunTimeUnit) -> u8 {
        // Catch up to CPU state.
        let _ticks = self.catch_up(delta, false);

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

    fn write_u8(
        &mut self,
        port: u16,
        data: u8,
        _bus: Option<&mut BusInterface>,
        delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        let debug_port = port == 0x3D5 && self.debug;

        // Catch up to CPU state.
        let _ticks = self.catch_up(delta, debug_port);

        //self.rw_op(ticks, data, port as u32, RwSlotType::Io);

        if (port & !CRTC_REGISTER_MASK) == CRTC_REGISTER_BASE {
            // Write is to CRTC register.
            if port & 0x01 == 0 {
                self.handle_crtc_register_select(data);
            }
            else {
                self.handle_crtc_register_write(data);
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
                _ => {}
            }
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![
            ("CRTC Address".into(), CRTC_REGISTER_SELECT0),
            ("CRTC Data".into(), CRTC_REGISTER0),
            ("CRTC Address".into(), CRTC_REGISTER_SELECT1),
            ("CRTC Data".into(), CRTC_REGISTER1),
            ("CRTC Address".into(), CRTC_REGISTER_SELECT2),
            ("CRTC Data".into(), CRTC_REGISTER2),
            ("CGA Mode Control".into(), CGA_MODE_CONTROL_REGISTER),
            ("CGA Color Control".into(), CGA_COLOR_CONTROL_REGISTER),
            ("CGA LP Latch Reset".into(), CGA_LIGHTPEN_LATCH_RESET),
            ("CGA LP Latch Set".into(), CGA_LIGHTPEN_LATCH_SET),
            ("CGA Status".into(), CGA_STATUS_REGISTER),
        ]
    }
}
