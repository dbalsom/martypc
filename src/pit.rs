/* 
    pit.rs 
    Implement the Intel 8253 Programmable Interval Timer
*/

use log;

use std::collections::BTreeMap;

use crate::io::{IoBusInterface, IoDevice};
use crate::bus::{BusInterface};
use crate::cpu_808x::CPU_MHZ;
use crate::pic;
use crate::dma;
use crate::ppi;
use crate::syntax_token::*;

pub type PitDisplayState = Vec<BTreeMap<&'static str, SyntaxToken>>;

const PIT_CHANNEL_PORT_BASE: u16 = 0x40;
pub const PIT_CHANNEL_0_DATA_PORT: u16 = 0x40;
pub const PIT_CHANNEL_1_DATA_PORT: u16 = 0x41;
pub const PIT_CHANNEL_2_DATA_PORT: u16 = 0x42;
pub const PIT_COMMAND_REGISTER: u16 = 0x43;

const PIT_CHANNEL_SELECT_MASK: u8 = 0b1100_0000;
const PIT_ACCESS_MODE_MASK: u8    = 0b0011_0000;
const PIT_OPERATING_MODE_MASK: u8 = 0b0000_1110;
const PIT_BCD_MODE_MASK: u8       = 0b0000_0001;

pub const PIT_MHZ: f64 = 1.193182;
pub const PIT_DIVISOR: f64 = 0.25;

macro_rules! dirty_update_checked {
    ($old: expr, $new: expr, $flag: expr) => {
        {
            if $old != $new {
                $flag = true;
            }
            $old = $new
        }
    };
}

macro_rules! dirty_update {
    ($old: expr, $new: expr, $flag: expr) => {
        {
            $flag = true;
            $old = $new
        }
    };
}

#[derive(Debug, PartialEq)]
enum ChannelMode {
    InterruptOnTerminalCount,
    HardwareRetriggerableOneShot,
    RateGenerator,
    SquareWaveGenerator,
    SoftwareTriggeredStrobe,
    HardwareTriggeredStrobe
}

#[derive(Debug, PartialEq)]
enum AccessMode {
    LatchCountValue,
    LoByteOnly,
    HiByteOnly,
    LoByteHiByte
}

pub struct PitChannel {
    mode: ChannelMode,
    mode_dirty: bool,
    access_mode: AccessMode,
    access_mode_dirty: bool,
    reload_value: u16,
    reload_value_dirty: bool,
    waiting_for_reload: bool,
    waiting_for_lobyte: bool,
    waiting_for_hibyte: bool,
    current_count: u16,
    current_count_dirty: bool,
    read_in_progress: bool,
    normal_lobyte_read: bool,    
    count_is_latched: bool,
    output: bool,
    output_dirty: bool,
    latched_lobyte_read: bool,
    latch_count: u16,
    latch_lobit: bool,
    latch_lobit_count: u32,
    bcd_mode: bool,
    input_gate: bool,
    input_gate_dirty: bool,
    one_shot_triggered: bool,
    gate_triggered: bool,
}

pub struct ProgrammableIntervalTimer {
    pit_cycles: u64,
    cycle_accumulator: f64,
    channels: Vec<PitChannel>,
}
pub type Pit = ProgrammableIntervalTimer;

#[derive(Default, Clone)]
pub struct PitStringState {
    pub c0_value: SyntaxToken,
    pub c0_reload_value: SyntaxToken,
    pub c0_access_mode: SyntaxToken,
    pub c0_channel_mode: SyntaxToken,
    pub c0_channel_output: SyntaxToken,
    pub c1_value: SyntaxToken,
    pub c1_reload_value: SyntaxToken,
    pub c1_access_mode: SyntaxToken,
    pub c1_channel_mode: SyntaxToken,
    pub c1_channel_output: SyntaxToken,
    pub c2_value: SyntaxToken,
    pub c2_reload_value: SyntaxToken,
    pub c2_access_mode: SyntaxToken,
    pub c2_channel_mode: SyntaxToken,
    pub c2_channel_output: SyntaxToken,
    pub c2_gate_status: SyntaxToken,
}

