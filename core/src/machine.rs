/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    machine.rs

    This module defines all the parts that make up the virtual computer.

    This module owns Cpu and thus Bus, and is reponsible for maintaining both
    machine and CPU execution state and running the emulated machine by calling
    the appropriate methods on Bus.
    
*/
use log;

use std::{
    cell::Cell, 
    collections::VecDeque,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf
};

use crate::{
    config::{ConfigFileParams, MachineType, VideoType, TraceMode},
    breakpoints::BreakPointType,
    bus::{BusInterface, ClockFactor, DeviceEvent, MEM_CP_BIT},
    devices::{
        pit::{self, PitDisplayState},
        pic::{PicStringState},
        ppi::{PpiStringState},
        dma::{DMAControllerStringState},
        fdc::{FloppyController},
        hdc::{HardDiskController},
        mouse::Mouse,
        keyboard::KeyboardModifiers
    },
    cpu_808x::{Cpu, CpuError, CpuAddress, StepResult, ServiceEvent },
    cpu_common::{CpuType, CpuOption},
    machine_manager::{MachineDescriptor},
    rom_manager::{RomManager, RawRomDescriptor},
    sound::{BUFFER_MS, VOLUME_ADJUST, SoundPlayer},
    tracelogger::TraceLogger,
    videocard::{VideoCard, VideoCardState, VideoOption},
    keys::MartyKey
};

use ringbuf::{RingBuffer, Producer, Consumer};

pub const STEP_OVER_TIMEOUT: u32 = 320000;

pub const NUM_HDDS: u32 = 2;

pub const MAX_MEMORY_ADDRESS: usize = 0xFFFFF;

#[derive(Copy, Clone, Debug)]
pub struct KeybufferEntry {
    pub keycode: MartyKey,
    pub pressed: bool,
    pub modifiers: KeyboardModifiers,
    pub translate: bool
}

#[derive(Copy, Clone, Debug)]
pub enum MachineState {
    On,
    Paused,
    Resuming,
    Rebooting,
    Off
}

#[derive(Copy, Clone, Debug)]
pub enum ExecutionState {
    Paused,
    BreakpointHit,
    Running,
    Halted
}

#[allow (dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum ExecutionOperation {
    None,
    Pause,
    Step,
    StepOver,
    Run,
    Reset
}

#[derive(Copy, Clone, Debug, Default)]
pub struct DelayParams {
    pub dram_delay: u32,
    pub halt_resume_delay: u32
}

pub struct ExecutionControl {
    pub state: ExecutionState,
    op: Cell<ExecutionOperation>,
}

impl ExecutionControl {
    pub fn new() -> Self {
        Self { 
            state: ExecutionState::Paused,
            op: Cell::new(ExecutionOperation::None),
        }
    }

    pub fn set_state(&mut self, state: ExecutionState) {
        self.state = state
    }

    pub fn get_state(&self) -> ExecutionState {
        self.state
    }

    /// Sets the last execution operation.
    pub fn set_op(&mut self, op: ExecutionOperation) {

        match op {

            ExecutionOperation::Pause => {
                // Can only pause if Running
                if let ExecutionState::Running = self.state {
                    self.state = ExecutionState::Paused;
                    self.op.set(op);
                }
            }
            ExecutionOperation::Step => {
                // Can only Step if paused / breakpointhit
                if let ExecutionState::Paused | ExecutionState::BreakpointHit = self.state {
                    self.op.set(op);
                }              
            }
            ExecutionOperation::StepOver => {
                // Can only Step Over if paused / breakpointhit
                if let ExecutionState::Paused | ExecutionState::BreakpointHit = self.state {
                    self.op.set(op);
                }            
            }            
            ExecutionOperation::Run => {
                // Can only Run if paused / breakpointhit
                if let ExecutionState::Paused | ExecutionState::BreakpointHit = self.state {
                    self.op.set(op);
                } 
            }
            ExecutionOperation::Reset => {
                // Can reset anytime.
                self.op.set(op);
            }
            _ => {}
        }
        
    }

    /// Simultaneously returns the set execution operation and resets it internally to None.
    pub fn get_op(&mut self) -> ExecutionOperation {
        let op = self.op.get();
        self.op.set(ExecutionOperation::None);
        op
    }

    /// Returns the set execution operation without resetting it
    pub fn peek_op(&mut self) -> ExecutionOperation {
        self.op.get()
    }    

}

