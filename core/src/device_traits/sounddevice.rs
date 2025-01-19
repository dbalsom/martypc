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

    device_traits::sounddevice.rs

    Defines the SoundDevice trait which any sound device (Adlib,SoundBlaster, etc)
    must implement.
*/

#[cfg(feature = "opl")]
use crate::devices::adlib::AdLibCard;
use crate::devices::null_sound::NullSoundDevice;
use enum_dispatch::enum_dispatch;

pub type AudioSample = f32;

#[enum_dispatch]
pub enum SoundDispatch {
    #[cfg(feature = "opl")]
    AdLibCard,
    NullSoundDevice,
}

#[enum_dispatch(SoundDispatch)]
pub trait SoundDevice {
    fn run(&mut self, usec: f64);
}
