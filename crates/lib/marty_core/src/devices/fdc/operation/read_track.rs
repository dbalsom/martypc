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
            controller::{DataOperationParameters, FloppyController, InterruptCode, Operation, FDC_DMA},
            data_interface::{FdcDataDirection, FdcDataError, FdcDataMode},
        },
        floppy_drive::FloppyDriveOperation,
    },
};

use super::{FdcOperation, FdcOperationMode, FdcOperationState};

pub(in crate::devices::fdc) struct ReadTrackOperation {
    params: DataOperationParameters,
    state: FdcOperationState,
    track_buf: Vec<u8>,
    track_buf_idx: usize,
    normal_interrupt: bool,
}

impl ReadTrackOperation {
    pub(in crate::devices::fdc) fn new(params: DataOperationParameters) -> Self {
        Self {
            params,
            state: FdcOperationState::new_transfer(params.id_chs, params.physical_head, params.eot, false),
            track_buf: Vec::new(),
            track_buf_idx: 0,
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
        self.finish(fdc, InterruptCode::AbnormalTermination, self.state.final_chs, true);
    }

    fn finish_on_terminal_count(&mut self, fdc: &mut FloppyController) {
        let byte_count = fdc.data_interface.byte_count();
        fdc.log_str(&format!(
            "DMA terminal count triggered end of Read Track operation, {} bytes read.",
            byte_count
        ));
        self.finish(
            fdc,
            InterruptCode::NormalTermination,
            self.state.final_chs,
            self.normal_interrupt,
        );
    }

    fn init_transfer(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController) -> bool {
        let sector_size = DiskChsn::n_to_bytes(self.params.sector_size);
        let dma_transfer_size = if fdc.in_dma {
            let transfer_size = dma.get_dma_transfer_size(FDC_DMA);
            if !transfer_size.is_multiple_of(sector_size) {
                log::warn!(
                    "ReadTrackOperation: DMA word count {} not multiple of sector size ({})",
                    transfer_size,
                    sector_size
                );
            }

            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            fdc.log_str(&format!(
                "ReadTrackOperation: DMA transfer: bytes:{} sector_size:{} dst:{:05X}",
                transfer_size, sector_size, dst_address
            ));
            Some(transfer_size)
        }
        else {
            None
        };

        fdc.operation_status.reset(FloppyDriveOperation::ReadData);
        fdc.operation_status_valid = true;

        let read_result = match fdc.drives[self.params.drive].read_track(
            self.state.current_phys_head,
            self.state.current_chs.into(),
            self.params.sector_size,
            self.params.eot,
        ) {
            Ok(result) => result,
            Err(err) => {
                self.finish_with_error(fdc, &format!("ReadTrackOperation: track read failed: {:#}", err));
                return false;
            }
        };

        fdc.merge_operation_status(read_result.status);
        if read_result.not_found {
            self.finish_with_error(
                fdc,
                &format!(
                    "ReadTrackOperation: drive reported sector ID not found: {}",
                    self.state.current_chs
                ),
            );
            return false;
        }

        let available_bytes = read_result.data.len();
        let transfer_size = dma_transfer_size.map_or(available_bytes, |dma_size| dma_size.min(available_bytes));
        self.track_buf = read_result.data;
        self.track_buf.truncate(transfer_size);
        self.track_buf_idx = 0;

        self.state.xfer_size_bytes = transfer_size;
        self.state.xfer_size_sectors = transfer_size.div_ceil(sector_size);
        self.state.final_chs = if transfer_size >= available_bytes {
            DiskChs::new(self.params.id_chs.c().wrapping_add(1), self.params.id_chs.h(), 1)
        }
        else {
            self.params.id_chs
        };

        fdc.log_str(&format!(
            "ReadTrackOperation: read {} sectors, transferring {} of {} bytes",
            read_result.sectors_read, transfer_size, available_bytes
        ));

        fdc.xfer_size_bytes = transfer_size;
        if fdc.in_dma {
            fdc.dma_byte_count = 0;
            fdc.dma_bytes_left = transfer_size;
        }
        else {
            fdc.pio_byte_count = 0;
            fdc.pio_sector_byte_count = 0;
            fdc.pio_bytes_left = transfer_size;
        }

        true
    }

    fn step(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        let transfer_complete = self.track_buf_idx >= self.track_buf.len() && fdc.data_interface.latch_empty();
        if transfer_complete {
            if fdc.in_dma && !fdc.data_interface.terminal_count() {
                const MESSAGE: &str = "ReadTrackOperation: transfer completed without DMA terminal count";
                log::warn!("{}", MESSAGE);
                fdc.log_str(MESSAGE);
            }
            self.finish(
                fdc,
                InterruptCode::NormalTermination,
                self.state.final_chs,
                self.normal_interrupt,
            );
            return;
        }

        if fdc.data_interface.terminal_count() {
            self.finish_on_terminal_count(fdc);
            return;
        }

        let byte = self.track_buf[self.track_buf_idx];
        match fdc.data_interface.send(byte, dma, bus) {
            Ok(()) => {
                self.track_buf_idx += 1;
            }
            Err(FdcDataError::Timeout) => {
                self.finish_with_error(fdc, "ReadTrackOperation: data interface timeout");
            }
            Err(err) => {
                self.finish_with_error(fdc, &format!("ReadTrackOperation: data interface error: {:?}", err));
            }
        }
    }
}

impl FdcOperation for ReadTrackOperation {
    fn init(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        if !self.state.is_uninitialized() {
            return;
        }

        self.normal_interrupt = fdc.in_dma;
        self.state.reset_position(self.params.id_chs, self.params.physical_head);
        self.track_buf.clear();
        self.track_buf_idx = 0;

        if !self.init_transfer(fdc, dma) {
            return;
        }

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
        if self.state.is_complete() {
            return;
        }

        if fdc.data_interface.pio_service_timed_out() {
            self.finish_with_error(fdc, "ReadTrackOperation: PIO byte service timeout");
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
        Operation::ReadTrack(self.params)
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

    fn read_track_params() -> DataOperationParameters {
        DataOperationParameters {
            drive: 0,
            physical_head: 0,
            id_chs: DiskChs::from((0, 0, 1)),
            sector_size: 2,
            eot: 9,
            gap3_len: 0x2A,
            data_len: 0xFF,
            mfm: true,
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
    fn operation_owns_command_parameters() {
        let params = read_track_params();
        let operation = ReadTrackOperation::new(params);

        assert_eq!(operation.operation_type(), Operation::ReadTrack(params));
        assert_eq!(operation.params, params);
    }

    #[test]
    fn pio_track_bytes_are_sent_through_data_interface() {
        let mut fdc = fdc_with_360k_image();
        fdc.in_dma = false;
        let mut operation = ReadTrackOperation::new(read_track_params());
        fdc.operation = operation.operation_type();

        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);

        assert!(fdc.data_interface.active());
        assert_eq!(fdc.data_interface.mode(), FdcDataMode::Pio);
        assert_eq!(fdc.data_interface.direction(), FdcDataDirection::ToHost);
        assert_eq!(operation.state.xfer_size_bytes, 9 * 512);
        assert_eq!(operation.state.final_chs, DiskChs::from((1, 0, 1)));

        operation.run(&mut fdc, &mut dma, &mut bus, 99.0);
        operation.run(&mut fdc, &mut dma, &mut bus, 1.0);
        let byte_period_us = fdc.data_rate.byte_period_us();
        operation.run(&mut fdc, &mut dma, &mut bus, byte_period_us);

        assert_eq!(operation.track_buf_idx, 1);
        assert!(fdc.data_interface.cpu_read().is_some());
    }

    #[test]
    fn dma_transfer_size_caps_track_data_sent_through_data_interface() {
        let mut fdc = fdc_with_360k_image();
        fdc.in_dma = true;
        let mut operation = ReadTrackOperation::new(read_track_params());
        fdc.operation = operation.operation_type();

        let mut dma = dma::DMAController::new();
        dma.handle_clear_flopflop();
        dma.handle_wc_port_write(FDC_DMA, 0xFF);
        dma.handle_wc_port_write(FDC_DMA, 0x01);
        assert_eq!(dma.get_dma_transfer_size(FDC_DMA), 512);

        let mut bus = BusInterface::default();
        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);

        assert_eq!(fdc.data_interface.mode(), FdcDataMode::Dma);
        assert_eq!(fdc.data_interface.direction(), FdcDataDirection::ToHost);
        assert_eq!(operation.track_buf.len(), 512);
        assert_eq!(operation.state.final_chs, DiskChs::from((0, 0, 1)));

        operation.run(&mut fdc, &mut dma, &mut bus, 100.0);

        assert_eq!(operation.track_buf_idx, 3);
        assert_eq!(fdc.data_interface.byte_count(), 3);
    }
}
