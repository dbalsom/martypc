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

    bus.rs

    Implement the system bus.

    The bus module implements memory read and write routines.

    The bus module is the logical owner of all memory and devices in the
    system, and implements both IO and MMIO dispatch to installed devices.
*/

#![allow(dead_code)]

use crate::devices::pit::SPEAKER_SAMPLE_RATE;
use anyhow::Error;
use crossbeam_channel::unbounded;
use fxhash::FxHashMap;
use ringbuf::Producer;
use std::{collections::VecDeque, fmt, io::Write, path::Path};

use crate::{
    bytequeue::*,
    cpu_808x::*,
    device_traits::{
        sounddevice::SoundDevice,
        videocard::{ClockingMode, VideoCard, VideoCardDispatch, VideoCardId, VideoCardInterface, VideoType},
    },
    devices::{
        cga::{self, CGACard},
        dma::*,
        fdc::FloppyController,
        hdc::*,
        keyboard::{KeyboardType, *},
        mda::{self, MDACard},
        mouse::*,
        pic::*,
        pit::Pit,
        ppi::*,
        serial::*,
    },
    machine::{KeybufferEntry, MachineCheckpoint, MachinePatch},
    machine_config::{normalize_conventional_memory, MachineConfiguration, MachineDescriptor},
    machine_types::{HardDiskControllerType, SerialControllerType, SerialMouseType},
    memerror::MemError,
    syntax_token::SyntaxToken,
    tracelogger::TraceLogger,
    updatable::*,
};

#[cfg(feature = "ega")]
use crate::devices::ega::{self, EGACard};
#[cfg(feature = "vga")]
use crate::devices::vga::{self, VGACard};
use crate::{
    cpu_common::{CpuDispatch, CpuType},
    device_traits::videocard::VideoCardSubType,
    devices::{
        a0::A0Register,
        adlib::AdLibCard,
        cartridge_slots::CartridgeSlot,
        game_port::GamePort,
        lotech_ems::LotechEmsCard,
        lpt_card::ParallelController,
        pit,
        tga,
        tga::TGACard,
    },
    machine_types::{EmsType, EmsType::LoTech2MB, FdcType, MachineType, SoundType},
    sound::{SoundOutputConfig, SoundSourceDescriptor},
    syntax_token::SyntaxFormatType,
};

pub const NO_IO_BYTE: u8 = 0xFF; // This is the byte read from a unconnected IO address.
pub const OPEN_BUS_BYTE: u8 = 0xFF; // This is the byte read from an unmapped memory address.

const ADDRESS_SPACE: usize = 0x10_0000;
const DEFAULT_WAIT_STATES: u32 = 0;

const MMIO_MAP_SIZE: usize = 0x2000;
const MMIO_MAP_SHIFT: usize = 13;
const MMIO_MAP_LEN: usize = ADDRESS_SPACE >> MMIO_MAP_SHIFT;

pub const MEM_ROM_BIT: u8 = 0b1000_0000; // Bit to signify that this address is ROM
pub const MEM_RET_BIT: u8 = 0b0100_0000; // Bit to signify that this address is a return address for a CALL or INT
pub const MEM_BPE_BIT: u8 = 0b0010_0000; // Bit to signify that this address is associated with a breakpoint on execute
pub const MEM_BPA_BIT: u8 = 0b0001_0000; // Bit to signify that this address is associated with a breakpoint on access
pub const MEM_CP_BIT: u8 = 0b0000_1000; // Bit to signify that this address is a ROM checkpoint
pub const MEM_MMIO_BIT: u8 = 0b0000_0100; // Bit to signify that this address is MMIO mapped
pub const MEM_SW_BIT: u8 = 0b0000_0010; // Bit to signify that this address is in a stopwatch

pub const KB_UPDATE_RATE: f64 = 5000.0; // Keyboard device update rate in microseconds

pub const TIMING_TABLE_LEN: usize = 512;

pub const IMMINENT_TIMER_INTERRUPT: u16 = 10;

pub const DEVICE_DESC_LEN: usize = 28;

#[derive(Copy, Clone, Debug)]
pub struct TimingTableEntry {
    pub sys_ticks: u32,
    pub us: f64,
}

#[derive(Copy, Clone, Debug)]
pub enum ClockFactor {
    Divisor(u8),
    Multiplier(u8),
}

#[derive(Clone, Debug)]
pub struct DeviceRunContext {
    pub delta_ticks: u32,
    pub delta_us: f64,
    pub break_on: Option<DeviceId>,
    pub events: VecDeque<DeviceEvent>,
}

impl Default for DeviceRunContext {
    fn default() -> Self {
        Self {
            delta_ticks: 0,
            delta_us: 0.0,
            break_on: None,
            events: VecDeque::with_capacity(16),
        }
    }
}

impl DeviceRunContext {
    pub fn new(cpu_ticks: u32, factor: ClockFactor, sysclock: f64) -> Self {
        let delta_ticks = match factor {
            ClockFactor::Divisor(n) => (cpu_ticks + (n as u32) - 1) / (n as u32),
            ClockFactor::Multiplier(n) => cpu_ticks * (n as u32),
        };
        let mhz = match factor {
            ClockFactor::Divisor(n) => sysclock / (n as f64),
            ClockFactor::Multiplier(n) => sysclock * (n as f64),
        };
        let delta_us = 1.0 / mhz * cpu_ticks as f64;

        Self {
            delta_ticks,
            delta_us,
            break_on: None,
            events: VecDeque::with_capacity(16),
        }
    }

    #[inline]
    pub fn set_break_on(&mut self, device: DeviceId) {
        self.break_on = Some(device);
    }

    #[inline]
    pub fn add_event(&mut self, event: DeviceEvent) {
        self.events.push_back(event);
    }
}

#[derive(Default)]
pub struct InstalledDevicesResult {
    pub sound_sources: Vec<SoundSourceDescriptor>,
}

impl InstalledDevicesResult {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum DeviceRunTimeUnit {
    SystemTicks(u32),
    Microseconds(f64),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DeviceId {
    None,
    Ppi,
    Pit,
    DmaPrimary,
    DmaSecondary,
    PicPrimary,
    PicSecondary,
    SerialController,
    ParallelController,
    FloppyController,
    HardDiskController,
    Mouse,
    Video,
}

#[derive(Clone, Debug)]
pub enum DeviceEvent {
    NmiTransition(bool),
    InterruptUpdate(u16, u16, bool),
    DramRefreshUpdate(u16, u16, u32, bool),
    DramRefreshEnable(bool),
    TurboToggled(bool),
}

pub trait MemoryMappedDevice {
    fn get_read_wait(&mut self, address: usize, cycles: u32) -> u32;
    fn mmio_read_u8(&mut self, address: usize, cycles: u32, cpumem: Option<&[u8]>) -> (u8, u32);
    fn mmio_read_u16(&mut self, address: usize, cycles: u32, cpumem: Option<&[u8]>) -> (u16, u32);
    fn mmio_peek_u8(&self, address: usize, cpumem: Option<&[u8]>) -> u8;
    fn mmio_peek_u16(&self, address: usize, cpumem: Option<&[u8]>) -> u16;

    fn get_write_wait(&mut self, address: usize, cycles: u32) -> u32;
    fn mmio_write_u8(&mut self, address: usize, data: u8, cycles: u32, cpumem: Option<&mut [u8]>) -> u32;
    fn mmio_write_u16(&mut self, address: usize, data: u16, cycles: u32, cpumem: Option<&mut [u8]>) -> u32;

    fn get_mapping(&self) -> Vec<MemRangeDescriptor>;
}

pub struct MemoryDebug {
    addr:  String,
    byte:  String,
    word:  String,
    dword: String,
    instr: String,
}

impl fmt::Display for MemoryDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            " ADDR: {}\n BYTE: {}\n WORD: {}\nDWORD: {}\nINSTR: {}",
            self.addr, self.byte, self.word, self.dword, self.instr
        )
    }
}

#[derive(Clone, Debug)]
pub struct MemRangeDescriptor {
    pub address: usize,
    pub size: usize,
    pub cycle_cost: u32,
    pub read_only: bool,
    pub priority: u32,
}

impl MemRangeDescriptor {
    pub fn new(address: usize, size: usize, read_only: bool) -> Self {
        Self {
            address,
            size,
            cycle_cost: 0,
            read_only,
            priority: 1,
        }
    }
}

pub enum IoDeviceType {
    A0Register,
    Ppi,
    Pit,
    DmaPrimary,
    DmaSecondary,
    PicPrimary,
    PicSecondary,
    Serial,
    Parallel,
    FloppyController,
    HardDiskController,
    Mouse,
    Ems,
    GamePort,
    Video(VideoCardId),
    Sound,
}

pub enum IoDeviceDispatch {
    Static(IoDeviceType),
    Dynamic(Box<dyn IoDevice + 'static>),
}

#[derive(Clone, Debug, Default)]
pub struct IoDeviceStats {
    last_read: u8,
    last_write: u8,
    reads: usize,
    reads_dirty: bool,
    writes: usize,
    writes_dirty: bool,
}

impl IoDeviceStats {
    pub fn one_read() -> Self {
        Self {
            last_read: 0xFF,
            last_write: 0,
            reads: 1,
            reads_dirty: true,
            writes: 0,
            writes_dirty: false,
        }
    }

