/* 
    pit.rs 
    Implement the Intel 8253 Programmable Interval Timer
*/

use log;

use crate::io::{IoBusInterface, IoDevice};
use crate::bus::{BusInterface};
use crate::cpu::CPU_MHZ;
use crate::pic;
use crate::dma;
use crate::ppi;

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

#[derive(Debug)]
enum ChannelMode {
    InterruptOnTerminalCount,
    HardwareRetriggerableOneShot,
    RateGenerator,
    SquareWaveGenerator,
    SoftwareTriggeredStrobe,
    HardwareTriggeredStrobe
}

#[derive(Debug)]
enum AccessMode {
    LatchCountValue,
    LoByteOnly,
    HiByteOnly,
    LoByteHiByte
}

pub struct PitChannel {
    mode: ChannelMode,
    access_mode: AccessMode,
    reload_value: u16,
    waiting_for_reload: bool,
    waiting_for_lobyte: bool,
    waiting_for_hibyte: bool,
    current_count: u16,
    read_in_progress: bool,
    normal_lobyte_read: bool,    
    count_is_latched: bool,
    output_is_high: bool,
    latched_lobyte_read: bool,
    latch_count: u16,
    latch_lobit: bool,
    latch_lobit_count: u32,
    bcd_mode: bool,
    input_gate: bool
}

pub struct ProgrammableIntervalTimer {
    pit_cycles: u64,
    cycle_accumulator: f64,
    channels: Vec<PitChannel>,
}
pub type Pit = ProgrammableIntervalTimer;

#[derive(Default, Debug, Clone)]
pub struct PitStringState {
    pub c0_value: String,
    pub c0_reload_value: String,
    pub c0_access_mode: String,
    pub c0_channel_mode: String,
    pub c0_channel_output: String,
    pub c1_value: String,
    pub c1_reload_value: String,
    pub c1_access_mode: String,
    pub c1_channel_mode: String,
    pub c1_channel_output: String,
    pub c2_value: String,
    pub c2_reload_value: String,
    pub c2_access_mode: String,
    pub c2_channel_mode: String,
    pub c2_channel_output: String,
    pub c2_gate_status: String,
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
                access_mode: AccessMode::HiByteOnly,
                reload_value: 0,
                waiting_for_reload: true,
                waiting_for_lobyte: false,
                waiting_for_hibyte: false,
                current_count: 0,
                read_in_progress: false,
                normal_lobyte_read: false,
                count_is_latched: false,
                output_is_high: false,
                latched_lobyte_read: false,
                latch_count: 0,
                latch_lobit: false,
                latch_lobit_count: 0,
                bcd_mode: false,
                input_gate: true,
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
            channel.access_mode = AccessMode::HiByteOnly;
            channel.reload_value = 0;
            channel.waiting_for_reload = true;
            channel.waiting_for_lobyte = false;
            channel.waiting_for_hibyte = false;
            channel.current_count = 0;
            channel.read_in_progress = false;
            channel.normal_lobyte_read = false;
            channel.count_is_latched = false;
            channel.output_is_high = false;
            channel.latched_lobyte_read = false;
            channel.latch_count = 0;
            channel.latch_lobit = false;
            channel.latch_lobit_count = 0;
            channel.bcd_mode = false;
            channel.input_gate = true;
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
            channel.mode = channel_mode;

            match channel.mode {
                ChannelMode::InterruptOnTerminalCount => {
                    // Intel: The output will be intiially low after the mode set operation.
                    channel.output_is_high = false;
                    channel.waiting_for_reload = true;
                }
                ChannelMode::HardwareRetriggerableOneShot => {
                    // osdev: When the mode/command register is written the output signal goes high
                    channel.output_is_high = true;
                    channel.waiting_for_reload = true;
                }
                ChannelMode::RateGenerator => {
                    channel.output_is_high = true;
                    channel.waiting_for_reload = true;
                },
                ChannelMode::SquareWaveGenerator => {
                    channel.output_is_high = true;
                    channel.waiting_for_reload = true;
                },
                ChannelMode::SoftwareTriggeredStrobe => {
                    channel.output_is_high = true;
                    channel.waiting_for_reload = true;
                },
                ChannelMode::HardwareTriggeredStrobe => {
                    channel.output_is_high = true;
                    channel.waiting_for_reload = true;
                }
            }
            