impl IoDevice for ProgrammableIntervalTimer {
    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            PIT_COMMAND_REGISTER => 0,
            PIT_CHANNEL_0_DATA_PORT => self.data_read(0),
            PIT_CHANNEL_1_DATA_PORT => self.data_read(1),
            PIT_CHANNEL_2_DATA_PORT => self.data_read(2),
            _ => panic!("PIT: Bad port #")
        }
    }
    fn write_u8(&mut self, port: u16, data: u8) {
        match port {
            PIT_COMMAND_REGISTER => self.command_register_write(data),
            PIT_CHANNEL_0_DATA_PORT => self.data_write(0, data),
            PIT_CHANNEL_1_DATA_PORT => self.data_write(1, data),
            PIT_CHANNEL_2_DATA_PORT => self.data_write(2, data),
            _ => panic!("PIT: Bad port #")
        }
    }

}

impl ProgrammableIntervalTimer {
    pub fn new() -> Self {
        /*
            The Intel documentation says: 
            "Prior to initialization, the mode, count, and output of all counters is undefined."
            This makes it a challenge to decide the initial state of the virtual PIT. The 5160 
            BIOS will halt during POST if there's a pending timer interrupt, so that's a clue we
            shouldn't initially start a timer running, but beyond that it's a guess.
        */
        let mut vec = Vec::<PitChannel>::new();
        for _ in 0..3 {
            let pit = PitChannel {
                mode: ChannelMode::InterruptOnTerminalCount,
                mode_dirty: false,
                access_mode: AccessMode::HiByteOnly,
                access_mode_dirty: false,
                reload_value: 0,
                reload_value_dirty: false,
                waiting_for_reload: true,
                waiting_for_lobyte: false,
                waiting_for_hibyte: false,
                current_count: 0,
                current_count_dirty: false,
                read_in_progress: false,
                normal_lobyte_read: false,
                count_is_latched: false,
                output: false,
                output_dirty: false,
                latched_lobyte_read: false,
                latch_count: 0,
                latch_lobit: false,
                latch_lobit_count: 0,
                bcd_mode: false,
                input_gate: true,
                input_gate_dirty: false,
                one_shot_triggered: false,
                gate_triggered: false,
            };
            vec.push(pit);
        }
        Self {
            pit_cycles: 0,
            cycle_accumulator: 0.0,
            channels: vec
        }
    }

    pub fn reset(&mut self) {

        self.cycle_accumulator = 0.0;
        
        for channel in &mut self.channels {
            channel.mode = ChannelMode::InterruptOnTerminalCount;
            channel.mode_dirty = false;
            channel.access_mode = AccessMode::HiByteOnly;
            channel.access_mode_dirty = false;
            channel.reload_value = 0;
            channel.reload_value_dirty = false;
            channel.waiting_for_reload = true;
            channel.waiting_for_lobyte = false;
            channel.waiting_for_hibyte = false;
            channel.current_count = 0;
            channel.current_count_dirty = false;
            channel.read_in_progress = false;
            channel.normal_lobyte_read = false;
            channel.count_is_latched = false;
            channel.output = false;
            channel.output_dirty = false;
            channel.latched_lobyte_read = false;
            channel.latch_count = 0;
            channel.latch_lobit = false;
            channel.latch_lobit_count = 0;
            channel.bcd_mode = false;
            channel.input_gate = true;
            channel.input_gate_dirty = false;
            channel.one_shot_triggered = false;
            channel.gate_triggered = false;
        }
    }

    fn is_latch_command(command_byte: u8) -> bool {
        command_byte & PIT_ACCESS_MODE_MASK == 0
    }

    fn get_pit_cycles(cpu_cycles: u32) -> f64 {
        cpu_cycles as f64 * PIT_DIVISOR
    }

