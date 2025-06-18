/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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
#![allow(dead_code)]
//! Module for modelling an abstract system bus.

//! The ISA bus is transparent enough that its particular details of are no
//! concern to the CPU.  The CPU only needs to know that it can read and write
//! to memory and I/O ports.
//! DMA-capable devices can also access memory directly.

mod dispatch;
mod io;
mod memory;
pub mod queue;

use std::{collections::VecDeque, fmt};

use crate::{
    bytequeue::*,
    cpu_common::{CpuType, LogicAnalyzer},
    device_traits::videocard::{
        ClockingMode,
        VideoCard,
        VideoCardDispatch,
        VideoCardId,
        VideoCardInterface,
        VideoCardSubType,
        VideoType,
    },
    devices::{
        a0::A0Register,
        cartridge_slots::CartridgeSlot,
        cga::CGACard,
        dma::*,
        fdc::FloppyController,
        game_port::GamePort,
        hdc::{xebec::HardDiskController, xtide::XtIdeController},
        keyboard::{KeyboardType, *},
        lotech_ems::LotechEmsCard,
        fantasy_ems::FantasyEmsCard,
        lpt_card::ParallelController,
        mda::MDACard,
        mouse::*,
        pic::*,
        pit::Pit,
        ppi::*,
        serial::*,
        sound_source::DSoundSource,
        tga::TGACard,
    },
    machine::{KeybufferEntry, MachineCheckpoint, MachinePatch},
    machine_config::{normalize_conventional_memory, MachineConfiguration, MachineDescriptor},
    machine_types::{EmsType, FdcType, HardDiskControllerType, MachineType, SerialControllerType, SerialMouseType},
    syntax_token::{SyntaxFormatType, SyntaxToken},
    tracelogger::TraceLogger,
};

#[cfg(feature = "opl")]
use crate::devices::adlib::AdLibCard;
#[cfg(feature = "ega")]
use crate::devices::ega::EGACard;
#[cfg(feature = "vga")]
use crate::devices::vga::VGACard;

#[cfg(feature = "sound")]
use crate::{
    device_traits::sounddevice::SoundDevice,
    devices::pit::SPEAKER_SAMPLE_RATE,
    machine_config::SoundChipType,
    machine_types::SoundType,
    sound::{SoundOutputConfig, SoundSourceDescriptor},
};

use crate::devices::sn76489::Sn76489;

use crate::{
    bus::dispatch::MemoryDispatch,
    devices::{conventional_memory::ConventionalMemory, hdc::jr_ide::JrIdeController},
};
use anyhow::Error;
#[cfg(feature = "sound")]
use crossbeam_channel::unbounded;
use fxhash::FxHashMap;

pub(crate) const NO_IO_BYTE: u8 = 0xFF; // This is the byte read from an unconnected IO address.
pub(crate) const OPEN_BUS_BYTE: u8 = 0xFF; // This is the byte read from an unmapped memory address.

pub(crate) const ADDRESS_SPACE: usize = 0x10_0000;
pub(crate) const DEFAULT_WAIT_STATES: u32 = 0;

pub(crate) const MMIO_MAP_SIZE: usize = 0x2000;
pub(crate) const MMIO_MAP_SHIFT: usize = 13;
pub(crate) const MMIO_MAP_LEN: usize = ADDRESS_SPACE >> MMIO_MAP_SHIFT;

pub const MEM_ROM_BIT: u8 = 0b1000_0000; // Bit to signify that this address is ROM
pub const MEM_RET_BIT: u8 = 0b0100_0000; // Bit to signify that this address is a return address for a CALL or INT
pub const MEM_BPE_BIT: u8 = 0b0010_0000; // Bit to signify that this address is associated with a breakpoint on execute
pub const MEM_BPA_BIT: u8 = 0b0001_0000; // Bit to signify that this address is associated with a breakpoint on access
pub const MEM_CP_BIT: u8 = 0b0000_1000; // Bit to signify that this address is a ROM checkpoint
pub const MEM_MMIO_BIT: u8 = 0b0000_0100; // Bit to signify that this address is MMIO mapped
pub const MEM_SW_BIT: u8 = 0b0000_0010; // Bit to signify that this address is in a stopwatch

