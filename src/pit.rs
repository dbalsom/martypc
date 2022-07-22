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
    channel_mode: ChannelMode,
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
    bcd_mode: bool,
    input_gate: bool,
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
    pub c1_value: String,
    pub c1_reload_value: String,
    pub c1_access_mode: String,
    pub c1_channel_mode: String,
    pub c2_value: String,
    pub c2_reload_value: String,
    pub c2_access_mode: String,
    pub c2_channel_mode: String,
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
                channel_mode: ChannelMode::InterruptOnTerminalCount,
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
            channel.channel_mode = ChannelMode::InterruptOnTerminalCount;
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

    fn parse_command_register(&mut self, command_byte: u8) -> (u32, AccessMode, ChannelMode, bool) {
        
        let channel_select: u32 = (command_byte >> 6) as u32;

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
            self.channels[channel_select as usize].latch_count = self.channels[channel_select as usize].current_count;
            self.channels[channel_select as usize].count_is_latched = true;
            self.channels[channel_select as usize].latched_lobyte_read = false;
        }
        else {
            log::debug!("PIT: Channel {} selected, access mode {:?}, channel_mode {:?}", channel_select, access_mode, channel_mode );

            let channel = &mut self.channels[channel_select as usize];
            channel.channel_mode = channel_mode;

            match channel.channel_mode {
                ChannelMode::InterruptOnTerminalCount => {
                    channel.waiting_for_reload = true;
                    // Intel: The output will be intiially low after the mode set operation.
                    channel.output_is_high = false;
                }
                ChannelMode::HardwareRetriggerableOneShot => {
                    channel.waiting_for_reload = true;
                }
                ChannelMode::RateGenerator => {
                    channel.waiting_for_reload = true;
                },
                ChannelMode::SquareWaveGenerator => {
                    channel.waiting_for_reload = true;
                },
                ChannelMode::SoftwareTriggeredStrobe => {},
                ChannelMode::HardwareTriggeredStrobe => {}
            }
            
            channel.reload_value = 0;
            channel.access_mode = access_mode;
            channel.bcd_mode = bcd_enable;
        }        

    }

    pub fn data_write(&mut self, port_num: usize, data: u8) {
        
        let mut port = &mut self.channels[port_num];

        match port.access_mode {
            AccessMode::LoByteOnly => {
                port.reload_value = data as u16;
                port.waiting_for_reload = false;
                //log::trace!("Channel {} reloaded with value {} in LSB mode.", port_num, port.reload_value);
                port.current_count =  port.reload_value;
            }
            AccessMode::HiByteOnly => {
                port.reload_value = (data as u16) << 8;
                port.waiting_for_reload = false;
                //log::trace!("Channel {} reloaded with value {} in HSB mode.", port_num, port.reload_value);
                port.current_count =  port.reload_value;
            }
            AccessMode::LoByteHiByte => {
                // Expect lo byte first, hi byte second
                if port.waiting_for_lobyte {
                    port.reload_value = data as u16;
                    port.waiting_for_lobyte = false;
                    port.waiting_for_hibyte = true;
                }
                else if port.waiting_for_hibyte {
                    port.reload_value |= (data as u16) << 8;
                    port.waiting_for_hibyte = false;
                    port.waiting_for_reload = false;
                    //log::trace!("Channel {} reloaded with value {} in WORD mode.", port_num, port.reload_value);
                    port.current_count =  port.reload_value;
                }
                else {
                    port.reload_value = data as u16;
                    port.waiting_for_lobyte = false;
                    port.waiting_for_hibyte = true;
                }
            }
            AccessMode::LatchCountValue => {
                // Shouldn't reach here
            }
        }
    }

    pub fn data_read(&mut self, port: usize) -> u8 {
        let mut port = &mut self.channels[port];
        if port.count_is_latched {
            match port.access_mode {
                AccessMode::LoByteOnly => {
                    return (port.latch_count & 0xFF) as u8;
                }
                AccessMode::HiByteOnly => {
                    return (port.latch_count >> 8) as u8;
                }
                AccessMode::LoByteHiByte => {
                    if port.latched_lobyte_read {
                        // Return hi byte and unlatch output
                        port.count_is_latched = false;
                        port.latched_lobyte_read = false;
                        return (port.latch_count >> 8) as u8;
                    }
                    else {
                        // Return lo byte
                        return (port.latch_count & 0xFF) as u8;
                    }
                }
                _ => unreachable!()
            }
        }
        else {
            match port.access_mode {
                AccessMode::LoByteOnly => {
                    return (port.current_count & 0xFF) as u8;
                }
                AccessMode::HiByteOnly => {
                    return (port.current_count >> 8) as u8;
                }
                AccessMode::LoByteHiByte => {
                    // Output lo byte of counter, then on next read output hi byte
                    if port.read_in_progress {
                        // Return hi byte and unlatch output
                        port.read_in_progress = false;
                        return (port.latch_count >> 8) as u8;
                    }
                    else {
                        // Return lo byte and set read in progress flag
                        port.read_in_progress = true;
                        return (port.latch_count & 0xFF) as u8;
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
        let pit_cycles_remainder = pit_cycles.fract();

        // Add up fractional cycles until we can make a whole one. 
        // Attempts to compensate for clock drift because of unaccounted fractional cycles
        self.cycle_accumulator += pit_cycles_remainder;
        
        // If we have enough cycles, drain them out of accumulator into cycle count
        while self.cycle_accumulator > 1.0 {
            pit_cycles += 1.0;
            self.cycle_accumulator -= 1.0;
        }

        let pit_cycles_int = pit_cycles as u32;
        
        for _ in 0..pit_cycles_int {
            // Each tick, the state of PIT Channel #2 is pushed into the ringbuf
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

        for (i,t) in &mut self.channels.iter_mut().enumerate() {
            match t.channel_mode {
                ChannelMode::InterruptOnTerminalCount => {
                    // Don't count while waiting for reload value
                    if !t.waiting_for_reload {

                        // Reload value of 0 equates to to reload value of 2^16
                        if t.current_count == 0 {
                            if t.reload_value != 0 {
                                panic!("unexpected timer state");
                            }
                            t.current_count = u16::MAX - 1;
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
                            t.current_count = u16::MAX - 1;
                        }
                    }
                },
                ChannelMode::HardwareRetriggerableOneShot => {},
                ChannelMode::RateGenerator => {
                    // Don't count while waiting for reload value
                    if !t.waiting_for_reload {


                        if t.current_count == 0 {
                            if t.reload_value == 0 {
                                // 0 functions as a reload value of 65536
                                t.current_count = u16::MAX;
                            }
                            else {
                                t.current_count = t.reload_value;                            
                            }
                        }
                        else {
                            t.current_count -= 1;
                            if t.current_count == 1 {

                                // Only trigger interrupt on Channel #0
                                if i == 0 {                                
                                    pic.request_interrupt(0);
                                }
                                if i == 1 {
                                    // Channel 1 wants to do DMA refresh.
                                    dma.do_dma_read_u8(bus, 0);
                                }

                                // Output would go low here
                            }
                        }
                    }
                },
                ChannelMode::SquareWaveGenerator => {
                    // Don't count while waiting for reload value
                    if !t.waiting_for_reload {
                        
                        // Reload value of 0 equates to to reload value of 2^16
                        if t.current_count == 0 {
                            if t.reload_value != 0 {
                                panic!("unexpected timer state");
                            }
                            t.current_count = u16::MAX - 1;
                        }

                        // Intel: "If the count is odd and the output is high, the first clock pulse
                        // (after the count is loaded) decrements the count by 1. Subsequent pulses
                        // decrement the clock by 2..."
                        if t.current_count & 0x01 != 0 {
                            // Count is odd - can only occur immediately after reload of odd reload value

                            if t.output_is_high { 
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
                            t.current_count = t.current_count.wrapping_sub(2);
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
                                t.current_count = u16::MAX - 1; 
                            }
                            else {
                                t.current_count = t.reload_value;                            
                            }
                        }
                    }
                },
                ChannelMode::SoftwareTriggeredStrobe => {},
                ChannelMode::HardwareTriggeredStrobe => {},
            }

            let ppi_pb1 = ppi.get_pb1_state();

            // Push state of PIT channel #2 output to ring buffer. 
            // Note: Bit #1 of PPI Port B is AND'd with output.
            if i == 2 {

                let mut speaker_signal = t.output_is_high && ppi_pb1;
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
            c0_channel_mode: format!("{:?}", self.channels[0].channel_mode),
            c1_value: format!("{:06}", self.channels[1].current_count),
            c1_reload_value: format!("{:06}", self.channels[1].reload_value),
            c1_access_mode: format!("{:?}", self.channels[1].access_mode),
            c1_channel_mode: format!("{:?}", self.channels[1].channel_mode),
            c2_value: format!("{:06}", self.channels[2].current_count),
            c2_reload_value: format!("{:06}", self.channels[2].reload_value),
            c2_access_mode: format!("{:?}", self.channels[2].access_mode),
            c2_channel_mode: format!("{:?}", self.channels[2].channel_mode),
            c2_gate_status: format!("{:?}", self.channels[2].input_gate)
        }
    }
}