    fn parse_command_register(&mut self, command_byte: u8) -> (usize, AccessMode, ChannelMode, bool) {
        
        let channel_select: usize = (command_byte >> 6) as usize;

        let access_mode = match (command_byte & PIT_ACCESS_MODE_MASK) >> 4 {
            0b00 => AccessMode::LatchCountValue,
            0b01 => AccessMode::LoByteOnly,
            0b10 => AccessMode::HiByteOnly,
            0b11 => AccessMode::LoByteHiByte,
            _ => unreachable!("Bad PIT Access mode")
        };

        let channel_mode = match (command_byte & PIT_OPERATING_MODE_MASK) >> 1 {
            0b000 => ChannelMode::InterruptOnTerminalCount,
            0b001 => ChannelMode::HardwareRetriggerableOneShot,
            0b010 => ChannelMode::RateGenerator,
            0b011 => ChannelMode::SquareWaveGenerator,
            0b100 => ChannelMode::SoftwareTriggeredStrobe,
            0b101 => ChannelMode::HardwareTriggeredStrobe,
            0b110 => ChannelMode::RateGenerator,
            0b111 => ChannelMode::SquareWaveGenerator,
            _ => unreachable!("Bad PIT Operating mode")
        };

        let bcd_enable = command_byte & PIT_BCD_MODE_MASK == 0x01;
        if bcd_enable {
            log::error!("PIT: BCD mode unimplemented");
        }
        (channel_select, access_mode, channel_mode, bcd_enable)
    }

    fn command_register_write(&mut self, command_byte: u8) {

        let (channel_select, access_mode, channel_mode, bcd_enable) = self.parse_command_register(command_byte);

        if let AccessMode::LatchCountValue = access_mode {
            // All 0's access mode indicates a Latch Count Value command
            // Not an access mode itself, we now latch the current value of the channel until it is read
            // or a command byte is received
            self.channels[channel_select].latch_count = self.channels[channel_select].current_count;
            self.channels[channel_select].count_is_latched = true;
            self.channels[channel_select].latched_lobyte_read = false;
        }
        else {
            log::debug!("PIT: Channel {} selected, access mode {:?}, channel_mode {:?}, bcd: {:?}", channel_select, access_mode, channel_mode, bcd_enable );

            let channel = &mut self.channels[channel_select];

            dirty_update_checked!(channel.mode, channel_mode, channel.mode_dirty);

            match channel.mode {
                ChannelMode::InterruptOnTerminalCount => {
                    // Intel: The output will be intiially low after the mode set operation.
                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    channel.waiting_for_reload = true;
                }
                ChannelMode::HardwareRetriggerableOneShot => {
                    // osdev: When the mode/command register is written the output signal goes high
                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    channel.waiting_for_reload = true;
                    channel.one_shot_triggered = false;
                    channel.gate_triggered = false;
                }
                ChannelMode::RateGenerator => {
                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    channel.waiting_for_reload = true;
                },
                ChannelMode::SquareWaveGenerator => {
                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    channel.waiting_for_reload = true;
                },
                ChannelMode::SoftwareTriggeredStrobe => {
                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    channel.waiting_for_reload = true;
                },
                ChannelMode::HardwareTriggeredStrobe => {
                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    channel.waiting_for_reload = true;
                    channel.one_shot_triggered = false;
                    channel.gate_triggered = false;
                }
            }
            
            dirty_update_checked!(channel.reload_value, 0, channel.reload_value_dirty);
            dirty_update_checked!(channel.access_mode, access_mode, channel.access_mode_dirty);
            
            channel.bcd_mode = bcd_enable;
        }        

    }

