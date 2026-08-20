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

use fluxfox::prelude::{DiskCh, DiskChs};

use crate::{
    bus::BusInterface,
    devices::{
        dma,
        fdc::{
            controller::{DriveError, FloppyController, InterruptCode, Operation, FDC_DMA},
            data_interface::{FdcDataDirection, FdcDataError, FdcDataMode},
        },
    },
};

use super::{FdcOperation, FdcOperationMode, FdcOperationState};

const FORMAT_DESCRIPTOR_SIZE: usize = 4;

pub(in crate::devices::fdc) struct FormatTrackOperation {
    operation_type: Operation,
    state: FdcOperationState,
    format_buffer: Vec<u8>,
}

impl FormatTrackOperation {
    pub(in crate::devices::fdc) fn new(h: u8, n: u8, track_len: u8, gap3_len: u8, fill_byte: u8) -> Self {
        Self {
            operation_type: Operation::FormatTrack(h, n, track_len, gap3_len, fill_byte),
            state: FdcOperationState::new(),
            format_buffer: Vec::with_capacity(track_len as usize * FORMAT_DESCRIPTOR_SIZE),
        }
    }

    fn finish(&mut self, fdc: &mut FloppyController, interrupt_code: InterruptCode, result_chs: DiskChs) {
        self.state.final_chs = result_chs;
        self.state.mark_complete();

        let Operation::FormatTrack(_, n, _, _, _) = self.operation_type
        else {
            unreachable!("FormatTrackOperation must have FormatTrack operation type")
        };

        // The uPD765 documentation specifies that the ID information in the
        // Format Track result phase has no meaning.
        fdc.send_results_phase(interrupt_code, fdc.drive_select, result_chs, n, true);
        fdc.operation = Operation::NoOperation;
        fdc.dma_byte_count = 0;
        fdc.dma_bytes_left = 0;
        fdc.pio_bytes_left = 0;
        fdc.pio_byte_count = 0;
        fdc.pio_sector_byte_count = 0;
        fdc.data_interface.end();
        fdc.operation_init = false;
        fdc.operation_status_valid = false;
    }

    fn finish_with_error(&mut self, fdc: &mut FloppyController, message: &str) {
        log::warn!("{}", message);
        fdc.log_str(message);
        self.finish(fdc, InterruptCode::AbnormalTermination, self.state.current_chs);
    }

    fn format_track(&mut self, fdc: &mut FloppyController) {
        let Operation::FormatTrack(h, _, _, gap3_len, fill_byte) = self.operation_type
        else {
            unreachable!("FormatTrackOperation must have FormatTrack operation type")
        };

        let ch = DiskCh::new(fdc.drives[fdc.drive_select].chsn.c(), h);
        match fdc.drives[fdc.drive_select].format_track(ch, &self.format_buffer, gap3_len, fill_byte) {
            Ok(result) => {
                log::trace!(
                    "FormatTrackOperation: formatted {} sectors, next sector ID: {}",
                    result.sectors_formatted,
                    result.new_sid
                );
                self.finish(fdc, InterruptCode::NormalTermination, DiskChs::default());
            }
            Err(err) => {
                self.finish_with_error(fdc, &format!("FormatTrackOperation: format failed: {:#}", err));
            }
        }
    }

    fn step(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        if self.format_buffer.len() >= self.state.xfer_size_bytes {
            self.format_track(fdc);
            return;
        }

        match fdc.data_interface.recv(dma, bus) {
            Ok(byte) => {
                self.format_buffer.push(byte);

                if fdc.in_dma {
                    fdc.dma_byte_count = fdc.data_interface.byte_count();
                    fdc.dma_bytes_left = self.state.xfer_size_bytes.saturating_sub(fdc.dma_byte_count);
                }
                else {
                    fdc.pio_byte_count += 1;
                    fdc.pio_sector_byte_count = (fdc.pio_sector_byte_count + 1) % FORMAT_DESCRIPTOR_SIZE;
                    fdc.pio_bytes_left = fdc.pio_bytes_left.saturating_sub(1);
                }

                if self.format_buffer.len() >= self.state.xfer_size_bytes {
                    self.format_track(fdc);
                }
                else if fdc.data_interface.terminal_count() {
                    self.finish_with_error(
                        fdc,
                        "FormatTrackOperation: terminal count occurred before all CHRN descriptors were received",
                    );
                }
            }
            // The data interface owns whether the next byte is supplied by DMA
            // or by a CPU data-register write.
            Err(FdcDataError::Timeout) => {}
            Err(err) => {
                self.finish_with_error(fdc, &format!("FormatTrackOperation: data interface error: {:?}", err));
            }
        }
    }
}

