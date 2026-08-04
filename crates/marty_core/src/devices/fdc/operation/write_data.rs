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

use fluxfox::prelude::{DiskChs, DiskChsn};

use crate::{
    bus::BusInterface,
    devices::{
        dma,
        fdc::{
            controller::{DataOperationParameters, DriveError, FloppyController, InterruptCode, Operation, FDC_DMA},
            data_interface::{FdcDataDirection, FdcDataError, FdcDataMode},
        },
        floppy_drive::FloppyDriveOperation,
    },
};

use super::{FdcOperation, FdcOperationMode, FdcOperationState};

pub(in crate::devices::fdc) struct WriteDataOperation {
    params: DataOperationParameters,
    state: FdcOperationState,
    sector_buf: Vec<u8>,
    normal_interrupt: bool,
}

impl WriteDataOperation {
    pub(in crate::devices::fdc) fn new(params: DataOperationParameters) -> Self {
        Self {
            params,
            state: FdcOperationState::new_transfer(params.id_chs, params.physical_head, params.eot, params.multi_track),
            sector_buf: Vec::new(),
            normal_interrupt: false,
        }
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
            final_chs,
            self.params.sector_size,
            raise_interrupt,
        );
        fdc.drives[self.params.drive].chsn.set_chs(final_chs);
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
        self.finish(fdc, InterruptCode::AbnormalTermination, self.state.current_chs, true);
    }

    fn init_transfer(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController) -> bool {
        let sector_size = DiskChsn::n_to_bytes(self.params.sector_size);

        if fdc.in_dma {
            let transfer_size = dma.get_dma_transfer_size(FDC_DMA);
            if !transfer_size.is_multiple_of(sector_size) {
                self.finish_with_error(
                    fdc,
                    &format!(
                        "WriteDataOperation: DMA word count {} is not a multiple of sector size {}",
                        transfer_size, sector_size
                    ),
                );
                return false;
            }

            self.state.xfer_size_bytes = transfer_size;
            self.state.xfer_size_sectors = transfer_size / sector_size;

            let src_address = dma.get_dma_transfer_address(FDC_DMA);
            fdc.log_str(&format!(
                "WriteDataOperation: DMA transfer: sectors:{} bytes:{} sector_size:{} src:{:05X}",
                self.state.xfer_size_sectors, transfer_size, sector_size, src_address
            ));
            fdc.dma_byte_count = 0;
            fdc.dma_bytes_left = transfer_size;
        }
        else {
            self.state.xfer_size_sectors = self.state.sectors_to_eot();
            self.state.xfer_size_bytes = self.state.xfer_size_sectors * sector_size;
            fdc.pio_byte_count = 0;
            fdc.pio_sector_byte_count = 0;
            fdc.pio_bytes_left = self.state.xfer_size_bytes;
        }

        if self.state.xfer_size_bytes == 0 {
            self.finish_with_error(fdc, "WriteDataOperation: transfer contains no data");
            return false;
        }

        fdc.xfer_size_bytes = self.state.xfer_size_bytes;
        self.sector_buf = Vec::with_capacity(sector_size);
        true
    }

    fn write_current_sector(&mut self, fdc: &mut FloppyController) {
        let sector_size = DiskChsn::n_to_bytes(self.params.sector_size);
        if self.sector_buf.len() != sector_size {
            self.finish_with_error(
                fdc,
                &format!(
                    "WriteDataOperation: sector {} contains {} of {} bytes",
                    self.state.current_chs,
                    self.sector_buf.len(),
                    sector_size
                ),
            );
            return;
        }

        let write_result = fdc.drives[self.params.drive].write_sector(
            self.state.current_phys_head,
            self.state.current_chs,
            self.params.sector_size,
            &self.sector_buf,
            self.params.deleted_data,
        );

        match write_result {
            Ok(write_result) if write_result.not_found => {
                fdc.merge_operation_status(write_result.status);
                self.finish_with_error(
                    fdc,
                    &format!(
                        "WriteDataOperation: drive reported sector ID not found: {}",
                        self.state.current_chs
                    ),
                );
            }
            Ok(write_result) => {
                let status = write_result.status;
                fdc.merge_operation_status(status);
                if status.no_dam || status.address_crc_error {
                    self.finish_with_error(
                        fdc,
                        &format!("WriteDataOperation: failed to write sector {}", self.state.current_chs),
                    );
                    return;
                }

                self.sector_buf.clear();
                if self.state.complete_sector(false) {
                    self.finish(
                        fdc,
                        InterruptCode::NormalTermination,
                        self.state.final_chs,
                        self.normal_interrupt,
                    );
                }
                else if fdc.data_interface.terminal_count() {
                    self.finish(
                        fdc,
                        InterruptCode::NormalTermination,
                        self.state.current_chs,
                        self.normal_interrupt,
                    );
                }
            }
            Err(err) => {
                self.finish_with_error(fdc, &format!("WriteDataOperation: drive write failed: {:#}", err));
            }
        }
    }

    fn step(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        match fdc.data_interface.recv(dma, bus) {
            Ok(byte) => {
                self.sector_buf.push(byte);

                if fdc.in_dma {
                    fdc.dma_byte_count = fdc.data_interface.byte_count();
                    fdc.dma_bytes_left = self.state.xfer_size_bytes.saturating_sub(fdc.dma_byte_count);
                }
                else {
                    fdc.pio_byte_count += 1;
                    fdc.pio_sector_byte_count =
                        (fdc.pio_sector_byte_count + 1) % DiskChsn::n_to_bytes(self.params.sector_size);
                    fdc.pio_bytes_left = fdc.pio_bytes_left.saturating_sub(1);
                }

                if self.sector_buf.len() == DiskChsn::n_to_bytes(self.params.sector_size) {
                    self.write_current_sector(fdc);
                }
                else if fdc.data_interface.terminal_count() {
                    self.finish_with_error(
                        fdc,
                        "WriteDataOperation: terminal count occurred before a complete transfer",
                    );
                }
            }
            // The DMA channel may not be ready yet, or the CPU may not have
            // supplied the next PIO byte. The data interface owns that detail.
            Err(FdcDataError::Timeout) => {}
            Err(err) => {
                self.finish_with_error(fdc, &format!("WriteDataOperation: data interface error: {:?}", err));
            }
        }
    }
}