    /// Handle a write to one of the PIT's data registers
    /// Writes to this register specify the reload value for the given channel.
    pub fn data_write(&mut self, port_num: usize, data: u8) {
        
        let mut channel = &mut self.channels[port_num];

        // Only two timer modes will reload the count register immediately while counting is in progress
        let reload_immediately = match channel.mode {
            ChannelMode::InterruptOnTerminalCount | ChannelMode::SoftwareTriggeredStrobe => true,
            _ => false,
        };

        // Intel: Mode1: "If a new count value is loaded *while the output is low* it will not affect
        // the duration of the one-shot pulse until the succeeeding trigger."

        // Assumption is that a new count value while input is high *will* affect the output state.
        let output_low_on_reload = match channel.mode {
            ChannelMode::HardwareRetriggerableOneShot if channel.output => true,
            _ => false
        };

        match channel.mode {
            ChannelMode::InterruptOnTerminalCount | ChannelMode::HardwareRetriggerableOneShot => {
                // Reset output on port write
                dirty_update_checked!(channel.output, false, channel.output_dirty);
            }
            _=> {}
        }

        match channel.access_mode {
            AccessMode::LoByteOnly => {

                dirty_update_checked!(channel.reload_value, (data as u16), channel.reload_value_dirty);
                //channel.reload_value = data as u16;

                if channel.waiting_for_reload || reload_immediately {
                    //log::trace!("Channel {} reloaded with value {} in LSB mode.", port_num, port.reload_value);
                    channel.current_count =  channel.reload_value;
                }
                channel.waiting_for_reload = false;
                if output_low_on_reload {

                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    //channel.output = false;
                    channel.gate_triggered = false;
                }
            }
            AccessMode::HiByteOnly => {
                dirty_update_checked!(channel.reload_value, ((data as u16) << 8), channel.reload_value_dirty);
                //channel.reload_value = (data as u16) << 8;
                
                if channel.waiting_for_reload || reload_immediately {
                    //log::trace!("Channel {} reloaded with value {} in HSB mode.", port_num, port.reload_value);
                    channel.current_count =  channel.reload_value;
                }
                channel.waiting_for_reload = false;
                if output_low_on_reload {
                    dirty_update_checked!(channel.output, false, channel.output_dirty);
                    //channel.output = false;
                    channel.gate_triggered = false;
                }                
            }
            AccessMode::LoByteHiByte => {
                // Expect lo byte first, hi byte second
                if channel.waiting_for_hibyte {
                    // Receiving hi byte

                    let new_reload_value = channel.reload_value | (data as u16) << 8;
                    dirty_update_checked!(channel.reload_value, new_reload_value, channel.reload_value_dirty);
                    //channel.reload_value |= (data as u16) << 8;
                    channel.waiting_for_hibyte = false;

                    if channel.waiting_for_reload || reload_immediately {
                        dirty_update_checked!(channel.current_count, channel.reload_value, channel.current_count_dirty);
                        channel.current_count =  channel.reload_value;              
                        //log::trace!("Channel {} reloaded with value {} in WORD mode.", port_num, port.reload_value);
                    }
                    channel.waiting_for_reload = false;
                    if output_low_on_reload {
                        dirty_update_checked!(channel.output, false, channel.output_dirty);
                        //channel.output = false;
                        channel.gate_triggered = false;
                    }                    
                }
                else {
                    // Receiving lo byte
                    
                    dirty_update_checked!(channel.reload_value, (data as u16), channel.reload_value_dirty);
                    //channel.reload_value = data as u16;
                    channel.waiting_for_hibyte = true;
                }
            }
            AccessMode::LatchCountValue => {
                // Shouldn't reach here
            }
        }
    }

    pub fn data_read(&mut self, port: usize) -> u8 {
        let mut channel = &mut self.channels[port];
        if channel.count_is_latched {
            match channel.access_mode {
                AccessMode::LoByteOnly => {
                    // Reset latch on read
                    channel.count_is_latched = false;

                    let mut byte = (channel.latch_count & 0xFF) as u8;

                    // Hack to avoid halts due to BIOS bit testing of channel timer.
                    // This should be unnecessary once we implement proper instruction timings
                    // Returning a constant cycle count for all instructions can 'lock' the lo bit to 0 or 1
                    let lobit = channel.latch_count & 0x01 != 0;
                    if lobit == channel.latch_lobit {
                        channel.latch_lobit_count += 1;
                    }
                    else {
                        channel.latch_lobit_count = 0;
                    }
                    channel.latch_lobit = lobit;

                    if channel.latch_lobit_count > 10 {
                        byte ^= 0x01;
                    }

                    return byte;
                }
                AccessMode::HiByteOnly => {
                    // Reset latch on read
                    channel.count_is_latched = false;
                    return (channel.latch_count >> 8) as u8;
                }
                AccessMode::LoByteHiByte => {
                    if channel.latched_lobyte_read {
                        // Return hi byte and unlatch output
                        channel.count_is_latched = false;
                        channel.latched_lobyte_read = false;
                        return (channel.latch_count >> 8) as u8;
                    }
                    else {
                        // Return lo byte
                        // Reset latch on full read
                        channel.latched_lobyte_read = true;
                        return (channel.latch_count & 0xFF) as u8;
                    }
                }
                _ => unreachable!()
            }
        }
        else {
            match channel.access_mode {
                AccessMode::LoByteOnly => {
                    return (channel.current_count & 0xFF) as u8;
                }
                AccessMode::HiByteOnly => {
                    return (channel.current_count >> 8) as u8;
                }
                AccessMode::LoByteHiByte => {
                    // Output lo byte of counter, then on next read output hi byte
                    if channel.read_in_progress {
                        // Return hi byte and unlatch output
                        channel.read_in_progress = false;
                        return (channel.current_count >> 8) as u8;
                    }
                    else {
                        // Return lo byte and set read in progress flag
                        channel.read_in_progress = true;
                        return (channel.current_count & 0xFF) as u8;
                    }
                }
                _ => unreachable!()
            }            
        }
    }

