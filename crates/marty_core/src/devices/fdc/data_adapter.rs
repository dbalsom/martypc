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
*/

//! The DataAdapter is designed to abstract away the differences between DMA and non-DMA (PIO)
//! transfers for a floppy disk controller implementation.

use crate::devices::fdc::controller::DataMode;

#[derive(Copy, Clone, Debug)]
pub enum TransferState {
    Idle,
    Reading,
    Writing,
}

pub struct DataAdapter {
    pub data_mode: DataMode,
    pub state: TransferState,
    pub data: Vec<u8>,
    pub data_cursor: usize,
}

impl DataAdapter {
    pub fn mode(&self) -> DataMode {
        self.data_mode
    }
    pub fn set_mode(&mut self, mode: DataMode) {
        self.data_mode = mode;
    }

    pub fn state(&self) -> TransferState {
        self.state
    }
    pub fn set_state(&mut self, state: TransferState) {
        self.state = state;
    }

    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
        self.data_cursor = 0;
    }
}
