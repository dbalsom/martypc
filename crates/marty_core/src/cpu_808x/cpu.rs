/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    ---------------------------------------------------------------------------

    cpu_808x::cpu.rs

    Implements the Cpu trait for the Intel 808x CPUs

*/

use crate::{
    breakpoints::{BreakPointType, StopWatchData},
    bus::BusInterface,
    cpu_808x::{Intel808x, Register16},
    cpu_common::{Cpu, CpuAddress, CpuError, CpuOption, CpuStringState, CpuType, ServiceEvent, StepResult},
    syntax_token::SyntaxToken,
};

use crate::cpu_common::{Disassembly, LogicAnalyzer, Register8, TraceMode};

#[cfg(feature = "cpu_validator")]
use crate::cpu_808x::CpuValidatorState;
#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{CpuValidator, CycleState, VRegisters};

impl Cpu for Intel808x {
    fn reset(&mut self) {
        self.reset()
    }

    #[inline]
    fn set_reset_vector(&mut self, address: CpuAddress) {
        self.set_reset_vector(address);
    }

    #[inline]
    fn set_reset_queue_contents(&mut self, contents: Vec<u8>) {
        self.set_reset_queue_contents(contents)
    }

    #[inline]
    fn set_end_address(&mut self, address: CpuAddress) {
        let end_addr;
        match address {
            CpuAddress::Segmented(segment, offset) => {
                end_addr = Intel808x::calc_linear_address(segment, offset);
            }
            CpuAddress::Flat(addr) => {
                end_addr = addr;
            }
            _ => {
                panic!("Invalid CpuAddress for end address.");
            }
        }
        self.set_end_address(end_addr as usize);
    }

    #[inline]
    fn set_nmi(&mut self, state: bool) {
        self.set_nmi(state);
    }

    #[inline]
    fn set_intr(&mut self, state: bool) {
        self.set_intr(state);
    }

    #[inline]
    fn step(&mut self, skip_breakpoint: bool) -> Result<(StepResult, u32), CpuError> {
        self.step(skip_breakpoint)
    }

    #[inline]
    fn step_finish(&mut self, disassembly: Option<&mut Disassembly>) -> Result<StepResult, CpuError> {
        self.step_finish(disassembly)
    }

    #[inline]
    fn in_rep(&self) -> bool {
        self.in_rep
    }

    #[inline]
    fn get_type(&self) -> CpuType {
        self.cpu_type
    }

    #[inline]
    fn get_ip(&mut self) -> u16 {
        self.ip()
    }

    #[inline]
    fn get_register16(&self, reg: Register16) -> u16 {
        self.get_register16(reg)
    }

    #[inline]
    fn set_register16(&mut self, reg: Register16, value: u16) {
        self.set_register16(reg, value);
    }

    #[inline]
    fn get_register8(&self, reg: Register8) -> u8 {
        self.get_register8(reg)
    }

    #[inline]
    fn set_register8(&mut self, reg: Register8, value: u8) {
        self.set_register8(reg, value);
    }

    #[inline]
    fn get_flags(&self) -> u16 {
        self.get_flags()
    }

    #[inline]
    fn set_flags(&mut self, flags: u16) {
        self.set_flags(flags);
    }

    #[inline]
    fn get_cycle_ct(&self) -> (u64, u64) {
        self.get_cycle_ct()
    }

    #[inline]
    fn get_instruction_ct(&self) -> u64 {
        self.get_instruction_ct()
    }

    /// Return the resolved flat address of CS:CORR(PC)
    #[inline]
    fn flat_ip(&self) -> u32 {
        Intel808x::calc_linear_address(self.cs, self.ip())
    }

    /// Return the resolved flat address of CS:CORR(PC), adjusted for reentrant instructions
    #[inline]
    fn flat_ip_disassembly(&self) -> u32 {
        Intel808x::calc_linear_address(self.cs, self.disassembly_ip())
    }

    #[inline]
    fn flat_sp(&self) -> u32 {
        self.flat_sp()
    }

    #[inline]
    fn dump_instruction_history_string(&self) -> String {
        self.dump_instruction_history_string()
    }

    #[inline]
    fn dump_instruction_history_tokens(&self) -> Vec<Vec<SyntaxToken>> {
        self.dump_instruction_history_tokens()
    }

    fn dump_call_stack(&self) -> String {
        self.dump_call_stack()
    }

    #[inline]
    fn get_service_event(&mut self) -> Option<ServiceEvent> {
        self.service_events.pop_front()
    }

    #[inline]
    #[cfg(feature = "cpu_validator")]
    fn get_cycle_states(&self) -> &Vec<CycleState> {
        self.get_cycle_states_internal()
    }

    fn get_cycle_trace(&self) -> &Vec<String> {
        self.get_cycle_trace()
    }

    fn get_cycle_trace_tokens(&self) -> &Vec<Vec<SyntaxToken>> {
        self.get_cycle_trace_tokens()
    }

    #[inline]
    fn get_string_state(&self) -> CpuStringState {
        self.get_string_state()
    }

    fn eval_address(&self, expr: &str) -> Option<CpuAddress> {
        self.eval_address(expr)
    }

    #[inline]
    fn clear_breakpoint_flag(&mut self) {
        self.clear_breakpoint_flag();
    }

    #[inline]
    fn set_breakpoints(&mut self, bp_list: Vec<BreakPointType>) {
        self.set_breakpoints(bp_list)
    }

    #[inline]
    fn get_step_over_breakpoint(&self) -> Option<CpuAddress> {
        self.get_step_over_breakpoint()
    }