    pub fn set_channel_gate(&mut self, channel: usize, state: bool ) {
        if channel > 2 {
            return
        }
        // Note: Only the gate to PIT channel #2 is connected to anything (PPI port)
        self.channels[channel].input_gate = state;
    }

    pub fn run(
        &mut self, 
        bus: &mut BusInterface, 
        pic: &mut pic::Pic, 
        dma: &mut dma::DMAController,
        ppi: &mut ppi::Ppi,
        buffer_producer: &mut ringbuf::Producer<u8>,
        cpu_cycles: u32 ) {

        let mut pit_cycles = Pit::get_pit_cycles(cpu_cycles);
        //let pit_cycles_remainder = pit_cycles.fract();
//
        //// Add up fractional cycles until we can make a whole one. 
        //// Attempts to compensate for clock drift because of unaccounted fractional cycles
        //self.cycle_accumulator += pit_cycles_remainder;
        //
        //// If we have enough cycles, drain them out of accumulator into cycle count
        //while self.cycle_accumulator > 1.0 {
        //    pit_cycles += 1.0;
        //    self.cycle_accumulator -= 1.0;
        //}
//
        //let pit_cycles_int = pit_cycles as u32;
//
        ////log::trace!("pit cycles: {}", pit_cycles_int );
        //for _ in 0..pit_cycles_int {
        //    // Each tick, the state of PIT Channel #2 is pushed into the ringbuf
        //    self.tick(bus, pic, dma, ppi, buffer_producer);
        //}

        self.cycle_accumulator += pit_cycles;
        while self.cycle_accumulator > 1.0 {
            pit_cycles += 1.0;
            self.cycle_accumulator -= 1.0;
            self.tick(bus, pic, dma, ppi, buffer_producer);
        }
        
    }

    pub fn get_cycles(&self) -> u64 {
        self.pit_cycles
    }

    pub fn get_output_state(&self, channel: usize) -> bool {
        self.channels[channel].output
    }

