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

    devices::dma.rs

    Implements the Intel 8237 DMA Controller

*/

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    cpu_common::LogicAnalyzer,
};

pub const DMA_CHANNEL_0_ADDR_PORT: u16 = 0x00; // R/W
pub const DMA_CHANNEL_0_WC_PORT: u16 = 0x01; // R/W
pub const DMA_CHANNEL_1_ADDR_PORT: u16 = 0x02; // R/W
pub const DMA_CHANNEL_1_WC_PORT: u16 = 0x03; // R/W
pub const DMA_CHANNEL_2_ADDR_PORT: u16 = 0x04; // R/W
pub const DMA_CHANNEL_2_WC_PORT: u16 = 0x05; // R/W
pub const DMA_CHANNEL_3_ADDR_PORT: u16 = 0x06; // R/W
pub const DMA_CHANNEL_3_WC_PORT: u16 = 0x07; // R/W

// Status/Command register depending on read/write
pub const DMA_STATUS_REGISTER: u16 = 0x08; // R
pub const DMA_COMMAND_REGISTER: u16 = 0x08; // W
pub const DMA_WRITE_REQ_REGISTER: u16 = 0x09; // W
pub const DMA_CHANNEL_MASK_REGISTER: u16 = 0x0A; // R/W
pub const DMA_CHANNEL_MODE_REGISTER: u16 = 0x0B; // R/W
pub const DMA_CLEAR_FLIPFLOP: u16 = 0x0C; // W

// Read Temp/Master Clear register depending on read/write
pub const DMA_READ_TEMP_REGISTER: u16 = 0x0D; // R
pub const DMA_MASTER_CLEAR: u16 = 0x0D; // W
pub const DMA_CLEAR_MASK_REGISTER: u16 = 0x0E; // W
pub const DMA_WRITE_MASK_REGISTER: u16 = 0x0F; // W

// The following page registers are not in logical order (why?)
pub const DMA_CHANNEL_0_PAGE_REGISTER: u16 = 0x87; // R/W
pub const DMA_CHANNEL_1_PAGE_REGISTER: u16 = 0x83; // R/W
pub const DMA_CHANNEL_2_PAGE_REGISTER: u16 = 0x81; // R/W
pub const DMA_CHANNEL_3_PAGE_REGISTER: u16 = 0x82; // R/W

// Control byte bit fields - not all of these are implemented
pub const DMA_COMMAND_MEM_TO_MEM: u8 = 0x01;
pub const DMA_COMMAND_CHANNEL_0_HOLD: u8 = 0x02;
pub const DMA_COMMAND_DISABLE: u8 = 0x04;
pub const DMA_COMMAND_TIMING: u8 = 0x08;
pub const DMA_COMMAND_PRIORITY: u8 = 0x10;

pub const DMA_CHANNEL_COUNT: usize = 4;

pub enum TimingMode {
    NormalTiming,
    CompressedTiming,
}

pub enum PriorityMode {
    Fixed,
    Rotating,
}

#[derive(Debug)]
#[derive(Default)]
pub enum ServiceMode {
    #[default]
    Demand,
    Single,
    Block,
    Cascade,
}
#[derive(Debug)]
#[derive(Default)]
pub enum AddressMode {
    #[default]
    Increment,
    Decrement,
}

#[derive(Debug)]
#[derive(Default)]
pub enum TransferType {
    #[default]
    Verify,
    Write,
    Read,
    Illegal,
}


#[derive(Default)]
pub struct DMAChannel {
    current_address_reg: u16,
    current_word_count_reg: u16,
    base_address_reg: u16,
    base_word_count_reg: u16,

    mode_reg: u8,
    auto_init: bool,
    service_mode: ServiceMode,
    address_mode: AddressMode,
    transfer_type: TransferType,
    terminal_count: bool,
    terminal_count_reached: bool,
    request: bool,
    masked: bool,
    page: u8,
}

#[derive(Default)]
pub struct DMAChannelStringState {
    pub current_address_reg: String,
    pub current_word_count_reg: String,
    pub base_address_reg: String,
    pub base_word_count_reg: String,

    pub service_mode: String,
    pub address_mode: String,
    pub transfer_type: String,
    pub auto_init: String,
    pub terminal_count: String,
    pub terminal_count_reached: String,
    pub masked: String,
    pub page: String,
}