    #[inline]
    fn set_step_over_breakpoint(&mut self, address: CpuAddress) {
        self.set_step_over_breakpoint(address)
    }

    #[inline]
    fn get_sw_data(&self) -> Vec<StopWatchData> {
        self.get_sw_data()
    }

    #[inline]
    fn set_stopwatch(&mut self, sw_idx: usize, start: u32, stop: u32) {
        self.set_stopwatch(sw_idx, start, stop)
    }

    fn set_option(&mut self, opt: CpuOption) {
        match opt {
            CpuOption::InstructionHistory(state) => {
                log::debug!("Setting InstructionHistory to: {:?}", state);
                self.instruction_history.clear();
                self.instruction_history_on = state;
            }
            CpuOption::ScheduleInterrupt(_state, cycle_target, cycles, retrigger) => {
                log::debug!("Setting InterruptHint to: ({},{})", cycle_target, cycles);
                self.interrupt_scheduling = true;
                self.interrupt_cycle_period = cycle_target;
                self.interrupt_cycle_num = cycles;
                self.interrupt_retrigger = retrigger;
            }
            CpuOption::ScheduleDramRefresh(state, cycle_target, cycles, retrigger) => {
                log::trace!(
                    "Setting SimulateDramRefresh to: {:?} ({},{})",
                    state,
                    cycle_target,
                    cycles
                );
                self.dram_refresh_simulation = state;
                self.dram_refresh_cycle_period = cycle_target;
                self.dram_refresh_cycle_num = cycles;
                self.dram_refresh_retrigger = retrigger;
                self.dram_refresh_tc = false;
            }
            CpuOption::DramRefreshAdjust(adj) => {
                log::debug!("Setting DramRefreshAdjust to: {}", adj);
                self.dram_refresh_adjust = adj;
            }
            CpuOption::HaltResumeDelay(delay) => {
                log::debug!("Setting HaltResumeDelay to: {}", delay);
                self.halt_resume_delay = delay;
            }
            CpuOption::OffRailsDetection(state) => {
                log::debug!("Setting OffRailsDetection to: {:?}", state);
                self.off_rails_detection = state;
            }
            CpuOption::EnableWaitStates(state) => {
                log::debug!("Setting EnableWaitStates to: {:?}", state);
                self.enable_wait_states = state;
            }
            CpuOption::TraceLoggingEnabled(state) => {
                log::debug!("Setting TraceLoggingEnabled to: {:?}", state);
                self.trace_enabled = state;

                // Flush the trace log file on stopping trace so that we can immediately
                // see results otherwise buffered
                if state == false {
                    self.trace_flush();
                }
            }
            CpuOption::EnableServiceInterrupt(state) => {
                log::debug!("Setting EnableServiceInterrupt to: {:?}", state);
                self.enable_service_interrupt = state;
            }
        }
    }

    fn get_option(&self, opt: CpuOption) -> bool {
        match opt {
            CpuOption::InstructionHistory(_) => self.instruction_history_on,
            CpuOption::ScheduleInterrupt(..) => self.interrupt_cycle_period > 0,
            CpuOption::ScheduleDramRefresh(..) => self.dram_refresh_simulation,
            CpuOption::DramRefreshAdjust(..) => true,
            CpuOption::HaltResumeDelay(..) => true,
            CpuOption::OffRailsDetection(_) => self.off_rails_detection,
            CpuOption::EnableWaitStates(_) => self.enable_wait_states,
            CpuOption::TraceLoggingEnabled(_) => self.trace_enabled,
            CpuOption::EnableServiceInterrupt(_) => self.enable_service_interrupt,
        }
    }

    #[inline]
    fn bus(&self) -> &BusInterface {
        &self.bus
    }

    #[inline]
    fn bus_mut(&mut self) -> &mut BusInterface {
        &mut self.bus
    }

    // Logging methods
    fn cycle_table_header(&self) -> Vec<String> {
        self.cycle_table_header()
    }

    fn emit_header(&mut self) {
        self.emit_header();
    }

    fn trace_flush(&mut self) {
        self.trace_flush();
    }

    #[inline]
    #[cfg(feature = "cpu_validator")]
    fn get_vregisters(&self) -> VRegisters {
        self.get_vregisters()
    }

    // Validation methods

    #[cfg(feature = "cpu_validator")]
    fn get_validator(&self) -> &Option<Box<dyn CpuValidator>> {
        self.get_validator()
    }

    #[cfg(feature = "cpu_validator")]
    fn get_validator_mut(&mut self) -> &mut Option<Box<dyn CpuValidator>> {
        self.get_validator_mut()
    }

    fn randomize_seed(&mut self, seed: u64) {
        self.randomize_seed(seed);
    }

    fn randomize_mem(&mut self) {
        self.randomize_mem();
    }

    fn randomize_regs(&mut self) {
        self.randomize_regs();
    }

    fn random_grp_instruction(&mut self, opcode: u8, extension_list: &[u8]) {
        self.random_grp_instruction(opcode, extension_list)
    }

    fn random_inst_from_opcodes(&mut self, opcode_list: &[u8], _prefix: Option<u8>) {
        self.random_inst_from_opcodes(opcode_list);
    }

    fn logic_analyzer(&mut self) -> Option<&mut LogicAnalyzer> {
        Some(&mut self.analyzer)
    }

    fn bus_and_analyzer_mut(&mut self) -> (&mut BusInterface, Option<&mut LogicAnalyzer>) {
        if self.trace_enabled && matches!(self.trace_mode, TraceMode::CycleSigrok) {
            (&mut self.bus, Some(&mut self.analyzer))
        }
        else {
            (&mut self.bus, None)
        }
    }
}
