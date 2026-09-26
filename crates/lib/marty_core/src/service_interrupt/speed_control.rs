/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

//! Emulation speed control service.

use crate::cpu_common::{Cpu, Register16, Register8, ServiceEvent};

use super::{clear_carry, set_service_error, ServiceError, ServiceInterruptManager};

pub const SPEED_CONTROL_QUERY: u8 = 0x00;
pub const SPEED_CONTROL_SET: u8 = 0x01;

pub const DEFAULT_SPEED_CONTROL_MIN: u16 = 100;
pub const DEFAULT_SPEED_CONTROL_CURRENT: u16 = 1000;
pub const DEFAULT_SPEED_CONTROL_MAX: u16 = 2000;

impl ServiceInterruptManager {
    pub(super) fn handle_speed_control<C: Cpu>(&mut self, cpu: &mut C) -> Option<ServiceEvent> {
        match cpu.get_register8(Register8::AL) {
            SPEED_CONTROL_QUERY => {
                cpu.set_register16(Register16::BX, self.speed_control_min);
                cpu.set_register16(Register16::CX, self.speed_control_current);
                cpu.set_register16(Register16::DX, self.speed_control_max);
                clear_carry(cpu);
                None
            }
            SPEED_CONTROL_SET => {
                let requested_speed = cpu.get_register16(Register16::CX);
                let speed = requested_speed.clamp(self.speed_control_min, self.speed_control_max);
                self.speed_control_current = speed;
                clear_carry(cpu);
                Some(ServiceEvent::SetEmulationSpeed(speed))
            }
            _ => {
                set_service_error(cpu, ServiceError::InvalidParameter);
                None
            }
        }
    }

    pub fn configure_speed_control(&mut self, min: u16, current: u16, max: u16) {
        let max = max.max(min);
        self.speed_control_min = min;
        self.speed_control_max = max;
        self.speed_control_current = current.clamp(min, max);
    }

    pub fn set_speed_control_current(&mut self, current: u16) {
        self.speed_control_current = current.clamp(self.speed_control_min, self.speed_control_max);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::{ServiceFunction, CARRY_FLAG},
        *,
    };

    #[test]
    fn speed_changes_are_clamped() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        manager.configure_speed_control(500, 1000, 1500);
        cpu.set_register8(Register8::AL, SPEED_CONTROL_QUERY);
        cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
        assert!(manager
            .handle_interrupt(ServiceFunction::SpeedControl, &mut cpu)
            .is_none());
        assert_eq!(cpu.get_register16(Register16::BX), 500);
        assert_eq!(cpu.get_register16(Register16::CX), 1000);
        assert_eq!(cpu.get_register16(Register16::DX), 1500);
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, SPEED_CONTROL_SET);
        cpu.set_register16(Register16::CX, 2000);
        cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::SpeedControl, &mut cpu),
            Some(ServiceEvent::SetEmulationSpeed(1500))
        ));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, SPEED_CONTROL_QUERY);
        manager.handle_interrupt(ServiceFunction::SpeedControl, &mut cpu);
        assert_eq!(cpu.get_register16(Register16::CX), 1500);

        // An invalid request must leave the last accepted speed in place.
        cpu.set_register8(Register8::AL, 0xFF);
        cpu.set_register16(Register16::CX, 700);
        assert!(manager
            .handle_interrupt(ServiceFunction::SpeedControl, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            u16::from(ServiceError::InvalidParameter)
        );
        cpu.set_register8(Register8::AL, SPEED_CONTROL_QUERY);
        manager.handle_interrupt(ServiceFunction::SpeedControl, &mut cpu);
        assert_eq!(cpu.get_register16(Register16::CX), 1500);
    }
}