    pub fn tick(
        &mut self,
        bus: &mut BusInterface,
        pic: &mut pic::Pic,
        dma: &mut dma::DMAController,
        ppi: &mut ppi::Ppi,
        buffer_producer: &mut ringbuf::Producer<u8>) 
    {
        self.pit_cycles += 1;

        // The rising edge of a channel's input gate will reload a channel's counter value
        // in modes 1, 2, 3 and 5.

        // However, only channel 2 has a working input gate. The input gate is controlled by
        // writing line pb0 of the PPI chip.
        
        // Therefore we only need to handle channel 2 here. All other channel input gates will
        // remain always high.

        let channel2_gate = ppi.get_pb0_state();
        if channel2_gate && !self.channels[2].input_gate {
            // Input gate rising
            // Reload counter in modes 1, 2 & 3
            match self.channels[2].mode {
                ChannelMode::RateGenerator 
                | ChannelMode::HardwareRetriggerableOneShot
                | ChannelMode::SquareWaveGenerator 
                | ChannelMode::HardwareTriggeredStrobe 
                => 
                {
                    if self.channels[2].reload_value == 0 {
                        // 0 functions as a reload value of 65536

                        dirty_update_checked!(
                            self.channels[2].current_count, 
                            u16::MAX,
                            self.channels[2].current_count_dirty
                        ); 
                        //self.channels[2].current_count = u16::MAX;
                    }
                    else {
                        dirty_update_checked!(
                            self.channels[2].current_count, 
                            self.channels[2].reload_value,
                            self.channels[2].current_count_dirty
                        ); 
                        //self.channels[2].current_count = self.channels[2].reload_value;                            
                    }
                }
                _ => {}
            }

            if let ChannelMode::HardwareRetriggerableOneShot = self.channels[2].mode {
                // Rising edge of gate input sets output low.

                dirty_update_checked!(self.channels[2].output, false, self.channels[2].output_dirty);
                //self.channels[2].output = false;
                self.channels[2].one_shot_triggered = false;
            }
            self.channels[2].gate_triggered = true;
        }

        dirty_update_checked!(self.channels[2].input_gate, channel2_gate, self.channels[2].input_gate_dirty);
        //self.channels[2].input_gate = channel2_gate;

        // Tick each timer channel

        for (i,t) in &mut self.channels.iter_mut().enumerate() {
            match t.mode {
                ChannelMode::InterruptOnTerminalCount => {
                    // Don't count while waiting for reload value or if input gate is low
                    if !t.waiting_for_reload && t.input_gate {

                        // Counter value of 0 equates to to reload value of 2^16

                        dirty_update!(t.current_count, t.current_count.wrapping_sub(1), t.current_count_dirty);
                        //t.current_count = t.current_count.wrapping_sub(1);
                        
                        // Terminal Count reached.
                        if t.current_count == 0 {
                            
                            // Only trigger an interrupt on Channel #0, and only if output is going from low to high
                            if !t.output && i == 0 {
                                pic.request_interrupt(0);
                            }

                            dirty_update_checked!(t.output, true, t.output_dirty);
                            //t.output = true;
                            // Counter just wraps around in this mode, it is NOT reloaded.
                            dirty_update!(t.current_count, u16::MAX, t.current_count_dirty);
                            //t.current_count = u16::MAX;
                        }
                    }
                },
                ChannelMode::HardwareRetriggerableOneShot => {
                    // Counting waits for reload value and rising edge of gate input.
                    // Therefore this mode is only usable on channel #2.
                    if !t.waiting_for_reload && t.gate_triggered {
                        // Counter value of 0 equates to to reload value of 2^16

                        dirty_update!(t.current_count, t.current_count.wrapping_sub(1), t.current_count_dirty);
                        //t.current_count = t.current_count.wrapping_sub(1);
                        
                        // OSDev: When the current count decrements from one to zero, the output goes
                        // high and remains high until another mode/command register is written 
                        if t.current_count == 0 {

                            dirty_update!(t.current_count, u16::MAX, t.current_count_dirty);
                            //t.current_count = u16::MAX;

                            if t.one_shot_triggered == false {
                                t.one_shot_triggered = true;

                                dirty_update_checked!(t.output, true, t.output_dirty);
                                //t.output = true;
                            }
                        }
                        else if !t.one_shot_triggered {

                            dirty_update_checked!(t.output, false, t.output_dirty);
                            //t.output = false;
                        }

                    }
                },
                ChannelMode::RateGenerator => {
                    // Don't count while waiting for reload value or if input gate is low
                    if !t.waiting_for_reload && t.input_gate {

                        dirty_update!(t.current_count, t.current_count.wrapping_sub(1), t.current_count_dirty);
                            
                        if t.current_count == 1 {
                            // OSDev: When the current count decrements from two to one, the output goes low
                            //        the next falling edge of the clock it will go high again
                            
                            dirty_update_checked!(t.output, false, t.output_dirty);
                            //t.output = false;
                        }
                        if t.current_count == 0 {
                            // Decremented from 1 to 0, output high, reload counter and continue
                            
                            dirty_update_checked!(t.output, true, t.output_dirty);
                            //t.output = true;
                            
                            dirty_update!(t.current_count, t.reload_value, t.output_dirty);
                            //t.current_count = t.reload_value;

                            if i == 0 {                          
                                // Channel #0 is connected to PIC and generates interrupt
                                pic.request_interrupt(0);
                            }
                            if i == 1 {
                                // Channel #1 is connected to DMA for DMA refresh.
                                dma.do_dma_read_u8(bus, 0);
                            }
                        }
                    }
                    // Low gate input forces output high
                    if !t.input_gate {
                        dirty_update_checked!(t.output, true, t.output_dirty);
                        //t.output = true;
                    }
                },
                ChannelMode::SquareWaveGenerator => {
                    // Don't count while waiting for reload value, or if input gate is low
                    if !t.waiting_for_reload && t.input_gate {
                        
                        // Counter value of 0 equates to to reload value of 2^16
                        if t.current_count == 0 {
                            if t.reload_value != 0 {
                                panic!("unexpected timer state");
                            }
                            dirty_update!(t.current_count, u16::MAX, t.current_count_dirty );
                            //t.current_count = u16::MAX;
                        }
                        if t.current_count & 0x01 != 0 {
                            // Count is odd - can only occur immediately after reload of odd reload value

                            if t.output { 
                                // Intel: "If the count is odd and the output is high, the first clock pulse
                                // (after the count is loaded) decrements the count by 1. Subsequent pulses
                                // decrement the clock by 2..."                    
                                dirty_update!(t.current_count, t.current_count.wrapping_sub(1), t.current_count_dirty);
                                //t.current_count = t.current_count.wrapping_sub(1);

                                // count is even from now on
                            }
                            else {
                                // Intel: "... After timeout, the output goes low and the full count is reloaded.
                                // The first clock pulse decrements the counter by 3."

                                dirty_update!(t.current_count, t.current_count.wrapping_sub(3), t.current_count_dirty);
                                //t.current_count = t.current_count.wrapping_sub(3);
                                
                                // count is even from now on

                                // TODO: What happens on a reload value of 1? OSdev says you should avoid it - would 
                                // it wrap the counter?
                            }
                        }
                        else {

                            dirty_update!(t.current_count, t.current_count.saturating_sub(2), t.current_count_dirty);
                            //t.current_count = t.current_count.saturating_sub(2);
                        }

                        // Terminal count reached
                        if t.current_count == 0 {

                            // Change flipflop state

                            dirty_update!(t.output, !t.output, t.output_dirty);
                            //t.output = !t.output;

                            // Only Channel #0 generates interrupts
                            if i == 0 {
                                if t.output {
                                    pic.request_interrupt(0);
                                }
                                else {
                                    pic.clear_interrupt(0);
                                }
                            }  

                            // Reload counter
                            if t.reload_value == 0 {
                                dirty_update!(t.current_count, u16::MAX, t.current_count_dirty);
                                //t.current_count = u16::MAX; 
                            }
                            else {
                                dirty_update!(t.current_count, t.reload_value, t.current_count_dirty);
                                //t.current_count = t.reload_value;                            
                            }
                        }
                    }

                    //if !t.input_gate {
                    //    t.output_is_high = true;
                    //}
                },
                ChannelMode::SoftwareTriggeredStrobe => {},
                ChannelMode::HardwareTriggeredStrobe => {
                    if !t.waiting_for_reload {

                        // Counter value of 0 equates to to reload value of 2^16
                        dirty_update!(t.current_count, t.current_count.wrapping_sub(1), t.current_count_dirty);
                            
                        // OSDev: When the current count decrements from one to zero, the output goes
                        // low for one cycle of the input signal
                        if t.current_count == 0 {

                            if t.one_shot_triggered == false {
                                t.one_shot_triggered = true;

                                dirty_update!(t.output, false, t.output_dirty);
                                //t.output = false;
                            }
                        }
                        else {
                            dirty_update!(t.output, true, t.output_dirty);
                            //t.output = true;
                        }
                    }
                },
            }

            let ppi_pb1 = ppi.get_pb1_state();

            // Push state of PIT channel #2 output to ring buffer. 
            // Note: Bit #1 of PPI Port B is AND'd with output.
            if i == 2 {

                let speaker_signal = t.output && ppi_pb1;
                match buffer_producer.push(speaker_signal as u8) {
                    Ok(()) => (),
                    Err(_) => ()
                }
            }
        }
    }