#[derive(Default)]
pub struct DMAControllerStringState {
    pub enabled: String,
    pub flipflop: String,
    pub dreq: String,
    pub dma_channel_state: Vec<DMAChannelStringState>,
}
pub struct DMAController {
    enabled: bool,
    mem_to_mem_enabled: bool,
    channel_0_hold_enabled: bool,
    timing_mode: TimingMode,
    priority_mode: PriorityMode,

    flipflop: bool,
    channels: [DMAChannel; 4],

    command_register: u8,
    request_reg: u8,
    status_reg: u8,
    temp_reg: u8,

    dreq: bool,
}

impl IoDevice for DMAController {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port {
            DMA_CHANNEL_0_ADDR_PORT => self.handle_addr_port_read(0),
            DMA_CHANNEL_1_ADDR_PORT => self.handle_addr_port_read(1),
            DMA_CHANNEL_2_ADDR_PORT => self.handle_addr_port_read(2),
            DMA_CHANNEL_3_ADDR_PORT => self.handle_addr_port_read(3),
            DMA_CHANNEL_0_WC_PORT => self.handle_wc_port_read(0),
            DMA_CHANNEL_1_WC_PORT => self.handle_wc_port_read(1),
            DMA_CHANNEL_2_WC_PORT => self.handle_wc_port_read(2),
            DMA_CHANNEL_3_WC_PORT => self.handle_wc_port_read(3),
            DMA_STATUS_REGISTER => self.handle_status_register_read(),
            DMA_READ_TEMP_REGISTER => self.handle_temp_register_read(),
            DMA_CHANNEL_0_PAGE_REGISTER => self.handle_page_register_read(0),
            DMA_CHANNEL_1_PAGE_REGISTER => self.handle_page_register_read(1),
            DMA_CHANNEL_2_PAGE_REGISTER => self.handle_page_register_read(2),
            DMA_CHANNEL_3_PAGE_REGISTER => self.handle_page_register_read(3),
            _ => {
                log::warn!("Read from unhandled DMA Controller port: {:02X}", port);
                0
            }
        }
    }

    fn write_u8(
        &mut self,
        port: u16,
        data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        match port {
            DMA_CHANNEL_0_ADDR_PORT => {
                self.handle_addr_port_write(0, data);
            }
            DMA_CHANNEL_1_ADDR_PORT => {
                self.handle_addr_port_write(1, data);
            }
            DMA_CHANNEL_2_ADDR_PORT => {
                self.handle_addr_port_write(2, data);
            }
            DMA_CHANNEL_3_ADDR_PORT => {
                self.handle_addr_port_write(3, data);
            }
            DMA_CHANNEL_0_WC_PORT => {
                self.handle_wc_port_write(0, data);
            }
            DMA_CHANNEL_1_WC_PORT => {
                self.handle_wc_port_write(1, data);
            }
            DMA_CHANNEL_2_WC_PORT => {
                self.handle_wc_port_write(2, data);
            }
            DMA_CHANNEL_3_WC_PORT => {
                self.handle_wc_port_write(3, data);
            }
            DMA_COMMAND_REGISTER => {
                self.handle_command_register_write(data);
            }
            DMA_WRITE_REQ_REGISTER => {
                self.handle_write_req_register(data);
            }
            DMA_CHANNEL_MASK_REGISTER => {
                self.handle_channel_mask_register_write(data);
            }
            DMA_CHANNEL_MODE_REGISTER => {
                self.handle_channel_mode_register_write(data);
            }
            DMA_CLEAR_FLIPFLOP => {
                self.handle_clear_flopflop();
            }
            DMA_MASTER_CLEAR => {
                self.handle_master_clear();
            }
            DMA_CLEAR_MASK_REGISTER => {
                self.handle_clear_mask_register();
            }
            DMA_WRITE_MASK_REGISTER => {
                self.handle_write_mask_register(data);
            }
            DMA_CHANNEL_0_PAGE_REGISTER => {
                self.handle_page_register_write(0, data);
            }
            DMA_CHANNEL_1_PAGE_REGISTER => {
                self.handle_page_register_write(1, data);
            }
            DMA_CHANNEL_2_PAGE_REGISTER => {
                self.handle_page_register_write(2, data);
            }
            DMA_CHANNEL_3_PAGE_REGISTER => {
                self.handle_page_register_write(3, data);
            }
            _ => {
                log::warn!("Write to unhandled DMA register.")
            }
        }
        if port == DMA_COMMAND_REGISTER {}
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![
            (String::from("DMA Channel 0 Address"), DMA_CHANNEL_0_ADDR_PORT),
            (String::from("DMA Channel 0 Word Count"), DMA_CHANNEL_0_WC_PORT),
            (String::from("DMA Channel 1 Address"), DMA_CHANNEL_1_ADDR_PORT),
            (String::from("DMA Channel 1 Word Count"), DMA_CHANNEL_1_WC_PORT),
            (String::from("DMA Channel 2 Address"), DMA_CHANNEL_2_ADDR_PORT),
            (String::from("DMA Channel 2 Word Count"), DMA_CHANNEL_2_WC_PORT),
            (String::from("DMA Channel 3 Address"), DMA_CHANNEL_3_ADDR_PORT),
            (String::from("DMA Channel 3 Word Count"), DMA_CHANNEL_3_WC_PORT),
            (String::from("DMA Status/Command Register"), DMA_STATUS_REGISTER),
            (String::from("DMA Write Request Register"), DMA_WRITE_REQ_REGISTER),
            (String::from("DMA Channel Mask Register"), DMA_CHANNEL_MASK_REGISTER),
            (String::from("DMA Channel Mode Register"), DMA_CHANNEL_MODE_REGISTER),
            (String::from("DMA Clear Flip-Flop"), DMA_CLEAR_FLIPFLOP),
            (String::from("DMA Read Temp Register"), DMA_READ_TEMP_REGISTER),
            (String::from("DMA Master Clear"), DMA_MASTER_CLEAR),
            (String::from("DMA Clear Mask Register"), DMA_CLEAR_MASK_REGISTER),
            (String::from("DMA Write Mask Register"), DMA_WRITE_MASK_REGISTER),
            (String::from("DMA Channel 0 Page Register"), DMA_CHANNEL_0_PAGE_REGISTER),
            (String::from("DMA Channel 1 Page Register"), DMA_CHANNEL_1_PAGE_REGISTER),
            (String::from("DMA Channel 2 Page Register"), DMA_CHANNEL_2_PAGE_REGISTER),
            (String::from("DMA Channel 3 Page Register"), DMA_CHANNEL_3_PAGE_REGISTER),
        ]
    }
}

