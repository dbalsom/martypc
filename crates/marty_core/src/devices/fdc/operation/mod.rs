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

//! FDC operation trait and shared operation state.

mod format_track;
mod read_data;
mod read_track;
mod reset;
mod seek;
mod write_data;

pub(super) use format_track::FormatTrackOperation;
pub(super) use read_data::ReadDataOperation;
pub(super) use read_track::ReadTrackOperation;
pub(super) use reset::ResetOperation;
#[allow(unused_imports)]
pub(super) use seek::SeekOperation;
pub(super) use write_data::WriteDataOperation;

/// Approximate delay while the controller scans the track for the requested
/// sector ID. This is intentionally shorter than a realistic rotational delay;
/// it provides a distinct pre-transfer phase until track position is modeled.
pub(super) const FDC_SECTOR_SCAN_TIME_US: f64 = 100.0;

use fluxfox::prelude::DiskChs;

use crate::{
    bus::BusInterface,
    devices::{
        dma,
        fdc::controller::{FloppyController, Operation},
    },
};

pub(super) trait FdcOperation {
    fn init(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface);
    fn run(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64);
    fn is_complete(&self) -> bool;
    fn operation_type(&self) -> Operation;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum FdcOperationMode {
    Uninitialized,
    Waiting(f64),
    Seeking,
    Scanning,
    Transferring,
    Complete,
}

#[derive(Clone, Debug)]
pub(super) struct FdcOperationState {
    mode: FdcOperationMode,
    next_mode: FdcOperationMode,
    pub(super) scan_accumulator: f64,
    pub(super) scan_timeout_us: f64,
    pub(super) byte_accumulator: f64,
    pub(super) current_chs: DiskChs,
    pub(super) final_chs: DiskChs,
    pub(super) current_phys_head: u8,
    pub(super) eot: u8,
    pub(super) mt: bool,
    pub(super) xfer_size_sectors: usize,
    pub(super) xfer_size_bytes: usize,
    pub(super) completed_sectors: usize,
}

impl FdcOperationState {
    pub(super) fn new() -> Self {
        Self {
            mode: FdcOperationMode::Uninitialized,
            next_mode: FdcOperationMode::Complete,
            scan_accumulator: 0.0,
            scan_timeout_us: FDC_SECTOR_SCAN_TIME_US,
            byte_accumulator: 0.0,
            current_chs: DiskChs::default(),
            final_chs: DiskChs::default(),
            current_phys_head: 0,
            eot: 0,
            mt: false,
            xfer_size_sectors: 0,
            xfer_size_bytes: 0,
            completed_sectors: 0,
        }
    }

    pub(super) fn new_transfer(chs: DiskChs, phys_head: u8, eot: u8, mt: bool) -> Self {
        Self {
            current_chs: chs,
            final_chs: chs,
            current_phys_head: phys_head,
            eot,
            mt,
            ..Self::new()
        }
    }

    pub(super) fn result_chs_after_sector(chs: DiskChs) -> DiskChs {
        let mut result_chs = chs;
        result_chs.set_s(result_chs.s().wrapping_add(1));
        result_chs
    }

    pub(super) fn reset_position(&mut self, chs: DiskChs, phys_head: u8) {
        self.current_chs = chs;
        self.final_chs = chs;
        self.current_phys_head = phys_head;
        self.completed_sectors = 0;
    }

    pub(super) fn sectors_to_eot(&self) -> usize {
        let current_head_sectors = self.eot.wrapping_sub(self.current_chs.s()) as usize + 1;
        if self.mt && self.current_phys_head == 0 && self.current_chs.h() == 0 {
            current_head_sectors + self.eot.wrapping_sub(1) as usize + 1
        }
        else {
            current_head_sectors
        }
    }

    pub(super) fn advance_chs(&mut self) -> bool {
        if self.current_chs.s() == self.eot {
            if self.mt && self.current_phys_head == 0 && self.current_chs.h() == 0 {
                self.current_phys_head = 1;
                self.current_chs.set_h(1);
                self.current_chs.set_s(1);
                return true;
            }
            return false;
        }

        self.advance_sector_id();
        true
    }

