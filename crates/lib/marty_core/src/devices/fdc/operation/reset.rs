/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------
*/

use crate::{
    bus::BusInterface,
    devices::{
        dma,
        fdc::controller::{AckCondition, FloppyController, Operation, FDC_RESET_TIME},
    },
};

use super::{FdcOperation, FdcOperationMode, FdcOperationState};

pub(in crate::devices::fdc) struct ResetOperation {
    state: FdcOperationState,
}

impl ResetOperation {
    pub(in crate::devices::fdc) fn new() -> Self {
        Self {
            state: FdcOperationState::new(),
        }
    }
}

impl FdcOperation for ResetOperation {
    fn init(&mut self, _fdc: &mut FloppyController, _dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        if self.state.is_uninitialized() {
            self.state.begin_wait(FDC_RESET_TIME, FdcOperationMode::Complete);
        }
    }

    fn run(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        if self.state.is_complete() {
            return;
        }

        if self.state.is_uninitialized() {
            self.init(fdc, dma, bus);
        }

        if matches!(self.state.run(us), FdcOperationMode::Complete) {
            log::trace!("FDC Operation Reset complete.");
            fdc.reset_internal(true);
            for drive_status in &mut fdc.drive_status {
                drive_status.unack_condition = Some(AckCondition::ReadyPoll);
            }
            fdc.raise_sense_interrupt();
            fdc.operation = Operation::NoOperation;
        }
    }

    fn is_complete(&self) -> bool {
        self.state.is_complete()
    }

    fn operation_type(&self) -> Operation {
        Operation::Reset
    }
}
