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

    lib.rs

    Main emulator core 

*/

pub mod devices;

pub mod breakpoints;
pub mod bus;
pub mod bytebuf;
pub mod bytequeue;
pub mod config;
pub mod cpu_common;
pub mod cpu_808x;
pub mod floppy_manager;
pub mod file_util;
pub mod interrupt;
pub mod machine;
pub mod machine_manager;
pub mod memerror;
pub mod rom_manager;
pub mod sound;
pub mod syntax_token;
pub mod tracelogger;
pub mod updatable;
pub mod util;

pub mod vhd;
pub mod vhd_manager;
pub mod videocard; // VideoCard trait
pub mod input;

pub mod cpu_validator; // CpuValidator trait

#[cfg(feature = "arduino_validator")]
#[macro_use]
pub mod arduino8088_client;
#[cfg(feature = "arduino_validator")]
#[macro_use]
pub mod arduino8088_validator;
