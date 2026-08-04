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
        fdc::controller::{AckCondition, DriveError, FloppyController, Operation},
    },
};
use marty_common::FloppyDriveEvent;

use super::{FdcOperation, FdcOperationState};

enum SeekStepResult {
    Continue,
    Complete,
}

pub(in crate::devices::fdc) struct SeekOperation {
    operation_type: Operation,
    drive_select: usize,
    cylinder: u16,
    state: FdcOperationState,
    step_accumulator_us: f64,
}

impl SeekOperation {
    pub(in crate::devices::fdc) fn new(operation_type: Operation, drive_select: usize, cylinder: u16) -> Self {
        Self {
            operation_type,
            drive_select,
            cylinder,
            state: FdcOperationState::new(),
            step_accumulator_us: 0.0,
        }
    }

    fn step_interval_us(fdc: &FloppyController) -> f64 {
        (fdc.step_rate as f64 + 1.0) * 1_000.0
    }

    fn finish(&mut self, fdc: &mut FloppyController) {
        self.state.mark_complete();
        log::trace!("FDC Operation Seek complete");
        fdc.drive_status[self.drive_select].seeking = false;
        fdc.drive_status[self.drive_select].unack_condition = Some(AckCondition::Seek);
        fdc.raise_sense_interrupt();
        fdc.drive_select = self.drive_select;
        fdc.operation = Operation::NoOperation;
        fdc.last_error = DriveError::NoError;
    }

    fn step(&mut self, fdc: &mut FloppyController) -> SeekStepResult {
        let target = self.cylinder as u8;
        let previous_pcn = fdc.drive_status[self.drive_select].pcn;
        match previous_pcn.cmp(&target) {
            std::cmp::Ordering::Less => fdc.drive_status[self.drive_select].pcn += 1,
            std::cmp::Ordering::Greater => fdc.drive_status[self.drive_select].pcn -= 1,
            std::cmp::Ordering::Equal => {
                self.finish(fdc);
                return SeekStepResult::Complete;
            }
        }

        let current_pcn = fdc.drive_status[self.drive_select].pcn;
        fdc.drives[self.drive_select].seek(current_pcn as u16);
        log::trace!(
            "SeekOperation: drive:{} step PCN:{} -> {} target:{}",
            self.drive_select,
            previous_pcn,
            current_pcn,
            target
        );

        if current_pcn == target {
            self.finish(fdc);
            SeekStepResult::Complete
        }
        else {
            SeekStepResult::Continue
        }
    }
}

impl FdcOperation for SeekOperation {
    fn init(&mut self, fdc: &mut FloppyController, _dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        if !self.state.is_uninitialized() {
            return;
        }

        let current_pcn = fdc.drive_status[self.drive_select].pcn as u16;
        let steps = current_pcn.abs_diff(self.cylinder);
        let log_str = format!(
            "SeekOperation: drive:{} PCN:{} target cylinder:{} steps:{}",
            self.drive_select, current_pcn, self.cylinder, steps
        );
        log::debug!("{}", log_str);
        fdc.log_str(&log_str);

        if steps > 0 {
            fdc.emit_presentable_event(
                self.drive_select,
                FloppyDriveEvent::HeadStep {
                    from_cylinder: current_pcn as u8,
                    to_cylinder:   self.cylinder as u8,
                },
            );
        }

        fdc.drive_select = self.drive_select;
        fdc.drive_status[self.drive_select].seeking = true;
        self.state.begin_seek();

        if current_pcn == self.cylinder {
            self.finish(fdc);
        }
    }