pub struct PitData {
    buffer_consumer: Consumer<u8>,
    samples_produced: u64,
    ticks_per_sample: f64,
    log_file: Option<Box<BufWriter<File>>>,
    logging_triggered: bool,
    fractional_part: f64,
    next_sample_size: usize
}

#[allow(dead_code)]
pub struct Machine 
{
    machine_type: MachineType,
    machine_desc: MachineDescriptor,
    state: MachineState,
    video_type: VideoType,
    sound_player: SoundPlayer,
    rom_manager: RomManager,
    load_bios: bool,
    cpu: Cpu, 
    speaker_buf_producer: Producer<u8>,
    pit_data: PitData,
    debug_snd_file: Option<File>,
    kb_buf: VecDeque<KeybufferEntry>,
    error: bool,
    error_str: Option<String>,
    cpu_factor: ClockFactor,
    next_cpu_factor: ClockFactor,
    cpu_cycles: u64,
    system_ticks: u64,
}

impl Machine {
    pub fn new(
        config: &ConfigFileParams,
        machine_type: MachineType,
        machine_desc: MachineDescriptor,
        trace_mode: TraceMode,
        video_type: VideoType,
        sound_player: SoundPlayer,
        rom_manager: RomManager,
        ) -> Machine 
    {

        //let mut io_bus = IoBusInterface::new();
        
        //let mut trace_file_option: Box<dyn Write + 'a> = Box::new(std::io::stdout());

        let mut trace_logger = TraceLogger::None;

        if config.emulator.trace_mode != TraceMode::None {
            // Open the trace file if specified
            if let Some(filename) = &config.emulator.trace_file {

                trace_logger = TraceLogger::from_filename(filename);

                if !trace_logger.is_some() {
                    log::error!("Couldn't create specified CPU tracelog file: {}", filename);
                    eprintln!("Couldn't create specified CPU tracelog file: {}", filename);
                }
            }
        }

        // Create PIT output log file if specified
        let mut pit_output_file_option = None;
        if let Some(filename) = &config.emulator.pit_output_file {
            match File::create(filename) {
                Ok(file) => {
                    pit_output_file_option = Some(Box::new(BufWriter::new(file)));
                },
                Err(e) => {
                    eprintln!("Couldn't create specified PIT log file: {}", e);
                }
            }
        }

        // Create the validator trace file, if specified
        #[cfg(feature = "cpu_validator")]
        let mut validator_trace = TraceLogger::None;
        #[cfg(feature = "cpu_validator")]
        {
            if let Some(trace_filename) = &config.validator.trace_file {
                validator_trace = TraceLogger::from_filename(&trace_filename);
            }
        }            

        let mut cpu = Cpu::new(
            CpuType::Intel8088,
            trace_mode,
            trace_logger,
            #[cfg(feature = "cpu_validator")]
            config.validator.vtype.unwrap(),
            #[cfg(feature = "cpu_validator")]
            validator_trace
        );

        cpu.set_option(CpuOption::TraceLoggingEnabled(config.emulator.trace_on));
        cpu.set_option(CpuOption::OffRailsDetection(config.cpu.off_rails_detection)); 

        // Set up Ringbuffer for PIT channel #2 sampling for PC speaker
        let speaker_buf_size = ((pit::PIT_MHZ * 1_000_000.0) * (BUFFER_MS as f64 / 1000.0)) as usize;
        let speaker_buf: RingBuffer<u8> = RingBuffer::new(speaker_buf_size);
        let (speaker_buf_producer, speaker_buf_consumer) = speaker_buf.split();
        let sample_rate = sound_player.sample_rate();
        let pit_ticks_per_sample = (pit::PIT_MHZ * 1_000_000.0) / sample_rate as f64;

        let pit_data = PitData {
            buffer_consumer: speaker_buf_consumer,
            ticks_per_sample: pit_ticks_per_sample,
            samples_produced: 0,
            log_file: pit_output_file_option,
            logging_triggered: false,
            fractional_part: pit_ticks_per_sample.fract(),
            next_sample_size: pit_ticks_per_sample.trunc() as usize
        };

        // open a file to write the sound to
        //let mut debug_snd_file = File::create("output.pcm").expect("Couldn't open debug pcm file");
        
        log::trace!("Sample rate: {} pit_ticks_per_sample: {}", sample_rate, pit_ticks_per_sample);

        // Create the video trace file, if specified
        let mut video_trace = TraceLogger::None;
        if let Some(trace_filename) = &config.emulator.video_trace_file {
            video_trace = TraceLogger::from_filename(&trace_filename);
        }

        // Install devices
        cpu.bus_mut().install_devices(
            video_type, 
            &machine_desc, 
            video_trace, 
            config.emulator.video_frame_debug
        );

        // Load keyboard translation file if specified.

        if let Some(kb_string) = &config.machine.keyboard_layout {
            let mut kb_translation_path = PathBuf::new();
            kb_translation_path.push(config.emulator.basedir.clone());
            kb_translation_path.push("keyboard");
            kb_translation_path.push(format!("keyboard_{}.toml", kb_string));

            match cpu.bus_mut().keyboard_mut().load_mapping(&kb_translation_path) {
                Ok(_) => {
                    println!("Loaded keyboard mapping file: {}", kb_translation_path.display());
                }
                Err(e) => {
                    eprintln!("Failed to load keyboard mapping file: {} Err: {}", kb_translation_path.display(), e )
                }
            }
        }

        // Set keyboard debug flag.
        cpu.bus_mut().keyboard_mut().set_debug(config.emulator.debug_keyboard);

        // Load BIOS ROM images unless config option suppressed rom loading
        if !config.emulator.no_bios {

            rom_manager.copy_into_memory(cpu.bus_mut());

            // Load checkpoint flags into memory
            rom_manager.install_checkpoints(cpu.bus_mut());

            // Set entry point for ROM (mostly used for diagnostic ROMs that used the wrong jump at reset vector)
    
            let rom_entry_point = rom_manager.get_entrypoint();
            cpu.set_reset_vector(CpuAddress::Segmented(rom_entry_point.0, rom_entry_point.1));
        }

        // Set CPU clock divisor/multiplier
        let cpu_factor;
        if config.machine.turbo { 
            cpu_factor = machine_desc.cpu_turbo_factor;
        }
        else {
            cpu_factor = machine_desc.cpu_factor;
        }

        cpu.reset();

        Machine {
            machine_type,
            machine_desc,
            state: MachineState::On,
            video_type,
            sound_player,
            rom_manager,
            load_bios: !config.emulator.no_bios,
            cpu,
            speaker_buf_producer,
            pit_data,
            debug_snd_file: None,
            kb_buf: VecDeque::new(),
            error: false,
            error_str: None,
            cpu_factor,
            next_cpu_factor: cpu_factor,
            cpu_cycles: 0,
            system_ticks: 0
        }
    }