pub const KB_UPDATE_RATE: f64 = 5000.0; // Keyboard device update rate in microseconds

pub const TIMING_TABLE_LEN: usize = 2048;

pub const IMMINENT_TIMER_INTERRUPT: u16 = 10;

pub const DEVICE_DESC_LEN: usize = 28;

pub const NULL_DELTA_US: DeviceRunTimeUnit = DeviceRunTimeUnit::Microseconds(0.0);

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
            ClockFactor::Divisor(n) => cpu_ticks.div_ceil(n as u32),
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
    pub dummy: bool,
    #[cfg(feature = "sound")]
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
    Sn76489,
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
    pub(crate) writes: usize,
    pub(crate) writes_dirty: bool,
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
    /// Read a byte from the specified port, given a delta time that may be used to 'catch up'
    /// the device state, if timing is critical. The default implementation returns NO_IO_BYTE (0xFF).
    fn read_u8(&mut self, _port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        NO_IO_BYTE
    }

    /// Write a byte to the specified port, given a delta time that may be used to 'catch up'
    /// the device state, if timing is critical. A mutable reference to the BusInterface is provided
    /// if the device needs to perform any bus operations on write.
    /// The default implementation does nothing.
    fn write_u8(
        &mut self,
        _port: u16,
        _data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        // Default implementation does nothing
    }

    /// Return the number of waits (in system ticks) to be incurred by an immediate read from the
    /// specified port.
    /// The default implementation returns 0.
    fn read_wait(&mut self, _port: u16, _delta: DeviceRunTimeUnit) -> u32 {
        0
    }

    /// Return the number of waits (in system ticks) to be incurred by an immediate write to the
    /// specified port.
    /// The default implementation returns 0.
    fn write_wait(&mut self, _port: u16, _delta: DeviceRunTimeUnit) -> u32 {
        0
    }

    /// Return a list of ports the device should service, comprised of a vector of tuples of
    /// (port description, port number).
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MmioDeviceType {
    None,
    Memory,
    Video(VideoCardId),
    Cga,
    Ega,
    Vga,
    Rom,
    Ems,
    MemoryExpansion(usize),
    Cart,
    JrIde,
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

    memory_expansions: Vec<MemoryDispatch>,
    ppi: Option<Box<Ppi>>,
    a0: Option<A0Register>,
    a0_data: u8,
    nmi_latch: bool,
    pit: Option<Pit>,
    speaker_src: Option<usize>,
    dma_counter: u16,
    dma1: Option<Box<DMAController>>,
    dma2: Option<Box<DMAController>>,
    pic1: Option<Box<Pic>>,
    pic2: Option<Pic>,
    serial: Option<SerialPortController>,
    parallel: Option<ParallelController>,
    fdc: Option<Box<FloppyController>>,
    hdc: Option<Box<HardDiskController>>,
    xtide: Option<Box<XtIdeController>>,
    jride: Option<Box<JrIdeController>>,
    mouse: Option<Mouse>,
    ems: Option<LotechEmsCard>,
    fantasy_ems: Option<FantasyEmsCard>,
    cart_slot: Option<CartridgeSlot>,
    game_port: Option<GamePort>,
    #[cfg(feature = "opl")]
    adlib: Option<AdLibCard>,
    sound_source: Option<DSoundSource>,
    sn76489: Option<Sn76489>,

    videocards:    FxHashMap<VideoCardId, VideoCardDispatch>,
    videocard_ids: Vec<VideoCardId>,

    cycles_to_ticks:   [u32; 256], // TODO: Benchmarks don't show any faster than raw multiplication. It's not slower either though.
    pit_ticks_advance: u32, // We can schedule extra PIT ticks to add when run() occurs. This is generally used for PIT phase offset adjustment.

    do_title_hacks: bool,
    timer_trigger1_armed: bool,
    timer_trigger2_armed: bool,

    cga_tick_accum: u32,
    tga_tick_accum: u32,
    kb_us_accum: f64,
    refresh_enabled: bool,
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

            memory_expansions: Vec::new(),
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
            xtide: None,
            jride: None,
            mouse: None,
            ems: None,
            fantasy_ems: None,
            cart_slot: None,
            game_port: None,
            #[cfg(feature = "opl")]
            adlib: None,
            sound_source: None,
            sn76489: None,
            videocards: FxHashMap::default(),
            videocard_ids: Vec::new(),

            cycles_to_ticks:   [0; 256],
            pit_ticks_advance: 0,

            do_title_hacks: false,
            timer_trigger1_armed: false,
            timer_trigger2_armed: false,

            cga_tick_accum: 0,
            tga_tick_accum: 0,
            kb_us_accum: 0.0,
            refresh_active: false,
            refresh_enabled: false,

            terminal_port: None,
        }
    }
}