    pub fn get_string_state(&mut self, clean: bool) -> PitStringState {
        let state = PitStringState {
            c0_value:           SyntaxToken::StateString(format!("{:06}", self.channels[0].current_count), self.channels[0].current_count_dirty, 0),
            c0_reload_value:    SyntaxToken::StateString(format!("{:06}", self.channels[0].reload_value), self.channels[0].reload_value_dirty, 0),
            c0_access_mode:     SyntaxToken::StateString(format!("{:?}", self.channels[0].access_mode), self.channels[0].access_mode_dirty, 0),
            c0_channel_mode:    SyntaxToken::StateString(format!("{:?}", self.channels[0].mode), self.channels[0].mode_dirty, 0),
            c0_channel_output:  SyntaxToken::StateString(format!("{:?}", self.channels[0].output), self.channels[0].access_mode_dirty, 0),
            c1_value:           SyntaxToken::StateString(format!("{:06}", self.channels[1].current_count), self.channels[1].current_count_dirty, 0),
            c1_reload_value:    SyntaxToken::StateString(format!("{:06}", self.channels[1].reload_value), self.channels[1].reload_value_dirty, 0),
            c1_access_mode:     SyntaxToken::StateString(format!("{:?}", self.channels[1].access_mode), self.channels[1].access_mode_dirty, 0),
            c1_channel_mode:    SyntaxToken::StateString(format!("{:?}", self.channels[1].mode), self.channels[1].mode_dirty, 0),
            c1_channel_output:  SyntaxToken::StateString(format!("{:?}", self.channels[1].output), self.channels[1].access_mode_dirty, 0),
            c2_value:           SyntaxToken::StateString(format!("{:06}", self.channels[2].current_count), self.channels[2].current_count_dirty, 0),
            c2_reload_value:    SyntaxToken::StateString(format!("{:06}", self.channels[2].reload_value), self.channels[2].reload_value_dirty, 0),
            c2_access_mode:     SyntaxToken::StateString(format!("{:?}", self.channels[2].access_mode), self.channels[2].access_mode_dirty, 0),
            c2_channel_mode:    SyntaxToken::StateString(format!("{:?}", self.channels[2].mode), self.channels[1].mode_dirty, 0),
            c2_channel_output:  SyntaxToken::StateString(format!("{:?}", self.channels[2].output), self.channels[2].access_mode_dirty, 0),
            c2_gate_status:     SyntaxToken::StateString(format!("{:?}", self.channels[2].input_gate), self.channels[2].input_gate_dirty, 0),
        };

        if clean {
            for i in 0..3 {
                self.channels[i].current_count_dirty = false;
                self.channels[i].reload_value_dirty = false;
                self.channels[i].access_mode_dirty = false;
                self.channels[i].mode_dirty = false;
                self.channels[i].input_gate_dirty = false;
            }

        }

        state
    }

