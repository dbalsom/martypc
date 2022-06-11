/*

    Implements the Intel 8237 DMA Controller


*/


use std::ops::Add;

use crate::io::{IoBusInterface, IoDevice};
use crate::bus::BusInterface;

use log;

pub const DMA_CHANNEL_0_ADDR_PORT: u16  = 0x00; // R/W
pub const DMA_CHANNEL_0_WC_PORT: u16    = 0x01; // R/W
pub const DMA_CHANNEL_1_ADDR_PORT: u16  = 0x02; // R/W
pub const DMA_CHANNEL_1_WC_PORT: u16    = 0x03; // R/W
pub const DMA_CHANNEL_2_ADDR_PORT: u16  = 0x04; // R/W
pub const DMA_CHANNEL_2_WC_PORT: u16    = 0x05; // R/W
pub const DMA_CHANNEL_3_ADDR_PORT: u16  = 0x06; // R/W
pub const DMA_CHANNEL_3_WC_PORT: u16    = 0x07; // R/W

// Status/Command register depending on read/write
pub const DMA_STATUS_REGISTER: u16      = 0x08; // R
pub const DMA_COMMAND_REGISTER: u16     = 0x08; // W
pub const DMA_WRITE_REQ_REGISTER: u16   = 0x09; // W
pub const DMA_CHANNEL_MASK_REGISTER: u16    = 0x0A; // R/W
pub const DMA_CHANNEL_MODE_REGISTER: u16    = 0x0B; // R/W
pub const DMA_CLEAR_FLIPFLOP: u16       = 0x0C; // W

// Read Temp/Master Clear register depending on read/write
pub const DMA_READ_TEMP_REGISTER: u16   = 0x0D; // R
pub const DMA_MASTER_CLEAR: u16         = 0x0D; // W
pub const DMA_CLEAR_MASK_REGISTER: u16  = 0x0E; // W
pub const DMA_WRITE_MASK_REGISTER: u16  = 0x0F; // W

// The following page registers are not in logical order (why?)
pub const DMA_CHANNEL_0_PAGE_REGISTER: u16 = 0x87; // R/W
pub const DMA_CHANNEL_1_PAGE_REGISTER: u16 = 0x83; // R/W
pub const DMA_CHANNEL_2_PAGE_REGISTER: u16 = 0x81; // R/W
pub const DMA_CHANNEL_3_PAGE_REGISTER: u16 = 0x82; // R/W

// Control byte bit fields - not all of these are implemented
pub const DMA_COMMAND_MEM_TO_MEM: u8        = 0x01;
pub const DMA_COMMAND_CHANNEL_0_HOLD: u8    = 0x02;
pub const DMA_COMMAND_DISABLE: u8           = 0x04;
pub const DMA_COMMAND_TIMING: u8            = 0x08;
pub const DMA_COMMAND_PRIORITY: u8          = 0x10;


pub const DMA_CHANNEL_COUNT: usize = 4;



pub enum TimingMode {
    NormalTiming,
    CompressedTiming,
}

pub enum PriorityMode {
    Fixed,
    Rotating,
}

#[derive (Debug)]
pub enum ServiceMode {
    Demand,
    Single,
    Block,
    Cascade
}
impl Default for ServiceMode {
    fn default() -> Self { ServiceMode::Demand }
}
#[derive (Debug)]
pub enum AddressMode {
    Increment,
    Decrement
}
impl Default for AddressMode {
    fn default() -> Self { AddressMode::Increment }
}

#[derive (Debug)]
pub enum TransferType {
    Verify,
    Write,
    Read,
    Illegal
}
impl Default for TransferType {
    fn default() -> Self { TransferType::Verify }
}

#[derive (Default)]
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
    request: bool,
    masked: bool,
    page: u8
}

#[derive (Default)]
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
    pub masked: String,
    pub page: String
}

#[derive (Default)]
pub struct DMAControllerStringState {
    pub flopflop: String,
    pub dma_channel_state: Vec<DMAChannelStringState>
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
    temp_reg: u8
}