    pub fn one_write() -> Self {
        Self {
            last_read: 0,
            last_write: 0xFF,
            reads: 0,
            reads_dirty: false,
            writes: 1,
            writes_dirty: true,
        }
    }
}

pub trait IoDevice {
    fn read_u8(&mut self, port: u16, delta: DeviceRunTimeUnit) -> u8;
    fn write_u8(&mut self, port: u16, data: u8, bus: Option<&mut BusInterface>, delta: DeviceRunTimeUnit);
    fn port_list(&self) -> Vec<(String, u16)>;
}

pub struct MmioData {
    first_map: usize,
    last_map:  usize,
}

impl MmioData {
    fn new() -> Self {
        Self {
            first_map: 0xFFFFF,
            last_map:  0x00000,
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum MmioDeviceType {
    None,
    Memory,
    Video(VideoCardId),
    Cga,
    Ega,
    Vga,
    Rom,
    Ems,
    Cart,
}

// Main bus struct.
// Bus contains both the system memory and IO, and owns all connected devices.
// This ownership hierarchy allows us to avoid needing RefCells for devices.
//
// All devices are wrapped in Options. Some devices are actually optional, depending
// on the machine type.
// But this allows us to 'disassociate' devices from the bus on io writes to allow
// us to call them with bus as an argument.
pub struct BusInterface {
    cpu_factor: ClockFactor,
    timing_table: Box<[TimingTableEntry; TIMING_TABLE_LEN]>,
    machine_desc: Option<MachineDescriptor>,
    keyboard_type: KeyboardType,
    keyboard: Option<Keyboard>,
    conventional_size: usize,
    memory: Vec<u8>,
    memory_mask: Vec<u8>,
    open_bus_byte: u8,
    desc_vec: Vec<MemRangeDescriptor>,
    mmio_map: Vec<(MemRangeDescriptor, MmioDeviceType)>,
    mmio_map_fast: [MmioDeviceType; MMIO_MAP_LEN],
    mmio_data: MmioData,
    cursor: usize,
    intr_imminent: bool,

    io_map: FxHashMap<u16, IoDeviceType>,
    io_desc_map: FxHashMap<u16, String>,
    io_stats: FxHashMap<u16, (bool, IoDeviceStats)>,
    ppi: Option<Ppi>,
    a0: Option<A0Register>,
    a0_data: u8,
    nmi_latch: bool,
    pit: Option<Pit>,
    speaker_src: Option<usize>,
    dma_counter: u16,
    dma1: Option<DMAController>,
    dma2: Option<DMAController>,
    pic1: Option<Pic>,
    pic2: Option<Pic>,
    serial: Option<SerialPortController>,
    parallel: Option<ParallelController>,
    fdc: Option<FloppyController>,
    hdc: Option<HardDiskController>,
    mouse: Option<Mouse>,
    ems: Option<LotechEmsCard>,
    cart_slot: Option<CartridgeSlot>,
    game_port: Option<GamePort>,
    adlib: Option<AdLibCard>,

    videocards:    FxHashMap<VideoCardId, VideoCardDispatch>,
    videocard_ids: Vec<VideoCardId>,

    cycles_to_ticks:   [u32; 256], // TODO: Benchmarks don't show any faster than raw multiplication. It's not slower either though.
    pit_ticks_advance: u32, // We can schedule extra PIT ticks to add when run() occurs. This is generally used for PIT phase offset adjustment.

    do_title_hacks: bool,
    timer_trigger1_armed: bool,
    timer_trigger2_armed: bool,

    cga_tick_accum: u32,
    tga_tick_accum: u32,
    kb_us_accum:    f64,
    refresh_active: bool,

    terminal_port: Option<u16>,
}

#[macro_export]
macro_rules! add_io_device {
    ($self:expr, $device:expr, $device_type:expr) => {{
        let port_list = $device.port_list();
        $self.io_desc_map.extend(port_list.iter().map(|p| (p.1, p.0.clone())));
        $self.io_map.extend(port_list.into_iter().map(|p| (p.1, $device_type)));
    }};
}

#[macro_export]
macro_rules! add_mmio_device {
    ($self:expr, $device:expr, $device_type:expr) => {{
        let mapping = $device.get_mapping();
        for desc in mapping.iter() {
            $self.register_map($device_type, desc.clone());
        }
    }};
}

impl ByteQueue for BusInterface {
    fn seek(&mut self, pos: usize) {
        self.cursor = pos;
    }

    fn tell(&self) -> usize {
        self.cursor
    }

    fn wait(&mut self, _cycles: u32) {}
    fn wait_i(&mut self, _cycles: u32, _instr: &[u16]) {}
    fn wait_comment(&mut self, _comment: &str) {}
    fn set_pc(&mut self, _pc: u16) {}

    fn q_read_u8(&mut self, _dtype: QueueType, _reader: QueueReader) -> u8 {
        if self.cursor < self.memory.len() {
            let (b, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            self.cursor += 1;
            return b;
        }
        0xffu8
    }

    fn q_read_i8(&mut self, _dtype: QueueType, _reader: QueueReader) -> i8 {
        if self.cursor < self.memory.len() {
            let (b, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            self.cursor += 1;
            return b as i8;
        }
        -1i8
    }

    fn q_read_u16(&mut self, _dtype: QueueType, _reader: QueueReader) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let (b0, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            let (b1, _) = self.read_u8(self.cursor + 1, 0).unwrap_or((0xFF, 0));
            let w: u16 = (b0 as u16 | (b1 as u16) << 8);
            self.cursor += 2;
            return w;
        }
        0xffffu16
    }

    fn q_read_i16(&mut self, _dtype: QueueType, _reader: QueueReader) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            let (b0, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            let (b1, _) = self.read_u8(self.cursor + 1, 0).unwrap_or((0xFF, 0));
            let w: u16 = (b0 as u16 | (b1 as u16) << 8);
            self.cursor += 2;
            return w as i16;
        }
        -1i16
    }

    fn q_peek_u8(&mut self) -> u8 {
        if self.cursor < self.memory.len() {
            let b = self.peek_u8(self.cursor).unwrap_or(0xFF);
            return b;
        }
        0xffu8
    }

    fn q_peek_i8(&mut self) -> i8 {
        if self.cursor < self.memory.len() {
            let b = self.peek_u8(self.cursor).unwrap_or(0xFF);
            return b as i8;
        }
        -1i8
    }

    fn q_peek_u16(&mut self) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let w: u16 = (self.peek_u8(self.cursor).unwrap_or(0xFF) as u16
                | self.peek_u8(self.cursor + 1).unwrap_or(0xFF) as u16)
                << 8;
            return w;
        }
        0xffffu16
    }

    fn q_peek_i16(&mut self) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            let w: u16 = (self.peek_u8(self.cursor).unwrap_or(0xFF) as u16
                | self.peek_u8(self.cursor + 1).unwrap_or(0xFF) as u16)
                << 8;
            return w as i16;
        }
        -1i16
    }

    fn q_peek_farptr16(&mut self) -> (u16, u16) {
        if self.cursor < self.memory.len() - 3 {
            let offset: u16 = (self.peek_u8(self.cursor).unwrap_or(0xFF) as u16
                | self.peek_u8(self.cursor + 1).unwrap_or(0xFF) as u16)
                << 8;
            let segment: u16 = (self.peek_u8(self.cursor + 2).unwrap_or(0xFF) as u16
                | self.peek_u8(self.cursor + 3).unwrap_or(0xFF) as u16)
                << 8;
            return (segment, offset);
        }
        (0xffffu16, 0xffffu16)
    }
}

impl Default for BusInterface {
    fn default() -> Self {
        BusInterface {
            cpu_factor: ClockFactor::Divisor(3),
            timing_table: Box::new([TimingTableEntry { sys_ticks: 0, us: 0.0 }; TIMING_TABLE_LEN]),
            machine_desc: None,
            keyboard_type: KeyboardType::ModelF,
            keyboard: None,
            conventional_size: ADDRESS_SPACE,
            memory: vec![0; ADDRESS_SPACE],
            memory_mask: vec![0; ADDRESS_SPACE],
            open_bus_byte: 0xFF,
            desc_vec: Vec::new(),
            mmio_map: Vec::new(),
            mmio_map_fast: [MmioDeviceType::Memory; MMIO_MAP_LEN],
            mmio_data: MmioData::new(),
            cursor: 0,
            intr_imminent: false,

            io_map: FxHashMap::default(),
            io_desc_map: FxHashMap::default(),
            io_stats: FxHashMap::default(),
            ppi: None,
            a0: None,
            a0_data: 0,
            nmi_latch: false,
            pit: None,
            speaker_src: None,
            dma_counter: 0,
            dma1: None,
            dma2: None,
            pic1: None,
            pic2: None,
            serial: None,
            parallel: None,
            fdc: None,
            hdc: None,
            mouse: None,
            ems: None,
            cart_slot: None,
            game_port: None,
            adlib: None,
            videocards: FxHashMap::default(),
            videocard_ids: Vec::new(),

            cycles_to_ticks:   [0; 256],
            pit_ticks_advance: 0,

            do_title_hacks: false,
            timer_trigger1_armed: false,
            timer_trigger2_armed: false,

            cga_tick_accum: 0,
            tga_tick_accum: 0,
            kb_us_accum:    0.0,
            refresh_active: false,

            terminal_port: None,
        }
    }
}

impl BusInterface {
    pub fn new(cpu_factor: ClockFactor, machine_desc: MachineDescriptor, keyboard_type: KeyboardType) -> BusInterface {
        let mut timing_table = Box::new([TimingTableEntry { sys_ticks: 0, us: 0.0 }; TIMING_TABLE_LEN]);
        Self::update_timing_table(&mut timing_table, cpu_factor, machine_desc.system_crystal);

        BusInterface {
            cpu_factor,
            timing_table,
            machine_desc: Some(machine_desc),
            keyboard_type,
            ..BusInterface::default()
        }
    }

    pub fn set_options(&mut self, do_timing_hacks: bool) {
        self.do_title_hacks = do_timing_hacks;
    }

    /// Update the bus timing table.
    /// The bus keeps a timing table which is a lookup table of system ticks and microseconds for each possible CPU
    /// instruction cycle count from 0 to TIMING_TABLE_LEN. This table needs to be updated whenever the clock divisor
    /// or cpu crystal frequency changes.
    pub fn update_timing_table(
        timing_table: &mut [TimingTableEntry; TIMING_TABLE_LEN],
        clock_factor: ClockFactor,
        cpu_crystal: f64,
    ) {
        for cycles in 0..TIMING_TABLE_LEN {
            let entry = &mut timing_table[cycles];

            entry.sys_ticks = match clock_factor {
                ClockFactor::Divisor(n) => ((cycles as u32) + (n as u32) - 1) / (n as u32),
                ClockFactor::Multiplier(n) => (cycles as u32) * (n as u32),
            };
            let mhz = match clock_factor {
                ClockFactor::Divisor(n) => cpu_crystal / (n as f64),
                ClockFactor::Multiplier(n) => cpu_crystal * (n as f64),
            };
            entry.us = 1.0 / mhz * cycles as f64;
        }
    }

    #[inline]
    pub fn get_timings_for_cycles(&self, cycles: u32) -> &TimingTableEntry {
        &self.timing_table[cycles as usize]
    }

    #[inline]
    pub fn set_context_timings(&self, context: &mut DeviceRunContext, cycles: u32) {
        context.delta_ticks = self.timing_table[cycles as usize].sys_ticks;
        context.delta_us = self.timing_table[cycles as usize].us;
    }

    /// Set the checkpoint bit in memory flags for all checkpoints provided.
    pub fn install_checkpoints(&mut self, checkpoints: &Vec<MachineCheckpoint>) {
        for checkpoint in checkpoints.iter() {
            self.memory_mask[checkpoint.addr as usize & 0xFFFFF] |= MEM_CP_BIT;
        }
    }

    pub fn install_patch_checkpoints(&mut self, patches: &Vec<MachinePatch>) {
        for patch in patches.iter() {
            log::debug!("Arming patch trigger [{:05X}] for patch: {}", patch.trigger, patch.desc);
            self.memory_mask[patch.trigger as usize & 0xFFFFF] |= MEM_CP_BIT;
        }
    }

    pub fn clear_checkpoints(&mut self) {
        for byte_ref in &mut self.memory_mask {
            *byte_ref &= !MEM_CP_BIT;
        }
    }

    pub fn install_patch(&mut self, patch: &mut MachinePatch) {
        if patch.installed {
            // Don't install patch twice (we might be revisiting the same checkpoint)
            return;
        }
        let patch_size = patch.bytes.len();
        let patch_start = patch.addr as usize & 0xFFFFF;
        let patch_end = patch_start + patch_size;

        if patch_end > self.memory.len() {
            log::error!("Patch out of range: {} len: {}", patch_start, patch_size);
            return;
        }

        for (dst, src) in self.memory[patch_start..patch_end].iter_mut().zip(patch.bytes.iter()) {
            *dst = *src;
        }
        patch.installed = true;
    }

    pub fn set_conventional_size(&mut self, size: usize) {
        self.conventional_size = size;
    }

    pub fn conventional_size(&self) -> usize {
        self.conventional_size
    }

    pub fn size(&self) -> usize {
        self.memory.len()
    }

    /// Register a memory-mapped device.
    ///
    /// The MemoryMappedDevice trait's read & write methods will be called instead for memory in the range
    /// specified withing MemRangeDescriptor.
    pub fn register_map(&mut self, device: MmioDeviceType, mem_descriptor: MemRangeDescriptor) {
        if mem_descriptor.address < self.mmio_data.first_map {
            self.mmio_data.first_map = mem_descriptor.address;
        }
        if (mem_descriptor.address + mem_descriptor.size) > self.mmio_data.last_map {
            self.mmio_data.last_map = mem_descriptor.address + mem_descriptor.size;
        }

        // Mark memory flag bit as MMIO for this range.
        for i in mem_descriptor.address..(mem_descriptor.address + mem_descriptor.size) {
            self.memory_mask[i] |= MEM_MMIO_BIT;
        }

        // Add entry to mmio_map_fast
        assert_eq!(mem_descriptor.size % MMIO_MAP_SIZE, 0);
        let map_segs = mem_descriptor.size / MMIO_MAP_SIZE;

        for i in 0..map_segs {
            self.mmio_map_fast[(mem_descriptor.address >> MMIO_MAP_SHIFT) + i] = device.clone();
        }

        self.mmio_map.push((mem_descriptor, device));
    }

    pub fn copy_from(&mut self, src: &[u8], location: usize, cycle_cost: u32, read_only: bool) -> Result<(), bool> {
        let src_size = src.len();
        if location + src_size > self.memory.len() {
            // copy request goes out of bounds
            log::error!("copy out of range: {} len: {}", location, src_size);
            return Err(false);
        }

        let mem_slice: &mut [u8] = &mut self.memory[location..location + src_size];
        let mask_slice: &mut [u8] = &mut self.memory_mask[location..location + src_size];

        for (dst, src) in mem_slice.iter_mut().zip(src) {
            *dst = *src;
        }

        // Write access mask
        let access_bit = match read_only {
            true => MEM_ROM_BIT,
            false => 0x00,
        };
        for dst in mask_slice.iter_mut() {
            *dst |= access_bit;
        }

        self.desc_vec.push({
            MemRangeDescriptor {
                address: location,
                size: src_size,
                cycle_cost,
                read_only,
                priority: 1,
            }
        });

        Ok(())
    }