impl DMAController {
    pub fn new() -> Self {
        Self {
            enabled: true,
            mem_to_mem_enabled: true,
            channel_0_hold_enabled: false,
            timing_mode: TimingMode::NormalTiming,
            priority_mode: PriorityMode::Fixed,

            flipflop: false,
            channels: [
                DMAChannel::default(),
                DMAChannel::default(),
                DMAChannel::default(),
                DMAChannel::default(),
            ],
            command_register: 0,
            request_reg: 0,
            status_reg: 0,
            temp_reg: 0,

            dreq: false,
        }
    }

    /// Reset the DMA controller
    pub fn reset(&mut self) {
        // TODO:
    }

    pub fn handle_addr_port_read(&mut self, channel: usize) -> u8 {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        let chan = &mut self.channels[channel];

        // Send MSB when flipflop set, LSB when clear
        let byte = match self.flipflop {
            true => ((chan.current_address_reg >> 8) & 0xFF) as u8,
            false => (chan.current_address_reg & 0xFF) as u8,
        };
        // Flop the flop on read
        self.flipflop = !self.flipflop;
        // Return the byte
        byte
    }

    // Set the base address of a DMA channel transfer
    pub fn handle_addr_port_write(&mut self, channel: usize, data: u8) {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        let chan = &mut self.channels[channel];

        // Set MSB when flipflop set, LSB when clear
        match self.flipflop {
            true => {
                chan.base_address_reg = (chan.base_address_reg & 0xFF) | ((data as u16) << 8);
                chan.current_address_reg = (chan.current_address_reg & 0xFF) | ((data as u16) << 8);
            }
            false => {
                chan.base_address_reg = (chan.base_address_reg & 0xFF00) | (data as u16);
                chan.current_address_reg = (chan.current_address_reg & 0xFF00) | (data as u16);
            }
        }
        //log::trace!("Address port write: {:02X} BAR: {:04X} CAR: {:04X}", data, chan.base_address_reg, chan.current_address_reg);

        // Flop the flop on write
        self.flipflop = !self.flipflop;
    }

    pub fn handle_wc_port_read(&mut self, channel: usize) -> u8 {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        let chan = &mut self.channels[channel];

        // Send MSB when flipflop set, LSB when clear
        let byte = match self.flipflop {
            true => ((chan.current_word_count_reg >> 8) & 0xFF) as u8,
            false => (chan.current_word_count_reg & 0xFF) as u8,
        };
        // Flop the flop on read
        self.flipflop = !self.flipflop;
        // Return the byte
        byte
    }