impl IoDevice for DMAController {
    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            DMA_CHANNEL_0_ADDR_PORT => {
                self.handle_addr_port_read(0)
            },
            DMA_CHANNEL_1_ADDR_PORT => {
                self.handle_addr_port_read(1)
            },
            DMA_CHANNEL_2_ADDR_PORT => {
                self.handle_addr_port_read(2)
            },
            DMA_CHANNEL_3_ADDR_PORT => {
                self.handle_addr_port_read(3)
            },
            DMA_CHANNEL_0_WC_PORT => {
                self.handle_wc_port_read(0)
            },
            DMA_CHANNEL_1_WC_PORT => {
                self.handle_wc_port_read(1)
            },
            DMA_CHANNEL_2_WC_PORT => {
                self.handle_wc_port_read(2)
            },
            DMA_CHANNEL_3_WC_PORT => {
                self.handle_wc_port_read(3)
            }
            DMA_STATUS_REGISTER => {
                self.handle_status_register_read()
            }
            DMA_READ_TEMP_REGISTER => {
                self.handle_temp_register_read()
            }
            DMA_CHANNEL_0_PAGE_REGISTER => {
                self.handle_page_register_read(0)
            }
            DMA_CHANNEL_1_PAGE_REGISTER => {
                self.handle_page_register_read(1)
            }
            DMA_CHANNEL_2_PAGE_REGISTER => {
                self.handle_page_register_read(2)
            }
            DMA_CHANNEL_3_PAGE_REGISTER => {
                self.handle_page_register_read(3)
            }                                    
            _ => {
                log::warn!("Read from unhandled DMA Controller port: {:02X}", port);
                0
            }
            
        }

    }
    fn write_u8(&mut self, port: u16, data: u8) {

        match port {
            DMA_CHANNEL_0_ADDR_PORT => {
                self.handle_addr_port_write(0, data);
            },
            DMA_CHANNEL_1_ADDR_PORT => {
                self.handle_addr_port_write(1, data);
            },
            DMA_CHANNEL_2_ADDR_PORT => {
                self.handle_addr_port_write(2, data);
            },
            DMA_CHANNEL_3_ADDR_PORT => {
                self.handle_addr_port_write(3, data);
            },
            DMA_CHANNEL_0_WC_PORT => {
                self.handle_wc_port_write(0, data);
            },
            DMA_CHANNEL_1_WC_PORT => {
                self.handle_wc_port_write(1, data);
            },
            DMA_CHANNEL_2_WC_PORT => {
                self.handle_wc_port_write(2, data);
            },
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
        if port == DMA_COMMAND_REGISTER {
         
        }
    }
    fn read_u16(&mut self, port: u16) -> u16 {
        0
    }
    fn write_u16(&mut self, port: u16, data: u16) {

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
            temp_reg: 0
        }
    }

    pub fn handle_addr_port_read(&mut self, channel: usize) -> u8 {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        let chan = &mut self.channels[channel];

        // Send MSB when flipflop set, LSB when clear
        let byte = match self.flipflop {
            true => ((chan.current_address_reg >> 8) & 0xFF) as u8,
            false => (chan.current_address_reg & 0xFF) as u8
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
            },
            false => {
                chan.base_address_reg = (chan.base_address_reg & 0xFF00) | (data as u16);
                chan.current_address_reg = (chan.current_address_reg & 0xFF00) | (data as u16);
            }
        }

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
            false => (chan.current_word_count_reg & 0xFF) as u8
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
            },
            false => {
                chan.base_word_count_reg = (chan.base_word_count_reg  & 0xFF00) | (data as u16);
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
        for (i, chan) in self.channels.iter().enumerate() {
            if chan.terminal_count {
                status_byte |= 0x01 << i;
            }
            
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

    pub fn handle_write_req_register(&mut self, data: u8 ) {
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
            _ => TransferType::Illegal
        };

        chan.auto_init = (data & 0x10) != 0;

        chan.address_mode = match (data >> 5) & 0x01 {
            0b01 => AddressMode::Decrement,
            _=> AddressMode::Increment
        };

        chan.service_mode = match (data >> 6) & 0x03 {
            0b00 => ServiceMode::Demand,
            0b01 => ServiceMode::Single,
            0b10 => ServiceMode::Block,
            _=> ServiceMode::Cascade
        };

        chan.mode_reg = data;
        chan.terminal_count = false;

        log::trace!("DMA Channel {} mode set: Transfer type: {:?}, Auto init: {:?}, Address Mode: {:?}, Service Mode: {:?}",
            chan_n, chan.transfer_type, chan.auto_init, chan.address_mode, chan.service_mode );


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

            chan_vec.push(DMAChannelStringState{
                current_address_reg: format!("{:04X}", chan.current_address_reg),
                current_word_count_reg: format!("{}", chan.current_word_count_reg),
                base_address_reg: format!("{:04X}", chan.base_address_reg),
                base_word_count_reg: format!("{}", chan.base_word_count_reg),
            
                service_mode: format!("{:?}", chan.service_mode),
                address_mode: format!("{:?}", chan.address_mode),
                transfer_type: format!("{:?}", chan.transfer_type),
                auto_init: format!("{:?}", chan.auto_init),
                terminal_count: format!("{:?}", chan.terminal_count),
                masked: format!("{:?}", chan.masked),
                page: format!("{:02X}", chan.page)
            });
        }

        DMAControllerStringState { 
            flopflop: format!("{:?}", self.flipflop),
            dma_channel_state: chan_vec 
        }
    }

    pub fn get_dma_transfer_size(&self, channel: usize) -> usize {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }  

        let size: usize = self.channels[channel].base_word_count_reg as usize + 1;
        return size
    }

    pub fn get_dma_transfer_address(&self, channel: usize) -> usize {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }  
        let address: usize = ((self.channels[channel].page as usize) << 16) + self.channels[channel].current_address_reg as usize;
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

    pub fn check_terminal_count(&self, channel: usize) -> bool {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }
        
        self.channels[channel].terminal_count
    }

    pub fn do_dma_read_u8(&mut self, bus: &mut BusInterface, channel: usize ) -> u8 {
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

                    (data, _cost) = bus.read_u8(bus_address).unwrap();
                    
                    if self.channels[channel].current_word_count_reg == 1 {
                        //log::trace!("car: {} cwc: {} ", self.channels[channel].current_address_reg, self.channels[channel].current_word_count_reg);
                    }

                    // Internal address register wraps around
                    self.channels[channel].current_address_reg.wrapping_add(1);
                    self.channels[channel].current_word_count_reg -= 1;

                    //log::trace!("DMA read {:02X} from address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                }
                else if self.channels[channel].current_word_count_reg == 0 && !self.channels[channel].terminal_count {
                    
                    // Transfer one more on a 0 count, then set TC
                    (data, _cost) = bus.read_u8(bus_address).unwrap();

                    //self.channels[channel].current_address_reg += 1;

                    //log::trace!("DMA read {:02X} from address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                    if self.channels[channel].auto_init {
                        // Reload channel if auto-init on
                        self.channels[channel].current_address_reg = self.channels[channel].base_address_reg;
                        self.channels[channel].current_word_count_reg  = self.channels[channel].base_word_count_reg;
                    }
                    else {
                        self.channels[channel].terminal_count = true;
                        log::trace!("Terminal count reached on DMA channel {:01X}", channel);
                    }
                }
                else {
                    // Trying to transfer on a terminal count
                }                
            }
            _=> panic!("DMA Decrement address mode unimplemented")
        }        
        
        0
    }

    pub fn do_dma_transfer_u8(&mut self, bus: &mut BusInterface, channel: usize, data: u8) {
        if channel >= DMA_CHANNEL_COUNT {
            panic!("Invalid DMA Channel");
        }  

        let bus_address = self.get_dma_transfer_address(channel);

        match self.channels[channel].address_mode {
            AddressMode::Increment => {

                if self.channels[channel].current_word_count_reg > 0 {
                    bus.write_u8(bus_address, data);
                    
                    self.channels[channel].current_address_reg += 1;
                    self.channels[channel].current_word_count_reg -= 1;

                    //log::trace!("DMA write {:02X} to address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                }
                else if self.channels[channel].current_word_count_reg == 0 && !self.channels[channel].terminal_count {
                    
                    // Transfer one more on a 0 count, then set TC
                    bus.write_u8(bus_address, data);
                    self.channels[channel].current_address_reg += 1;

                    //log::trace!("DMA write {:02X} to address: {:06X} CWC: {}", data, bus_address, self.channels[channel].current_word_count_reg);
                    self.channels[channel].terminal_count = true;
                    log::trace!("Terminal count reached on DMA channel {:01X}", channel);
                }
                else {
                    // Trying to transfer on a terminal count
                }
            }
            _=> panic!("DMA Decrement address mode unimplemented")
        }        
    }

    pub fn run(&mut self, io_bus: &mut IoBusInterface) {


    }
}