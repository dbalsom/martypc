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

use anyhow::{anyhow, Error};
use fluxfox::prelude::DiskChs;

use crate::{
    bus::BusInterface,
    devices::{
        dma,
        fdc::{
            controller::{DataOperationParameters, FloppyController, InterruptCode, Operation, FDC_DMA},
            data_interface::{FdcDataDirection, FdcDataError, FdcDataMode},
        },
        floppy_drive::{FloppyDriveOperation, OperationStatus},
    },
};

use super::{FdcOperation, FdcOperationMode, FdcOperationState};

enum ReadDataLoad {
    Loaded,
    Finished,
}

pub(in crate::devices::fdc) struct ReadDataOperation {
    params: DataOperationParameters,
    state: FdcOperationState,
    sector_buf: Vec<u8>,
    sector_buf_idx: usize,
    initial_sector_pending: bool,
    terminate_after_sector: bool,
    normal_interrupt: bool,
}

impl ReadDataOperation {
    pub(in crate::devices::fdc) fn new(params: DataOperationParameters) -> Self {
        Self {
            params,
            state: FdcOperationState::new_transfer(params.id_chs, params.physical_head, params.eot, params.multi_track),
            sector_buf: Vec::new(),
            sector_buf_idx: 0,
            initial_sector_pending: true,
            terminate_after_sector: false,
            normal_interrupt: false,
        }
    }

    fn merge_sector_status(&mut self, fdc: &mut FloppyController, mut status: OperationStatus, skipped: bool) {
        if skipped {
            status.data_crc_error = false;
        }
        // ST2's Control Mark bit reports a mismatch between the command and
        // the sector's data-address mark. For Read Deleted Data, a deleted
        // mark is therefore the expected case rather than an error.
        status.deleted_mark = status.deleted_mark != self.params.deleted_data;
        fdc.merge_operation_status(status);
    }

    fn load_next_sector(&mut self, fdc: &mut FloppyController) -> Result<ReadDataLoad, Error> {
        if self.state.completed_sectors >= self.state.xfer_size_sectors {
            self.state.final_chs = self.state.current_chs;
            return Ok(ReadDataLoad::Finished);
        }

        let skip_flag = self.params.skip;
        let mut skipped_sectors = 0usize;
        let max_skips = self.state.sectors_to_eot();

        loop {
            let read_result = fdc.drives[self.params.drive].read_sector(
                self.state.current_phys_head,
                self.state.current_chs,
                self.params.sector_size,
            )?;
            let data_mark_mismatch = read_result.status.deleted_mark != self.params.deleted_data;
            let skipped_sector = skip_flag && data_mark_mismatch;
            self.merge_sector_status(fdc, read_result.status, skipped_sector);

            if read_result.not_found || read_result.status.no_dam || read_result.status.address_crc_error {
                self.state.final_chs = self.state.current_chs;
                return Err(anyhow!(
                    "Failed to read sector {}: not_found={}, no_dam={}, address_crc_error={}",
                    self.state.current_chs,
                    read_result.not_found,
                    read_result.status.no_dam,
                    read_result.status.address_crc_error
                ));
            }

            if skipped_sector {
                skipped_sectors += 1;
                let advanced = self.state.advance_chs();

                if skipped_sectors > max_skips || !advanced {
                    self.state.final_chs = self.state.current_chs;
                    return Ok(ReadDataLoad::Finished);
                }
                continue;
            }

            self.terminate_after_sector = data_mark_mismatch || read_result.status.data_crc_error;
            self.sector_buf = read_result.data;
            self.sector_buf_idx = 0;

            if self.sector_buf.is_empty() {
                self.state.final_chs = self.state.current_chs;
                return Err(anyhow!("Sector {} contained no data", self.state.current_chs));
            }

            return Ok(ReadDataLoad::Loaded);
        }
    }

    fn complete_sector(&mut self) -> bool {
        let terminate = self.terminate_after_sector;
        self.terminate_after_sector = false;

        self.state.mt = self.params.multi_track;
        self.state.complete_sector(terminate)
    }

    fn finish(
        &mut self,
        fdc: &mut FloppyController,
        interrupt_code: InterruptCode,
        final_chs: DiskChs,
        raise_interrupt: bool,
    ) {
        self.state.final_chs = final_chs;
        self.state.mark_complete();

        fdc.send_results_phase(
            interrupt_code,
            self.params.drive,
            self.state.final_chs,
            self.params.sector_size,
            raise_interrupt,
        );
        fdc.drives[self.params.drive].chsn.set_chs(self.state.final_chs);
        fdc.operation = Operation::NoOperation;
        fdc.read_sector_buf.clear();
        fdc.read_sector_buf_idx = 0;
        fdc.dma_byte_count = 0;
        fdc.dma_bytes_left = 0;
        fdc.pio_bytes_left = 0;
        fdc.pio_byte_count = 0;
        fdc.pio_sector_byte_count = 0;
        fdc.data_interface.end();
        fdc.operation_init = false;
        fdc.operation_status_valid = false;
    }