    // Set the DMA channel transfer count
    pub fn handle_wc_port_write(&mut self, channel: usize, data: u8) {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        let chan = &mut self.channels[channel];

        // Set MSB when flipflop set, LSB when clear
        match self.flipflop {
            true => {
                chan.base_word_count_reg = (chan.base_word_count_reg & 0xFF) | ((data as u16) << 8);
                chan.current_word_count_reg = (chan.current_word_count_reg & 0xFF) | ((data as u16) << 8);
            }
            false => {
                chan.base_word_count_reg = (chan.base_word_count_reg & 0xFF00) | (data as u16);
                chan.current_word_count_reg = (chan.current_word_count_reg & 0xFF00) | (data as u16);
            }
        }

        // Flop the flop on write
        self.flipflop = !self.flipflop;
    }

    pub fn handle_page_register_read(&self, channel: usize) -> u8 {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        self.channels[channel].page
    }

    pub fn handle_page_register_write(&mut self, channel: usize, data: u8) {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        self.channels[channel].page = data;
    }

    pub fn handle_status_register_read(&mut self) -> u8 {
        let mut status_byte = 0;
        for (i, chan) in self.channels.iter_mut().enumerate() {
            // Intel: Bits 0-3 are set every time a TC is reached by that channel or an external EOP is applied.
            // These bits are cleared upon Reset and on each Status Read.
            if chan.terminal_count_reached {
                status_byte |= 0x01 << i;

                chan.terminal_count_reached = false;
            }

            // Intel: Bits 4-7 are set whenever their corresponding channel is requesting service.
            if chan.request {
                status_byte |= 0x01 << (i + 4);
            }
        }
        status_byte
    }

    pub fn handle_command_register_write(&mut self, control_byte: u8) {
        // Uncertain how much of the command register needs to be implemented.
        // We set a few known flags but they may not be used.
        self.mem_to_mem_enabled = control_byte & DMA_COMMAND_MEM_TO_MEM != 0;
        self.channel_0_hold_enabled = control_byte & DMA_COMMAND_CHANNEL_0_HOLD != 0;

        if (control_byte & DMA_COMMAND_DISABLE != 0) && self.enabled {
            log::trace!("DMA: Disabling DMA controller");
            self.enabled = false
        }
        else if control_byte & DMA_COMMAND_DISABLE == 0 {
            log::trace!("DMA: Enabling DMA controller");
            self.enabled = true
        }

        self.timing_mode = if control_byte & DMA_COMMAND_TIMING == 0 {
            TimingMode::NormalTiming
        }
        else {
            TimingMode::CompressedTiming
        };

        self.priority_mode = if control_byte & DMA_COMMAND_PRIORITY == 0 {
            PriorityMode::Fixed
        }
        else {
            PriorityMode::Rotating
        };

        self.command_register = control_byte;
    }

    pub fn handle_write_req_register(&mut self, data: u8) {
        log::debug!("DMA: Unimplemented write to Write Request Register: {:02X}", data);
    }

    pub fn handle_channel_mask_register_write(&mut self, data: u8) {
        // Bits 0-1: Channel Number
        // Bit 2: Mask bit state
        let chan = data & 0x03;
        self.channels[chan as usize].masked = (data & 0x04) != 0;
    }

    pub fn handle_channel_mode_register_write(&mut self, data: u8) {
        // Bits 0-1: Channel Number
        // Bits 2-3: Verify Operation
        // Bit 5: Address increment/decrement
        // Bit 6-7: Mode
        let chan_n = data & 0x03;
        let chan = &mut self.channels[chan_n as usize];

        chan.transfer_type = match (data >> 2) & 0x03 {
            0b00 => TransferType::Verify,
            0b01 => TransferType::Write,
            0b10 => TransferType::Read,
            _ => TransferType::Illegal,
        };

        chan.auto_init = (data & 0x10) != 0;

        chan.address_mode = match (data >> 5) & 0x01 {
            0b01 => AddressMode::Decrement,
            _ => AddressMode::Increment,
        };

        chan.service_mode = match (data >> 6) & 0x03 {
            0b00 => ServiceMode::Demand,
            0b01 => ServiceMode::Single,
            0b10 => ServiceMode::Block,
            _ => ServiceMode::Cascade,
        };

        chan.mode_reg = data;
        chan.terminal_count = false;

        log::trace!(
            "DMA Channel {} mode set: Transfer type: {:?}, Auto init: {:?}, Address Mode: {:?}, Service Mode: {:?}",
            chan_n,
            chan.transfer_type,
            chan.auto_init,
            chan.address_mode,
            chan.service_mode
        );
    }