    /// Write the specified bytes from src_vec into memory at location 'location'
    ///
    /// Does not obey memory mapping
    pub fn patch_from(&mut self, src_vec: &Vec<u8>, location: usize) -> Result<(), bool> {
        let src_size = src_vec.len();
        if location + src_size > self.memory.len() {
            // copy request goes out of bounds
            return Err(false);
        }

        let mem_slice: &mut [u8] = &mut self.memory[location..location + src_size];

        for (dst, src) in mem_slice.iter_mut().zip(src_vec.as_slice()) {
            *dst = *src;
        }
        Ok(())
    }

    /// Return a slice of memory at the specified location and length.
    /// Does not resolve mmio addresses.
    pub fn get_slice_at(&self, start: usize, len: usize) -> &[u8] {
        if start >= self.memory.len() {
            return &[];
        }

        &self.memory[start..std::cmp::min(start + len, self.memory.len())]
    }

    /// Return a vector of memory at the specified location and length.
    /// Does not resolve mmio addresses.
    pub fn get_vec_at(&self, start: usize, len: usize) -> Vec<u8> {
        if start >= self.memory.len() {
            return Vec::new();
        }

        self.memory[start..std::cmp::min(start + len, self.memory.len())].to_vec()
    }

    /// Return a vector representing the contents of memory starting from the specified location,
    /// and continuing for the specified length. This function resolves mmio addresses.
    pub fn get_vec_at_ex(&self, start: usize, len: usize) -> Vec<u8> {
        if start >= self.memory.len() {
            return Vec::new();
        }

        let start_mmio_block = start >> MMIO_MAP_SHIFT;
        let end_mmio_block = (start + len) >> MMIO_MAP_SHIFT;

        // First, scan the mmio map to see if the range contains any mmio mapped devices.
        // If one is found, set a flag to fall back to the slow path.
        let mut range_has_mmio = false;
        for i in start_mmio_block..=end_mmio_block {
            if self.mmio_map_fast[i] != MmioDeviceType::None {
                range_has_mmio = true;
            }
        }

        if !range_has_mmio {
            // Fast path: No mmio. Just return a slice of conventional.
            self.memory[start..std::cmp::min(start + len, self.memory.len())].to_vec()
        }
        else {
            // Slow path: Slice has mmio. Build a vector from bus peeks.
            let mut result = Vec::with_capacity(len);
            for addr in start..std::cmp::min(start + len, self.memory.len()) {
                match self.peek_u8(addr) {
                    Ok(byte) => result.push(byte),
                    Err(_) => result.push(0xFF),
                }
            }
            result
        }
    }

    pub fn set_descriptor(&mut self, start: usize, size: usize, cycle_cost: u32, read_only: bool) {
        // TODO: prevent overlapping descriptors
        self.desc_vec.push({
            MemRangeDescriptor {
                address: start,
                size,
                cycle_cost,
                read_only,
                priority: 1,
            }
        });
    }

    pub fn clear(&mut self) {
        // Remove return flags
        for byte_ref in &mut self.memory_mask {
            *byte_ref &= !MEM_RET_BIT;
        }

        // Set all bytes to open bus byte
        for byte_ref in &mut self.memory {
            *byte_ref = self.open_bus_byte;
        }
        // Then clear conventional memory
        for byte_ref in &mut self.memory[0..self.conventional_size] {
            *byte_ref = 0;
        }

        // Reset IO statistics
        self.io_stats.clear();
    }

    pub fn reset(&mut self) {
        // Clear mem range descriptors
        self.desc_vec.clear();
        self.clear();
    }

    pub fn set_cpu_factor(&mut self, cpu_factor: ClockFactor) {
        self.cpu_factor = cpu_factor;

        self.recalculate_cycle_lut();
    }

    pub fn recalculate_cycle_lut(&mut self) {
        for c in 0..256 {
            self.cycles_to_ticks[c as usize] = self.cpu_cycles_to_system_ticks(c);
        }
    }