impl BusInterface {
    // pub fn new(
    //     cpu_factor: ClockFactor,
    //     machine_desc: MachineDescriptor,
    //     keyboard_type: KeyboardType,
    // ) -> BusInterface {
    //     let mut timing_table = Box::new([TimingTableEntry { sys_ticks: 0, us: 0.0 }; TIMING_TABLE_LEN]);
    //     Self::update_timing_table(&mut timing_table, cpu_factor, machine_desc.system_crystal);
    //
    //     BusInterface {
    //         cpu_factor,
    //         timing_table,
    //         machine_desc: Some(machine_desc),
    //         keyboard_type,
    //         refresh_enabled,
    //         ..BusInterface::default()
    //     }
    // }

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
                ClockFactor::Divisor(n) => (cycles as u32).div_ceil(n as u32),
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
    pub fn install_checkpoints(&mut self, checkpoints: &[MachineCheckpoint]) {
        for checkpoint in checkpoints.iter() {
            self.memory_mask[checkpoint.addr as usize & 0xFFFFF] |= MEM_CP_BIT;
        }
    }

    pub fn install_patch_checkpoints(&mut self, patches: &[MachinePatch]) {
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
        log::debug!("install_patch(): Patch of {} bytes installed!", patch_size);
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
    /// The [MemoryMappedDevice] trait's read & write methods will be called instead for memory in
    /// the range specified by the device's [MemRangeDescriptor].
    ///
    /// Ranges must have a granularity no less than MMIO_MAP_SIZE (8K).
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

        log::debug!(
            "register_map: Registering memory map for device: {:?} at {:#X} size: {:#X}, ({} segments)",
            device,
            mem_descriptor.address,
            mem_descriptor.size,
            map_segs
        );
        for i in 0..map_segs {
            self.mmio_map_fast[(mem_descriptor.address >> MMIO_MAP_SHIFT) + i] = device;
        }

        self.mmio_map.push((mem_descriptor, device));
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
    pub(crate) fn cpu_cycles_to_system_ticks(&self, cycles: u32) -> u32 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => cycles * (n as u32),
            ClockFactor::Multiplier(n) => cycles / (n as u32),
        }
    }

    #[inline]
    /// Convert a count of system clock ticks to CPU cycles based on the current CPU
    /// clock divisor. If a clock Divisor is set, the dividend will be rounded upwards.
    pub(crate) fn system_ticks_to_cpu_cycles(&self, ticks: u32) -> u32 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => ticks.div_ceil(n as u32),
            //ClockFactor::Divisor(n) => (ticks + (n as u32) - 1) / (n as u32),
            ClockFactor::Multiplier(n) => ticks * (n as u32),
        }
    }

    pub fn dump_ivt_tokens(&mut self) -> Vec<Vec<SyntaxToken>> {
        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        for v in 0..256 {
            let mut ivr_vec = Vec::new();
            let (ip, _) = self.read_u16((v * 4) as usize, 0).unwrap();
            let (cs, _) = self.read_u16(((v * 4) + 2) as usize, 0).unwrap();

            ivr_vec.push(SyntaxToken::Text(format!("{:02X}h", v)));
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
        #[cfg(feature = "sound")] sound_config: &SoundOutputConfig,
        terminal_port: Option<u16>,
        refresh_enabled: bool,
    ) -> Result<InstalledDevicesResult, Error> {
        #[allow(unused_mut)]
        let mut installed_devices = InstalledDevicesResult::new();
        let video_frame_debug = false;
        let clock_mode = ClockingMode::Default;

        if let Some(terminal_port) = terminal_port {
            log::debug!("Terminal port set to: {:04X}", terminal_port);
        }
        self.terminal_port = terminal_port;
        self.refresh_enabled = refresh_enabled;

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

        // Create memory expansion cards.

        for (ec_idx, expansion_card_config) in machine_config.conventional_expansion.iter().enumerate() {
            log::debug!(
                "Installing memory expansion card({}) address: {:05X} size: {:05X}",
                ec_idx,
                expansion_card_config.address,
                expansion_card_config.size
            );
            let expansion_card = ConventionalMemory::new(
                expansion_card_config.address as usize,
                expansion_card_config.size as usize,
                expansion_card_config.wait_states,
                false,
            );
            add_mmio_device!(
                self,
                expansion_card,
                MmioDeviceType::MemoryExpansion(self.memory_expansions.len())
            );
            self.memory_expansions
                .push(MemoryDispatch::Conventional(expansion_card));
        }

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
            self.ppi = Some(Box::new(Ppi::new(
                machine_desc.machine_type,
                conventional_memory,
                false,
                video_types,
                num_floppies,
            )));
            // Add PPI ports to io_map

            add_io_device!(self, self.ppi.as_mut().unwrap(), IoDeviceType::Ppi);
        }

        // Create the crossbeam channel for the PIT to send sound samples to the sound output thread.
        #[cfg(feature = "sound")]
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
        #[cfg(not(feature = "sound"))]
        let pit_sample_sender = None;

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
        self.dma1 = Some(Box::new(dma1));

        // Create PIC. One PIC will always exist.
        let pic1 = Pic::new();
        // Add PIC ports to io_map
        add_io_device!(self, pic1, IoDeviceType::PicPrimary);
        self.pic1 = Some(Box::new(pic1));

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
            //let floppy_ct = fdc_config.drive.len();
            let fdc_type = fdc_config.fdc_type;

            // Create the correct kind of FDC (currently only NEC supported)
            match fdc_type {
                FdcType::IbmNec | FdcType::IbmPCJrNec => {
                    let fdc = FloppyController::new(fdc_type, fdc_config.drive.clone());
                    // Add FDC ports to io_map
                    add_io_device!(self, fdc, IoDeviceType::FloppyController);
                    self.fdc = Some(Box::new(fdc));
                }
            }
        }

        // Create a HardDiskController if specified
        if let Some(hdc_config) = &machine_config.hdc {
            match hdc_config.hdc_type {
                HardDiskControllerType::IbmXebec => {
                    // TODO: Get the correct drive type from the specified VHD...?
                    let hdc = HardDiskController::new(2);
                    // Add HDC ports to io_map
                    add_io_device!(self, hdc, IoDeviceType::HardDiskController);
                    self.hdc = Some(Box::new(hdc));
                }
                HardDiskControllerType::XtIde => {
                    let xtide = XtIdeController::new(None, 2);
                    add_io_device!(self, xtide, IoDeviceType::HardDiskController);
                    self.xtide = Some(Box::new(xtide));
                }
                HardDiskControllerType::JrIde => {
                    let jride = JrIdeController::new(None, None, 2);
                    add_io_device!(self, jride, IoDeviceType::HardDiskController);
                    add_mmio_device!(self, jride, MmioDeviceType::JrIde);
                    self.jride = Some(Box::new(jride));
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
        else if !machine_config.parallel.is_empty() {
            if machine_config.parallel.len() > 1 {
                log::warn!(
                    "Support for multiple parallel controllers is not implemented. Only the first parallel controller will be created."
                );
            }

            if let Some(controller) = machine_config.parallel.first() {
                log::debug!("Creating parallel port...");

                if let Some(port) = controller.port.first() {
                    let parallel = ParallelController::new(Some(port.io_base as u16));

                    // Add Parallel Port ports to io_map
                    add_io_device!(self, parallel, IoDeviceType::Parallel);
                    self.parallel = Some(parallel);
                }
            }
            else {
                log::error!("Parallel controller was specified with no ports!");
            }
        }

        // If we installed a parallel card, attach any specified parallel port DAC
        if let Some(_parallel) = &self.parallel {
            // Add parallel port to installed devices
            #[cfg(feature = "sound")]
            {
                let mut parallel_dac = machine_config.sound.clone();
                parallel_dac.retain(|s| s.sound_type.is_parallel());

                if parallel_dac.len() > 1 {
                    log::warn!("More than one parallel DAC specified. Only the first parallel DAC will be created.");
                }

                if !parallel_dac.is_empty() {
                    match parallel_dac[0].sound_type {
                        SoundType::SoundSource => {
                            // Create audio sample channel
                            let (sample_sender, sample_receiver) = unbounded();

                            let device_channel = _parallel.device_channel();
                            let sound_source = DSoundSource::new(device_channel, sample_sender);
                            installed_devices.sound_sources.push(SoundSourceDescriptor::new(
                                "Sound Source",
                                sound_source.sample_rate(),
                                1,
                                sample_receiver,
                            ));

                            self.sound_source = Some(sound_source);
                        }
                        _ => {
                            unimplemented!();
                        }
                    }
                }
            }
        }

        // Create a Serial card if specified
        if let Some(serial_config) = machine_config.serial.first() {
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
                        let mouse = Mouse::new(serial_mouse_config.port as usize, None, None);
                        self.mouse = Some(mouse);
                    }
                }
            }
        }

        // Create an EMS board if specified
        if let Some(ems_config) = &machine_config.ems {
            #[allow(irrefutable_let_patterns)]
            if let EmsType::LoTech2MB = ems_config.ems_type {
                // Add EMS ports to io_map
                let ems = LotechEmsCard::new(Some(ems_config.io_base), Some(ems_config.window as usize));
                add_io_device!(self, ems, IoDeviceType::Ems);
                add_mmio_device!(self, ems, MmioDeviceType::Ems);
                self.ems = Some(ems);
            }

            if let EmsType::Fantasy4MB = ems_config.ems_type {
                // Add EMS ports to io_map
                let fantasy_ems = FantasyEmsCard::new(Some(ems_config.window as usize), Some(16384)); //todo pull this from a new type of config
                add_io_device!(self, fantasy_ems, IoDeviceType::Ems);
                add_mmio_device!(self, fantasy_ems, MmioDeviceType::Ems);
                self.fantasy_ems = Some(fantasy_ems);
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
        let mut game_port_layout = None;
        // Is there one onboard?
        if machine_desc.game_port.is_some() {
            game_port_addr = machine_desc.game_port;
        }
        // Is there one in the machine config?
        else if let Some(game_port) = &machine_config.game_port {
            game_port_addr = Some(game_port.io_base);

            // Take the controller layout from the main machine config first, as this is set by
            // the main configuration file and command line arguments.
            // If not specified, fall back to the port specified in the game port configuration.
            if let Some(config_layout) = machine_config.controller_layout {
                game_port_layout = Some(config_layout);
            }
            else if let Some(port_layout) = game_port.controller_layout {
                game_port_layout = Some(port_layout);
            }
        }
        // Either way, install it if present
        if let Some(game_port_addr) = game_port_addr {
            let game_port = GamePort::new(Some(game_port_addr), game_port_layout);
            add_io_device!(self, game_port, IoDeviceType::GamePort);
            self.game_port = Some(game_port);
        }

        // Create sound chips
        #[cfg(feature = "sound")]
        if let Some((chip, io_base, factor)) = machine_desc.onboard_sound {
            match chip {
                SoundChipType::Sn76489 => {
                    let (s, r) = unbounded();
                    installed_devices.sound_sources.push(SoundSourceDescriptor::new(
                        "SN76489 Sound Chip",
                        sound_config.sample_rate,
                        1,
                        r,
                    ));
                    let sn76489 = Sn76489::new(io_base, machine_desc.system_crystal, factor, s);
                    add_io_device!(self, sn76489, IoDeviceType::Sn76489);
                    self.sn76489 = Some(sn76489);
                }
                _ => {
                    log::warn!("Sound chip {:?} not implemented", chip);
                }
            }
        }

        // Create sound cards
        #[cfg(feature = "sound")]
        for card in machine_config.sound.iter() {
            #[cfg(feature = "opl")]
            {
                #[allow(clippy::single_match)]
                match card.sound_type {
                    SoundType::AdLib => {
                        // Create an AdLib card.
                        let (s, r) = unbounded();
                        installed_devices.sound_sources.push(SoundSourceDescriptor::new(
                            "AdLib Music Synthesizer",
                            sound_config.sample_rate,
                            2,
                            r,
                        ));
                        let adlib = AdLibCard::new(card.io_base.unwrap_or(0x388), 48000, s);
                        add_io_device!(self, adlib, IoDeviceType::Sound);
                        self.adlib = Some(adlib);
                    }
                    _ => {}
                }
            }
        }

        // Create video cards
        for (i, card) in machine_config.video.iter().enumerate() {
            let video_id = VideoCardId {
                idx:   i,
                vtype: card.video_type,
            };

            log::debug!("Creating video card of type: {:?}", card.video_type);
            let video_dispatch = match card.video_type {
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
                    VideoCardDispatch::Mda(Box::new(mda))
                }
                VideoType::CGA => {
                    let cga = CGACard::new(TraceLogger::None, clock_mode, video_frame_debug);
                    add_io_device!(self, cga, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, cga, MmioDeviceType::Video(video_id));
                    VideoCardDispatch::Cga(Box::new(cga))
                }
                VideoType::TGA => {
                    // Subtype can be Tandy1000 or PCJr
                    let subtype = card.video_subtype.unwrap_or(VideoCardSubType::Tandy1000);
                    let tga = TGACard::new(subtype, TraceLogger::None, clock_mode, video_frame_debug);
                    add_io_device!(self, tga, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, tga, MmioDeviceType::Video(video_id));
                    VideoCardDispatch::Tga(Box::new(tga))
                }
                #[cfg(feature = "ega")]
                VideoType::EGA => {
                    let ega = EGACard::new(TraceLogger::None, clock_mode, video_frame_debug, card.dip_switch);
                    add_io_device!(self, ega, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, ega, MmioDeviceType::Video(video_id));
                    VideoCardDispatch::Ega(Box::new(ega))
                }
                #[cfg(feature = "vga")]
                VideoType::VGA => {
                    let vga = VGACard::new(TraceLogger::None, clock_mode, video_frame_debug, card.dip_switch);
                    add_io_device!(self, vga, IoDeviceType::Video(video_id));
                    add_mmio_device!(self, vga, MmioDeviceType::Video(video_id));
                    VideoCardDispatch::Vga(Box::new(vga))
                }
                #[allow(unreachable_patterns)]
                _ => {
                    panic!(
                        "card type {:?} not implemented or feature not compiled",
                        card.video_type
                    );
                }
            };

            self.videocards.insert(video_id, video_dispatch);
            self.videocard_ids.push(video_id);
        }

        self.machine_desc = Some(*machine_desc);
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
        mut logic_analyzer: Option<&mut LogicAnalyzer>,
    ) -> Option<DeviceEvent> {
        let mut event = None;

        //let analyzer_ref = logic_analyzer.as_mut();

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

        // Currently the timer can only update the logic analyzer if it is ticked via system ticks.
        if let Some(_crystal) = self.machine_desc.unwrap().timer_crystal {
            pit.run(self, DeviceRunTimeUnit::Microseconds(us), None);
        }
        else {
            // We can only adjust phase of PIT if we are using system ticks, and that's okay. It's only really useful
            // on an 5150/5160.

            pit.run(
                self,
                DeviceRunTimeUnit::SystemTicks(sys_ticks + self.pit_ticks_advance),
                logic_analyzer.as_deref_mut(),
            );
            self.pit_ticks_advance = 0;
        }

        if self.refresh_enabled {
            self.handle_refresh_scheduling(&mut pit, &mut event);
        }

        // Save current count info.
        let (_pit_reload_value, pit_counting_element, pit_counting) = pit.get_channel_count(0);

        // Set imminent interrupt flag. The CPU can use this as a hint to adjust cycles for halt instructions - using
        // more cycles when an interrupt is not imminent, and one cycle when it is. This allows for cycle-precise wake
        // from halt.
        self.intr_imminent = pit_counting & (pit_counting_element <= IMMINENT_TIMER_INTERRUPT);

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
        // Run the XT-IDE controller, passing it DMA controller while DMA is still unattached.
        // (No, it doesn't need the DMA controller. A future version might)
        if let Some(mut xtide) = self.xtide.take() {
            xtide.run(&mut dma1, self, us);
            self.xtide = Some(xtide);
        }
        // Run the JR-IDE controller, passing it DMA controller while DMA is still unattached.
        // (No, it doesn't need the DMA controller. A future version might)
        if let Some(mut jride) = self.jride.take() {
            jride.run(&mut dma1, self, us);
            self.jride = Some(jride);
        }
        // Run the DMA controller.
        dma1.run(self);

        // Replace the DMA controller.
        self.dma1 = Some(dma1);

        // Run the serial port and mouse.
        if let Some(serial) = &mut self.serial {
            serial.run(self.pic1.as_mut().unwrap(), us);

            if let Some(mouse) = &mut self.mouse {
                mouse.run(serial, us);
            }
        }

        // Run the parallel port
        if let Some(parallel) = &mut self.parallel {
            parallel.run(self.pic1.as_mut().unwrap(), us);
        }

        // Run the game port
        if let Some(game_port) = &mut self.game_port {
            game_port.run(us);
        }

        // Run the adlib card
        #[cfg(feature = "opl")]
        if let Some(adlib) = &mut self.adlib {
            adlib.run(us);
        }

        // Run the Sound Source
        if let Some(sound_source) = &mut self.sound_source {
            sound_source.run(us);
        }

        // Run the SN76489 sound chip
        if let Some(sn76489) = &mut self.sn76489 {
            sn76489.run(DeviceRunTimeUnit::SystemTicks(sys_ticks));
        }

        // Run all video cards
        for (_vid, video_dispatch) in self.videocards.iter_mut() {
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

        // Commit logic analyzer if present
        logic_analyzer.as_mut().map(|la| la.commit());
        event
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

            // DRAM refresh DMA counter has changed. Update the CPU's DRAM refresh simulation.
            log::trace!(
                "DRAM refresh DMA counter updated: reload: {}, count: {}, adj: +{}, retrigger: {}",
                dma_count_register,
                dma_counting_element,
                dma_add_ticks,
                retriggers
            );

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

        // Reset fdc
        if let Some(fdc) = self.fdc.as_mut() {
            fdc.reset();
        }

        // Reset video cards
        let vids: Vec<_> = self.videocards.keys().cloned().collect();
        for vid in vids {
            if let Some(video) = self.video_mut(&vid) {
                video.reset()
            }
        }

        // Reset the A0 register
        if let Some(a0) = self.a0.as_mut() {
            a0.reset();
        }
    }

    /// Call the reset methods for devices to be reset on warm boot
    pub fn reset_devices_warm(&mut self) {
        self.pit.as_mut().unwrap().reset();
        //self.pic1.as_mut().unwrap().reset();
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

    pub fn fdc(&self) -> &Option<Box<FloppyController>> {
        &self.fdc
    }

    pub fn pit_mut(&mut self) -> &mut Option<Pit> {
        &mut self.pit
    }

    pub fn pic(&self) -> &Option<Box<Pic>> {
        &self.pic1
    }

    pub fn pic_mut(&mut self) -> &mut Option<Box<Pic>> {
        &mut self.pic1
    }

    pub fn ppi_mut(&mut self) -> &mut Option<Box<Ppi>> {
        &mut self.ppi
    }

    pub fn dma_mut(&mut self) -> &mut Option<Box<DMAController>> {
        &mut self.dma1
    }

    pub fn serial_mut(&mut self) -> &mut Option<SerialPortController> {
        &mut self.serial
    }

    pub fn fdc_mut(&mut self) -> &mut Option<Box<FloppyController>> {
        &mut self.fdc
    }

    pub fn hdc_mut(&mut self) -> &mut Option<Box<HardDiskController>> {
        &mut self.hdc
    }

    pub fn xtide_mut(&mut self) -> &mut Option<Box<XtIdeController>> {
        &mut self.xtide
    }

    pub fn jride_mut(&mut self) -> &mut Option<Box<JrIdeController>> {
        &mut self.jride
    }

    pub fn cart_slot_mut(&mut self) -> &mut Option<CartridgeSlot> {
        &mut self.cart_slot
    }

    pub fn game_port(&self) -> &Option<GamePort> {
        &self.game_port
    }

    pub fn game_port_mut(&mut self) -> &mut Option<GamePort> {
        &mut self.game_port
    }

    pub fn mouse_mut(&mut self) -> &mut Option<Mouse> {
        &mut self.mouse
    }

    pub fn sn_chip(&self) -> &Option<Sn76489> {
        &self.sn76489
    }

    pub fn sn_chip_mut(&mut self) -> &mut Option<Sn76489> {
        &mut self.sn76489
    }

    pub fn primary_video(&self) -> Option<Box<&dyn VideoCard>> {
        if !self.videocard_ids.is_empty() {
            self.video(&self.videocard_ids[0])
        }
        else {
            None
        }
    }

    pub fn primary_video_mut(&mut self) -> Option<Box<&mut dyn VideoCard>> {
        if !self.videocard_ids.is_empty() {
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
                VideoCardDispatch::Mda(mda) => Some(Box::new(&**mda as &dyn VideoCard)),
                VideoCardDispatch::Cga(cga) => Some(Box::new(&**cga as &dyn VideoCard)),
                VideoCardDispatch::Tga(tga) => Some(Box::new(&**tga as &dyn VideoCard)),
                #[cfg(feature = "ega")]
                VideoCardDispatch::Ega(ega) => Some(Box::new(&**ega as &dyn VideoCard)),
                #[cfg(feature = "vga")]
                VideoCardDispatch::Vga(vga) => Some(Box::new(&**vga as &dyn VideoCard)),
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
                VideoCardDispatch::Mda(mda) => Some(Box::new(&mut **mda as &mut dyn VideoCard)),
                VideoCardDispatch::Cga(cga) => Some(Box::new(&mut **cga as &mut dyn VideoCard)),
                VideoCardDispatch::Tga(tga) => Some(Box::new(&mut **tga as &mut dyn VideoCard)),
                #[cfg(feature = "ega")]
                VideoCardDispatch::Ega(ega) => Some(Box::new(&mut **ega as &mut dyn VideoCard)),
                #[cfg(feature = "vga")]
                VideoCardDispatch::Vga(vga) => Some(Box::new(&mut **vga as &mut dyn VideoCard)),
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
                    card: Box::new(&mut **mda as &mut dyn VideoCard),
                    id:   *vid,
                }),
                VideoCardDispatch::Cga(cga) => f(VideoCardInterface {
                    card: Box::new(&mut **cga as &mut dyn VideoCard),
                    id:   *vid,
                }),
                VideoCardDispatch::Tga(tga) => f(VideoCardInterface {
                    card: Box::new(&mut **tga as &mut dyn VideoCard),
                    id:   *vid,
                }),
                #[cfg(feature = "ega")]
                VideoCardDispatch::Ega(ega) => f(VideoCardInterface {
                    card: Box::new(&mut **ega as &mut dyn VideoCard),
                    id:   *vid,
                }),
                #[cfg(feature = "vga")]
                VideoCardDispatch::Vga(vga) => f(VideoCardInterface {
                    card: Box::new(&mut **vga as &mut dyn VideoCard),
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
            .map(|serial| serial.enumerate_ports())
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
        else if let Some(xtide) = &self.xtide {
            xtide.drive_ct()
        }
        else if let Some(jride) = &self.jride {
            jride.drive_ct()
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

        token_vec.sort_by(|a, b| a.0.cmp(b.0));
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
