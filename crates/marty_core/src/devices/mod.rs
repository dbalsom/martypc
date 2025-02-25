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

    devices::mod.rs

    Module to organize all device implementations

*/

pub mod a0;
#[cfg(feature = "opl")]
pub mod adlib;
pub mod cartridge_slots;
pub mod cga;
pub mod dipswitch;
pub mod dma;
#[cfg(feature = "ega")]
pub mod ega;
pub mod fdc;
pub mod floppy_drive;
pub mod game_port;
pub mod hdc;
pub mod keyboard;
pub mod lotech_ems;
pub mod lpt_card;
pub mod lpt_port;
pub mod mc6845;
pub mod mda;
pub mod mouse;
pub mod null_sound;
pub mod pic;
pub mod pit;
pub mod ppi;
pub mod serial;
pub mod tga;
#[cfg(feature = "vga")]
pub mod vga;
