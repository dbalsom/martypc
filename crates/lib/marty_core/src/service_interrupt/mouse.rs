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

//! Virtual mouse service requests and completion.

use crate::cpu_common::{Cpu, Register16, Register8, ServiceEvent};

use super::{clear_carry, set_service_error, ServiceError, ServiceInterruptManager};

pub const MOUSE_STATE_QUERY: u8 = 0x00;
pub const MOUSE_IRQ_QUERY: u8 = 0x01;
pub const MOUSE_CONSUMER_RANGE_REPORT: u8 = 0x02;
pub const MOUSE_CONSUMER_STATUS_REPORT: u8 = 0x03;
pub const MOUSE_DISPLAY_APERTURE_QUERY: u8 = 0x04;
pub const MOUSE_HOST_CURSOR_VISIBILITY: u8 = 0x05;
pub const MOUSE_STATE_FLAG_CAPTURED: u16 = 0x0001;

impl ServiceInterruptManager {
    /// Complete a virtual mouse state request after the machine has sampled the device.
    pub fn complete_mouse_state<C: Cpu>(&self, cpu: &mut C, state: Option<(u16, u16, u16, u16, i16, i16, u16)>) {
        let Some((x, y, buttons, change_counter, relative_x, relative_y, flags)) = state
        else {
            set_service_error(cpu, ServiceError::NotSupported);
            return;
        };

        cpu.set_register16(Register16::AX, x);
        cpu.set_register16(Register16::BX, y);
        cpu.set_register16(Register16::CX, buttons);
        cpu.set_register16(Register16::DX, change_counter);
        cpu.set_register16(Register16::SI, relative_x as u16);
        cpu.set_register16(Register16::DI, relative_y as u16);
        cpu.set_register16(Register16::BP, flags);
        clear_carry(cpu);
    }

    /// Complete a virtual mouse IRQ query after the machine has inspected the device.
    pub fn complete_mouse_irq<C: Cpu>(&self, cpu: &mut C, irq: Option<u8>) {
        let Some(irq) = irq
        else {
            set_service_error(cpu, ServiceError::NotSupported);
            return;
        };

        cpu.set_register16(Register16::DX, u16::from(irq));
        clear_carry(cpu);
    }

    /// Complete a display-aperture query after the machine has inspected its primary video card.
    pub fn complete_display_aperture_size<C: Cpu>(&self, cpu: &mut C, size: Option<(u16, u16)>) {
        let Some((width, height)) = size
        else {
            set_service_error(cpu, ServiceError::NotSupported);
            return;
        };

        cpu.set_register16(Register16::BX, width);
        cpu.set_register16(Register16::CX, height);
        clear_carry(cpu);
    }

    /// Complete a virtual mouse consumer-range report after the machine has inspected the device.
    pub fn complete_mouse_consumer_range<C: Cpu>(&self, cpu: &mut C, supported: bool) {
        if supported {
            clear_carry(cpu);
        }
        else {
            set_service_error(cpu, ServiceError::NotSupported);
        }
    }

    /// Complete a virtual mouse consumer-status report after the machine has inspected the device.
    pub fn complete_mouse_consumer_status<C: Cpu>(&self, cpu: &mut C, supported: bool) {
        if supported {
            clear_carry(cpu);
        }
        else {
            set_service_error(cpu, ServiceError::NotSupported);
        }
    }