impl FdcOperation for WriteDataOperation {
    fn init(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        if !self.state.is_uninitialized() {
            return;
        }

        self.normal_interrupt = fdc.in_dma;
        self.state.reset_position(self.params.id_chs, self.params.physical_head);
        self.sector_buf.clear();
        if !self.init_transfer(fdc, dma) {
            return;
        }

        if fdc.drives[self.params.drive].write_protected {
            fdc.last_error = DriveError::WriteProtect;
            self.finish(fdc, InterruptCode::AbnormalTermination, self.state.current_chs, true);
            return;
        }

        fdc.operation_status.reset(FloppyDriveOperation::WriteData);
        fdc.operation_status_valid = true;
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
            self.finish_with_error(fdc, "WriteDataOperation: PIO byte service timeout");
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
        Operation::WriteData(self.params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        devices::fdc::controller::{ST0_ABNORMAL_TERMINATION, ST1_WRITE_PROTECT},
        machine_config::FloppyDriveConfig,
        machine_types::{FdcType, FloppyDriveType},
    };
    use fluxfox::prelude::StandardFormat;

    fn write_params(
        physical_head: u8,
        id_chs: DiskChs,
        eot: u8,
        multi_track: bool,
        skip: bool,
        deleted_data: bool,
    ) -> DataOperationParameters {
        DataOperationParameters {
            physical_head,
            id_chs,
            sector_size: 2,
            eot,
            gap3_len: 0x2A,
            data_len: 0xFF,
            multi_track,
            skip,
            deleted_data,
            ..Default::default()
        }
    }

    #[test]
    fn write_variants_ignore_skip_and_forward_deleted_data() {
        for (skip, deleted_data) in [(false, false), (true, false), (false, true), (true, true)] {
            let operation =
                WriteDataOperation::new(write_params(0, DiskChs::from((0, 0, 1)), 1, false, skip, deleted_data));

            assert_eq!(operation.params.skip, skip);
            assert_eq!(operation.params.deleted_data, deleted_data);
            assert!(matches!(
                operation.operation_type(),
                Operation::WriteData(params) if params.deleted_data == deleted_data
            ));
        }
    }

    #[test]
    fn pio_write_bytes_are_received_through_data_interface() {
        let mut fdc = FloppyController::default();
        fdc.in_dma = false;
        fdc.drives[0].write_protected = false;
        let mut operation = WriteDataOperation::new(write_params(0, DiskChs::from((0, 0, 1)), 1, false, false, false));
        fdc.operation = operation.operation_type();

        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);

        assert!(fdc.data_interface.active());
        assert_eq!(fdc.data_interface.mode(), FdcDataMode::Pio);
        assert_eq!(fdc.data_interface.direction(), FdcDataDirection::FromHost);

        fdc.data_interface.cpu_write(0xA5).unwrap();
        operation.run(&mut fdc, &mut dma, &mut bus, 100.0);

        assert_eq!(operation.sector_buf, vec![0xA5]);
        assert_eq!(fdc.pio_byte_count, 1);
        assert_eq!(fdc.pio_bytes_left, 511);
    }

    #[test]
    fn pio_multitrack_write_sizes_both_heads() {
        let mut fdc = FloppyController::default();
        fdc.in_dma = false;
        fdc.drives[0].write_protected = false;
        let mut operation = WriteDataOperation::new(write_params(0, DiskChs::from((38, 0, 8)), 9, true, false, false));

        let mut dma = dma::DMAController::new();
        assert!(operation.init_transfer(&mut fdc, &mut dma));

        assert!(operation.state.mt);
        assert_eq!(operation.state.xfer_size_sectors, 11);
        assert_eq!(operation.state.xfer_size_bytes, 11 * 512);
        assert_eq!(fdc.pio_bytes_left, 11 * 512);
    }