    pub fn change_state(&mut self, new_state: MachineState) {

        match (self.state, new_state) {

            (MachineState::Off, MachineState::On) => {
                log::debug!("Turning machine on...");
                self.state = new_state;
            }
            (MachineState::On, MachineState::Off) => {
                log::debug!("Turning machine off...");
                self.reset();
                self.state = new_state;
            }
            (MachineState::On, MachineState::Rebooting) => {
                log::debug!("Rebooting machine...");
                self.reset();
                self.state = MachineState::On;
            }
            (MachineState::On, MachineState::Paused) => {
                log::debug!("Pausing machine...");
                self.state = new_state;
            }
            (MachineState::Paused, MachineState::Resuming) => {
                log::debug!("Resuming machine...");
                self.state = MachineState::On;
            }
            _ => {}
        }

    }

    pub fn get_state(&self) -> MachineState {
        self.state
    }

    pub fn load_program(&mut self, program: &[u8], program_seg: u16, program_ofs: u16) -> Result<(), bool> {

        let location = Cpu::calc_linear_address(program_seg, program_ofs);
        
        self.cpu.bus_mut().copy_from(program, location as usize, 0, false)?;

        self.cpu.set_reset_vector(CpuAddress::Segmented(program_seg, program_ofs));
        self.cpu.reset();

        self.cpu.set_end_address(((location as usize) + program.len()) & 0xFFFFF );

        Ok(())
    }

    pub fn bus(&self) -> &BusInterface {
        self.cpu.bus()
    }

    pub fn bus_mut(&mut self) -> &mut BusInterface {
        self.cpu.bus_mut()
    }

    //pub fn cga(&self) -> Rc<RefCell<CGACard>> {
    //    self.cga.clone()
    //}