    fn completion_interrupt_code(fdc: &FloppyController) -> InterruptCode {
        let status = fdc.operation_status;
        if status.sector_not_found || status.address_crc_error || status.data_crc_error || status.no_dam {
            InterruptCode::AbnormalTermination
        }
        else {
            InterruptCode::NormalTermination
        }
    }

    fn load_or_finish_next_sector(&mut self, fdc: &mut FloppyController) -> bool {
        match self.load_next_sector(fdc) {
            Ok(ReadDataLoad::Loaded) => false,
            Ok(ReadDataLoad::Finished) => {
                let interrupt_code = Self::completion_interrupt_code(fdc);
                self.finish(fdc, interrupt_code, self.state.final_chs, self.normal_interrupt);
                true
            }
            Err(e) => {
                log::warn!("ReadDataOperation: sector read failed: {:#}", e);
                self.finish(fdc, InterruptCode::AbnormalTermination, self.state.final_chs, true);
                true
            }
        }
    }

    fn finish_on_terminal_count(&mut self, fdc: &mut FloppyController) {
        let byte_count = fdc.data_interface.byte_count();
        fdc.log_str(&format!(
            "DMA terminal count triggered end of Sector Read operation, {} bytes read.",
            byte_count
        ));
        let interrupt_code = Self::completion_interrupt_code(fdc);
        self.finish(fdc, interrupt_code, self.state.current_chs, true);
    }

    fn init_transfer(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController) {
        let sector_size_decoded = FloppyController::decode_sector_size(self.params.sector_size);

        if fdc.in_dma {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);
            if !xfer_size.is_multiple_of(sector_size_decoded) {
                log::warn!(
                    "DMA word count {} not multiple of sector size ({})",
                    xfer_size,
                    sector_size_decoded
                );
            }

            let mut xfer_sectors = xfer_size / sector_size_decoded;
            if xfer_sectors == 0 && xfer_size > 0 {
                fdc.log_str(&format!(
                    "DMA programmed for transfer of partial sector: ({} bytes)",
                    xfer_size
                ));
                xfer_sectors = 1;
            }
            else if xfer_sectors == 0 {
                log::warn!("DMA not programmed for transfer!");
                fdc.log_str("DMA not programmed for transfer!");
            }

            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            fdc.log_str(&format!(
                "DMA transfer: sectors:{} bytes:{} sector_size:{} dst:{:05X}",
                xfer_sectors, xfer_size, sector_size_decoded, dst_address
            ));

            self.state.xfer_size_sectors = xfer_sectors;
            self.state.xfer_size_bytes = xfer_sectors * sector_size_decoded;
            fdc.xfer_size_bytes = self.state.xfer_size_bytes;
            fdc.dma_bytes_left = self.state.xfer_size_bytes;
            fdc.dma_byte_count = 0;
        }
        else {
            self.state.xfer_size_sectors = self.state.sectors_to_eot();
            self.state.xfer_size_bytes = self.state.xfer_size_sectors * sector_size_decoded;
            fdc.xfer_size_bytes = self.state.xfer_size_bytes;
            fdc.pio_bytes_left = self.state.xfer_size_bytes;
            fdc.pio_byte_count = 0;
            fdc.pio_sector_byte_count = 0;
        }
    }

    fn step(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        let sector_ready_to_complete = !self.sector_buf.is_empty()
            && self.sector_buf_idx >= self.sector_buf.len()
            && fdc.data_interface.latch_empty();

        if sector_ready_to_complete {
            if self.complete_sector() {
                let interrupt_code = Self::completion_interrupt_code(fdc);
                self.finish(fdc, interrupt_code, self.state.final_chs, self.normal_interrupt);
                return;
            }

            if fdc.data_interface.terminal_count() {
                self.finish_on_terminal_count(fdc);
                return;
            }

            if self.load_or_finish_next_sector(fdc) {
                return;
            }
        }

        if fdc.data_interface.terminal_count() {
            self.finish_on_terminal_count(fdc);
            return;
        }

        if self.sector_buf_idx >= self.sector_buf.len() {
            return;
        }

        let byte = self.sector_buf[self.sector_buf_idx];
        match fdc.data_interface.send(byte, dma, bus) {
            Ok(()) => {
                self.sector_buf_idx += 1;
            }
            Err(FdcDataError::Timeout) => {
                log::warn!("ReadDataOperation: data interface timeout");
                self.finish(fdc, InterruptCode::AbnormalTermination, self.state.final_chs, true);
            }
            Err(err) => {
                log::warn!("ReadDataOperation: data interface error: {:?}", err);
                self.finish(fdc, InterruptCode::AbnormalTermination, self.state.final_chs, true);
            }
        }
    }
}