    pub(super) fn handle_mouse_state<C: Cpu>(&self, cpu: &mut C) -> Option<ServiceEvent> {
        match cpu.get_register8(Register8::AL) {
            MOUSE_STATE_QUERY => Some(ServiceEvent::GetVirtualMouseState),
            MOUSE_IRQ_QUERY => Some(ServiceEvent::GetVirtualMouseIrq),
            MOUSE_DISPLAY_APERTURE_QUERY => Some(ServiceEvent::GetDisplayApertureSize),
            MOUSE_CONSUMER_RANGE_REPORT => Some(ServiceEvent::SetVirtualMouseConsumerRange {
                min_x: cpu.get_register16(Register16::BX),
                max_x: cpu.get_register16(Register16::CX),
                min_y: cpu.get_register16(Register16::DX),
                max_y: cpu.get_register16(Register16::SI),
            }),
            MOUSE_CONSUMER_STATUS_REPORT => match cpu.get_register16(Register16::BX) {
                0 => Some(ServiceEvent::SetVirtualMouseConsumerStatus { loaded: false }),
                1 => Some(ServiceEvent::SetVirtualMouseConsumerStatus { loaded: true }),
                _ => {
                    set_service_error(cpu, ServiceError::InvalidParameter);
                    None
                }
            },
            MOUSE_HOST_CURSOR_VISIBILITY => match cpu.get_register16(Register16::BX) {
                0 => {
                    clear_carry(cpu);
                    Some(ServiceEvent::SetHostCursorVisibility { visible: false })
                }
                1 => {
                    clear_carry(cpu);
                    Some(ServiceEvent::SetHostCursorVisibility { visible: true })
                }
                _ => {
                    set_service_error(cpu, ServiceError::InvalidParameter);
                    None
                }
            },
            _ => {
                set_service_error(cpu, ServiceError::InvalidParameter);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::{ServiceFunction, CARRY_FLAG},
        *,
    };

    #[test]
    fn mouse_calls_pass_state_through_registers() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        cpu.set_register8(Register8::AL, MOUSE_STATE_QUERY);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::GetVirtualMouseState)
        ));

        cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
        manager.complete_mouse_state(
            &mut cpu,
            Some((0x1234, 0x5678, 0x0003, 0x9ABC, -12, 34, MOUSE_STATE_FLAG_CAPTURED)),
        );

        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), 0x1234);
        assert_eq!(cpu.get_register16(Register16::BX), 0x5678);
        assert_eq!(cpu.get_register16(Register16::CX), 0x0003);
        assert_eq!(cpu.get_register16(Register16::DX), 0x9ABC);
        assert_eq!(cpu.get_register16(Register16::SI), (-12i16) as u16);
        assert_eq!(cpu.get_register16(Register16::DI), 34);
        assert_eq!(cpu.get_register16(Register16::BP), MOUSE_STATE_FLAG_CAPTURED);

        cpu.set_register8(Register8::AL, MOUSE_IRQ_QUERY);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::GetVirtualMouseIrq)
        ));
        manager.complete_mouse_irq(&mut cpu, Some(5));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::DX), 5);

        cpu.set_register8(Register8::AL, MOUSE_DISPLAY_APERTURE_QUERY);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::GetDisplayApertureSize)
        ));
        manager.complete_display_aperture_size(&mut cpu, Some((640, 480)));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::BX), 640);
        assert_eq!(cpu.get_register16(Register16::CX), 480);

        cpu.set_register8(Register8::AL, MOUSE_CONSUMER_RANGE_REPORT);
        cpu.set_register16(Register16::BX, 10);
        cpu.set_register16(Register16::CX, 639);
        cpu.set_register16(Register16::DX, 20);
        cpu.set_register16(Register16::SI, 199);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetVirtualMouseConsumerRange {
                min_x: 10,
                max_x: 639,
                min_y: 20,
                max_y: 199,
            })
        ));
        manager.complete_mouse_consumer_range(&mut cpu, true);
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_CONSUMER_STATUS_REPORT);
        cpu.set_register16(Register16::BX, 1);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetVirtualMouseConsumerStatus { loaded: true })
        ));
        manager.complete_mouse_consumer_status(&mut cpu, true);
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_HOST_CURSOR_VISIBILITY);
        cpu.set_register16(Register16::BX, 0);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetHostCursorVisibility { visible: false })
        ));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_HOST_CURSOR_VISIBILITY);
        cpu.set_register16(Register16::BX, 1);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetHostCursorVisibility { visible: true })
        ));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_HOST_CURSOR_VISIBILITY);
        cpu.set_register16(Register16::BX, 2);
        assert!(manager
            .handle_interrupt(ServiceFunction::MouseState, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            u16::from(ServiceError::InvalidParameter)
        );

        cpu.set_register8(Register8::AL, MOUSE_CONSUMER_STATUS_REPORT);
        cpu.set_register16(Register16::BX, 2);
        assert!(manager
            .handle_interrupt(ServiceFunction::MouseState, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            u16::from(ServiceError::InvalidParameter)
        );

        cpu.set_register8(Register8::AL, 0xFF);
        assert!(manager
            .handle_interrupt(ServiceFunction::MouseState, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            u16::from(ServiceError::InvalidParameter)
        );
    }

    #[test]
    fn missing_mouse_returns_not_supported() {
        let manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        cpu.set_flags(0);

        manager.complete_mouse_state(&mut cpu, None);

        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            u16::from(ServiceError::NotSupported)
        );
    }
}
