
/*
    MartyPC Emulator 
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

    --------------------------------------------------------------------------

    devices::cga::mmio.rs

    Implementation of the IoDevice interface trait for the IBM CGA card.

*/

use crate::devices::cga::*;
use crate::bus::{IoDevice};

// CRTC registers are mirrored from 0x3D0 - 0x3D5 due to incomplete
// address decoding.
pub const CRTC_REGISTER_SELECT0: u16        = 0x3D0;
pub const CRTC_REGISTER0: u16               = 0x3D1;
pub const CRTC_REGISTER_SELECT1: u16        = 0x3D2;
pub const CRTC_REGISTER1: u16               = 0x3D3;
pub const CRTC_REGISTER_SELECT2: u16        = 0x3D4;
pub const CRTC_REGISTER2: u16               = 0x3D5;

pub const CRTC_REGISTER_BASE: u16           = 0x3D0;
pub const CRTC_REGISTER_MASK: u16           = 0x007;

pub const CGA_MODE_CONTROL_REGISTER: u16    = 0x3D8;
pub const CGA_COLOR_CONTROL_REGISTER: u16   = 0x3D9;
pub const CGA_STATUS_REGISTER: u16          = 0x3DA;
pub const CGA_LIGHTPEN_REGISTER: u16        = 0x3DB;

impl IoDevice for CGACard {
    fn read_u8(&mut self, port: u16, delta: DeviceRunTimeUnit) -> u8 {

        // Catch up to CPU state.
        self.catch_up(delta);

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
                CGA_STATUS_REGISTER => {
                    self.handle_status_register_read()
                }
                _ => {
                    0
                }
            }
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, delta: DeviceRunTimeUnit) {

        // Catch up to CPU state.
        self.catch_up(delta);

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
                _ => {}
            }
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![
            CRTC_REGISTER_SELECT0,
            CRTC_REGISTER0,
            CRTC_REGISTER_SELECT1,
            CRTC_REGISTER1,
            CRTC_REGISTER_SELECT2,
            CRTC_REGISTER2,
            CGA_MODE_CONTROL_REGISTER,
            CGA_COLOR_CONTROL_REGISTER,
            CGA_LIGHTPEN_REGISTER,
            CGA_STATUS_REGISTER,
        ]
    }

}
