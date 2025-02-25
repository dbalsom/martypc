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

    devices::types::chs.rs

    Defines the CHS type to be used by FDC and HDC implementations
*/

use std::fmt::Display;

#[derive(Copy, Clone, Debug)]
pub struct DiskChs {
    pub c: u8,
    pub h: u8,
    pub s: u8,
}

impl Default for DiskChs {
    fn default() -> Self {
        Self { c: 0, h: 0, s: 1 }
    }
}

impl From<(u8, u8, u8)> for DiskChs {
    fn from((c, h, s): (u8, u8, u8)) -> Self {
        Self { c, h, s }
    }
}

impl From<DiskChs> for (u8, u8, u8) {
    fn from(chs: DiskChs) -> Self {
        (chs.c, chs.h, chs.s)
    }
}

impl From<fluxfox::prelude::DiskChs> for DiskChs {
    fn from(chs: fluxfox::prelude::DiskChs) -> Self {
        Self {
            c: chs.c() as u8,
            h: chs.h(),
            s: chs.s(),
        }
    }
}

impl Display for DiskChs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[c:{} h:{} s:{}]", self.c, self.h, self.s)
    }
}

impl DiskChs {
    pub fn new(c: u8, h: u8, s: u8) -> Self {
        Self { c, h, s }
    }

    pub fn get(&self) -> (u8, u8, u8) {
        (self.c, self.h, self.s)
    }
    pub fn c(&self) -> u8 {
        self.c
    }
    pub fn h(&self) -> u8 {
        self.h
    }
    pub fn s(&self) -> u8 {
        self.s
    }

    pub fn set(&mut self, c: u8, h: u8, s: u8) {
        self.c = c;
        self.h = h;
        self.s = s;
    }
    pub fn set_c(&mut self, c: u8) {
        self.c = c;
    }
    pub fn set_h(&mut self, h: u8) {
        self.h = h;
    }
    pub fn set_s(&mut self, s: u8) {
        self.s = s;
    }

    /// Seek to the specified CHS. This should be called over 'set' as eventually it will calculate appropriate
    /// timings.
    pub fn seek(&mut self, c: u8, h: u8, s: u8) {
        self.seek_to(&DiskChs::from((c, h, s)));
    }

    /// Seek to the specified CHS. This should be called over 'set' as eventually it will calculate appropriate
    /// timings.
    pub fn seek_to(&mut self, dst_chs: &DiskChs) {
        self.c = dst_chs.c;
        self.h = dst_chs.h;
        self.s = dst_chs.s;
    }

    pub fn get_sector_count(&self) -> u32 {
        (self.c as u32) * (self.h as u32) * (self.s as u32)
    }

    /// Convert the CHS to an LBA address. A reference drive geometry is required to calculate the LBA.
    pub fn to_lba(&self, geom: &DiskChs) -> usize {
        let hpc = geom.h as usize;
        let spt = geom.s as usize;
        (self.c as usize * hpc + (self.h as usize)) * spt + (self.s as usize - 1)
    }

    /// Return a new CHS that is the next sector on the disk.
    /// If the current CHS is the last sector on the disk, the next CHS will be the first sector on the disk.
    fn get_next_sector(&self, geom: &DiskChs) -> DiskChs {
        if self.s < geom.s {
            // Not at last sector, just return next sector
            DiskChs::from((self.c, self.h, self.s + 1))
        }
        else if self.h < geom.h - 1 {
            // At last sector, but not at last head, go to next head, same cylinder, sector 1
            DiskChs::from((self.c, self.h + 1, 1))
        }
        else if self.c < geom.c - 1 {
            // At last sector and last head, go to next cylinder, head 0, sector 1
            DiskChs::from((self.c + 1, 0, 1))
        }
        else {
            // Return start of drive? TODO: Research what does this do on real hardware
            DiskChs::from((0, 0, 1))
        }
    }

    pub fn seek_forward(&mut self, sectors: u32, geom: &DiskChs) -> &mut Self {
        for _i in 0..sectors {
            *self = self.get_next_sector(geom);
        }
        self
    }
}