    pub fn get_display_state(&mut self, clean: bool) -> PitDisplayState {

        let mut state_vec = Vec::new();

        for i in 0..3 {

            let mut channel_map = BTreeMap::<&str, SyntaxToken>::new();

            channel_map.insert(
                "Access Mode:", 
                SyntaxToken::StateString(
                    format!("{:?}", self.channels[i].access_mode), self.channels[i].access_mode_dirty, 0
                )
            );
            channel_map.insert(
                "Channel Mode:", 
                SyntaxToken::StateString(
                    format!("{:?}", self.channels[i].mode), self.channels[i].mode_dirty, 0
                )
            );
            channel_map.insert(
                "Counter:", 
                SyntaxToken::StateString(
                    format!("{:?}", self.channels[i].current_count), self.channels[i].current_count_dirty, 0
                )
            );
            channel_map.insert(
                "Reload Value:", 
                SyntaxToken::StateString(
                    format!("{:?}", self.channels[i].reload_value), self.channels[i].reload_value_dirty, 0
                )
            );
            channel_map.insert(
                "Output Signal:", 
                SyntaxToken::StateString(
                    format!("{:?}", self.channels[i].output), self.channels[i].output_dirty, 0
                )
            );
            channel_map.insert(
                "Gate Status:", 
                SyntaxToken::StateString(
                    format!("{:?}", self.channels[i].input_gate), self.channels[i].input_gate_dirty, 0
                )
            );

            state_vec.push(channel_map);
        }

        if clean {
            for i in 0..3 {
                self.channels[i].current_count_dirty = false;
                self.channels[i].reload_value_dirty = false;
                self.channels[i].access_mode_dirty = false;
                self.channels[i].mode_dirty = false;
                self.channels[i].input_gate_dirty = false;
                self.channels[i].output_dirty = false;
            }

        }

        state_vec
    }
}