    #[inline]
    /// Convert a count of CPU cycles to system clock ticks based on the current CPU
    /// clock divisor.
    fn cpu_cycles_to_system_ticks(&self, cycles: u32) -> u32 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => cycles * (n as u32),
            ClockFactor::Multiplier(n) => cycles / (n as u32),
        }
    }

    #[inline]
    /// Convert a count of system clock ticks to CPU cycles based on the current CPU
    /// clock divisor. If a clock Divisor is set, the dividend will be rounded upwards.
    fn system_ticks_to_cpu_cycles(&self, ticks: u32) -> u32 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => (ticks + (n as u32) - 1) / (n as u32),
            ClockFactor::Multiplier(n) => ticks * (n as u32),
        }
    }

    pub fn get_read_wait(&mut self, address: usize, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                // Handle memory-mapped devices
                let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            match card_dispatch {
                                VideoCardDispatch::Mda(mda) => {
                                    let syswait = mda.get_read_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                VideoCardDispatch::Cga(cga) => {
                                    let syswait = cga.get_read_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                VideoCardDispatch::Tga(tga) => {
                                    let syswait = tga.get_read_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                #[cfg(feature = "ega")]
                                VideoCardDispatch::Ega(ega) => {
                                    let syswait = ega.get_read_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                #[cfg(feature = "vga")]
                                VideoCardDispatch::Vga(vga) => {
                                    let syswait = vga.get_read_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                _ => {}
                            }
                        }
                    }
                    MmioDeviceType::Cart => {
                        return Ok(0);
                    }
                    _ => {}
                }
                // We didn't match any mmio devices, return raw memory
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn get_write_wait(&mut self, address: usize, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                // Handle memory-mapped devices
                let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            match card_dispatch {
                                VideoCardDispatch::Mda(mda) => {
                                    let syswait = mda.get_write_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                VideoCardDispatch::Cga(cga) => {
                                    let syswait = cga.get_write_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                VideoCardDispatch::Tga(tga) => {
                                    let syswait = tga.get_write_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                #[cfg(feature = "ega")]
                                VideoCardDispatch::Ega(ega) => {
                                    let syswait = ega.get_write_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                #[cfg(feature = "vga")]
                                VideoCardDispatch::Vga(vga) => {
                                    let syswait = vga.get_write_wait(address, system_ticks);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                }
                                _ => {}
                            }
                        }
                    }
                    MmioDeviceType::Cart => {
                        return Ok(0);
                    }
                    _ => {}
                }
                // We didn't match any mmio devices, return raw memory
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_u8(&mut self, address: usize, cycles: u32) -> Result<(u8, u32), MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                let data: u8 = self.memory[address];
                return Ok((data, 0));
            }
            else {
                // Handle memory-mapped devices
                let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            match card_dispatch {
                                VideoCardDispatch::Mda(mda) => {
                                    let (data, _waits) =
                                        MemoryMappedDevice::mmio_read_u8(mda, address, system_ticks, None);
                                    return Ok((data, 0));
                                }
                                VideoCardDispatch::Cga(cga) => {
                                    let (data, _waits) =
                                        MemoryMappedDevice::mmio_read_u8(cga, address, system_ticks, None);
                                    return Ok((data, 0));
                                }
                                VideoCardDispatch::Tga(tga) => {
                                    let (data, _waits) = MemoryMappedDevice::mmio_read_u8(
                                        tga,
                                        address,
                                        system_ticks,
                                        Some(&self.memory),
                                    );
                                    return Ok((data, 0));
                                }
                                #[cfg(feature = "ega")]
                                VideoCardDispatch::Ega(ega) => {
                                    let (data, _waits) =
                                        MemoryMappedDevice::mmio_read_u8(ega, address, system_ticks, None);
                                    return Ok((data, 0));
                                }
                                #[cfg(feature = "vga")]
                                VideoCardDispatch::Vga(vga) => {
                                    let (data, _waits) =
                                        MemoryMappedDevice::mmio_read_u8(vga, address, system_ticks, None);
                                    return Ok((data, 0));
                                }
                                _ => {}
                            }
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            let (data, _waits) = MemoryMappedDevice::mmio_read_u8(ems, address, system_ticks, None);
                            return Ok((data, 0));
                        }
                    }
                    MmioDeviceType::Cart => {
                        if let Some(cart_slot) = &mut self.cart_slot {
                            let (data, _waits) =
                                MemoryMappedDevice::mmio_read_u8(cart_slot, address, system_ticks, None);
                            return Ok((data, 0));
                        }
                    }
                    _ => {}
                }
                return Err(MemError::MmioError);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn peek_range(&self, address: usize, len: usize) -> Result<&[u8], MemError> {
        if address + len < self.memory.len() {
            Ok(&self.memory[address..address + len])
        }
        else {
            Err(MemError::ReadOutOfBoundsError)
        }
    }

    pub fn peek_u8(&self, address: usize) -> Result<u8, MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                let b: u8 = self.memory[address];
                return Ok(b);
            }
            else {
                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get(&vid) {
                            match card_dispatch {
                                VideoCardDispatch::Mda(mda) => {
                                    let data = MemoryMappedDevice::mmio_peek_u8(mda, address, None);
                                    return Ok(data);
                                }
                                VideoCardDispatch::Cga(cga) => {
                                    let data = MemoryMappedDevice::mmio_peek_u8(cga, address, None);
                                    return Ok(data);
                                }
                                VideoCardDispatch::Tga(tga) => {
                                    let data = MemoryMappedDevice::mmio_peek_u8(tga, address, Some(&self.memory));
                                    return Ok(data);
                                }
                                #[cfg(feature = "ega")]
                                VideoCardDispatch::Ega(ega) => {
                                    let data = MemoryMappedDevice::mmio_peek_u8(ega, address, None);
                                    return Ok(data);
                                }
                                #[cfg(feature = "vga")]
                                VideoCardDispatch::Vga(vga) => {
                                    let data = MemoryMappedDevice::mmio_peek_u8(vga, address, None);
                                    return Ok(data);
                                }
                                _ => {}
                            }
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &self.ems {
                            let data = MemoryMappedDevice::mmio_peek_u8(ems, address, None);
                            return Ok(data);
                        }
                    }
                    MmioDeviceType::Cart => {
                        if let Some(cart_slot) = &self.cart_slot {
                            let data = MemoryMappedDevice::mmio_peek_u8(cart_slot, address, None);
                            return Ok(data);
                        }
                    }
                    _ => {}
                }
                return Err(MemError::MmioError);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_u16(&mut self, address: usize, cycles: u32) -> Result<(u16, u32), MemError> {
        if address < self.memory.len() - 1 {
            if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                let w: u16 = self.memory[address] as u16 | (self.memory[address + 1] as u16) << 8;
                return Ok((w, DEFAULT_WAIT_STATES));
            }
            else {
                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let system_ticks = self.cycles_to_ticks[cycles as usize];
                            match card_dispatch {
                                VideoCardDispatch::Mda(mda) => {
                                    //let (data, syswait) = MemoryMappedDevice::read_u16(cga, address, system_ticks);
                                    let (data, syswait) = mda.mmio_read_u16(address, system_ticks, None);
                                    return Ok((data, self.system_ticks_to_cpu_cycles(syswait)));
                                }
                                VideoCardDispatch::Cga(cga) => {
                                    //let (data, syswait) = MemoryMappedDevice::read_u16(cga, address, system_ticks);
                                    let (data, syswait) = cga.mmio_read_u16(address, system_ticks, None);
                                    return Ok((data, self.system_ticks_to_cpu_cycles(syswait)));
                                }
                                VideoCardDispatch::Tga(tga) => {
                                    //let (data, syswait) = MemoryMappedDevice::read_u16(cga, address, system_ticks);
                                    let (data, syswait) = tga.mmio_read_u16(address, system_ticks, Some(&self.memory));
                                    return Ok((data, self.system_ticks_to_cpu_cycles(syswait)));
                                }
                                #[cfg(feature = "ega")]
                                VideoCardDispatch::Ega(ega) => {
                                    let (data, _syswait) =
                                        MemoryMappedDevice::mmio_read_u16(ega, address, system_ticks, None);
                                    return Ok((data, 0));
                                }
                                #[cfg(feature = "vga")]
                                VideoCardDispatch::Vga(vga) => {
                                    let (data, _syswait) =
                                        MemoryMappedDevice::mmio_read_u16(vga, address, system_ticks, None);
                                    return Ok((data, 0));
                                }
                                _ => {}
                            }
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            let (data, syswait) = MemoryMappedDevice::mmio_read_u16(ems, address, 0, None);
                            return Ok((data, self.system_ticks_to_cpu_cycles(syswait)));
                        }
                    }
                    _ => {}
                }
                return Ok((0xFFFF, 0));
                //return Err(MemError::MmioError);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn write_u8(&mut self, address: usize, data: u8, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & (MEM_MMIO_BIT | MEM_ROM_BIT) == 0 {
                // Address is not mapped and not ROM, write to it if it is within conventional memory.
                if address < self.conventional_size {
                    self.memory[address] = data;
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                // Handle memory-mapped devices.
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let system_ticks = self.cycles_to_ticks[cycles as usize];
                            match card_dispatch {
                                VideoCardDispatch::Mda(mda) => {
                                    let _syswait = mda.mmio_write_u8(address, data, system_ticks, None);
                                    //return Ok(self.system_ticks_to_cpu_cycles(syswait)); // temporary wait state value.
                                    return Ok(0);
                                }
                                VideoCardDispatch::Cga(cga) => {
                                    let _syswait = cga.mmio_write_u8(address, data, system_ticks, None);
                                    //return Ok(self.system_ticks_to_cpu_cycles(syswait)); // temporary wait state value.
                                    return Ok(0);
                                }
                                VideoCardDispatch::Tga(tga) => {
                                    let _syswait = tga.mmio_write_u8(
                                        address,
                                        data,
                                        system_ticks,
                                        Some(self.memory.as_mut_slice()),
                                    );
                                    //return Ok(self.system_ticks_to_cpu_cycles(syswait)); // temporary wait state value.
                                    return Ok(0);
                                }
                                #[cfg(feature = "ega")]
                                VideoCardDispatch::Ega(ega) => {
                                    MemoryMappedDevice::mmio_write_u8(ega, address, data, system_ticks, None);
                                }
                                #[cfg(feature = "vga")]
                                VideoCardDispatch::Vga(vga) => {
                                    MemoryMappedDevice::mmio_write_u8(vga, address, data, system_ticks, None);
                                }
                                _ => {}
                            }
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            MemoryMappedDevice::mmio_write_u8(ems, address, data, 0, None);
                        }
                    }
                    _ => {}
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn write_u16(&mut self, address: usize, data: u16, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {
            if self.memory_mask[address] & (MEM_MMIO_BIT | MEM_ROM_BIT) == 0 {
                // Address is not mapped. Write to memory if within conventional memory size.
                if address < self.conventional_size - 1 {
                    self.memory[address] = (data & 0xFF) as u8;
                    self.memory[address + 1] = (data >> 8) as u8;
                }
                else if address < self.conventional_size {
                    self.memory[address] = (data & 0xFF) as u8;
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let system_ticks = self.cycles_to_ticks[cycles as usize];

                            match card_dispatch {
                                VideoCardDispatch::Mda(mda) => {
                                    let mut syswait;
                                    syswait = MemoryMappedDevice::mmio_write_u8(
                                        mda,
                                        address,
                                        (data & 0xFF) as u8,
                                        system_ticks,
                                        None,
                                    );
                                    syswait +=
                                        MemoryMappedDevice::mmio_write_u8(mda, address + 1, (data >> 8) as u8, 0, None);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                    // temporary wait state value.
                                }
                                VideoCardDispatch::Cga(cga) => {
                                    let mut syswait;
                                    syswait = MemoryMappedDevice::mmio_write_u8(
                                        cga,
                                        address,
                                        (data & 0xFF) as u8,
                                        system_ticks,
                                        None,
                                    );
                                    syswait +=
                                        MemoryMappedDevice::mmio_write_u8(cga, address + 1, (data >> 8) as u8, 0, None);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                    // temporary wait state value.
                                }
                                VideoCardDispatch::Tga(tga) => {
                                    let mut syswait;
                                    syswait = MemoryMappedDevice::mmio_write_u8(
                                        tga,
                                        address,
                                        (data & 0xFF) as u8,
                                        system_ticks,
                                        None,
                                    );
                                    syswait +=
                                        MemoryMappedDevice::mmio_write_u8(tga, address + 1, (data >> 8) as u8, 0, None);
                                    return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                    // temporary wait state value.
                                }
                                #[cfg(feature = "ega")]
                                VideoCardDispatch::Ega(ega) => {
                                    MemoryMappedDevice::mmio_write_u8(
                                        ega,
                                        address,
                                        (data & 0xFF) as u8,
                                        system_ticks,
                                        None,
                                    );
                                    MemoryMappedDevice::mmio_write_u8(ega, address + 1, (data >> 8) as u8, 0, None);
                                }
                                #[cfg(feature = "vga")]
                                VideoCardDispatch::Vga(vga) => {
                                    MemoryMappedDevice::mmio_write_u8(
                                        vga,
                                        address,
                                        (data & 0xFF) as u8,
                                        system_ticks,
                                        None,
                                    );
                                    MemoryMappedDevice::mmio_write_u8(vga, address + 1, (data >> 8) as u8, 0, None);
                                }
                                _ => {}
                            }
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            MemoryMappedDevice::mmio_write_u16(ems, address, data, 0, None);
                        }
                    }
                    _ => {}
                }
                return Ok(0);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    /// Get bit flags for the specified byte at address
    #[inline]
    pub fn get_flags(&self, address: usize) -> u8 {
        if address < self.memory.len() - 1 {
            self.memory_mask[address]
        }
        else {
            0
        }
    }

    /// Set bit flags for the specified byte at address
    pub fn set_flags(&mut self, address: usize, flags: u8) {
        if address < self.memory.len() - 1 {
            //log::trace!("set flag for address: {:05X}: {:02X}", address, flags);
            self.memory_mask[address] |= flags;
        }
    }

    /// Clear the specified flags for the specified byte at address
    /// Do not allow ROM bit to be cleared
    pub fn clear_flags(&mut self, address: usize, flags: u8) {
        if address < self.memory.len() - 1 {
            self.memory_mask[address] &= !(flags & 0x7F);
        }
    }

    /// Dump memory to a string representation.
    ///
    /// Does not honor memory mappings.
    pub fn dump_flat(&self, address: usize, size: usize) -> String {
        if address + size >= self.memory.len() {
            return "REQUEST OUT OF BOUNDS".to_string();
        }
        else {
            let mut dump_str = String::new();
            let dump_slice = &self.memory[address..address + size];
            let mut display_address = address;

            for dump_row in dump_slice.chunks_exact(16) {
                let mut dump_line = String::new();
                let mut ascii_line = String::new();

                for byte in dump_row {
                    dump_line.push_str(&format!("{:02x} ", byte));

                    let char_str = match byte {
                        00..=31 => ".".to_string(),
                        32..=127 => format!("{}", *byte as char),
                        128.. => ".".to_string(),
                    };
                    ascii_line.push_str(&char_str)
                }
                dump_str.push_str(&format!("{:05X} {} {}\n", display_address, dump_line, ascii_line));
                display_address += 16;
            }
            return dump_str;
        }
    }

    /// Dump memory to a vector of string representations.
    ///
    /// Does not honor memory mappings.
    pub fn dump_flat_vec(&self, address: usize, size: usize) -> Vec<String> {
        let mut vec = Vec::new();

        if address + size >= self.memory.len() {
            vec.push("REQUEST OUT OF BOUNDS".to_string());
            return vec;
        }
        else {
            let dump_slice = &self.memory[address..address + size];
            let mut display_address = address;

            for dump_row in dump_slice.chunks_exact(16) {
                let mut dump_line = String::new();
                let mut ascii_line = String::new();

                for byte in dump_row {
                    dump_line.push_str(&format!("{:02x} ", byte));

                    let char_str = match byte {
                        00..=31 => ".".to_string(),
                        32..=127 => format!("{}", *byte as char),
                        128.. => ".".to_string(),
                    };
                    ascii_line.push_str(&char_str)
                }

                vec.push(format!("{:05X} {} {}\n", display_address, dump_line, ascii_line));

                display_address += 16;
            }
        }
        vec
    }

    /// Dump memory to a vector of vectors of SyntaxTokens.
    ///
    /// Does not honor memory mappings.
    pub fn dump_flat_tokens(&self, address: usize, cursor: usize, mut size: usize) -> Vec<Vec<SyntaxToken>> {
        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        if address >= self.memory.len() {
            // Start address is invalid. Send only an error token.
            let mut linevec = Vec::new();

            linevec.push(SyntaxToken::ErrorString("REQUEST OUT OF BOUNDS".to_string()));
            vec.push(linevec);

            return vec;
        }
        else if address + size >= self.memory.len() {
            // Request size invalid. Send truncated result.
            let new_size = size - ((address + size) - self.memory.len());
            size = new_size
        }

        let dump_slice = &self.memory[address..address + size];
        let mut display_address = address;

        for dump_row in dump_slice.chunks_exact(16) {
            let mut line_vec = Vec::new();

            // Push memory flat address tokens
            line_vec.push(SyntaxToken::MemoryAddressFlat(
                display_address as u32,
                format!("{:05X}", display_address),
            ));

            // Build hex byte value tokens
            let mut i = 0;
            for byte in dump_row {
                if (display_address + i) == cursor {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        *byte,
                        format!("{:02X}", *byte),
                        true, // Set cursor on this byte
                        0,
                    ));
                }
                else {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        *byte,
                        format!("{:02X}", *byte),
                        false,
                        0,
                    ));
                }
                i += 1;
            }

            // Build ASCII representation tokens
            let mut i = 0;
            for byte in dump_row {
                let char_str = match byte {
                    00..=31 => ".".to_string(),
                    32..=127 => format!("{}", *byte as char),
                    128.. => ".".to_string(),
                };
                line_vec.push(SyntaxToken::MemoryByteAsciiValue(
                    (display_address + i) as u32,
                    *byte,
                    char_str,
                    0,
                ));
                i += 1;
            }

            vec.push(line_vec);
            display_address += 16;
        }

        vec
    }

    /// Dump memory to a vector of vectors of SyntaxTokens.
    ///
    /// Uses bus peek functions to resolve MMIO addresses.
    pub fn dump_flat_tokens_ex(&self, address: usize, cursor: usize, mut size: usize) -> Vec<Vec<SyntaxToken>> {
        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        if address >= self.memory.len() {
            // Start address is invalid. Send only an error token.
            let mut linevec = Vec::new();

            linevec.push(SyntaxToken::ErrorString("REQUEST OUT OF BOUNDS".to_string()));
            vec.push(linevec);

            return vec;
        }
        else if address + size >= self.memory.len() {
            // Request size invalid. Send truncated result.
            let new_size = size - ((address + size) - self.memory.len());
            size = new_size
        }

        let addr_vec = Vec::from_iter(address..address + size);
        let mut display_address = address;

        for dump_addr_row in addr_vec.chunks_exact(16) {
            let mut line_vec = Vec::new();

            // Push memory flat address tokens
            line_vec.push(SyntaxToken::MemoryAddressFlat(
                display_address as u32,
                format!("{:05X}", display_address),
            ));

            // Build hex byte value tokens
            let mut i = 0;
            for addr in dump_addr_row {
                let byte = self.peek_u8(*addr).unwrap();

                if (display_address + i) == cursor {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        byte,
                        format!("{:02X}", byte),
                        true, // Set cursor on this byte
                        0,
                    ));
                }
                else {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        byte,
                        format!("{:02X}", byte),
                        false,
                        0,
                    ));
                }
                i += 1;
            }

            // Build ASCII representation tokens
            let mut i = 0;
            for addr in dump_addr_row {
                let byte = self.peek_u8(*addr).unwrap();

                let char_str = match byte {
                    00..=31 => ".".to_string(),
                    32..=127 => format!("{}", byte as char),
                    128.. => ".".to_string(),
                };
                line_vec.push(SyntaxToken::MemoryByteAsciiValue(
                    (display_address + i) as u32,
                    byte,
                    char_str,
                    0,
                ));
                i += 1;
            }

            vec.push(line_vec);
            display_address += 16;
        }

        vec
    }

    pub fn dump_mem(&self, path: &Path) {
        let filename = path.to_path_buf();

        let len = 0x100000;
        let address = 0;
        log::debug!("Dumping {} bytes at address {:05X}", len, address);

        match std::fs::write(filename.clone(), &self.memory) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename.display())
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename.display(), e)
            }
        }
    }

    pub fn dump_mem_range(&self, start: u32, end: u32, path: &Path) {
        let filename = path.to_path_buf();

        let len = end.saturating_sub(start) as usize;
        let end = (start as usize + len) & 0xFFFFF;
        log::debug!("Dumping {} bytes at address {:05X}", len, start);

        match std::fs::write(filename.clone(), &self.memory[(start as usize)..=end]) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename.display())
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename.display(), e)
            }
        }
    }

    pub fn dump_ivt_tokens(&mut self) -> Vec<Vec<SyntaxToken>> {
        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        for v in 0..256 {
            let mut ivr_vec = Vec::new();
            let (ip, _) = self.read_u16((v * 4) as usize, 0).unwrap();
            let (cs, _) = self.read_u16(((v * 4) + 2) as usize, 0).unwrap();

            ivr_vec.push(SyntaxToken::Text(format!("{:03}", v)));
            ivr_vec.push(SyntaxToken::Colon);
            ivr_vec.push(SyntaxToken::OpenBracket);
            ivr_vec.push(SyntaxToken::StateMemoryAddressSeg16(
                cs,
                ip,
                format!("{:04X}:{:04X}", cs, ip),
                255,
            ));
            ivr_vec.push(SyntaxToken::CloseBracket);
            // TODO: The bus should eventually register IRQs, and then we would query the bus for the device identifier
            //       for each IRQ.
            match v {
                0 => ivr_vec.push(SyntaxToken::Text("Divide Error".to_string())),
                1 => ivr_vec.push(SyntaxToken::Text("Single Step".to_string())),
                2 => ivr_vec.push(SyntaxToken::Text("NMI".to_string())),
                3 => ivr_vec.push(SyntaxToken::Text("Breakpoint".to_string())),
                4 => ivr_vec.push(SyntaxToken::Text("Overflow".to_string())),
                8 => ivr_vec.push(SyntaxToken::Text("Timer".to_string())),
                9 => ivr_vec.push(SyntaxToken::Text("Keyboard".to_string())),
                //10 => ivr_vec.push(SyntaxToken::Text("Slave PIC".to_string())),
                11 => ivr_vec.push(SyntaxToken::Text("Serial Port 2".to_string())),
                12 => ivr_vec.push(SyntaxToken::Text("Serial Port 1".to_string())),
                13 => ivr_vec.push(SyntaxToken::Text("HDC".to_string())),
                14 => ivr_vec.push(SyntaxToken::Text("FDC".to_string())),
                15 => ivr_vec.push(SyntaxToken::Text("Parallel Port 1".to_string())),
                _ => {}
            }
            vec.push(ivr_vec);
        }
        vec
    }

    /// Returns a MemoryDebug struct containing information about the memory at the specified address.
    /// This is used in the Memory Viewer debug window to show a popup when hovering over a byte.
    pub fn get_memory_debug(&mut self, cpu_type: CpuType, address: usize) -> MemoryDebug {
        let mut debug = MemoryDebug {
            addr:  format!("{:05X}", address),
            byte:  String::new(),
            word:  String::new(),
            dword: String::new(),
            instr: String::new(),
        };

        if address < self.memory.len() - 1 {
            debug.byte = format!("{:02X}", self.peek_u8(address).unwrap_or(0xFF));
        }
        if address < self.memory.len() - 2 {
            debug.word = format!(
                "{:04X}",
                self.peek_u8(address).unwrap_or(0xFF) as u16 | (self.peek_u8(address + 1).unwrap_or(0xFF) as u16) << 8
            );
        }
        if address < self.memory.len() - 4 {
            debug.dword = format!(
                "{:04X}",
                self.peek_u8(address).unwrap_or(0xFF) as u32
                    | ((self.peek_u8(address + 1).unwrap_or(0xFF) as u32) << 8)
                    | ((self.peek_u8(address + 2).unwrap_or(0xFF) as u32) << 16)
                    | ((self.peek_u8(address + 3).unwrap_or(0xFF) as u32) << 24)
            );
        }

        self.seek(address);

        debug.instr = match cpu_type.decode(self, true) {
            Ok(instruction) => {
                format!("{}", instruction)
            }
            Err(_) => "Invalid".to_string(),
        };
        debug
    }

    pub fn install_devices(
        &mut self,
        machine_desc: &MachineDescriptor,
        machine_config: &MachineConfiguration,
        sound_config: &SoundOutputConfig,
        terminal_port: Option<u16>,
    ) -> Result<InstalledDevicesResult, Error> {
        let mut installed_devices = InstalledDevicesResult::new();
        let video_frame_debug = false;
        let clock_mode = ClockingMode::Default;

        if let Some(terminal_port) = terminal_port {
            log::debug!("Terminal port set to: {:04X}", terminal_port);
        }
        self.terminal_port = terminal_port;

        // First we need to initialize the PPI. The PPI is used to read the system's DIP switches, so the PPI must be
        // given several parameters from the machine configuration.

        // Create vector of video types for PPI initialization.
        let video_types = machine_config
            .video
            .iter()
            .map(|vcd| vcd.video_type)
            .collect::<Vec<VideoType>>();

        // Get the number of floppies.
        let num_floppies = machine_config
            .fdc
            .as_ref()
            .map(|fdc| fdc.drive.len() as u32)
            .unwrap_or(0);

        // Get normalized conventional memory and set it.
        let conventional_memory = normalize_conventional_memory(machine_config)?;
        self.set_conventional_size(conventional_memory as usize);
        self.open_bus_byte = machine_desc.open_bus_byte;

        // Create the A0 register if specified.
        // TODO: Wrap this up in a motherboard device type?
        if let Some(a0_type) = machine_desc.a0 {
            let a0 = A0Register::new(a0_type);

            log::debug!("Creating A0 register...");
            add_io_device!(self, a0, IoDeviceType::A0Register);
            self.a0 = Some(a0);
        }

        // Set the expansion rom flag for DIP if there is anything besides a video card
        // that needs an expansion ROM.
        //let mut have_expansion = { machine_config.hdc.is_some() };
        //have_expansion = false;

        // Create PPI if PPI is defined for this machine type
        if machine_desc.have_ppi {
            self.ppi = Some(Ppi::new(
                machine_desc.machine_type,
                conventional_memory,
                false,
                video_types,
                num_floppies,
            ));
            // Add PPI ports to io_map

            add_io_device!(self, self.ppi.as_mut().unwrap(), IoDeviceType::Ppi);
        }

        // Create the crossbeam channel for the PIT to send sound samples to the sound output thread.
        let pit_sample_sender = if machine_config.speaker {
            // Add this sound source.
            let (s, r) = unbounded();
            installed_devices
                .sound_sources
                .push(SoundSourceDescriptor::new("PC Speaker", SPEAKER_SAMPLE_RATE, 1, r));

            // Speaker will always be first sound source, if enabled.
            self.speaker_src = Some(0);
            Some(s)
        }
        else {
            None
        };

        // Create the PIT. One PIT will always exist, but it may be an 8253 or 8254.
        // Pick the device type from MachineDesc.
        // Provide the timer with its base crystal and divisor.
        let mut pit = Pit::new(
            machine_desc.pit_type,
            if let Some(crystal) = machine_desc.timer_crystal {
                crystal
            }
            else {
                machine_desc.system_crystal
            },
            machine_desc.timer_divisor,
            pit_sample_sender,
        );

        // Add PIT ports to io_map
        add_io_device!(self, pit, IoDeviceType::Pit);

        // Tie gates for pit channel 0 & 1 high.
        pit.set_channel_gate(0, true, self);
        pit.set_channel_gate(1, true, self);

        self.pit = Some(pit);

        // Create DMA. One DMA controller will always exist.
        let dma1 = DMAController::new();

        // Add DMA ports to io_map
        add_io_device!(self, dma1, IoDeviceType::DmaPrimary);
        self.dma1 = Some(dma1);

        // Create PIC. One PIC will always exist.
        let pic1 = Pic::new();
        // Add PIC ports to io_map
        add_io_device!(self, pic1, IoDeviceType::PicPrimary);
        self.pic1 = Some(pic1);

        // Create keyboard if specified.
        if let Some(kb_config) = &machine_config.keyboard {
            let mut keyboard = Keyboard::new(kb_config.kb_type, false);

            keyboard.set_typematic_params(
                Some(kb_config.typematic),
                kb_config.typematic_delay,
                kb_config.typematic_rate,
            );

            self.keyboard = Some(keyboard);
        }

        // Create FDC if specified.
        if let Some(fdc_config) = &machine_config.fdc {
            let floppy_ct = fdc_config.drive.len();
            let fdc_type = fdc_config.fdc_type;

            // Create the correct kind of FDC (currently only NEC supported)
            match fdc_type {
                FdcType::IbmNec | FdcType::IbmPCJrNec => {
                    let fdc = FloppyController::new(fdc_type, fdc_config.drive.clone());
                    // Add FDC ports to io_map
                    add_io_device!(self, fdc, IoDeviceType::FloppyController);
                    self.fdc = Some(fdc);
                }
            }
        }

        // Create a HardDiskController if specified
        if let Some(hdc_config) = &machine_config.hdc {
            match hdc_config.hdc_type {
                HardDiskControllerType::IbmXebec => {
                    // TODO: Get the correct drive type from the specified VHD...?
                    let hdc = HardDiskController::new(2, DRIVE_TYPE2_DIP);
                    // Add HDC ports to io_map
                    add_io_device!(self, hdc, IoDeviceType::HardDiskController);
                    self.hdc = Some(hdc);
                }
            }
        }

        // Create an onboard parallel port if specified
        if let Some(port_base) = machine_desc.onboard_parallel {
            log::debug!("Creating on-board parallel port...");
            let parallel = ParallelController::new(Some(port_base));
            // Add Parallel Port ports to io_map
            add_io_device!(self, parallel, IoDeviceType::Parallel);
            self.parallel = Some(parallel);
        }

        // Create a Serial card if specified
        if let Some(serial_config) = machine_config.serial.get(0) {
            let mut out2_suppresses_int = true;
            if let MachineType::IbmPCJr = machine_desc.machine_type {
                out2_suppresses_int = false;
            }
            match serial_config.sc_type {
                SerialControllerType::IbmAsync => {
                    let serial = SerialPortController::new(out2_suppresses_int);
                    // Add Serial Controller ports to io_map
                    add_io_device!(self, serial, IoDeviceType::Serial);
                    self.serial = Some(serial);
                }
            }
        }

        // Create a Serial mouse if specified
        if let Some(serial_mouse_config) = &machine_config.serial_mouse {
            // Only create mouse if we have as serial card to plug it into!
            if self.serial.is_some() {
                match serial_mouse_config.mouse_type {
                    SerialMouseType::Microsoft => {
                        let mouse = Mouse::new(serial_mouse_config.port as usize);
                        self.mouse = Some(mouse);
                    }
                }
            }
        }

        // Create an EMS board if specified
        if let Some(ems_config) = &machine_config.ems {
            if let EmsType::LoTech2MB = ems_config.ems_type {
                // Add EMS ports to io_map
                let ems = LotechEmsCard::new(Some(ems_config.io_base), Some(ems_config.window as usize));
                add_io_device!(self, ems, IoDeviceType::Ems);
                add_mmio_device!(self, ems, MmioDeviceType::Ems);
                self.ems = Some(ems);
            }
            else {
                log::error!("Bad EMS type {:?}", ems_config.ems_type);
            }
        }

        // Create PCJr cartridge slot
        if machine_desc.pcjr_cart_slot {
            let cart_slot = CartridgeSlot::new();
            add_mmio_device!(self, cart_slot, MmioDeviceType::Cart);
            self.cart_slot = Some(cart_slot);
        }

        // Create a game port
        let mut game_port_addr = None;
        // Is there one onboard?
        if machine_desc.game_port.is_some() {
            game_port_addr = machine_desc.game_port;
        }
        // Is there one in the machine config?
        else if let Some(game_port) = &machine_config.game_port {
            game_port_addr = Some(game_port.io_base);
        }
        // Either way, install it if present
        if let Some(game_port_addr) = game_port_addr {
            let game_port = GamePort::new(Some(game_port_addr));
            add_io_device!(self, game_port, IoDeviceType::GamePort);
            self.game_port = Some(game_port);
        }

        // Create sound cards
        for (i, card) in machine_config.sound.iter().enumerate() {
            if let SoundType::AdLib = card.sound_type {
                // Create an AdLib card.

                let (s, r) = unbounded();
                installed_devices.sound_sources.push(SoundSourceDescriptor::new(
                    "AdLib Music Synthesizer",
                    sound_config.sample_rate,
                    2,
                    r,
                ));

                let mut adlib = AdLibCard::new(card.io_base, 48000, s);
                println!(">>> TESTING ADLIB <<<");

                add_io_device!(self, adlib, IoDeviceType::Sound);
                self.adlib = Some(adlib);
            }
        }

        // Create video cards
        for (i, card) in machine_config.video.iter().enumerate() {
            let video_dispatch;
            let video_id = VideoCardId {
                idx:   i,
                vtype: card.video_type,
            };

            log::debug!("Creating video card of type: {:?}", card.video_type);
            match card.video_type {
                VideoType::MDA => {
                    let mda = MDACard::new(
                        card.video_subtype.unwrap_or(VideoCardSubType::None),
                        TraceLogger::None,
                        clock_mode,
                        true,
                        video_frame_debug,
                    );
                    add_io_device!(self, mda, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, mda, MmioDeviceType::Video(video_id));
                    video_dispatch = VideoCardDispatch::Mda(mda)
                }
                VideoType::CGA => {
                    let cga = CGACard::new(TraceLogger::None, clock_mode, video_frame_debug);
                    add_io_device!(self, cga, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, cga, MmioDeviceType::Video(video_id));
                    video_dispatch = VideoCardDispatch::Cga(cga)
                }
                VideoType::TGA => {
                    // Subtype can be Tandy1000 or PCJr
                    let subtype = card.video_subtype.unwrap_or(VideoCardSubType::Tandy1000);
                    let tga = TGACard::new(subtype, TraceLogger::None, clock_mode, video_frame_debug);
                    add_io_device!(self, tga, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, tga, MmioDeviceType::Video(video_id));
                    video_dispatch = VideoCardDispatch::Tga(tga)
                }
                #[cfg(feature = "ega")]
                VideoType::EGA => {
                    let ega = EGACard::new(TraceLogger::None, clock_mode, video_frame_debug, card.dip_switch);
                    add_io_device!(self, ega, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, ega, MmioDeviceType::Video(video_id));
                    video_dispatch = VideoCardDispatch::Ega(ega)
                }
                #[cfg(feature = "vga")]
                VideoType::VGA => {
                    let vga = VGACard::new(TraceLogger::None, clock_mode, video_frame_debug, card.dip_switch);
                    add_io_device!(self, vga, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, vga, MmioDeviceType::Video(video_id));
                    video_dispatch = VideoCardDispatch::Vga(vga)
                }
                #[allow(unreachable_patterns)]
                _ => {
                    panic!(
                        "card type {:?} not implemented or feature not compiled",
                        card.video_type
                    );
                }
            }

            self.videocards.insert(video_id, video_dispatch);
            self.videocard_ids.push(video_id);
        }

        self.machine_desc = Some(machine_desc.clone());
        Ok(installed_devices)
    }

    /// Return whether NMI is enabled.
    /// On the 5150 & 5160, NMI generation can be disabled via the PPI.
    pub fn nmi_enabled(&self) -> bool {
        match self.machine_desc.unwrap().machine_type {
            // TODO: Add other types?
            MachineType::Ibm5150v64K | MachineType::Ibm5150v256K | MachineType::Ibm5160 => {
                if let Some(ppi) = &self.ppi {
                    ppi.nmi_enabled()
                }
                else {
                    true
                }
            }
            // Add other types that use A0 register?
            MachineType::IbmPCJr => {
                if let Some(a0) = &self.a0 {
                    a0.is_nmi_enabled()
                }
                else {
                    panic!("PCJr should have A0!")
                }
            }
            _ => true,
        }
    }

    // Schedule extra ticks for the PIT.
    pub fn adjust_pit(&mut self, ticks: u32) {
        log::debug!("Scheduling {} extra system ticks for PIT", ticks);
        self.pit_ticks_advance += ticks;
    }

    pub fn process_keyboard_input(&mut self) {
        if let Some(keyboard) = &mut self.keyboard {
            // Read a byte from the keyboard
            if let Some(kb_byte) = keyboard.recv_scancode() {
                //log::debug!("Received keyboard byte: {:02X}", kb_byte);

                // Do we have a PPI? if so, send the scancode to the PPI
                if let Some(ppi) = &mut self.ppi {
                    ppi.send_keyboard(kb_byte);

                    match self.machine_desc.unwrap().machine_type {
                        MachineType::IbmPCJr => {
                            // The PCJr is an odd duck and uses NMI for keyboard interrupts.
                            if let Some(a0) = &mut self.a0 {
                                a0.set_nmi_latch(true);
                            }
                        }
                        _ => {
                            if ppi.kb_enabled() {
                                if let Some(pic) = &mut self.pic1 {
                                    // TODO: Should we let the PPI do this directly?
                                    //log::warn!("sending kb interrupt for byte: {:02X}", kb_byte);
                                    pic.pulse_interrupt(1);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn run_devices(
        &mut self,
        us: f64,
        sys_ticks: u32,
        kb_event_opt: Option<KeybufferEntry>,
        kb_buf: &mut VecDeque<KeybufferEntry>,
    ) -> Option<DeviceEvent> {
        let mut event = None;

        let mut process_keyboard = false;
        if let Some(keyboard) = &mut self.keyboard {
            self.kb_us_accum += us;

            // Handle user-initiated keyboard events
            if let Some(kb_event) = kb_event_opt {
                //log::debug!("Got keyboard byte: {:02X}", kb_byte);
                match kb_event.pressed {
                    true => keyboard.key_down(kb_event.keycode, &kb_event.modifiers, Some(kb_buf)),
                    false => keyboard.key_up(kb_event.keycode),
                }
                process_keyboard = true;
            }

            // Run the keyboard device and handle resulting events (typematic repeat)
            if self.kb_us_accum > KB_UPDATE_RATE {
                keyboard.run(KB_UPDATE_RATE);
                self.kb_us_accum -= KB_UPDATE_RATE;
                process_keyboard = true;
            }
        }

        if process_keyboard {
            self.process_keyboard_input();
        }

        // There will always be a PIC, so safe to unwrap.
        let pic = self.pic1.as_mut().unwrap();

        pic.run(sys_ticks);

        // There will always be a PIT, so safe to unwrap.
        let mut pit = self.pit.take().unwrap();

        // Run the A0 register. It doesn't need a time delta.
        let mut ppi_nmi_latch = None;
        if let Some(a0) = &mut self.a0 {
            let new_nmi_latch = a0.run(&mut pit, 0.0);
            self.a0_data = a0.read();

            if !self.nmi_latch && new_nmi_latch {
                event = Some(DeviceEvent::NmiTransition(true));
            }
            else if self.nmi_latch && !new_nmi_latch {
                log::debug!("Clearing NMI line to CPU.");
                event = Some(DeviceEvent::NmiTransition(false));
            }

            ppi_nmi_latch = Some(new_nmi_latch);
            self.nmi_latch = new_nmi_latch;
        }

        // Run the PPI if present. PPI takes PIC to generate keyboard interrupts.
        if let Some(ppi) = &mut self.ppi {
            if let Some(latch_state) = ppi_nmi_latch {
                ppi.set_nmi_latch_bit(latch_state);
            }
            ppi.run(pic, us);
        }

        // Run the PIT. The PIT communicates with lots of things, so we send it the entire bus.
        // The PIT may have a separate clock crystal, such as in the IBM AT. In this case, there may not
        // be an integer number of PIT ticks per system ticks. Therefore, the PIT can take either
        // system ticks (PC/XT) or microseconds as an update parameter.
        if let Some(_crystal) = self.machine_desc.unwrap().timer_crystal {
            pit.run(self, DeviceRunTimeUnit::Microseconds(us));
        }
        else {
            // We can only adjust phase of PIT if we are using system ticks, and that's okay. It's only really useful
            // on an 5150/5160.
            pit.run(self, DeviceRunTimeUnit::SystemTicks(sys_ticks + self.pit_ticks_advance));
            self.pit_ticks_advance = 0;
        }

        self.handle_refresh_scheduling(&mut pit, &mut event);

        // Save current count info.
        let (pit_reload_value, pit_counting_element, pit_counting) = pit.get_channel_count(0);

        // Set imminent interrupt flag. The CPU can use this as a hint to adjust cycles for halt instructions - using
        // more cycles when an interrupt is not imminent, and one cycle when it is. This allows for cycle-precise wake
        // from halt.
        self.intr_imminent = pit_counting & (pit_counting_element <= IMMINENT_TIMER_INTERRUPT);

        if self.do_title_hacks {
            // Arm timer adjustment triggers for Area5150 lake/wibble effects.
            // The ISR chains that set up these effects are a worst-case situation for emulators.
            if pit_reload_value == 5117 && !self.timer_trigger1_armed {
                self.timer_trigger1_armed = true;
                log::debug!("Area5150 hack armed for lake effect.");
            }
            else if pit_reload_value == 5162 && !self.timer_trigger2_armed {
                self.timer_trigger2_armed = true;
                log::debug!("Area5150 hack armed for wibble effect.");
            }
        }

        // Put the PIT back.
        self.pit = Some(pit);

        let mut dma1 = self.dma1.take().unwrap();

        // Run the FDC, passing it DMA controller while DMA is still unattached.
        if let Some(mut fdc) = self.fdc.take() {
            fdc.run(&mut dma1, self, us);
            self.fdc = Some(fdc);
        }

        // Run the HDC, passing it DMA controller while DMA is still unattached.
        if let Some(mut hdc) = self.hdc.take() {
            hdc.run(&mut dma1, self, us);
            self.hdc = Some(hdc);
        }

        // Run the DMA controller.
        dma1.run(self);

        // Replace the DMA controller.
        self.dma1 = Some(dma1);

        // Run the serial port and mouse.
        if let Some(serial) = &mut self.serial {
            serial.run(&mut self.pic1.as_mut().unwrap(), us);

            if let Some(mouse) = &mut self.mouse {
                mouse.run(serial, us);
            }
        }

        // Run the game port {
        if let Some(game_port) = &mut self.game_port {
            game_port.run(us);
        }

        // Run the adlib card {
        if let Some(adlib) = &mut self.adlib {
            adlib.run(us);
        }

        let mut do_area5150_hack = false;
        let mut save_cga: VideoCardId = Default::default();

        // Run all video cards
        for (vid, video_dispatch) in self.videocards.iter_mut() {
            match video_dispatch {
                VideoCardDispatch::Mda(mda) => {
                    mda.run(DeviceRunTimeUnit::Microseconds(us), &mut self.pic1, None);
                }
                VideoCardDispatch::Cga(cga) => {
                    self.cga_tick_accum += sys_ticks;

                    if self.cga_tick_accum > 8 {
                        cga.run(
                            DeviceRunTimeUnit::SystemTicks(self.cga_tick_accum),
                            &mut self.pic1,
                            None,
                        );
                        self.cga_tick_accum = 0;

                        if (self.timer_trigger1_armed || self.timer_trigger2_armed) && (pit_reload_value == 19912) {
                            do_area5150_hack = true;
                            save_cga = *vid;
                        }
                    }
                }
                VideoCardDispatch::Tga(tga) => {
                    self.cga_tick_accum += sys_ticks;

                    if self.cga_tick_accum > 8 {
                        tga.set_a0(self.a0_data);
                        tga.run(
                            DeviceRunTimeUnit::SystemTicks(self.cga_tick_accum),
                            &mut self.pic1,
                            Some(&self.memory),
                        );
                        self.cga_tick_accum = 0;
                    }
                }
                #[cfg(feature = "ega")]
                VideoCardDispatch::Ega(ega) => {
                    ega.run(DeviceRunTimeUnit::Microseconds(us), &mut self.pic1, None);
                }
                #[cfg(feature = "vga")]
                VideoCardDispatch::Vga(vga) => {
                    vga.run(DeviceRunTimeUnit::Microseconds(us), &mut self.pic1, None);
                }
                VideoCardDispatch::None => {}
            }
        }

        if self.do_title_hacks && do_area5150_hack {
            if let VideoCardDispatch::Cga(cga) = self.videocards.get_mut(&save_cga).unwrap() {
                Self::do_area5150_hack(
                    pit_counting_element,
                    self.timer_trigger1_armed,
                    self.timer_trigger2_armed,
                    cga,
                );
            }
            self.timer_trigger1_armed = false;
            self.timer_trigger2_armed = false;
        }

        event
    }

    pub fn do_area5150_hack(pit_counting_element: u16, trigger1: bool, trigger2: bool, cga: &mut CGACard) {
        let mut screen_target = 21960;
        let screen_tick_pos = cga.get_screen_ticks();
        let mut effect: String = String::new();

        if trigger1 {
            screen_target = 21960;
            effect = "Lake".to_string();
        }
        else if trigger2 {
            screen_target = 21952;
            effect = "Wibble".to_string();
        }

        if screen_tick_pos > screen_target {
            // Adjust if we are late - we can't tick the card backwards, so we tick an entire frame minus delta
            let ticks_adj = screen_tick_pos - screen_target;
            log::warn!(
                "Doing Area5150 hack for {} effect. Target: {} Pos: {} Rewinding CGA by {} ticks. (Timer: {})",
                effect,
                screen_target,
                screen_tick_pos,
                ticks_adj,
                pit_counting_element
            );
            cga.debug_tick(233472 - ticks_adj as u32, None);
        }
        else {
            // Adjust if we are early
            let ticks_adj = screen_target - screen_tick_pos;
            log::warn!(
                "Doing Area5150 hack for {} effect. Target: {} Pos: {} Advancing CGA by {} ticks. (Timer: {})",
                effect,
                screen_target,
                screen_tick_pos,
                ticks_adj,
                pit_counting_element
            );
            cga.debug_tick(ticks_adj as u32, None);
        }
    }

    pub fn handle_refresh_scheduling(&mut self, pit: &mut Pit, event: &mut Option<DeviceEvent>) {
        // Has PIT channel 1 (DMA timer) changed?
        let (pit_dirty, pit_counting, pit_ticked) = pit.is_dirty(1);

        if pit_dirty {
            log::trace!("Pit is dirty! counting: {} ticked: {}", pit_counting, pit_ticked);
        }

        if pit_counting && pit_dirty {
            // Pit is dirty and counting. Update the DMA scheduler.

            let (dma_count_register, dma_counting_element, _counting) = pit.get_channel_count(1);
            let retriggers = pit.does_channel_retrigger(1);

            // Get the timer accumulator to provide tick offset to DMA scheduler.
            // The timer ticks every 12 system ticks by default on PC/XT; if 11 ticks are stored in the accumulator,
            // this represents two CPU cycles, so we need to adjust the scheduler by that much.
            let dma_add_ticks = pit.get_timer_accum();

            log::trace!(
                "pit dirty and counting! count register: {} counting element: {} ",
                dma_count_register,
                dma_counting_element
            );

            if dma_counting_element <= dma_count_register {
                // DRAM refresh DMA counter has changed. If the counting element is in range,
                // update the CPU's DRAM refresh simulation.
                log::trace!(
                    "DRAM refresh DMA counter updated: {}, {}, +{}",
                    dma_count_register,
                    dma_counting_element,
                    dma_add_ticks
                );
                self.dma_counter = dma_count_register;

                // Invert the dma counter value as Cpu counts up toward total

                if dma_counting_element == 0 && !pit_ticked {
                    // Counter is still at initial 0 - not a terminal count.
                    *event = Some(DeviceEvent::DramRefreshUpdate(dma_count_register, 0, 0, retriggers));
                }
                else {
                    // Timer is at terminal count!
                    *event = Some(DeviceEvent::DramRefreshUpdate(
                        dma_count_register,
                        dma_counting_element,
                        dma_add_ticks,
                        retriggers,
                    ));
                }
                self.refresh_active = true;
            }
        }
        else if !pit_counting && self.refresh_active {
            // Timer 1 isn't counting anymore! Disable DRAM refresh...
            log::debug!("Channel 1 not counting. Disabling DRAM refresh...");
            *event = Some(DeviceEvent::DramRefreshEnable(false));
            self.refresh_active = false;
        }
    }

    //noinspection RsBorrowChecker
    /// Call the reset methods for all devices on the bus
    pub fn reset_devices(&mut self) {
        // Reset PIT
        if let Some(pit) = self.pit.as_mut() {
            pit.reset();
        }

        // Reset PIC
        if let Some(pic1) = self.pic1.as_mut() {
            pic1.reset();
        }

        // Reset DMA
        if let Some(dma1) = self.dma1.as_mut() {
            dma1.reset();
        }

        // Reset Serial controller
        if let Some(serial) = self.serial.as_mut() {
            serial.reset();
        }

        // Reset video cards
        let vids: Vec<_> = self.videocards.keys().cloned().collect();
        for vid in vids {
            self.video_mut(&vid).map(|video| video.reset());
        }
    }

    /// Call the reset methods for devices to be reset on warm boot
    pub fn reset_devices_warm(&mut self) {
        self.pit.as_mut().unwrap().reset();
        //self.pic1.as_mut().unwrap().reset();
    }

    /// Read an 8-bit value from an IO port.
    ///
    /// We provide the elapsed cycle count for the current instruction. This allows a device
    /// to optionally tick itself to bring itself in sync with CPU state.
    pub fn io_read_u8(&mut self, port: u16, cycles: u32) -> u8 {
        // Convert cycles to system clock ticks
        let sys_ticks = match self.cpu_factor {
            ClockFactor::Divisor(d) => d as u32 * cycles,
            ClockFactor::Multiplier(m) => cycles / m as u32,
        };
        let nul_delta = DeviceRunTimeUnit::Microseconds(0.0);

        let mut handled = false;
        let mut byte = None;
        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::A0Register => {
                    if let Some(a0) = &mut self.a0 {
                        byte = Some(a0.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Ppi => {
                    if let Some(ppi) = &mut self.ppi {
                        byte = Some(ppi.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Pit => {
                    // There will always be a PIT, so safe to unwrap
                    byte = Some(
                        self.pit
                            .as_mut()
                            .unwrap()
                            .read_u8(port, DeviceRunTimeUnit::SystemTicks(sys_ticks)),
                    );
                    //self.pit.as_mut().unwrap().read_u8(port, nul_delta)
                }
                IoDeviceType::DmaPrimary => {
                    // There will always be a primary DMA, so safe to unwrap
                    byte = Some(self.dma1.as_mut().unwrap().read_u8(port, nul_delta));
                }
                IoDeviceType::DmaSecondary => {
                    // Secondary DMA may not exist
                    if let Some(dma2) = &mut self.dma2 {
                        byte = Some(dma2.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::PicPrimary => {
                    // There will always be a primary PIC, so safe to unwrap
                    byte = Some(self.pic1.as_mut().unwrap().read_u8(port, nul_delta));
                }
                IoDeviceType::PicSecondary => {
                    // Secondary PIC may not exist
                    if let Some(pic2) = &mut self.pic2 {
                        byte = Some(pic2.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::FloppyController => {
                    if let Some(fdc) = &mut self.fdc {
                        byte = Some(fdc.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::HardDiskController => {
                    if let Some(hdc) = &mut self.hdc {
                        byte = Some(hdc.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        byte = Some(serial.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Parallel => {
                    if let Some(parallel) = &mut self.parallel {
                        byte = Some(parallel.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Ems => {
                    if let Some(ems) = &mut self.ems {
                        byte = Some(ems.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::GamePort => {
                    if let Some(game_port) = &mut self.game_port {
                        byte = Some(game_port.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Video(vid) => {
                    if let Some(video_dispatch) = self.videocards.get_mut(&vid) {
                        byte = match video_dispatch {
                            VideoCardDispatch::Mda(mda) => {
                                Some(IoDevice::read_u8(mda, port, DeviceRunTimeUnit::SystemTicks(sys_ticks)))
                            }
                            VideoCardDispatch::Cga(cga) => {
                                Some(IoDevice::read_u8(cga, port, DeviceRunTimeUnit::SystemTicks(sys_ticks)))
                            }
                            VideoCardDispatch::Tga(tga) => {
                                Some(IoDevice::read_u8(tga, port, DeviceRunTimeUnit::SystemTicks(sys_ticks)))
                            }
                            #[cfg(feature = "ega")]
                            VideoCardDispatch::Ega(ega) => Some(IoDevice::read_u8(ega, port, nul_delta)),
                            #[cfg(feature = "vga")]
                            VideoCardDispatch::Vga(vga) => Some(IoDevice::read_u8(vga, port, nul_delta)),
                            VideoCardDispatch::None => None,
                        }
                    }
                }
                IoDeviceType::Sound => {
                    if let Some(adlib) = &mut self.adlib {
                        byte = Some(adlib.read_u8(port, nul_delta));
                    }
                }
                _ => {}
            }
        }

        let byte_val = byte.unwrap_or(NO_IO_BYTE);

        self.io_stats
            .entry(port)
            .and_modify(|e| {
                e.1.last_read = byte_val;
                e.1.reads += 1;
                e.1.reads_dirty = true;
            })
            .or_insert((byte.is_some(), IoDeviceStats::one_read()));

        byte_val
    }

    /// Write an 8-bit value to an IO port.
    ///
    /// We provide the elapsed cycle count for the current instruction. This allows a device
    /// to optionally tick itself to bring itself in sync with CPU state.
    pub fn io_write_u8(&mut self, port: u16, data: u8, cycles: u32) {
        // Convert cycles to system clock ticks
        let sys_ticks = match self.cpu_factor {
            ClockFactor::Divisor(n) => cycles * (n as u32),
            ClockFactor::Multiplier(n) => cycles / (n as u32),
        };

        // Handle terminal debug port
        if let Some(terminal_port) = self.terminal_port {
            if port == terminal_port {
                //log::debug!("Write to terminal port: {:02X}", data);

                // Filter Escape character to avoid terminal shenanigans.
                // See: https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters
                if data != 0x1B {
                    print!("{}", data as char);
                    _ = std::io::stdout().flush();
                }
            }
        }

        let nul_delta = DeviceRunTimeUnit::Microseconds(0.0);

        let mut resolved = false;
        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::A0Register => {
                    if let Some(a0) = &mut self.a0 {
                        a0.write_u8(port, data, None, nul_delta);
                        resolved = true;
                    }
                }
                IoDeviceType::Ppi => {
                    if let Some(mut ppi) = self.ppi.take() {
                        ppi.write_u8(port, data, Some(self), nul_delta);
                        resolved = true;
                        self.ppi = Some(ppi);
                    }
                }
                IoDeviceType::Pit => {
                    if let Some(mut pit) = self.pit.take() {
                        //log::debug!("writing PIT with {} cycles", cycles);
                        pit.write_u8(port, data, Some(self), DeviceRunTimeUnit::SystemTicks(sys_ticks));
                        resolved = true;
                        self.pit = Some(pit);
                    }
                }
                IoDeviceType::DmaPrimary => {
                    if let Some(mut dma1) = self.dma1.take() {
                        dma1.write_u8(port, data, Some(self), nul_delta);
                        resolved = true;
                        self.dma1 = Some(dma1);
                    }
                }
                IoDeviceType::DmaSecondary => {
                    if let Some(mut dma2) = self.dma2.take() {
                        dma2.write_u8(port, data, Some(self), nul_delta);
                        resolved = true;
                        self.dma2 = Some(dma2);
                    }
                }
                IoDeviceType::PicPrimary => {
                    if let Some(mut pic1) = self.pic1.take() {
                        pic1.write_u8(port, data, Some(self), nul_delta);
                        resolved = true;
                        self.pic1 = Some(pic1);
                    }
                }
                IoDeviceType::PicSecondary => {
                    if let Some(mut pic2) = self.pic2.take() {
                        pic2.write_u8(port, data, Some(self), nul_delta);
                        resolved = true;
                        self.pic2 = Some(pic2);
                    }
                }
                IoDeviceType::FloppyController => {
                    if let Some(mut fdc) = self.fdc.take() {
                        fdc.write_u8(port, data, Some(self), nul_delta);
                        resolved = true;
                        self.fdc = Some(fdc);
                    }
                }
                IoDeviceType::HardDiskController => {
                    if let Some(mut hdc) = self.hdc.take() {
                        hdc.write_u8(port, data, Some(self), nul_delta);
                        resolved = true;
                        self.hdc = Some(hdc);
                    }
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        serial.write_u8(port, data, None, nul_delta);
                        resolved = true;
                    }
                }
                IoDeviceType::Parallel => {
                    if let Some(parallel) = &mut self.parallel {
                        parallel.write_u8(port, data, None, nul_delta);
                        resolved = true;
                    }
                }
                IoDeviceType::Ems => {
                    if let Some(ems) = &mut self.ems {
                        ems.write_u8(port, data, None, nul_delta);
                        resolved = true;
                    }
                }
                IoDeviceType::GamePort => {
                    if let Some(game_port) = &mut self.game_port {
                        game_port.write_u8(port, data, None, nul_delta);
                        resolved = true;
                    }
                }
                IoDeviceType::Video(vid) => {
                    if let Some(video_dispatch) = self.videocards.get_mut(&vid) {
                        match video_dispatch {
                            VideoCardDispatch::Mda(mda) => {
                                IoDevice::write_u8(mda, port, data, None, DeviceRunTimeUnit::SystemTicks(sys_ticks));
                                resolved = true;
                            }
                            VideoCardDispatch::Cga(cga) => {
                                IoDevice::write_u8(cga, port, data, None, DeviceRunTimeUnit::SystemTicks(sys_ticks));
                                resolved = true;
                            }
                            VideoCardDispatch::Tga(tga) => {
                                IoDevice::write_u8(tga, port, data, None, DeviceRunTimeUnit::SystemTicks(sys_ticks));
                                resolved = true;
                            }
                            #[cfg(feature = "ega")]
                            VideoCardDispatch::Ega(ega) => {
                                IoDevice::write_u8(ega, port, data, None, nul_delta);
                                resolved = true;
                            }
                            #[cfg(feature = "vga")]
                            VideoCardDispatch::Vga(vga) => {
                                IoDevice::write_u8(vga, port, data, None, nul_delta);
                                resolved = true;
                            }
                            VideoCardDispatch::None => {}
                        }
                    }
                }
                IoDeviceType::Sound => {
                    if let Some(adlib) = &mut self.adlib {
                        IoDevice::write_u8(adlib, port, data, None, nul_delta);
                    }
                }
                _ => {}
            }
        }

        self.io_stats
            .entry(port)
            .and_modify(|e| {
                e.1.writes += 1;
                e.1.writes_dirty = true;
            })
            .or_insert((resolved, IoDeviceStats::one_read()));
    }

    /// Return a boolean indicating whether a timer interrupt is imminent.
    /// This is intended to be called by the CPU to determine the required cycle granularity of the HLT state.
    #[inline]
    pub fn is_intr_imminent(&self) -> bool {
        self.intr_imminent
    }

    // Device accessors
    pub fn pit(&self) -> &Option<Pit> {
        &self.pit
    }

    pub fn fdc(&self) -> &Option<FloppyController> {
        &self.fdc
    }

    pub fn pit_mut(&mut self) -> &mut Option<Pit> {
        &mut self.pit
    }

    pub fn pic_mut(&mut self) -> &mut Option<Pic> {
        &mut self.pic1
    }

    pub fn ppi_mut(&mut self) -> &mut Option<Ppi> {
        &mut self.ppi
    }

    pub fn dma_mut(&mut self) -> &mut Option<DMAController> {
        &mut self.dma1
    }

    pub fn serial_mut(&mut self) -> &mut Option<SerialPortController> {
        &mut self.serial
    }

    pub fn fdc_mut(&mut self) -> &mut Option<FloppyController> {
        &mut self.fdc
    }

    pub fn hdc_mut(&mut self) -> &mut Option<HardDiskController> {
        &mut self.hdc
    }

    pub fn cart_slot_mut(&mut self) -> &mut Option<CartridgeSlot> {
        &mut self.cart_slot
    }

    pub fn game_port_mut(&mut self) -> &mut Option<GamePort> {
        &mut self.game_port
    }

    pub fn mouse_mut(&mut self) -> &mut Option<Mouse> {
        &mut self.mouse
    }

    pub fn primary_video(&self) -> Option<Box<&dyn VideoCard>> {
        if self.videocard_ids.len() > 0 {
            self.video(&self.videocard_ids[0])
        }
        else {
            None
        }
    }

    pub fn primary_video_mut(&mut self) -> Option<Box<&mut dyn VideoCard>> {
        if self.videocard_ids.len() > 0 {
            let vid = self.videocard_ids[0];
            self.video_mut(&vid)
        }
        else {
            None
        }
    }

    pub fn video(&self, vid: &VideoCardId) -> Option<Box<&dyn VideoCard>> {
        if let Some(video_dispatch) = self.videocards.get(vid) {
            match video_dispatch {
                VideoCardDispatch::Mda(mda) => Some(Box::new(mda as &dyn VideoCard)),
                VideoCardDispatch::Cga(cga) => Some(Box::new(cga as &dyn VideoCard)),
                VideoCardDispatch::Tga(tga) => Some(Box::new(tga as &dyn VideoCard)),
                #[cfg(feature = "ega")]
                VideoCardDispatch::Ega(ega) => Some(Box::new(ega as &dyn VideoCard)),
                #[cfg(feature = "vga")]
                VideoCardDispatch::Vga(vga) => Some(Box::new(vga as &dyn VideoCard)),
                VideoCardDispatch::None => None,
            }
        }
        else {
            None
        }
    }

    pub fn video_mut(&mut self, vid: &VideoCardId) -> Option<Box<&mut dyn VideoCard>> {
        if let Some(video_dispatch) = self.videocards.get_mut(vid) {
            match video_dispatch {
                VideoCardDispatch::Mda(mda) => Some(Box::new(mda as &mut dyn VideoCard)),
                VideoCardDispatch::Cga(cga) => Some(Box::new(cga as &mut dyn VideoCard)),
                VideoCardDispatch::Tga(tga) => Some(Box::new(tga as &mut dyn VideoCard)),
                #[cfg(feature = "ega")]
                VideoCardDispatch::Ega(ega) => Some(Box::new(ega as &mut dyn VideoCard)),
                #[cfg(feature = "vga")]
                VideoCardDispatch::Vga(vga) => Some(Box::new(vga as &mut dyn VideoCard)),
                VideoCardDispatch::None => None,
            }
        }
        else {
            None
        }
    }

    /// Call the provided closure in sequence with every video card defined on the bus.
    pub fn for_each_videocard<F>(&mut self, mut f: F)
    where
        F: FnMut(VideoCardInterface),
    {
        // For the moment we only support a primary video card.
        for (vid, video_dispatch) in self.videocards.iter_mut() {
            match video_dispatch {
                VideoCardDispatch::Mda(mda) => f(VideoCardInterface {
                    card: Box::new(mda as &mut dyn VideoCard),
                    id:   *vid,
                }),
                VideoCardDispatch::Cga(cga) => f(VideoCardInterface {
                    card: Box::new(cga as &mut dyn VideoCard),
                    id:   *vid,
                }),
                VideoCardDispatch::Tga(tga) => f(VideoCardInterface {
                    card: Box::new(tga as &mut dyn VideoCard),
                    id:   *vid,
                }),
                #[cfg(feature = "ega")]
                VideoCardDispatch::Ega(ega) => f(VideoCardInterface {
                    card: Box::new(ega as &mut dyn VideoCard),
                    id:   *vid,
                }),
                #[cfg(feature = "vga")]
                VideoCardDispatch::Vga(vga) => f(VideoCardInterface {
                    card: Box::new(vga as &mut dyn VideoCard),
                    id:   *vid,
                }),
                _ => {}
            };
        }
    }

    pub fn enumerate_videocards(&self) -> Vec<VideoCardId> {
        self.videocard_ids.clone()
    }

    pub fn enumerate_serial_ports(&self) -> Vec<SerialPortDescriptor> {
        self.serial
            .as_ref()
            .and_then(|serial| Some(serial.enumerate_ports()))
            .unwrap_or_default()
    }

    pub fn floppy_drive_ct(&self) -> usize {
        if let Some(fdc) = &self.fdc {
            fdc.drive_ct()
        }
        else {
            0
        }
    }

    pub fn hdd_ct(&self) -> usize {
        if let Some(hdc) = &self.hdc {
            hdc.drive_ct()
        }
        else {
            0
        }
    }

    pub fn cart_ct(&self) -> usize {
        if self.cart_slot.is_some() {
            2
        }
        else {
            0
        }
    }

    pub fn keyboard_mut(&mut self) -> Option<&mut Keyboard> {
        self.keyboard.as_mut()
    }

    pub fn dump_io_stats(&mut self) -> Vec<Vec<SyntaxToken>> {
        let mut token_vec: Vec<_> = self
            .io_stats
            .iter_mut()
            .map(|(port, stats)| {
                let mut port_desc = self.io_desc_map.get(port).unwrap_or(&String::new()).clone();
                if port_desc.len() > DEVICE_DESC_LEN {
                    port_desc.truncate(DEVICE_DESC_LEN);
                }
                else {
                    port_desc = format!("{:width$}", port_desc, width = DEVICE_DESC_LEN);
                }

                let mut tokens = Vec::new();
                tokens.push(SyntaxToken::Text(format!(
                    "{:04X}{}",
                    port,
                    if stats.0 { " " } else { "*" }
                )));
                tokens.push(SyntaxToken::Colon);
                tokens.push(SyntaxToken::Text(port_desc));
                tokens.push(SyntaxToken::Formatter(SyntaxFormatType::Tab));
                tokens.push(SyntaxToken::OpenBracket);
                tokens.push(SyntaxToken::Text(format!("{:02X}", stats.1.last_read)));
                tokens.push(SyntaxToken::CloseBracket);
                tokens.push(SyntaxToken::StateString(
                    format!("{}", stats.1.reads),
                    stats.1.reads_dirty,
                    0,
                ));
                tokens.push(SyntaxToken::Comma);
                tokens.push(SyntaxToken::Formatter(SyntaxFormatType::Tab));
                //tokens.push(SyntaxToken::Formatter(SyntaxFormatType::Tab));
                tokens.push(SyntaxToken::StateString(
                    format!("{}", stats.1.writes),
                    stats.1.writes_dirty,
                    0,
                ));

                //stats.reads_dirty = false;
                //stats.writes_dirty = false;
                (port, tokens)
            })
            .collect();

        token_vec.sort_by(|a, b| a.0.cmp(&b.0));
        token_vec.iter().map(|(_, tokens)| tokens.clone()).collect()
    }

    pub fn reset_io_stats(&mut self) {
        for (_, stats) in self.io_stats.iter_mut() {
            stats.1.last_read = 0;
            stats.1.last_write = 0;
            stats.1.reads = 0;
            stats.1.writes = 0;
            stats.1.reads_dirty = false;
            stats.1.writes_dirty = false;
        }
    }
}
