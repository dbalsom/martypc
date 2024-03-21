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

    --------------------------------------------------------------------------

    devices::pit.rs

    Implements functionality for the Intel 8253 Programmable Interval Timer.

*/

use log;

use std::collections::{BTreeMap, VecDeque};

use modular_bitfield::prelude::*;

use crate::bus::{BusInterface, DeviceRunTimeUnit, IoDevice};

use crate::{syntax_token::*, updatable::*};

pub type PitDisplayState = Vec<BTreeMap<&'static str, SyntaxToken>>;

pub const PIT_CHANNEL_0_DATA_PORT: u16 = 0x40;
pub const PIT_CHANNEL_1_DATA_PORT: u16 = 0x41;
pub const PIT_CHANNEL_2_DATA_PORT: u16 = 0x42;
pub const PIT_COMMAND_REGISTER: u16 = 0x43;

/*
const PIT_CHANNEL_SELECT_MASK: u8 = 0b1100_0000;
const PIT_ACCESS_MODE_MASK: u8    = 0b0011_0000;
const PIT_OPERATING_MODE_MASK: u8 = 0b0000_1110;
const PIT_BCD_MODE_MASK: u8       = 0b0000_0001;
*/

//pub const PIT_FREQ: f64 = 1_193_182.0;
pub const PIT_MHZ: f64 = 1.193182;
pub const PIT_TICK_US: f64 = 1.0 / PIT_MHZ;
//pub const PIT_DIVISOR: f64 = 0.25;

#[derive(Debug, PartialEq)]
pub enum ChannelMode {
    InterruptOnTerminalCount,
    HardwareRetriggerableOneShot,
    RateGenerator,
    SquareWaveGenerator,
    SoftwareTriggeredStrobe,
    HardwareTriggeredStrobe,
}