impl FdcOperation for ReadDataOperation {
    fn init(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        if !self.state.is_uninitialized() {
            return;
        }

        self.normal_interrupt = fdc.in_dma;
        self.state.reset_position(self.params.id_chs, self.params.physical_head);
        self.state.mt = self.params.multi_track;
        self.sector_buf.clear();
        self.sector_buf_idx = 0;
        self.initial_sector_pending = true;
        self.terminate_after_sector = false;
        self.init_transfer(fdc, dma);

        fdc.operation_status.reset(FloppyDriveOperation::ReadData);
        fdc.operation_status_valid = true;
        fdc.data_interface.begin(
            if fdc.in_dma { FdcDataMode::Dma } else { FdcDataMode::Pio },
            FdcDataDirection::ToHost,
        );
        fdc.operation_init = true;

        self.state.begin_scan(FdcOperationMode::Transferring);
    }

    fn run(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        if self.state.is_uninitialized() {
            self.init(fdc, dma, bus);
            return;
        }

        if fdc.data_interface.pio_service_timed_out() {
            const TIMEOUT_MSG: &str = "ReadDataOperation: PIO byte service timeout";
            log::warn!("{}", TIMEOUT_MSG);
            fdc.log_str(TIMEOUT_MSG);
            self.finish(fdc, InterruptCode::AbnormalTermination, self.state.final_chs, true);
            return;
        }

        if !matches!(self.state.run(us), FdcOperationMode::Transferring) {
            return;
        }

        if self.initial_sector_pending {
            self.initial_sector_pending = false;
            if self.load_or_finish_next_sector(fdc) {
                return;
            }
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
        Operation::ReadData(self.params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        machine_config::FloppyDriveConfig,
        machine_types::{FdcType, FloppyDriveType},
    };
    use fluxfox::prelude::StandardFormat;

    fn read_params(id_chs: DiskChs, eot: u8, multi_track: bool, deleted_data: bool) -> DataOperationParameters {
        DataOperationParameters {
            physical_head: id_chs.h(),
            id_chs,
            sector_size: 2,
            eot,
            multi_track,
            deleted_data,
            ..Default::default()
        }
    }

    fn fdc_with_360k_image() -> FloppyController {
        let mut fdc = FloppyController::new(
            FdcType::IbmNec,
            vec![FloppyDriveConfig {
                fd_type: FloppyDriveType::Floppy360K,
                image:   None,
            }],
        );
        fdc.create_new_image(0, StandardFormat::PcFloppy360, true).unwrap();
        fdc
    }

    #[test]
    fn read_operation_owns_command_parameters() {
        let params = DataOperationParameters {
            drive: 1,
            physical_head: 1,
            id_chs: DiskChs::from((38, 0, 8)),
            sector_size: 2,
            eot: 9,
            gap3_len: 0x2A,
            data_len: 0xFF,
            multi_track: true,
            mfm: true,
            skip: true,
            deleted_data: true,
        };
        let operation = ReadDataOperation::new(params);

        assert_eq!(operation.operation_type(), Operation::ReadData(params));
        assert_eq!(operation.params, params);
        assert_eq!(operation.state.current_phys_head, 1);
        assert_eq!(operation.state.current_chs, DiskChs::from((38, 0, 8)));
        assert!(operation.state.mt);
    }

    #[test]
    fn single_sector_at_eot_reports_next_sector_not_next_head() {
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((15, 0, 1)), 1, true, false));
        op.state.current_chs = DiskChs::from((15, 0, 1));
        op.state.current_phys_head = 0;
        op.state.completed_sectors = 0;
        op.state.xfer_size_sectors = 1;

        assert!(op.complete_sector());
        assert_eq!(op.state.final_chs.c(), 15);
        assert_eq!(op.state.final_chs.h(), 0);
        assert_eq!(op.state.final_chs.s(), 2);
    }