    #[test]
    fn partial_sector_dma_write_is_rejected_before_receiving_data() {
        let mut fdc = FloppyController::default();
        fdc.in_dma = true;
        fdc.drives[0].write_protected = false;
        let mut operation = WriteDataOperation::new(write_params(0, DiskChs::from((0, 0, 1)), 1, false, false, false));
        let mut dma = dma::DMAController::new();
        dma.handle_clear_flopflop();
        dma.handle_wc_port_write(FDC_DMA, 0x00);
        dma.handle_wc_port_write(FDC_DMA, 0x02);

        assert_eq!(dma.get_dma_transfer_size(FDC_DMA), 513);
        assert!(!operation.init_transfer(&mut fdc, &mut dma));
        assert!(operation.state.is_complete());
        assert!(operation.sector_buf.is_empty());
        assert!(fdc
            .get_debug_state()
            .cmd_log
            .iter()
            .any(|entry| entry.contains("DMA word count 513 is not a multiple of sector size 512")));
    }

    #[test]
    fn write_protect_uses_abnormal_termination() {
        let mut fdc = FloppyController::default();
        fdc.in_dma = false;
        fdc.drives[0].write_protected = true;
        let mut operation = WriteDataOperation::new(write_params(0, DiskChs::from((0, 0, 1)), 1, false, false, false));
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();

        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);

        let status = fdc.get_debug_state().last_status;
        assert_eq!(status[0] & 0xC0, ST0_ABNORMAL_TERMINATION);
        assert_ne!(status[1] & ST1_WRITE_PROTECT, 0);
    }

    #[test]
    fn multitrack_write_crosses_from_head_zero_to_head_one() {
        let mut fdc = FloppyController::new(
            FdcType::IbmNec,
            vec![FloppyDriveConfig {
                fd_type: FloppyDriveType::Floppy360K,
                image:   None,
            }],
        );
        fdc.create_new_image(0, StandardFormat::PcFloppy360, true).unwrap();
        fdc.drives[0].write_protected = false;
        fdc.drives[0].seek(38);
        fdc.operation_status.reset(FloppyDriveOperation::WriteData);
        fdc.operation_status_valid = true;

        let mut operation = WriteDataOperation::new(write_params(0, DiskChs::from((38, 0, 9)), 9, true, false, false));
        operation.state.xfer_size_sectors = 2;
        operation.state.xfer_size_bytes = 1024;

        operation.sector_buf.resize(512, 0xA5);
        operation.write_current_sector(&mut fdc);
        assert!(!operation.state.is_complete());
        assert_eq!(operation.state.current_phys_head, 1);
        assert_eq!(operation.state.current_chs, DiskChs::from((38, 1, 1)));

        operation.sector_buf.resize(512, 0x5A);
        operation.write_current_sector(&mut fdc);
        assert!(operation.state.is_complete());
        assert_eq!(operation.state.final_chs, DiskChs::from((38, 1, 2)));

        let head_zero = fdc.drives[0].read_sector(0, DiskChs::from((38, 0, 9)), 2).unwrap();
        let head_one = fdc.drives[0].read_sector(1, DiskChs::from((38, 1, 1)), 2).unwrap();
        assert_eq!(head_zero.data, vec![0xA5; 512]);
        assert_eq!(head_one.data, vec![0x5A; 512]);
    }

    #[test]
    fn multitrack_write_starting_on_head_one_does_not_wrap() {
        let mut operation = WriteDataOperation::new(write_params(1, DiskChs::from((38, 1, 9)), 9, true, false, false));
        operation.state.xfer_size_sectors = 2;

        assert!(operation.state.complete_sector(false));
        assert_eq!(operation.state.current_phys_head, 1);
        assert_eq!(operation.state.final_chs, DiskChs::from((38, 1, 10)));
    }

    #[test]
    fn drive_write_error_uses_compact_display_format_in_fdc_log() {
        let mut fdc = FloppyController::default();
        let mut operation = WriteDataOperation::new(write_params(0, DiskChs::from((0, 0, 1)), 1, false, false, false));
        fdc.operation = operation.operation_type();
        operation.sector_buf.resize(512, 0);

        operation.write_current_sector(&mut fdc);

        let state = fdc.get_debug_state();
        let error = state
            .cmd_log
            .iter()
            .find(|entry| entry.starts_with("WriteDataOperation: drive write failed:"))
            .expect("write error should be recorded in the FDC log");
        assert_eq!(error, "WriteDataOperation: drive write failed: No media in drive");
        assert!(!error.contains('\n'));
    }
}