// We implement From<u8> for this enum ourselves rather than deriving BitfieldSpecfier
// as there is more than one bit mapping per Enum variant (6 and 7 map to modes 2 & 3 again)
impl From<u8> for ChannelMode {
    fn from(orig: u8) -> Self {
        match orig {
            0x0 => return ChannelMode::InterruptOnTerminalCount,
            0x1 => return ChannelMode::HardwareRetriggerableOneShot,
            0x2 => return ChannelMode::RateGenerator,
            0x3 => return ChannelMode::SquareWaveGenerator,
            0x4 => return ChannelMode::SoftwareTriggeredStrobe,
            0x5 => return ChannelMode::HardwareTriggeredStrobe,
            0x6 => return ChannelMode::RateGenerator,
            0x7 => return ChannelMode::SquareWaveGenerator,
            _ => panic!("From<u8> for ChannelMode: Invalid u8 value"),
        };
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ReloadFlag {
    Normal,
    ReloadNextCycle,
}

#[derive(Debug, Copy, Clone, PartialEq, BitfieldSpecifier)]
pub enum PitType {
    Model8253,
    Model8254,
}

#[derive(Debug, PartialEq, BitfieldSpecifier)]
enum RwModeField {
    LatchCommand,
    Lsb,
    Msb,
    LsbMsb,
}

#[derive(Debug, PartialEq)]
pub enum RwMode {
    Lsb,
    Msb,
    LsbMsb,
}

#[bitfield]
#[allow(dead_code)]
pub struct ControlByte {
    bcd: bool,
    channel_mode: B3,
    rw_mode: RwModeField,
    channel: B2,
}

#[derive(Debug, PartialEq)]
pub enum ChannelState {
    WaitingForReload,
    WaitingForGate,
    DeferLoadCycle,
    WaitingForLoadCycle,
    WaitingForLoadTrigger,
    Counting(ReloadFlag),
}

#[derive(Debug, PartialEq)]
enum LoadState {
    WaitingForLsb,
    WaitingForMsb,
    //Loaded
}

#[derive(Debug, PartialEq)]
enum LoadType {
    InitialLoad,
    SubsequentLoad,
}

#[derive(Debug, PartialEq)]
pub enum ReadState {
    NoRead,
    ReadLsb,
}

pub struct Channel {
    c: usize,
    ptype: PitType,
    mode: Updatable<ChannelMode>,
    rw_mode: Updatable<RwMode>,
    channel_state: ChannelState,
    cycles_in_state: u32,
    count_register: Updatable<u16>,
    load_state: LoadState,
    load_type: LoadType,
    load_mask: u16,
    reload_value: Updatable<u16>,
    counting_element: Updatable<u16>,
    ce_undefined: bool,
    armed: bool,
    read_state: ReadState,
    count_is_latched: bool,
    output: Updatable<bool>,
    output_on_reload: bool,
    reload_on_trigger: bool,
    output_latch: Updatable<u16>,
    bcd_mode: bool,
    gate: Updatable<bool>,
    incomplete_reload: bool,
    dirty: bool,  // Have channel parameters changed since last checked?
    ticked: bool, // Has the counting element been ticked at least once?
    defer_reload_flag: bool,
}

#[allow(dead_code)]
pub struct ProgrammableIntervalTimer {
    ptype: PitType,
    _crystal: f64,
    clock_divisor: u32,
    pit_cycles: u64,
    sys_tick_accumulator: u32,
    sys_ticks_advance: u32,
    cycle_accumulator: f64,
    channels: Vec<Channel>,
    timewarp: DeviceRunTimeUnit,
    speaker_buf: VecDeque<u8>,
    defer_reload_flag: bool,
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
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        // Catch up to CPU state.
        //self.catch_up(delta);

        match port {
            PIT_COMMAND_REGISTER => 0,
            PIT_CHANNEL_0_DATA_PORT => self.data_read(0),
            PIT_CHANNEL_1_DATA_PORT => self.data_read(1),
            PIT_CHANNEL_2_DATA_PORT => self.data_read(2),
            _ => panic!("PIT: Bad port #"),
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, bus_opt: Option<&mut BusInterface>, delta: DeviceRunTimeUnit) {
        let bus = bus_opt.unwrap();

        // Catch up to CPU state.
        self.catch_up(bus, delta);

        // PIT will always receive a reference to bus, so it is safe to unwrap.
        match port {
            PIT_COMMAND_REGISTER => self.control_register_write(data, bus),
            PIT_CHANNEL_0_DATA_PORT => self.data_write(0, data, bus),
            PIT_CHANNEL_1_DATA_PORT => self.data_write(1, data, bus),
            PIT_CHANNEL_2_DATA_PORT => self.data_write(2, data, bus),
            _ => panic!("PIT: Bad port #"),
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![
            PIT_CHANNEL_0_DATA_PORT,
            PIT_CHANNEL_1_DATA_PORT,
            PIT_CHANNEL_2_DATA_PORT,
            PIT_COMMAND_REGISTER,
        ]
    }
}

impl Channel {
    pub fn new(c: usize, ptype: PitType) -> Self {
        Channel {
            c,
            ptype,
            mode: Updatable::Dirty(ChannelMode::InterruptOnTerminalCount, false),
            rw_mode: Updatable::Dirty(RwMode::Lsb, false),
            channel_state: ChannelState::WaitingForReload,
            cycles_in_state: 0,
            count_register: Updatable::Dirty(0, false),
            load_state: LoadState::WaitingForLsb,
            load_type: LoadType::InitialLoad,
            load_mask: 0xFFFF,
            reload_value: Updatable::Dirty(0, false),
            counting_element: Updatable::Dirty(0, false),
            ce_undefined: false,
            armed: false,

            read_state: ReadState::NoRead,
            count_is_latched: false,
            output: Updatable::Dirty(false, false),
            output_on_reload: false,
            reload_on_trigger: false,
            output_latch: Updatable::Dirty(0, false),
            bcd_mode: false,
            gate: Updatable::Dirty(false, false),
            incomplete_reload: false,
            dirty: false,
            ticked: false,
            defer_reload_flag: false,
        }
    }

    pub fn set_mode(&mut self, mode: ChannelMode, rw_mode: RwMode, bcd: bool, bus: &mut BusInterface) {
        self.output_latch.update(0);
        self.counting_element.update(0);

        self.count_is_latched = false;
        self.armed = false;
        //self.ce_undefined = false;

        // Default load mask
        //self.load_mask = 0xFFFF;

        log::trace!(
            "PIT: Channel {} selected, channel_mode {:?}, rw mode {:?}, bcd: {:?}",
            self.c,
            mode,
            rw_mode,
            bcd
        );

        match mode {
            ChannelMode::InterruptOnTerminalCount => {
                self.change_output_state(false, bus);
                self.output_on_reload = false;
                self.reload_on_trigger = false;
            }
            ChannelMode::HardwareRetriggerableOneShot => {
                self.change_output_state(true, bus);
                self.output_on_reload = false;
                self.reload_on_trigger = true;
            }
            ChannelMode::RateGenerator => {
                self.change_output_state(true, bus);
                self.output_on_reload = true; // Output in this mode stays high except for one cycle.
                self.reload_on_trigger = false;
            }
            ChannelMode::SquareWaveGenerator => {
                self.change_output_state(true, bus);
                self.output_on_reload = true;
                self.reload_on_trigger = false;
                // Only allow even values into counting element on 8254
                self.load_mask = if self.ptype == PitType::Model8254 {
                    0xFFFE
                }
                else {
                    0xFFFF
                };
            }
            ChannelMode::SoftwareTriggeredStrobe => {
                self.change_output_state(true, bus);
            }
            ChannelMode::HardwareTriggeredStrobe => {
                self.change_output_state(true, bus);
            }
        }

        // Set the new mode.
        self.mode.update(mode);
        self.rw_mode.update(rw_mode);
        self.bcd_mode = bcd;
        self.dirty = true;

        // Setting any mode stops counter.
        self.change_channel_state(ChannelState::WaitingForReload);
        self.read_state = ReadState::NoRead;
        self.load_state = LoadState::WaitingForLsb;
        self.load_type = LoadType::InitialLoad;
    }

    /// Return (and reset) the dirty flag, along with whether we are counting and if the counting
    /// element has ticked.  The latter is to help discriminate whether a 0 count value indicates
    /// initial vs terminal count.
    #[inline]
    pub fn is_dirty(&mut self) -> (bool, bool, bool) {
        let is_dirty = self.dirty;
        self.dirty = false;

        let is_counting = match self.channel_state {
            ChannelState::Counting(_) => true,
            _ => false,
        };

        (is_dirty, is_counting, self.ticked)
    }

    pub fn change_output_state(&mut self, state: bool, bus: &mut BusInterface) {
        if *self.output != state {
            self.output.set(state);
            // Do things specific to channel #
            match (self.c, state) {
                (0, true) => bus.pic_mut().as_mut().unwrap().request_interrupt(0),
                (0, false) => bus.pic_mut().as_mut().unwrap().clear_interrupt(0),
                (1, true) => {
                    let dma = bus.dma_mut().as_mut().unwrap();
                    // Channel 1 is dedicated to sending DREQ0 signals to the DMA controller
                    // to perform DRAM refresh.
                    dma.request_service(0);
                }
                (1, false) => {}
                (2, true) => {}
                (2, false) => {}
                (_, _) => {}
            }
        }
    }

    /// Latch the timer.
    /// Reading from the timer always occurs directly from the output latch.
    /// In normal operation, the output latch updates synchronously with the count element.
    /// When latched, the output latch simply stops updating.
    pub fn latch_count(&mut self) {
        self.output_latch.update(*self.counting_element);
        self.count_is_latched = true;
        self.dirty = true;
    }

    pub fn set_gate(&mut self, new_state: bool, bus: &mut BusInterface) {
        if (*self.gate == false) && (new_state == true) {
            // Rising edge of input gate.
            // This is ignored if we are waiting for a reload value.
            if self.channel_state != ChannelState::WaitingForReload {
                match *self.mode {
                    ChannelMode::InterruptOnTerminalCount => {
                        // Rising gate has no effect.
                    }
                    ChannelMode::HardwareRetriggerableOneShot => {
                        self.change_channel_state(ChannelState::WaitingForLoadCycle);
                    }
                    ChannelMode::RateGenerator => {
                        self.change_channel_state(ChannelState::WaitingForLoadCycle);
                    }
                    ChannelMode::SquareWaveGenerator => {
                        self.change_channel_state(ChannelState::WaitingForLoadCycle);
                    }
                    ChannelMode::SoftwareTriggeredStrobe => {
                        // Rising gate has no effect (?)
                    }
                    ChannelMode::HardwareTriggeredStrobe => {
                        self.change_channel_state(ChannelState::WaitingForLoadCycle);
                    }
                }
            }
        }
        else if (*self.gate == true) && (new_state == false) {
            // Falling edge of input gate.
            // This is ignored if we are waiting for a reload value.
            if self.channel_state != ChannelState::WaitingForReload {
                match *self.mode {
                    ChannelMode::InterruptOnTerminalCount => {
                        // Falling gate has no effect.
                    }
                    ChannelMode::HardwareRetriggerableOneShot => {
                        // Falling gate has no effect.
                    }
                    ChannelMode::RateGenerator => {
                        // Falling gate stops count. Output goes high.
                        self.change_channel_state(ChannelState::WaitingForGate);
                        self.change_output_state(true, bus);
                    }
                    ChannelMode::SquareWaveGenerator => {
                        // Falling gate stops count. Output goes high.
                        self.change_channel_state(ChannelState::WaitingForGate);
                        self.change_output_state(true, bus);
                    }
                    ChannelMode::SoftwareTriggeredStrobe => {
                        // Falling gate stops count. Output unchanged.
                        self.change_channel_state(ChannelState::WaitingForGate);
                    }
                    ChannelMode::HardwareTriggeredStrobe => {
                        // Falling gate has no effect.
                    }
                }
            }
        }

        self.gate.update(new_state);
    }

    /// Read a byte from the PIT channel.
    /// Reading always occurs from the value in the output latch.
    /// When the timer is not latched, the output latch updates synchronously with the
    /// counting element per tick. When latched, the output latch stops updating.
    pub fn read_byte(&mut self) -> u8 {
        match self.read_state {
            ReadState::NoRead => {
                // No read in progress
                match *self.rw_mode {
                    RwMode::Lsb => {
                        self.count_is_latched = false;
                        (*self.output_latch & 0xFF) as u8
                    }
                    RwMode::Msb => {
                        self.count_is_latched = false;
                        ((*self.output_latch >> 8) & 0xFF) as u8
                    }
                    RwMode::LsbMsb => {
                        self.change_read_state(ReadState::ReadLsb);
                        (*self.output_latch & 0xFF) as u8
                    }
                }
            }
            ReadState::ReadLsb => {
                // Word read in progress
                self.count_is_latched = false;
                self.change_read_state(ReadState::NoRead);
                ((*self.output_latch >> 8) & 0xFF) as u8
            }
        }
    }

    pub fn write_byte(&mut self, byte: u8, defer_reload: bool, bus: &mut BusInterface) {
        match *self.rw_mode {
            RwMode::Lsb => {
                self.count_register.update(byte as u16);
                self.finalize_load(defer_reload);
            }
            RwMode::Msb => {
                self.count_register.update((byte as u16) << 8);
                self.finalize_load(defer_reload);
            }
            RwMode::LsbMsb => {
                //log::debug!("rw mode: {:?} byte: {:02X} load_state: {:?}", *self.rw_mode, byte, self.load_state);
                match self.load_state {
                    LoadState::WaitingForLsb => {
                        self.count_register.update(byte as u16);

                        if *self.mode == ChannelMode::InterruptOnTerminalCount {
                            // Beginning a load will stop the timer in InterruptOnTerminalCount mode
                            // and set output immediately to low.
                            self.change_output_state(false, bus);
                            self.change_channel_state(ChannelState::WaitingForReload);
                        }

                        self.load_state = LoadState::WaitingForMsb;
                        //log::debug!("got lsb in lsbmsb mode: {:02X} new load_state: {:?}", byte, self.load_state);
                    }
                    LoadState::WaitingForMsb => {
                        let new_count = (*self.count_register & 0x00FF) | ((byte as u16) << 8);
                        //log::debug!("got msb in lsbmsb mode: {:02X} new count in lsbmsb mode: {}", byte, new_count);
                        self.count_register.update(new_count);
                        self.load_state = LoadState::WaitingForLsb;
                        self.finalize_load(defer_reload);
                    }
                }
            }
        }
    }

    pub fn finalize_load(&mut self, defer_reload: bool) {
        // The count register is transferred to the counting element when a complete count is written.
        self.reload_value.update(*self.count_register);

        let next_reload_state = match defer_reload {
            true => ChannelState::DeferLoadCycle,
            false => ChannelState::WaitingForLoadCycle,
        };

        match self.load_type {
            LoadType::InitialLoad => {
                // This was the first load. Enter either WaitingForLoadTrigger or WaitingForLoadCycle
                // depending on the flag set by the configured mode.
                if self.reload_on_trigger {
                    self.change_channel_state(ChannelState::WaitingForLoadTrigger);
                }
                else {
                    self.change_channel_state(next_reload_state);
                }
                // Arm the timer (applicable only to one-shot modes, but doesn't hurt anything to set)
                self.armed = true;
                // Next load will be a SubsequentLoad
                self.load_type = LoadType::SubsequentLoad;
            }
            LoadType::SubsequentLoad => {
                // This was a subsequent load for an already loaded timer.
                match *self.mode {
                    ChannelMode::InterruptOnTerminalCount => {
                        // In InterruptOnTerminalCount mode, completing a load will reload that value on the next cycle.
                        self.change_channel_state(next_reload_state);
                    }
                    ChannelMode::SoftwareTriggeredStrobe => {
                        // In SoftwareTriggeredStrobe mode, completing a load will reload that value on the next cycle.
                        self.change_channel_state(next_reload_state);
                    }
                    _ => {
                        // Other modes are not reloaded on a subsequent change of the reload value until gate trigger or
                        // terminal count.
                    }
                }
            }
        }

        self.dirty = true;
    }

    pub fn change_read_state(&mut self, new_state: ReadState) {
        if let ReadState::NoRead = new_state {
            self.count_is_latched = false;
        }

        self.read_state = new_state;
    }

    pub fn change_channel_state(&mut self, new_state: ChannelState) {
        self.cycles_in_state = 0;

        match (&self.channel_state, &new_state) {
            (ChannelState::Counting(_), ChannelState::Counting(_)) => {}
            (_, ChannelState::Counting(_)) => {
                self.dirty = true;
            }
            _ => {}
        }
        self.channel_state = new_state;
    }

    pub fn count(&mut self) {
        // Decrement and wrap counter appropriately depending on mode.

        if self.bcd_mode {
            // Wrap BCD counter
            if *self.counting_element == 0 {
                *self.counting_element = 0x9999;
            }
            else {
                // Countdown in BCD...
                if (*self.counting_element & 0x000F) != 0 {
                    // Ones place is not 0
                    self.counting_element.set((*self.counting_element).wrapping_sub(1));
                }
                else if (*self.counting_element & 0x00F0) != 0 {
                    // Tenths place is not 0, borrow from it
                    self.counting_element.set((*self.counting_element).wrapping_sub(0x7));
                // (0x10 (16) - 7 = 0x09))
                }
                else if (*self.counting_element & 0x0F00) != 0 {
                    // Hundredths place is not 0, borrow from it
                    self.counting_element.set((*self.counting_element).wrapping_sub(0x67));
                // (0x100 (256) - 0x67 (103) = 0x99)
                }
                else {
                    // Borrow from thousandths place
                    self.counting_element.set((*self.counting_element).wrapping_sub(0x667));
                    // (0x1000 (4096) - 0x667 () = 0x999)
                }
            }
        }
        else {
            self.counting_element.set((*self.counting_element).wrapping_sub(1));
            // Counter wraps in binary mode.
        }

        // Update output latch with value of counting_element, if we are not latched
        if !self.count_is_latched {
            self.output_latch.set(*self.counting_element);
        }

        self.ticked = true;

        return;
    }

    #[inline]
    pub fn count2(&mut self) {
        self.count();
        self.count();
    }

    #[inline]
    pub fn count3(&mut self) {
        self.count();
        self.count();
        self.count();
    }

    pub fn tick(&mut self, bus: &mut BusInterface, _buffer_producer: Option<&mut ringbuf::Producer<u8>>) {
        if self.channel_state == ChannelState::DeferLoadCycle {
            // We were too late to load the counter this tick. We'll load it next tick.
            self.change_channel_state(ChannelState::WaitingForLoadCycle);
            return;
        }

        if self.channel_state == ChannelState::WaitingForLoadCycle
            || self.channel_state == ChannelState::Counting(ReloadFlag::ReloadNextCycle)
        {
            // Load the current reload value into the counting element, applying the load mask
            //self.counting_element.update(*self.reload_value & self.load_mask);
            self.counting_element.update(*self.reload_value);

            // Start counting.
            self.change_channel_state(ChannelState::Counting(ReloadFlag::Normal));

            // Set output state as appropriate for mode.
            self.change_output_state(self.output_on_reload, bus);

            // Counting Element is now defined
            self.ce_undefined = false;

            // Don't count this tick.
            return;
        }

        if (self.channel_state == ChannelState::WaitingForLoadTrigger)
            && (self.cycles_in_state == 0)
            && (self.armed == true)
        {
            // First cycle of kWaitingForLoadTrigger. An undefined value is loaded into the counting element.
            self.counting_element.update(0x03);
            self.ce_undefined = true;

            self.cycles_in_state += 1;

            // Don't count this tick.
            return;
        }

        if let ChannelState::Counting(ReloadFlag::Normal) | ChannelState::WaitingForLoadTrigger = self.channel_state {
            match *self.mode {
                ChannelMode::InterruptOnTerminalCount => {
                    // Gate controls counting.
                    if *self.gate {
                        self.count();

                        if *self.counting_element == 0 {
                            // Terminal count. Set output high.
                            self.change_output_state(true, bus);
                        }
                    }
                }
                ChannelMode::HardwareRetriggerableOneShot => {
                    self.count();
                    if *self.counting_element == 0 {
                        // Terminal count. Set output high only if timer is armed.
                        if self.armed {
                            self.change_output_state(true, bus);
                        }
                    }
                }
                ChannelMode::RateGenerator => {
                    // Gate controls counting.
                    if *self.gate {
                        self.count();
                        // Output goes low for one clock cycle when count reaches 1.
                        // Counter is reloaded next cycle and output goes HIGH.
                        if *self.counting_element == 1 {
                            self.change_output_state(false, bus);
                            self.output_on_reload = true;
                            self.change_channel_state(ChannelState::Counting(ReloadFlag::ReloadNextCycle));
                        }
                    }
                }
                ChannelMode::SquareWaveGenerator => {
                    // Gate controls counting.
                    if *self.gate {
                        if (*self.count_register & 1) == 0 {
                            // Even reload value. Count decrements by two and reloads on terminal count.
                            self.count2();
                            if *self.counting_element == 0 {
                                self.change_output_state(!*self.output, bus); // Toggle output state
                                self.counting_element.update(*self.reload_value);
                                // Reload counting element
                            }
                        }
                        else {
                            // Odd reload value.
                            if self.ptype == PitType::Model8254 {
                                // On the 8254, odd values are not allowed into the counting element.
                                self.count2();
                                if *self.counting_element == 0 {
                                    if *self.output {
                                        // When output is high, reload is delayed one cycle.
                                        self.output_on_reload = !*self.output; // Toggle output state next cycle
                                        self.change_channel_state(ChannelState::Counting(ReloadFlag::ReloadNextCycle));
                                    // Reload next cycle
                                    }
                                    else {
                                        // Output is low. Reload and update output immediately.
                                        self.change_output_state(!*self.output, bus); // Toggle output state
                                        self.counting_element.update(*self.reload_value);
                                        // Reload counting element
                                    }
                                }
                            }
                            else {
                                // On the 8253, odd values are allowed into the counting element. An odd value
                                // triggers special behavior of output is high.
                                if *self.output && (*self.counting_element & 1) != 0 {
                                    // If output is high and count is odd, decrement by one. The counting element will be even
                                    // from now on until counter is reloaded.
                                    self.count();
                                }
                                else if !*self.output && (*self.counting_element & 1) != 0 {
                                    // If output is low and count is odd, decrement by three. The counting element will be even
                                    // from now on until counter is reloaded.
                                    self.count3();
                                }
                                else {
                                    self.count2();
                                }

                                if *self.counting_element == 0 {
                                    // Counting element is immediately reloaded and output toggled.
                                    self.change_output_state(!*self.output, bus); // Toggle output state
                                    self.counting_element.update(*self.reload_value);
                                }
                            }
                        }
                    }
                }
                ChannelMode::SoftwareTriggeredStrobe => {
                    // Gate controls counting.
                    if *self.gate {
                        self.count();
                        if *self.counting_element == 0 {
                            self.change_output_state(false, bus); // Output goes low for one cycle on terminal count.
                        }
                        else {
                            self.change_output_state(true, bus);
                        }
                    }
                }
                ChannelMode::HardwareTriggeredStrobe => {
                    self.count();
                    if *self.counting_element == 0 {
                        self.change_output_state(false, bus); // Output goes low for one cycle on terminal count.
                    }
                    else {
                        self.change_output_state(true, bus);
                    }
                }
            }
        }

        self.cycles_in_state = self.cycles_in_state.saturating_add(1);
    }
}

impl ProgrammableIntervalTimer {
    pub fn new(ptype: PitType, _crystal: f64, clock_divisor: u32) -> Self {
        /*
            The Intel documentation says:
            "Prior to initialization, the mode, count, and output of all counters is undefined."
            This makes it a challenge to decide the initial state of the virtual PIT. The 5160
            BIOS will halt during POST if there's a pending timer interrupt, so that's a clue we
            shouldn't initially start a timer running, but beyond that it's a guess.
        */
        let mut vec = Vec::<Channel>::new();
        for i in 0..3 {
            vec.push(Channel::new(i, ptype));
        }
        Self {
            ptype,
            _crystal,
            clock_divisor,
            pit_cycles: 0,
            sys_tick_accumulator: 0,
            sys_ticks_advance: 0,
            cycle_accumulator: 0.0,
            channels: vec,
            timewarp: DeviceRunTimeUnit::SystemTicks(0),
            speaker_buf: VecDeque::new(),
            defer_reload_flag: false,
        }
    }

    pub fn reset(&mut self) {
        self.cycle_accumulator = 0.0;

        // Reset the PIT back to sensible defaults.
        // Note: We do not change the gate input state. The PIT does not control gate status.
        for i in 0..3 {
            self.channels[i].mode.update(ChannelMode::InterruptOnTerminalCount);
            self.channels[i].channel_state = ChannelState::WaitingForReload;
            self.channels[i].count_register.update(0);
            self.channels[i].counting_element.update(0);
            self.channels[i].read_state = ReadState::NoRead;
            self.channels[i].count_is_latched = false;
            self.channels[i].ce_undefined = false;
            self.channels[i].output.update(false);
            self.channels[i].bcd_mode = false;
        }
    }

    fn catch_up(&mut self, bus: &mut BusInterface, delta: DeviceRunTimeUnit) {
        // Catch PIT up to CPU.
        let ticks = self.ticks_from_time(delta, self.timewarp);

        //log::debug!("ticking PIT {} times on IO write. delta: {:?} timewarp: {:?}", ticks, delta, self.timewarp);

        //self.timewarp = self.time_from_ticks(ticks);
        self.timewarp = delta; // the above is technically the correct way but it breaks stuff(?)

        self.defer_reload_flag = false;
        if self.sys_tick_accumulator > 2 {
            self.defer_reload_flag = true; // Too late in the bus cycle to reload this tick.
        }

        for _ in 0..ticks {
            self.tick(bus, None);
        }
    }

    /// Return the number of PIT cycles that elapsed for the provided microsecond period.
    fn get_pit_cycles(us: f64) -> f64 {
        us / PIT_TICK_US
    }

    fn control_register_write(&mut self, byte: u8, bus: &mut BusInterface) {
        let control_reg = ControlByte::from_bytes([byte]);

        let c = control_reg.channel() as usize;

        if c > 2 {
            // This is a read-back command.
            match self.ptype {
                PitType::Model8253 => {
                    // Readback command not supported. Do nothing.
                }
                PitType::Model8254 => {
                    // Do readback command here and return.
                }
            }
            return;
        }

        let channel = &mut self.channels[c];

        if let RwModeField::LatchCommand = control_reg.rw_mode() {
            // All 0's access mode indicates a Latch Count Value command
            // Not an access mode itself, we now latch the current value of the channel until it is read
            // or a command byte is received

            channel.latch_count();
            return;
        }

        // Convert rw_mode_field enum to rw_mode enum (drops latch command as possibile variant, as we
        // handled it above)
        let rw_mode = match control_reg.rw_mode() {
            RwModeField::Lsb => RwMode::Lsb,
            RwModeField::Msb => RwMode::Msb,
            RwModeField::LsbMsb => RwMode::LsbMsb,
            _ => unreachable!("Invalid rw_mode"),
        };

        channel.set_mode(control_reg.channel_mode().into(), rw_mode, control_reg.bcd(), bus);
    }

    /// Handle a write to one of the PIT's data registers
    /// Writes to this register specify the reload value for the given channel.
    pub fn data_write(&mut self, port_num: usize, data: u8, bus: &mut BusInterface) {
        self.channels[port_num].write_byte(data, self.defer_reload_flag, bus);
    }

    pub fn data_read(&mut self, port: usize) -> u8 {
        self.channels[port].read_byte()
    }

    pub fn set_channel_gate(&mut self, channel: usize, state: bool, bus: &mut BusInterface) {
        if channel > 2 {
            return;
        }
        // Note: Only the gate to PIT channel #2 is connected to anything (PPI port)

        self.channels[channel].set_gate(state, bus);
    }

    #[inline]
    pub fn time_from_ticks(&mut self, ticks: u32) -> DeviceRunTimeUnit {
        DeviceRunTimeUnit::SystemTicks(ticks * self.clock_divisor)
    }

    pub fn ticks_from_time(&mut self, run_unit: DeviceRunTimeUnit, advance: DeviceRunTimeUnit) -> u32 {
        let mut do_ticks = 0;
        match (run_unit, advance) {
            (DeviceRunTimeUnit::Microseconds(us), DeviceRunTimeUnit::Microseconds(_warp_us)) => {
                let pit_cycles = Pit::get_pit_cycles(us);
                //log::debug!("Got {:?} pit cycles", pit_cycles);

                // Add up fractional cycles until we can make a whole one.
                self.cycle_accumulator += pit_cycles;
                while self.cycle_accumulator > 1.0 {
                    // We have one or more full PIT cycles. Drain the cycle accumulator
                    // by ticking the PIT until the accumulator drops below 1.0.
                    self.cycle_accumulator -= 1.0;
                    do_ticks += 1;
                }
                do_ticks
            }
            (DeviceRunTimeUnit::SystemTicks(ticks), DeviceRunTimeUnit::SystemTicks(warp_ticks)) => {
                // Add up system ticks, then tick the PIT if we have enough ticks for
                // a PIT cycle.

                // We subtract warp ticks - ticks processed during CPU execution to warp
                // device to current CPU cycle. Warp ticks should always be less than or equal to
                // ticks provided to run.
                self.sys_tick_accumulator += ticks - warp_ticks;

                while self.sys_tick_accumulator >= self.clock_divisor {
                    self.sys_tick_accumulator -= self.clock_divisor;
                    do_ticks += 1;
                }
                do_ticks
            }
            _ => {
                panic!("Invalid TimeUnit combination");
            }
        }
    }

    /*    pub fn ticks_from_time_advance(&mut self, run_unit: DeviceRunTimeUnit) -> u32 {
        let mut do_ticks = 0;
        match run_unit {
            DeviceRunTimeUnit::Microseconds(us) => {
                let pit_cycles = Pit::get_pit_cycles(us);
                //log::debug!("Got {:?} pit cycles", pit_cycles);

                // Add up fractional cycles until we can make a whole one.
                self.cycle_accumulator += pit_cycles;
                while self.cycle_accumulator > 1.0 {
                    // We have one or more full PIT cycles. Drain the cycle accumulator
                    // by ticking the PIT until the accumulator drops below 1.0.
                    self.cycle_accumulator -= 1.0;
                    do_ticks += 1;
                }
                do_ticks
            }
            DeviceRunTimeUnit::SystemTicks(ticks) => {
                // Add up system ticks, then tick the PIT if we have enough ticks for
                // a PIT cycle.

                // We want to save the number of ticks advanced now so they can be subtracted
                // from the number of ticks in the post-step() run() call. However, drain
                // the accumulator now as this represents time between the last run() and now.

                self.sys_tick_accumulator += ticks;

                while self.sys_tick_accumulator >= self.clock_divisor {
                    self.sys_tick_accumulator -= self.clock_divisor;
                    do_ticks += 1;
                }

                do_ticks
            }
        }
    }*/

    pub fn run(
        &mut self,
        bus: &mut BusInterface,
        buffer_producer: &mut ringbuf::Producer<u8>,
        run_unit: DeviceRunTimeUnit,
    ) {
        let do_ticks = self.ticks_from_time(run_unit, self.timewarp);

        //log::trace!("doing {} ticks, run_unit: {:?} timewarp: {:?}", do_ticks, run_unit, self.timewarp);

        //assert!(do_ticks >= self.timewarp);

        self.timewarp = DeviceRunTimeUnit::SystemTicks(0);

        for _ in 0..do_ticks {
            self.tick(bus, Some(buffer_producer));
        }
    }

    pub fn get_cycles(&self) -> u64 {
        self.pit_cycles
    }

    pub fn get_output_state(&self, channel: usize) -> bool {
        *self.channels[channel].output
    }

    /// Returns the specified channels' count register (reload value) and counting element
    /// in a tuple.
    #[inline]
    pub fn get_channel_count(&self, channel: usize) -> (u16, u16, bool) {
        (
            *self.channels[channel].count_register.get(),
            *self.channels[channel].counting_element.get(),
            matches!(self.channels[channel].channel_state, ChannelState::Counting(_)),
        )
    }

    #[inline]
    pub fn does_channel_retrigger(&self, channel: usize) -> bool {
        match *self.channels[channel].mode {
            ChannelMode::InterruptOnTerminalCount
            | ChannelMode::HardwareRetriggerableOneShot
            | ChannelMode::SoftwareTriggeredStrobe
            | ChannelMode::HardwareTriggeredStrobe => false,
            _ => true,
        }
    }

    #[inline]
    pub fn get_timer_accum(&self) -> u32 {
        self.sys_tick_accumulator
    }

    /// Return the dirty flags for the specified timer channel. See the description of is_dirty under Channel.
    #[inline]
    pub fn is_dirty(&mut self, channel: usize) -> (bool, bool, bool) {
        self.channels[channel].is_dirty()
    }

    pub fn tick(&mut self, bus: &mut BusInterface, buffer_producer: Option<&mut ringbuf::Producer<u8>>) {
        self.pit_cycles += 1;

        // Get timer channel 2 state from ppi.
        // TODO: it would be better to push this state from PPI when changed then to poll it on tick here.
        let mut speaker_data = true;

        if let Some(ppi) = bus.ppi_mut() {
            speaker_data = ppi.get_pb1_state();
            self.channels[2].set_gate(ppi.get_pit_channel2_gate(), bus);
        }

        self.channels[0].tick(bus, None);
        self.channels[1].tick(bus, None);
        self.channels[2].tick(bus, None);

        //log::trace!("tick(): cycle: {} channel 1 count: {}", self.pit_cycles * 4 + 7, *self.channels[1].counting_element);

        let mut speaker_sample = *self.channels[2].output && speaker_data;

        if let ChannelMode::SquareWaveGenerator = *self.channels[2].mode {
            // Silence speaker if frequency is > 14Khz (approx)
            if *self.channels[2].count_register <= 170 {
                speaker_sample = false;
            }
        }

        // If we have been passed a buffer, fill it with any queued samples
        // and the current sample.
        if let Some(buffer) = buffer_producer {
            // Copy any samples that have accumulated in the buffer.

            for s in self.speaker_buf.drain(0..) {
                _ = buffer.push(s);
            }
            _ = buffer.push((speaker_sample) as u8);
        }
        else {
            // Otherwise, put the sample in the buffer.
            self.speaker_buf.push_back(speaker_sample as u8);
        }
    }

    // TODO: Remove this if no longer needed
    #[rustfmt::skip]
    #[allow(dead_code)]
    pub fn get_string_state(&mut self, clean: bool) -> PitStringState {
        let state = PitStringState {

            c0_value:           SyntaxToken::StateString(format!("{:06}", *self.channels[0].counting_element), self.channels[0].counting_element.is_dirty(), 0),
            c0_reload_value:    SyntaxToken::StateString(format!("{:06}", *self.channels[0].count_register), self.channels[0].count_register.is_dirty(), 0),
            c0_access_mode:     SyntaxToken::StateString(format!("{:?}", *self.channels[0].rw_mode), self.channels[0].rw_mode.is_dirty(), 0),
            c0_channel_output:  SyntaxToken::StateString(format!("{:?}", *self.channels[0].output), self.channels[0].output.is_dirty(), 0),
            c0_channel_mode:    SyntaxToken::StateString(format!("{:?}", *self.channels[0].mode), self.channels[0].mode.is_dirty(), 0),
            c1_value:           SyntaxToken::StateString(format!("{:06}", *self.channels[1].counting_element), self.channels[1].counting_element.is_dirty(), 0),
            c1_reload_value:    SyntaxToken::StateString(format!("{:06}", *self.channels[1].count_register), self.channels[1].count_register.is_dirty(), 0),
            c1_access_mode:     SyntaxToken::StateString(format!("{:?}", *self.channels[1].rw_mode), self.channels[1].rw_mode.is_dirty(), 0),
            c1_channel_output:  SyntaxToken::StateString(format!("{:?}", *self.channels[1].output), self.channels[1].output.is_dirty(), 0),
            c1_channel_mode:    SyntaxToken::StateString(format!("{:?}", *self.channels[1].mode), self.channels[1].mode.is_dirty(), 0),
            c2_value:           SyntaxToken::StateString(format!("{:06}", *self.channels[2].counting_element), self.channels[2].counting_element.is_dirty(), 0),
            c2_reload_value:    SyntaxToken::StateString(format!("{:06}", *self.channels[2].count_register), self.channels[2].count_register.is_dirty(), 0),
            c2_access_mode:     SyntaxToken::StateString(format!("{:?}", *self.channels[2].rw_mode), self.channels[2].rw_mode.is_dirty(), 0),
            c2_channel_output:  SyntaxToken::StateString(format!("{:?}", *self.channels[2].output), self.channels[2].output.is_dirty(), 0),
            c2_channel_mode:    SyntaxToken::StateString(format!("{:?}", *self.channels[2].mode), self.channels[2].mode.is_dirty(), 0),
            c2_gate_status:     SyntaxToken::StateString(format!("{:?}", self.channels[2].gate), self.channels[2].gate.is_dirty(), 0),
        };

        if clean {
            for i in 0..3 {
                self.channels[i].mode.clean();
                self.channels[i].reload_value.clean();
                self.channels[i].counting_element.clean();
                self.channels[i].count_register.clean();
                self.channels[i].rw_mode.clean();
                self.channels[i].gate.clean();
            }

        }

        state
    }

    pub fn get_display_state(&mut self, clean: bool) -> PitDisplayState {
        let mut state_vec = Vec::new();

        for i in 0..3 {
            let mut channel_map = BTreeMap::<&str, SyntaxToken>::new();

            channel_map.insert(
                "Rw Mode:",
                SyntaxToken::StateString(
                    format!("{:?}", *self.channels[i].rw_mode),
                    self.channels[i].rw_mode.is_dirty(),
                    0,
                ),
            );
            channel_map.insert(
                "Channel Mode:",
                SyntaxToken::StateString(
                    format!("{:?}", *self.channels[i].mode),
                    self.channels[i].mode.is_dirty(),
                    0,
                ),
            );
            channel_map.insert(
                "Channel State:",
                SyntaxToken::StateString(format!("{:?}", self.channels[i].channel_state), false, 0),
            );
            channel_map.insert(
                "Counting Element:",
                SyntaxToken::StateString(
                    format!(
                        "{:?} [{:04X}]",
                        *self.channels[i].counting_element, *self.channels[i].counting_element,
                    ),
                    self.channels[i].counting_element.is_dirty(),
                    0,
                ),
            );
            channel_map.insert(
                "Reload Value:",
                SyntaxToken::StateString(
                    format!(
                        "{:?} [{:04X}]",
                        *self.channels[i].reload_value, *self.channels[i].reload_value,
                    ),
                    self.channels[i].reload_value.is_dirty(),
                    0,
                ),
            );
            channel_map.insert(
                "Count Register:",
                SyntaxToken::StateString(
                    format!(
                        "{:?} [{:04X}]",
                        *self.channels[i].count_register, *self.channels[i].count_register,
                    ),
                    self.channels[i].count_register.is_dirty(),
                    0,
                ),
            );
            channel_map.insert(
                "Output latch:",
                SyntaxToken::StateString(
                    format!(
                        "{:?} [{:04X}]",
                        *self.channels[i].output_latch, *self.channels[i].output_latch,
                    ),
                    self.channels[i].output_latch.is_dirty(),
                    0,
                ),
            );
            channel_map.insert(
                "Output Signal:",
                SyntaxToken::StateString(
                    format!("{:?}", *self.channels[i].output),
                    self.channels[i].output.is_dirty(),
                    0,
                ),
            );
            channel_map.insert(
                "Gate Status:",
                SyntaxToken::StateString(
                    format!("{:?}", *self.channels[i].gate),
                    self.channels[i].gate.is_dirty(),
                    0,
                ),
            );

            state_vec.push(channel_map);
        }

        if clean {
            for i in 0..3 {
                self.channels[i].mode.clean();
                self.channels[i].reload_value.clean();
                self.channels[i].counting_element.clean();
                self.channels[i].count_register.clean();
                self.channels[i].output_latch.clean();
                self.channels[i].rw_mode.clean();
                self.channels[i].gate.clean();
                self.channels[i].output.clean();
            }
        }

        state_vec
    }
}