            channel.reload_value = 0;
            channel.access_mode = access_mode;
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

        match channel.access_mode {
            AccessMode::LoByteOnly => {
                channel.reload_value = data as u16;

                if channel.waiting_for_reload || reload_immediately {
                    //log::trace!("Channel {} reloaded with value {} in LSB mode.", port_num, port.reload_value);
                    channel.current_count =  channel.reload_value;
                }
                channel.waiting_for_reload = false;
            }
            AccessMode::HiByteOnly => {
                channel.reload_value = (data as u16) << 8;
                
                if channel.waiting_for_reload || reload_immediately {
                    //log::trace!("Channel {} reloaded with value {} in HSB mode.", port_num, port.reload_value);
                    channel.current_count =  channel.reload_value;
                }
                channel.waiting_for_reload = false;
            }
            AccessMode::LoByteHiByte => {
                // Expect lo byte first, hi byte second
                if channel.waiting_for_hibyte {
                    // Receiving hi byte
                    channel.reload_value |= (data as u16) << 8;
                    channel.waiting_for_hibyte = false;

                    if channel.waiting_for_reload || reload_immediately {      
                        channel.current_count =  channel.reload_value;              
                        //log::trace!("Channel {} reloaded with value {} in WORD mode.", port_num, port.reload_value);
                    }
                    channel.waiting_for_reload = false;
                }
                else {
                    // Receiving lo byte
                    channel.reload_value = data as u16;
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
        io_bus: &mut IoBusInterface, 
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
        self.channels[channel].output_is_high
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

        // The rising edge of a channel's input gate will reload the channel's counter value.
        // However, only channel 2 has a working input gate. The input gate is controlled by
        // writing line pb0 of the PPI chip.
        // Therefore we only need to handle channel 2 here. All other channel input gates will
        // remain always high.
        let channel2_gate = ppi.get_pb0_state();
        if channel2_gate && !self.channels[2].input_gate {
            // Input gate rising
            // Reload counter in modes 2 & 3
            match self.channels[2].mode {
                ChannelMode::RateGenerator | ChannelMode::SquareWaveGenerator => {
                    if self.channels[2].reload_value == 0 {
                        // 0 functions as a reload value of 65536
                        self.channels[2].current_count = u16::MAX;
                    }
                    else {
                        self.channels[2].current_count = self.channels[2].reload_value;                            
                    }
                }
                _ => {}
            }
        }
        self.channels[2].input_gate = channel2_gate;

        // Tick each timer channel

        for (i,t) in &mut self.channels.iter_mut().enumerate() {
            match t.mode {
                ChannelMode::InterruptOnTerminalCount => {
                    // Don't count while waiting for reload value or if input gate is low
                    if !t.waiting_for_reload && t.input_gate {

                        // Counter value of 0 equates to to reload value of 2^16
                        if t.current_count == 0 {
                            if t.reload_value != 0 {
                                panic!("unexpected timer state");
                            }
                            t.current_count = u16::MAX;
                        }
                        else {
                            t.current_count -= 1;
                        }
                        
                        // Terminal Count reached.
                        if t.current_count == 0 {
                            
                            // Only trigger an interrupt on Channel #0, and only if output is going from low to high
                            if !t.output_is_high && i == 0 {
                                pic.request_interrupt(0);
                            }

                            t.output_is_high = true;
                            // Counter just wraps around in this mode, it is NOT reloaded.
                            t.current_count = u16::MAX;
                        }
                    }
                },
                ChannelMode::HardwareRetriggerableOneShot => {},
                ChannelMode::RateGenerator => {
                    // Don't count while waiting for reload value or if input gate is low
                    if !t.waiting_for_reload && t.input_gate {

                        if t.current_count == 0 {
                            // 0 essentially functions as a counter value of 65536
                            t.current_count = u16::MAX;
                        }
                        else {
                            t.current_count -= 1;
                            
                            if t.current_count == 1 {
                                // OSDev: When the current count decrements from two to one, the output goes low
                                //        the next falling edge of the clock it will go high again
                                t.output_is_high = false;
                            }
                            if t.current_count == 0 {
                                // Decremented from 1 to 0, output high, reload counter and continue
                                t.output_is_high = true;
                                t.current_count = t.reload_value;

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
                    }
                    // Low gate input forces output high
                    if !t.input_gate {
                        t.output_is_high = true;
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
                            t.current_count = u16::MAX;
                        }

                        if t.current_count & 0x01 != 0 {
                            // Count is odd - can only occur immediately after reload of odd reload value

                            if t.output_is_high { 
                                // Intel: "If the count is odd and the output is high, the first clock pulse
                                // (after the count is loaded) decrements the count by 1. Subsequent pulses
                                // decrement the clock by 2..."                                     
                                t.current_count = t.current_count.wrapping_sub(1);
                                // count is even from now on
                            }
                            else {
                                // Intel: "... After timeout, the output goes low and the full count is reloaded.
                                // The first clock pulse decrements the counter by 3."
                                t.current_count = t.current_count.wrapping_sub(3);
                                // count is even from now on

                                // TODO: What happens on a reload value of 1? OSdev says you should avoid it - would 
                                // it wrap the counter?
                            }
                        }
                        else {
                            t.current_count = t.current_count.saturating_sub(2);
                        }

                        // Terminal count reached
                        if t.current_count == 0 {

                            // Change flipflop state
                            t.output_is_high = !t.output_is_high;

                            // Only Channel #0 generates interrupts
                            if i == 0 {
                                if t.output_is_high {
                                    pic.request_interrupt(0);
                                }
                                else {
                                    pic.clear_interrupt(0);
                                }
                            }  

                            // Reload counter
                            if t.reload_value == 0 {
                                t.current_count = u16::MAX; 
                            }
                            else {
                                t.current_count = t.reload_value;                            
                            }
                        }
                    }

                    //if !t.input_gate {
                    //    t.output_is_high = true;
                    //}
                },
                ChannelMode::SoftwareTriggeredStrobe => {},
                ChannelMode::HardwareTriggeredStrobe => {},
            }

            let ppi_pb1 = ppi.get_pb1_state();

            // Push state of PIT channel #2 output to ring buffer. 
            // Note: Bit #1 of PPI Port B is AND'd with output.
            if i == 2 {

                let speaker_signal = t.output_is_high && ppi_pb1;
                match buffer_producer.push(speaker_signal as u8) {
                    Ok(()) => (),
                    Err(_) => ()
                }
            }
        }
    }

    pub fn get_string_repr(&self) -> PitStringState {
        PitStringState {
            c0_value: format!("{:06}", self.channels[0].current_count),
            c0_reload_value: format!("{:06}", self.channels[0].reload_value),
            c0_access_mode: format!("{:?}", self.channels[0].access_mode),
            c0_channel_mode: format!("{:?}", self.channels[0].mode),
            c0_channel_output: format!("{:?}", self.channels[0].output_is_high),
            c1_value: format!("{:06}", self.channels[1].current_count),
            c1_reload_value: format!("{:06}", self.channels[1].reload_value),
            c1_access_mode: format!("{:?}", self.channels[1].access_mode),
            c1_channel_mode: format!("{:?}", self.channels[1].mode),
            c1_channel_output: format!("{:?}", self.channels[1].output_is_high),
            c2_value: format!("{:06}", self.channels[2].current_count),
            c2_reload_value: format!("{:06}", self.channels[2].reload_value),
            c2_access_mode: format!("{:?}", self.channels[2].access_mode),
            c2_channel_mode: format!("{:?}", self.channels[2].mode),
            c2_channel_output: format!("{:?}", self.channels[2].output_is_high),
            c2_gate_status: format!("{:?}", self.channels[2].input_gate)
        }
    }
}