    pub fn videocard(&mut self) -> Option<Box<&mut dyn VideoCard>> {
        self.cpu.bus_mut().video_mut()
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    /// Set a CPU option. Avoids needing to borrow CPU.
    pub fn set_cpu_option(&mut self, opt: CpuOption) {
        self.cpu.set_option(opt);
    }

    /// Get a CPU option. Avoids needing to borrow CPU.
    pub fn get_cpu_option(&mut self, opt: CpuOption) -> bool {
        self.cpu.get_option(opt)
    }    

    /// Send the specified video option to the active videocard device
    pub fn set_video_option(&mut self, opt: VideoOption) {
        if let Some(video) = self.cpu.bus_mut().video_mut() {
            video.set_video_option(opt);
        }
    }

    /// Flush all trace logs for devices that have one
    pub fn flush_trace_logs(&mut self) {
        self.cpu.trace_flush();
        if let Some(video) = self.cpu.bus_mut().video_mut() {
            video.trace_flush();   
        }
    }

    /// Return the current CPU clock frequency in MHz.
    /// This can vary during system execution if state of turbo button is toggled.
    /// CPU speed is always some factor of the main system crystal frequency.
    /// The CPU itself has no concept of its operational frequency.
    pub fn get_cpu_mhz(&self) -> f64 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => {
                self.machine_desc.system_crystal / (n as f64)
            }
            ClockFactor::Multiplier(n) => {
                self.machine_desc.system_crystal * (n as f64)
            }
        }
    }

    /// Set the specified state of the turbo button. True will enable turbo mode
    /// and switch to the turbo mode CPU clock factor.
    /// 
    /// We must be careful not to update this between step() and run_devices() or devices' 
    /// advance_ticks may overflow device update ticks.
    pub fn set_turbo_mode(&mut self, state: bool) {
        
        if state {
            self.next_cpu_factor = self.machine_desc.cpu_turbo_factor;
        }
        else {
            self.next_cpu_factor = self.machine_desc.cpu_factor;
        }
        log::debug!("Set turbo mode to: {} New cpu factor is {:?}", state, self.next_cpu_factor);
    }

    pub fn fdc(&mut self) -> &mut Option<FloppyController> {
        self.cpu.bus_mut().fdc_mut()
    }

    pub fn hdc(&mut self) -> &mut Option<HardDiskController> {
        self.cpu.bus_mut().hdc_mut()
    }

    pub fn cpu_cycles(&self) -> u64 {
        self.cpu_cycles
    }

    pub fn system_ticks(&self) -> u64 {
        self.system_ticks
    }

    /// Return the number of cycles the PIT has ticked.
    pub fn pit_cycles(&self) -> u64 {
        // Safe to unwrap pit as a PIT will always exist on any machine type
        self.cpu.bus().pit().as_ref().unwrap().get_cycles()
    }

    /// Return the PIT's state as a PitDisplaySate struct. 
    /// This is a mutable function as receiving the display state resets the various
    /// state variable's dirty flags.
    pub fn pit_state(&mut self) -> PitDisplayState {
        // Safe to unwrap pit as a PIT will always exist on any machine type
        let pit = self.cpu.bus_mut().pit_mut().as_mut().unwrap();
        let pit_data = pit.get_display_state(true);
        pit_data
    }

    pub fn get_pit_buf(&self) -> Vec<u8> {
        let (a,b) = self.pit_data.buffer_consumer.as_slices();

        a.iter().cloned().chain(b.iter().cloned()).collect()
    }

    /// Adjust the relative phase of CPU and PIT; this is done by subtracting the relevant number of 
    /// system ticks from the next run of the PIT.
    pub fn pit_adjust(&mut self, ticks: u32) {

        self.cpu.bus_mut().adjust_pit(ticks);
    }

    pub fn pic_state(&mut self) -> PicStringState {
        // There will always be a primary PIC, so safe to unwrap.
        // TODO: Handle secondary PIC if present.
        self.cpu.bus_mut().pic_mut().as_mut().unwrap().get_string_state()
    }

    pub fn ppi_state(&mut self) -> Option<PpiStringState> {

        if let Some(ppi) = self.cpu.bus_mut().ppi_mut() {
            Some(ppi.get_string_state())
        }
        else {
            None
        }
    }
    
    pub fn set_nmi(&mut self, state: bool) {
        self.cpu.set_nmi(state);
    }

    pub fn dma_state(&mut self) -> DMAControllerStringState {
        // There will always be a primary DMA, so safe to unwrap.
        // TODO: Handle secondary DMA if present.
        self.cpu.bus_mut().dma_mut().as_mut().unwrap().get_string_state()
    }
    
    pub fn videocard_state(&mut self) -> Option<VideoCardState> {
        if let Some(video_card) = self.cpu.bus_mut().video_mut() {
            // A video card is present
            Some(video_card.get_videocard_string_state())
        }
        else {
            // no video card
            None
        }
    }

    pub fn get_error_str(&self) -> &Option<String> {
        &self.error_str
    }

    /// Enter a keypress keycode into the emulator keyboard buffer.
    pub fn key_press(&mut self, keycode: MartyKey, modifiers: KeyboardModifiers) {

        self.kb_buf.push_back(
            KeybufferEntry{
                keycode,
                pressed: true,
                modifiers,
                translate: true
            }
        );
    }

    /// Enter a key release keycode into the emulator keyboard buffer.
    pub fn key_release(&mut self, keycode: MartyKey ) {
        // HO Bit set converts a scancode into its 'release' code
        self.kb_buf.push_back(            
            KeybufferEntry{
                keycode,
                pressed: false,
                modifiers: KeyboardModifiers::default(),
                translate: true
            }
        );
    }

    /// Simulate the user pressing control-alt-delete.
    pub fn ctrl_alt_del(&mut self) {
        /*
        self.kb_buf.push_back(0x1D); // Left-control
        self.kb_buf.push_back(0x38); // Left-alt
        self.kb_buf.push_back(0x53); // Delete

        // Debugging only. A real PC does not reset anything on ctrl-alt-del
        //self.bus_mut().reset_devices_warm();

        self.kb_buf.push_back(0x1D | 0x80);
        self.kb_buf.push_back(0x38 | 0x80);
        self.kb_buf.push_back(0x53 | 0x80);
        */
    }

    pub fn mouse_mut(&mut self) -> &mut Option<Mouse> {
        self.cpu.bus_mut().mouse_mut()
    }

    pub fn bridge_serial_port(&mut self, port_num: usize, port_name: String) {

        if let Some(spc) = self.cpu.bus_mut().serial_mut() {
            if let Err(e) = spc.bridge_port(port_num, port_name) {
                log::error!("Failed to bridge serial port: {}", e );
            }
        }
        else {
            log::error!("No serial port controller present!");
        }
    }

    pub fn set_breakpoints(&mut self, bp_list: Vec<BreakPointType>) {
        self.cpu.set_breakpoints(bp_list)
    }

    pub fn reset(&mut self) {

        // TODO: Reload any program specified here?

        // Clear any error state.
        self.error = false;
        self.error_str = None;

        // Reset CPU.
        self.cpu.reset();

        // Clear RAM
        self.cpu.bus_mut().clear();

        // Reload BIOS ROM images
        if self.load_bios {
            self.rom_manager.copy_into_memory(self.cpu.bus_mut());
            // Clear patch installation status
            self.rom_manager.reset_patches();
        }

        // Reset all installed devices.
        self.cpu.bus_mut().reset_devices();
    }

    #[inline]
    /// Convert a count of CPU cycles to microseconds based on the current CPU clock
    /// divisor and system crystal speed.
    fn cpu_cycles_to_us(&self, cycles: u32) -> f64 {

        let mhz = match self.cpu_factor {
            ClockFactor::Divisor(n) => self.machine_desc.system_crystal / (n as f64),
            ClockFactor::Multiplier(n) => self.machine_desc.system_crystal * (n as f64)
        };

        1.0 / mhz * cycles as f64
    }
    
    #[inline]
    /// Convert a count of CPU cycles to system clock ticks based on the current CPU
    /// clock divisor.
    fn cpu_cycles_to_system_ticks(&self, cycles: u32) -> u32 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => cycles * (n as u32),
            ClockFactor::Multiplier(n) => cycles / (n as u32)
        }
    }

    pub fn run(&mut self, cycle_target: u32, exec_control: &mut ExecutionControl) -> u64 {

        let mut kb_event_processed = false;
        let mut skip_breakpoint = false;
        let mut instr_count = 0;

        // Update cpu factor.
        let new_factor = self.next_cpu_factor;
        self.cpu_factor = new_factor;
        self.bus_mut().set_cpu_factor(new_factor);

        // Was reset requested?
        if let ExecutionOperation::Reset = exec_control.peek_op() {
            _ = exec_control.get_op(); // Clear the reset operation
            self.reset();
            exec_control.state = ExecutionState::Paused;
            return 0
        }

        let mut step_over = false;
        let cycle_target_adj = match exec_control.state {
            ExecutionState::Paused => {
                match exec_control.get_op() {
                    ExecutionOperation::Step => {
                        // Skip current breakpoint, if any
                        skip_breakpoint = true;
                        // Execute 1 cycle
                        1
                    },
                    ExecutionOperation::StepOver => {
                        // Skip current breakpoint, if any
                        skip_breakpoint = true;
                        // Set step-over flag
                        step_over = true;
                        // Execute 1 cycle
                        1                        
                    }
                    ExecutionOperation::Run => {
                        // Transition to ExecutionState::Running
                        exec_control.state = ExecutionState::Running;
                        cycle_target
                    },                      
                    _ => return 0
                }
            
            },
            ExecutionState::Running => {
                _ = exec_control.get_op(); // Clear any pending operation
                cycle_target
            },
            ExecutionState::BreakpointHit => {
                match exec_control.get_op() {
                    ExecutionOperation::Step => {
                        log::trace!("BreakpointHit -> Step");
                        // Clear CPU's breakpoint flag
                        self.cpu.clear_breakpoint_flag();
                        // Skip current breakpoint, if any
                        skip_breakpoint = true;
                        // Transition to ExecutionState::Paused
                        exec_control.state = ExecutionState::Paused;

                        // Execute one instruction only
                        1
                    },
                    ExecutionOperation::StepOver => {
                        log::trace!("BreakpointHit -> StepOver");
                        // Clear CPU's breakpoint flag
                        self.cpu.clear_breakpoint_flag();
                        // Skip current breakpoint, if any
                        skip_breakpoint = true;
                        // Set the step over flag
                        step_over = true;
                        // Transition to ExecutionState::Paused
                        exec_control.state = ExecutionState::Paused;

                        // Execute one instruction only
                        1
                    },
                    ExecutionOperation::Run => {
                        // Clear CPU's breakpoint flag
                        self.cpu.clear_breakpoint_flag();
                        // Skip current breakpoint, if any
                        skip_breakpoint = true;
                        // Transition to ExecutionState::Running
                        exec_control.state = ExecutionState::Running;
                        cycle_target
                    },                    
                    _ => return 0
                }

            },
            ExecutionState::Halted => {
                match exec_control.get_op() {
                    ExecutionOperation::Run => {
                        // Transition to ExecutionState::Running
                        exec_control.state = ExecutionState::Running;
                        cycle_target
                    }
                    _ => return 0
                }
            }
        };

        let do_run = match self.state {
            MachineState::On => true,
            _ => false
        };

        if !do_run {
            return 0;
        }

        let mut cycles_elapsed = 0;

        while cycles_elapsed < cycle_target_adj {

            let fake_cycles: u32 = 7;
            let mut cpu_cycles;

            if self.cpu.is_error() {
                break;
            }

            let flat_address = self.cpu.get_linear_ip();

            // Match checkpoints
            if self.cpu.bus().get_flags(flat_address as usize) & MEM_CP_BIT != 0 {
                if let Some(cp) = self.rom_manager.get_checkpoint(flat_address) {
                    log::trace!("ROM CHECKPOINT: [{:05X}] {}", flat_address, cp);
                }

                // Check for patching checkpoint & install patches
                if self.rom_manager.is_patch_checkpoint(flat_address) {
                    log::trace!("ROM PATCH CHECKPOINT: [{:05X}] Installing ROM patches...", flat_address);
                    self.rom_manager.install_patch(self.cpu.bus_mut(), flat_address);
                }
            }
            
            let mut step_over_target = None;

            match self.cpu.step(skip_breakpoint) {
                Ok((step_result, step_cycles)) => {

                    match step_result {
                        StepResult::Normal => {
                            cpu_cycles = step_cycles;
                        },
                        StepResult::Call(target) => {
                            cpu_cycles = step_cycles;
                            step_over_target = Some(target);
                        }
                        StepResult::BreakpointHit => {
                            exec_control.state = ExecutionState::BreakpointHit;
                            return 1
                        }
                        StepResult::ProgramEnd => {
                            log::debug!("Program ended execution.");
                            exec_control.state = ExecutionState::Halted;
                            return 1
                        }                        
                    }
                    
                },
                Err(err) => {
                    if let CpuError::CpuHaltedError(_) = err {
                        log::error!("CPU Halted!");
                        self.cpu.trace_flush();
                        exec_control.state = ExecutionState::Halted;
                    }
                    self.error = true;
                    self.error_str = Some(format!("{}", err));
                    log::error!("CPU Error: {}\n{}", err, self.cpu.dump_instruction_history_string());
                    cpu_cycles = 0
                } 
            }

            if cpu_cycles > 200 {
                log::warn!("CPU instruction took too long! Cycles: {}", cpu_cycles);
            }

            instr_count += 1;
            cycles_elapsed += cpu_cycles;
            self.cpu_cycles += cpu_cycles as u64;            

            if cpu_cycles == 0 {
                log::warn!("Instruction returned 0 cycles");
                cpu_cycles = fake_cycles;
            }

            self.run_devices(cpu_cycles, &mut kb_event_processed);

            // If we returned a step over target address, execution is paused, and step over was requested, 
            // then consume as many instructions as needed to get to to the 'next' instruction. This will
            // skip over any CALL or interrupt encountered.
            if step_over {
                if let Some(step_over_target) = step_over_target {

                    log::debug!("Step over requested for CALL, return addr: {}", step_over_target );
                    let mut cs_ip = self.cpu.get_csip();
                    let mut step_over_cycles = 0;

                    while cs_ip != step_over_target {

                        match self.cpu.step(skip_breakpoint) {
                            Ok((step_result, step_cycles)) => {
            
                                match step_result {
                                    StepResult::Normal => {
                                        cpu_cycles = step_cycles
                                    },
                                    StepResult::Call(_) => {
                                        cpu_cycles = step_cycles
                                        // We are already stepping over a base CALL instruction, so ignore futher CALLS/interrupts.
                                    }
                                    StepResult::BreakpointHit => {
                                        // We can hit an 'inner' breakpoint while stepping over. This is fine, and ends the step
                                        // over operation at the breakpoint.
                                        exec_control.state = ExecutionState::BreakpointHit;
                                        return instr_count
                                    }
                                    StepResult::ProgramEnd => {
                                        exec_control.state = ExecutionState::Halted;
                                        return instr_count
                                    }
                                }
                            },
                            Err(err) => {
                                if let CpuError::CpuHaltedError(_) = err {
                                    log::error!("CPU Halted!");
                                    exec_control.state = ExecutionState::Halted;
                                }
                                self.error = true;
                                self.error_str = Some(format!("{}", err));
                                log::error!("CPU Error: {}\n{}", err, self.cpu.dump_instruction_history_string());
                                cpu_cycles = 0
                            } 
                        }

                        instr_count += 1;
                        cycles_elapsed += cpu_cycles;
                        self.cpu_cycles += cpu_cycles as u64;            

                        step_over_cycles += cpu_cycles;
            
                        if cpu_cycles == 0 {
                            log::warn!("Instruction returned 0 cycles");
                            cpu_cycles = fake_cycles;
                        }
            
                        self.run_devices(cpu_cycles, &mut kb_event_processed);

                        cs_ip = self.cpu.get_csip();

                        if step_over_cycles > STEP_OVER_TIMEOUT {
                            log::warn!("Step over operation timed out: No return after {} cycles.", STEP_OVER_TIMEOUT);
                            break;
                        }
                    }
                }
            }

            if let Some(event) = self.cpu.get_service_event() {
                match event {
                    ServiceEvent::TriggerPITLogging => {
                        log::debug!("PIT logging has been triggered.");
                        self.pit_data.logging_triggered = true;
                    }
                }
            }
        }

        //log::debug!("cycles_elapsed: {}", cycles_elapsed);
        
        instr_count
    }

    pub fn run_devices(&mut self, cpu_cycles: u32, kb_event_processed: &mut bool) -> u32 {

        // Convert cycles into elapsed microseconds
        let us = self.cpu_cycles_to_us(cpu_cycles);

        // Convert cycles into system clock ticks
        let sys_ticks = self.cpu_cycles_to_system_ticks(cpu_cycles);

        // Process a keyboard event once per frame.
        // A reasonably fast typist can generate two events in a single 16ms frame, and to the virtual cpu
        // they then appear to happen instantenously. The PPI has no buffer, so one scancode gets lost. 
        // 
        // If we limit keyboard events to once per frame, this avoids this problem. I'm a reasonably
        // fast typist and this method seems to work fine.
        let mut kb_event_opt: Option<KeybufferEntry> = None;
        if self.kb_buf.len() > 0 && !*kb_event_processed {

            kb_event_opt = self.kb_buf.pop_front();
            if kb_event_opt.is_some() {
                *kb_event_processed = true;
            }
        }

        // Run devices.
        // We send the IO bus the elapsed time in us, and a mutable reference to the PIT channel #2 ring buffer
        // so that we can collect output from the timer.
        let device_event = self.cpu.bus_mut().run_devices(
            us, 
            sys_ticks,
            kb_event_opt, 
            &mut self.kb_buf,
            &mut self.speaker_buf_producer,
        );

        // Currently only one device run event type
        if let Some(DeviceEvent::DramRefreshUpdate(dma_counter, dma_counter_val)) = device_event {
            self.cpu.set_option(
                CpuOption::SimulateDramRefresh(
                    true, 
                    self.timer_ticks_to_cpu_cycles(dma_counter), 
                    self.timer_ticks_to_cpu_cycles(dma_counter_val)
                    //self.timer_ticks_to_cpu_cycles(0)
                )
            )
        }

        // Sample the PIT channel #2 for sound
        while self.speaker_buf_producer.len() >= self.pit_data.next_sample_size {
            self.pit_buf_to_sound_buf();
        }

        self.system_ticks += sys_ticks as u64;
        sys_ticks
    }

    fn timer_ticks_to_cpu_cycles(&self, timer_ticks: u16) -> u32 {

        let timer_multiplier = 
            if let Some(_timer_crystal) = self.machine_desc.timer_crystal {
                // We have an alternate 
                todo!("Unimplemented conversion for AT timer");
                //1
            }
            else {
                match self.machine_desc.cpu_factor {
                    ClockFactor::Divisor(n) => {
                        self.machine_desc.timer_divisor / (n as u32)
                    }
                    ClockFactor::Multiplier(_n) => {
                        todo!("unimplemented conversion for CPU multiplier");
                        //1
                    }
                }
            };

        timer_ticks as u32 * timer_multiplier
    }

    /// Called to update machine once per frame.
    /// Mostly used for serial passthrouh function to synchronize virtual
    /// serial port with real serial port.
    pub fn frame_update(&mut self) {

        // Update serial port, if present
        if let Some(spc) =  self.cpu.bus_mut().serial_mut() {
            spc.update();
        }  
    }

    pub fn play_sound_buffer(&self) {
        self.sound_player.play();
    }

    pub fn pit_buf_to_sound_buf(&mut self) {

        let nsamples = self.pit_data.next_sample_size;
        if self.pit_data.buffer_consumer.len() < self.pit_data.next_sample_size {
            return
        }

        let mut sum = 0;
        let mut sample;
        let mut samples_read = false;

        // If logging enabled, read samples and log to file.
        if let Some(file) = self.pit_data.log_file.as_mut() {
            if self.pit_data.logging_triggered {
                for _ in 0..nsamples {

                    sample = match self.pit_data.buffer_consumer.pop() {
                        Some(s) => s,
                        None => {
                            log::trace!("No byte in pit buffer");
                            0
                        }
                    };
                    sum += sample;

                    let sample_f32: f32 = if sample == 0 { 0.0 } else { 1.0 };
                    file.write(&sample_f32.to_le_bytes()).expect("Error writing to debug sound file");

                }
                samples_read = true;
            }
        }

        // Otherwise, just read samples
        if !samples_read {
            for _ in 0..nsamples {
            
                sample = match self.pit_data.buffer_consumer.pop() {
                    Some(s) => s,
                    None => {
                        log::trace!("No byte in pit buffer");
                        0
                    }
                };
                sum += sample;
            }
        }

        // Averaging samples is effectively a poor lowpass filter.
        // TODO: replace with actual lowpass filter from biquad?
        let average: f32 = sum as f32 / nsamples as f32;

        //log::trace!("Sample: sum: {}, ticks: {}, avg: {}", sum, pit_ticks, average);
        self.pit_data.samples_produced += 1;
        //log::trace!("producer: {}", self.pit_samples_produced);
        self.sound_player.queue_sample(average as f32 * VOLUME_ADJUST);

        // Calculate size of next audio sample in pit samples by carrying over fractional part
        let next_sample_f: f64 = self.pit_data.ticks_per_sample + self.pit_data.fractional_part;

        self.pit_data.next_sample_size = next_sample_f as usize;
        self.pit_data.fractional_part = next_sample_f.fract();
    }

}