impl FdcOperation for FormatTrackOperation {
    fn init(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        if !self.state.is_uninitialized() {
            return;
        }

        let Operation::FormatTrack(h, _, track_len, _, _) = self.operation_type
        else {
            unreachable!("FormatTrackOperation must have FormatTrack operation type")
        };

        let current_chs = DiskChs::from((fdc.drives[fdc.drive_select].chsn.c(), h, 1));
        self.state.reset_position(current_chs, h);
        self.state.xfer_size_sectors = track_len as usize;
        self.state.xfer_size_bytes = track_len as usize * FORMAT_DESCRIPTOR_SIZE;

        if fdc.drives[fdc.drive_select].write_protected {
            fdc.last_error = DriveError::WriteProtect;
            self.finish(fdc, InterruptCode::AbnormalTermination, current_chs);
            return;
        }

        if self.state.xfer_size_bytes == 0 {
            self.finish_with_error(fdc, "FormatTrackOperation: command specified zero sectors");
            return;
        }

        if fdc.in_dma {
            let dma_size = dma.get_dma_transfer_size(FDC_DMA);
            if dma_size < self.state.xfer_size_bytes {
                self.finish_with_error(
                    fdc,
                    &format!(
                        "FormatTrackOperation: DMA transfer size {} is smaller than the {}-byte descriptor list",
                        dma_size, self.state.xfer_size_bytes
                    ),
                );
                return;
            }
            fdc.dma_byte_count = 0;
            fdc.dma_bytes_left = self.state.xfer_size_bytes;
        }
        else {
            fdc.pio_byte_count = 0;
            fdc.pio_sector_byte_count = 0;
            fdc.pio_bytes_left = self.state.xfer_size_bytes;
        }

        fdc.xfer_size_bytes = self.state.xfer_size_bytes;
        fdc.operation_status_valid = false;
        fdc.data_interface.begin(
            if fdc.in_dma { FdcDataMode::Dma } else { FdcDataMode::Pio },
            FdcDataDirection::FromHost,
        );
        fdc.operation_init = true;
        self.state.begin_scan(FdcOperationMode::Transferring);
    }

    fn run(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        if self.state.is_uninitialized() {
            self.init(fdc, dma, bus);
            return;
        }
        if self.state.is_complete() {
            return;
        }

        if fdc.data_interface.pio_service_timed_out() {
            self.finish_with_error(fdc, "FormatTrackOperation: PIO byte service timeout");
            return;
        }

        if !matches!(self.state.run(us), FdcOperationMode::Transferring) {
            return;
        }

        let elapsed_periods = self.state.byte_periods_elapsed(us, fdc.data_rate.byte_period_us());
        for _ in 0..elapsed_periods {
            self.step(fdc, dma, bus);
            if self.state.is_complete() {
                break;
            }
        }
    }

    fn is_complete(&self) -> bool {
        self.state.is_complete()
    }

    fn operation_type(&self) -> Operation {
        self.operation_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::fdc::controller::{ST0_ABNORMAL_TERMINATION, ST1_WRITE_PROTECT};

    #[test]
    fn pio_format_descriptors_are_received_through_data_interface() {
        let mut fdc = FloppyController::default();
        fdc.in_dma = false;
        fdc.drives[0].write_protected = false;
        let mut operation = FormatTrackOperation::new(0, 2, 2, 0x2A, 0xF6);
        fdc.operation = operation.operation_type();

        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);

        assert!(fdc.data_interface.active());
        assert_eq!(fdc.data_interface.mode(), FdcDataMode::Pio);
        assert_eq!(fdc.data_interface.direction(), FdcDataDirection::FromHost);
        assert_eq!(operation.state.xfer_size_bytes, 8);

        fdc.data_interface.cpu_write(0x00).unwrap();
        operation.run(&mut fdc, &mut dma, &mut bus, 100.0);

        assert_eq!(operation.format_buffer, vec![0x00]);
        assert_eq!(fdc.pio_byte_count, 1);
        assert_eq!(fdc.pio_bytes_left, 7);
    }

    #[test]
    fn write_protect_uses_abnormal_termination() {
        let mut fdc = FloppyController::default();
        fdc.in_dma = false;
        fdc.drives[0].write_protected = true;
        let mut operation = FormatTrackOperation::new(0, 2, 9, 0x2A, 0xF6);
        fdc.operation = operation.operation_type();

        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);

        let status = fdc.get_debug_state().last_status;
        assert_eq!(status[0] & 0xC0, ST0_ABNORMAL_TERMINATION);
        assert_ne!(status[1] & ST1_WRITE_PROTECT, 0);
    }
}