    #[test]
    fn multi_track_advances_head_only_when_continuing() {
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((15, 0, 1)), 1, true, false));
        op.state.current_chs = DiskChs::from((15, 0, 1));
        op.state.current_phys_head = 0;
        op.state.completed_sectors = 0;
        op.state.xfer_size_sectors = 2;

        assert!(!op.complete_sector());
        assert_eq!(op.state.current_chs.c(), 15);
        assert_eq!(op.state.current_chs.h(), 1);
        assert_eq!(op.state.current_chs.s(), 1);
        assert_eq!(op.state.final_chs.c(), 15);
        assert_eq!(op.state.final_chs.h(), 1);
        assert_eq!(op.state.final_chs.s(), 1);
    }

    #[test]
    fn eot_zero_does_not_end_the_operation_before_sector_id_wraps() {
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((15, 0, 2)), 0, true, false));
        op.state.current_chs = DiskChs::from((15, 0, 2));
        op.state.current_phys_head = 0;
        op.state.completed_sectors = 0;
        op.state.xfer_size_sectors = 2;

        assert!(!op.complete_sector());
        assert_eq!(op.state.current_chs.c(), 15);
        assert_eq!(op.state.current_chs.h(), 0);
        assert_eq!(op.state.current_chs.s(), 3);
        assert_eq!(op.state.final_chs.c(), 15);
        assert_eq!(op.state.final_chs.h(), 0);
        assert_eq!(op.state.final_chs.s(), 3);
    }

    #[test]
    fn skipped_sector_advancement_treats_eot_zero_as_an_ordinary_sector_id() {
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((15, 0, 2)), 0, true, false));
        op.state.current_chs = DiskChs::from((15, 0, 2));
        op.state.current_phys_head = 0;
        op.state.completed_sectors = 0;
        op.state.xfer_size_sectors = 1;

        assert!(op.state.advance_chs());
        assert_eq!(op.state.current_chs.c(), 15);
        assert_eq!(op.state.current_chs.h(), 0);
        assert_eq!(op.state.current_chs.s(), 3);

        assert!(op.complete_sector());
        assert_eq!(op.state.final_chs.c(), 15);
        assert_eq!(op.state.final_chs.h(), 0);
        assert_eq!(op.state.final_chs.s(), 4);
    }

    #[test]
    fn skipped_deleted_sector_suppresses_data_crc_status() {
        let mut fdc = FloppyController::default();
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((0, 0, 1)), 1, false, false));
        let status = OperationStatus {
            deleted_mark: true,
            data_crc_error: true,
            ..Default::default()
        };

        op.merge_sector_status(&mut fdc, status, true);

        assert!(fdc.operation_status.deleted_mark);
        assert!(!fdc.operation_status.data_crc_error);
    }

    #[test]
    fn unskipped_deleted_sector_preserves_data_crc_status() {
        let mut fdc = FloppyController::default();
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((0, 0, 1)), 1, false, false));
        let status = OperationStatus {
            deleted_mark: true,
            data_crc_error: true,
            ..Default::default()
        };

        op.merge_sector_status(&mut fdc, status, false);

        assert!(fdc.operation_status.deleted_mark);
        assert!(fdc.operation_status.data_crc_error);
    }

    #[test]
    fn read_deleted_data_treats_deleted_mark_as_expected() {
        let mut fdc = FloppyController::default();
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((0, 0, 1)), 1, false, true));

        op.merge_sector_status(
            &mut fdc,
            OperationStatus {
                deleted_mark: true,
                ..Default::default()
            },
            false,
        );

        assert!(!fdc.operation_status.deleted_mark);
    }

    #[test]
    fn read_deleted_data_reports_normal_mark_as_control_mark() {
        let mut fdc = FloppyController::default();
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((0, 0, 1)), 1, false, true));

        op.merge_sector_status(&mut fdc, OperationStatus::default(), false);

        assert!(fdc.operation_status.deleted_mark);
    }

    #[test]
    fn data_crc_error_selects_abnormal_completion_code() {
        let mut fdc = FloppyController::default();
        fdc.operation_status.reset(FloppyDriveOperation::ReadData);

        assert!(matches!(
            ReadDataOperation::completion_interrupt_code(&fdc),
            InterruptCode::NormalTermination
        ));

        fdc.operation_status.data_crc_error = true;

        assert!(matches!(
            ReadDataOperation::completion_interrupt_code(&fdc),
            InterruptCode::AbnormalTermination
        ));
    }

    #[test]
    fn initial_sector_lookup_waits_for_scan_timeout() {
        let mut fdc = fdc_with_360k_image();
        fdc.in_dma = true;
        let mut op = ReadDataOperation::new(read_params(DiskChs::from((0, 0, 10)), 10, false, false));
        fdc.operation = op.operation_type();

        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        let scan_timeout_us = op.state.scan_timeout_us;

        op.run(&mut fdc, &mut dma, &mut bus, 0.0);
        assert!(!op.is_complete());
        assert!(op.initial_sector_pending);
        assert!(!fdc.has_internal_interrupt_pending());

        op.run(&mut fdc, &mut dma, &mut bus, scan_timeout_us - 1.0);
        assert!(!op.is_complete());
        assert!(op.initial_sector_pending);
        assert!(!fdc.has_internal_interrupt_pending());

        op.run(&mut fdc, &mut dma, &mut bus, 1.0);
        assert!(op.is_complete());
        assert!(!op.initial_sector_pending);
        assert!(fdc.has_internal_interrupt_pending());
    }
}