    pub(super) fn advance_sector_id(&mut self) {
        self.current_chs.set_s(self.current_chs.s().wrapping_add(1));
    }

    pub(super) fn complete_sector(&mut self, terminate: bool) -> bool {
        self.completed_sectors += 1;

        let result_chs = Self::result_chs_after_sector(self.current_chs);
        if terminate || self.completed_sectors >= self.xfer_size_sectors {
            self.final_chs = result_chs;
            return true;
        }

        let advanced = self.advance_chs();
        self.final_chs = match advanced {
            true => self.current_chs,
            false => result_chs,
        };

        !advanced
    }

    pub(super) fn is_uninitialized(&self) -> bool {
        self.mode == FdcOperationMode::Uninitialized
    }

    pub(super) fn mark_complete(&mut self) {
        self.mode = FdcOperationMode::Complete
    }

    pub(super) fn is_complete(&self) -> bool {
        matches!(self.mode, FdcOperationMode::Complete)
    }

    pub(super) fn begin_scan(&mut self, next_mode: FdcOperationMode) {
        debug_assert_eq!(self.mode, FdcOperationMode::Uninitialized);

        self.mode = FdcOperationMode::Scanning;
        self.next_mode = next_mode;
        self.scan_accumulator = 0.0;
    }

    pub(super) fn begin_seek(&mut self) {
        debug_assert_eq!(self.mode, FdcOperationMode::Uninitialized);

        self.mode = FdcOperationMode::Seeking;
    }

    pub(super) fn begin_wait(&mut self, us: f64, next_mode: FdcOperationMode) {
        debug_assert_eq!(self.mode, FdcOperationMode::Uninitialized);

        self.mode = FdcOperationMode::Waiting(us.max(0.0));
        self.next_mode = next_mode;
    }

    /// Advances operation state and returns the current updated operation mode
    pub(super) fn run(&mut self, us: f64) -> FdcOperationMode {
        match self.mode {
            FdcOperationMode::Waiting(remaining_us) => {
                if us >= remaining_us {
                    self.mode = self.next_mode;
                    self.next_mode = FdcOperationMode::Complete;
                }
                else {
                    self.mode = FdcOperationMode::Waiting(remaining_us - us);
                }
            }
            FdcOperationMode::Scanning => {
                self.scan_accumulator += us;
                if self.scan_accumulator >= self.scan_timeout_us {
                    self.mode = FdcOperationMode::Transferring;
                }
            }
            _ => {}
        }

        self.mode
    }

    pub(super) fn byte_periods_elapsed(&mut self, us: f64, byte_period_us: f64) -> usize {
        self.byte_accumulator += us;
        let mut periods = 0;
        while self.byte_accumulator >= byte_period_us {
            self.byte_accumulator -= byte_period_us;
            periods += 1;
        }
        periods
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_phase_consumes_time_before_transfer() {
        let mut state = FdcOperationState::new_transfer(DiskChs::default(), 0, 1, false);

        assert_eq!(state.mode, FdcOperationMode::Uninitialized);
        state.begin_scan(FdcOperationMode::Transferring);
        assert_eq!(state.mode, FdcOperationMode::Scanning);
        assert!(matches!(state.run(40.0), FdcOperationMode::Scanning));
        assert_eq!(state.mode, FdcOperationMode::Scanning);
        assert!(matches!(state.run(60.0), FdcOperationMode::Transferring));
        assert_eq!(state.mode, FdcOperationMode::Transferring);
        state.run(1.0);
        assert_eq!(state.byte_accumulator, 0.0);
    }

    #[test]
    fn completing_operation_replaces_current_mode() {
        let mut state = FdcOperationState::new_transfer(DiskChs::default(), 0, 1, false);
        state.begin_scan(FdcOperationMode::Complete);
        state.mark_complete();

        assert_eq!(state.mode, FdcOperationMode::Complete);
    }

    #[test]
    fn waiting_mode_expires_after_requested_time() {
        let mut state = FdcOperationState::new();
        state.begin_wait(100.0, FdcOperationMode::Complete);

        state.run(40.0);
        assert_eq!(state.mode, FdcOperationMode::Waiting(60.0));
        state.run(60.0);
        assert_eq!(state.mode, FdcOperationMode::Complete);
    }
}