    pub fn handle_clear_flopflop(&mut self) {
        self.flipflop = false;
    }

    pub fn handle_temp_register_read(&self) -> u8 {
        self.temp_reg
    }

    pub fn handle_master_clear(&mut self) {
        // From Intel 8237 whitepaper:
        // This software instruction has the same effect as the hardware Reset. The Command, Status, Request, Temporary, and Internal
        // First/Last Flip-Flop registers are cleared and the Mask register is set.

        // Set mask for each channel
        for chan in &mut self.channels {
            chan.masked = true;
        }
        self.command_register = 0;
        self.status_reg = 0;
        self.temp_reg = 0;
        self.flipflop = false;
    }

    pub fn handle_clear_mask_register(&mut self) {
        // Set mask for all channels
        for chan in &mut self.channels {
            chan.masked = false;
        }
    }

    pub fn handle_write_mask_register(&mut self, data: u8) {
        // Set mask for each channel per bitmask
        let mut mask_byte = data;
        for chan in &mut self.channels {
            chan.masked = mask_byte & 0x01 != 0;
            mask_byte >>= 1;
        }
    }

    pub fn get_string_state(&self) -> DMAControllerStringState {
        let mut chan_vec = Vec::new();
        for chan in self.channels.iter() {
            chan_vec.push(DMAChannelStringState {
                current_address_reg: format!("{:04X}", chan.current_address_reg),
                current_word_count_reg: format!("{}", chan.current_word_count_reg),
                base_address_reg: format!("{:04X}", chan.base_address_reg),
                base_word_count_reg: format!("{}", chan.base_word_count_reg),

                service_mode: format!("{:?}", chan.service_mode),
                address_mode: format!("{:?}", chan.address_mode),
                transfer_type: format!("{:?}", chan.transfer_type),
                auto_init: format!("{:?}", chan.auto_init),
                terminal_count: format!("{:?}", chan.terminal_count),
                terminal_count_reached: format!("{:?}", chan.terminal_count_reached),
                masked: format!("{:?}", chan.masked),
                page: format!("{:02X}", chan.page),
            });
        }

        DMAControllerStringState {
            enabled: format!("{:?}", self.enabled),
            flipflop: format!("{:?}", self.flipflop),
            dreq: format!("{:?}", self.dreq),
            dma_channel_state: chan_vec,
        }
    }

    pub fn get_dma_transfer_size(&self, channel: usize) -> usize {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }

        let size: usize = self.channels[channel].base_word_count_reg as usize + 1;
        size
    }

    pub fn get_dma_transfer_address(&self, channel: usize) -> usize {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        let address: usize =
            ((self.channels[channel].page as usize) << 16) + self.channels[channel].current_address_reg as usize;
        address
    }

    pub fn check_dma_ready(&self, channel: usize) -> bool {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        let mut is_ready = false;
        if !self.channels[channel].masked {
            is_ready = true;
        }
        is_ready
    }

    /// Request DMA Serivce
    /// Equivalent to setting the DREQ line high for the given DMA channel
    pub fn request_service(&mut self, channel: usize) {
        self.request_reg |= 0x01 << channel;
    }

    /// Clear DMA Service
    /// Equivlaent to de-asserting the DREQ line for the given DMA channel
    pub fn clear_service(&mut self, channel: usize) {
        self.request_reg &= !(0x01 << channel);
    }

    /// Get the state of the DACK line for the specified DMA channel.
    /// DACK signals whether the requestin device can be serviced.
    pub fn read_dma_acknowledge(&self, _channel: usize) -> bool {
        true
    }

    pub fn check_terminal_count(&self, channel: usize) -> bool {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }

        self.channels[channel].terminal_count
    }

    pub fn do_dma_read_u8(&mut self, bus: &mut BusInterface, channel: usize) -> u8 {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }

        if !self.enabled {
            return 0;
        }

        let mut data: u8 = 0;
        let mut _cost = 0;
        let bus_address = self.get_dma_transfer_address(channel);

        match self.channels[channel].address_mode {
            AddressMode::Increment => {
                if self.channels[channel].current_word_count_reg > 0 {
                    (data, _cost) = bus.read_u8(bus_address, 0).unwrap();

                    if self.channels[channel].current_word_count_reg == 1 {
                        //log::trace!("car: {} cwc: {} ", self.channels[channel].current_address_reg, self.channels[channel].current_word_count_reg);
                    }

                    // Internal address register wraps around
                    self.channels[channel].current_address_reg =
                        self.channels[channel].current_address_reg.wrapping_add(1);
                    self.channels[channel].current_word_count_reg -= 1;

                    //log::trace!("DMA read {:02X} from address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                }
                else if self.channels[channel].current_word_count_reg == 0 && !self.channels[channel].terminal_count {
                    // Transfer one more on a 0 count, then set TC
                    (data, _cost) = bus.read_u8(bus_address, 0).unwrap();

                    //self.channels[channel].current_address_reg += 1;

                    //log::trace!("DMA read {:02X} from address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                    if self.channels[channel].auto_init {
                        // Reload channel if auto-init on
                        self.channels[channel].current_address_reg = self.channels[channel].base_address_reg;
                        self.channels[channel].current_word_count_reg = self.channels[channel].base_word_count_reg;
                    }
                    else {
                        self.channels[channel].terminal_count = true;
                        log::trace!("Terminal count reached on DMA channel {:01X}", channel);
                    }
                    // Set the tc status bit regardless of auto-init
                    self.channels[channel].terminal_count_reached = true;
                }
                else {
                    // Trying to transfer on a terminal count
                }
            }
            _ => panic!("DMA Decrement address mode unimplemented"),
        }

        data
    }

    pub fn do_dma_write_u8(&mut self, bus: &mut BusInterface, channel: usize, data: u8) {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }

        let bus_address = self.get_dma_transfer_address(channel);

        match self.channels[channel].address_mode {
            AddressMode::Increment => {
                if self.channels[channel].current_word_count_reg > 0 {
                    // Don't transfer anything if in Verify mode
                    if let TransferType::Write = self.channels[channel].transfer_type {
                        bus.write_u8(bus_address, data, 0).unwrap();
                    }

                    self.channels[channel].current_address_reg =
                        self.channels[channel].current_address_reg.wrapping_add(1);
                    self.channels[channel].current_word_count_reg -= 1;

                    //log::trace!("DMA write {:02X} to address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                }
                else if self.channels[channel].current_word_count_reg == 0 && !self.channels[channel].terminal_count {
                    // Transfer one more on a 0 count, then set TC
                    if let TransferType::Write = self.channels[channel].transfer_type {
                        bus.write_u8(bus_address, data, 0).unwrap();
                    }
                    //self.channels[channel].current_address_reg += 1;

                    //log::trace!("DMA write {:02X} to address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                    self.channels[channel].terminal_count = true;
                    log::trace!("Terminal count reached on DMA channel {:01X}", channel);
                    log::trace!(
                        "Completed DMA of {} bytes to address {:05X}",
                        self.channels[channel].base_word_count_reg + 1,
                        ((self.channels[channel].page as u32) << 16) + (self.channels[channel].base_address_reg as u32)
                    );

                    // TODO: Support auto-init here

                    // Set the tc status bit regardless of auto-init
                    self.channels[channel].terminal_count_reached = true;
                }
                else {
                    // Trying to transfer on a terminal count
                }
            }
            _ => panic!("DMA Decrement address mode unimplemented"),
        }
    }

    /// Fake the DMA controller. This should eventually be replaced by a tick procedure that
    /// ticks in line with the CPU.
    pub fn run(&mut self, bus: &mut BusInterface) {
        for i in 0..DMA_CHANNEL_COUNT {
            if self.request_reg & (0x01 << i) != 0 {
                // We have an active DREQ on this channel, service it
                match self.channels[i].service_mode {
                    ServiceMode::Single => {
                        // We can handle single byte mode
                        match self.channels[i].transfer_type {
                            TransferType::Read | TransferType::Verify => {
                                if i == 0 {
                                    self.do_dma_read_u8(bus, i);
                                }
                            }
                            TransferType::Write => {
                                // nothing to do here
                            }
                            TransferType::Illegal => {
                                log::error!("Illegal DMA TransferType: {:?}", self.channels[i].transfer_type);
                            }
                        }

                        // Since this is single byte service, we can now reset the request register bit.
                        self.request_reg &= !(0x01 << i);
                    }
                    _ => {
                        //log::warn!("Unhandled DMA service mode: {:?}", self.channels[i].service_mode);
                    }
                }
            }
        }
    }
}
