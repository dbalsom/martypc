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