    fn run(&mut self, fdc: &mut FloppyController, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        if self.state.is_uninitialized() {
            self.init(fdc, dma, bus);
        }
        if self.state.is_complete() {
            return;
        }

        let step_interval_us = Self::step_interval_us(fdc);
        self.step_accumulator_us += us;
        while self.step_accumulator_us >= step_interval_us {
            self.step_accumulator_us -= step_interval_us;
            match self.step(fdc) {
                SeekStepResult::Continue => {}
                SeekStepResult::Complete => break,
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
    use crate::{devices::floppy_drive::FloppyDiskDrive, machine_types::FloppyDriveType};

    fn seek_fixture(pcn: u8, target: u16, step_rate: u8) -> (FloppyController, SeekOperation) {
        let mut fdc = FloppyController::default();
        fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
        fdc.drives[0].seek(pcn as u16);
        fdc.drive_status[0].pcn = pcn;
        fdc.step_rate = step_rate;
        fdc.operation = Operation::Seek;

        (fdc, SeekOperation::new(Operation::Seek, 0, target))
    }

    #[test]
    fn seek_steps_forward_once_per_step_rate_interval() {
        let (mut fdc, mut operation) = seek_fixture(2, 4, 2);
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();

        operation.run(&mut fdc, &mut dma, &mut bus, 2_999.0);
        assert_eq!(fdc.drive_status[0].pcn, 2);
        assert!(fdc.drive_status[0].seeking);
        assert_eq!(fdc.drive_status[0].unack_condition, None);

        operation.run(&mut fdc, &mut dma, &mut bus, 1.0);
        assert_eq!(fdc.drive_status[0].pcn, 3);
        assert!(!operation.is_complete());
        assert!(!fdc.has_internal_interrupt_pending());

        operation.run(&mut fdc, &mut dma, &mut bus, 3_000.0);
        assert_eq!(fdc.drive_status[0].pcn, 4);
        assert_eq!(fdc.drives[0].chsn.c(), 4);
        assert!(operation.is_complete());
        assert!(fdc.has_internal_interrupt_pending());
        assert!(!fdc.drive_status[0].seeking);
        assert_eq!(fdc.drive_status[0].unack_condition, Some(AckCondition::Seek));
    }

    #[test]
    fn seek_steps_backward_until_pcn_reaches_target() {
        let (mut fdc, mut operation) = seek_fixture(3, 1, 1);
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();

        operation.run(&mut fdc, &mut dma, &mut bus, 2_000.0);
        assert_eq!(fdc.drive_status[0].pcn, 2);
        assert!(!operation.is_complete());

        operation.run(&mut fdc, &mut dma, &mut bus, 2_000.0);
        assert_eq!(fdc.drive_status[0].pcn, 1);
        assert_eq!(fdc.drives[0].chsn.c(), 1);
        assert!(operation.is_complete());
    }

    #[test]
    fn seek_processes_each_elapsed_step_interval() {
        let (mut fdc, mut operation) = seek_fixture(1, 5, 2);
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();

        operation.run(&mut fdc, &mut dma, &mut bus, 9_000.0);
        assert_eq!(fdc.drive_status[0].pcn, 4);
        assert_eq!(fdc.drives[0].chsn.c(), 4);
        assert!(!operation.is_complete());

        operation.run(&mut fdc, &mut dma, &mut bus, 3_000.0);
        assert_eq!(fdc.drive_status[0].pcn, 5);
        assert_eq!(fdc.drives[0].chsn.c(), 5);
        assert!(operation.is_complete());
    }

    #[test]
    fn seek_to_current_cylinder_completes_without_waiting() {
        let (mut fdc, mut operation) = seek_fixture(3, 3, 2);
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        fdc.drive_status[0].unack_condition = Some(AckCondition::ReadyPoll);

        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);
        assert_eq!(fdc.drive_status[0].pcn, 3);
        assert!(operation.is_complete());
        assert!(fdc.has_internal_interrupt_pending());
        assert!(!fdc.drive_status[0].seeking);
        assert_eq!(fdc.drive_status[0].unack_condition, Some(AckCondition::Seek));
    }

    #[test]
    fn zero_step_rate_represents_one_millisecond() {
        let (mut fdc, mut operation) = seek_fixture(1, 2, 0);
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();

        operation.run(&mut fdc, &mut dma, &mut bus, 999.0);
        assert_eq!(fdc.drive_status[0].pcn, 1);
        assert!(!operation.is_complete());

        operation.run(&mut fdc, &mut dma, &mut bus, 1.0);
        assert_eq!(fdc.drive_status[0].pcn, 2);
        assert!(operation.is_complete());
    }

    #[test]
    fn seek_emits_one_presentable_event_for_the_entire_seek() {
        let (mut fdc, mut operation) = seek_fixture(2, 5, 0);
        let (sender, receiver) = crossbeam_channel::bounded(1);
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        fdc.set_presentable_event_sender(0, sender);

        operation.run(&mut fdc, &mut dma, &mut bus, 0.0);

        assert_eq!(
            receiver.try_recv().unwrap(),
            marty_common::PresentableDeviceEvent::FloppyDrive {
                controller: 0,
                drive: 0,
                event: FloppyDriveEvent::HeadStep {
                    from_cylinder: 2,
                    to_cylinder:   5,
                },
            }
        );

        operation.run(&mut fdc, &mut dma, &mut bus, 3_000.0);
        assert!(operation.is_complete());
        assert!(receiver.try_recv().is_err());
    }
}
