/* 

    hdc.rs
    Implement the IBM/Xebec 20Mbit Fixed Disk Adapter

    
*/

#![allow (dead_code)]

use std::{
    cell::RefCell,
    collections::VecDeque,
    error::Error,
    rc::Rc,
};

use core::fmt::Display;

use crate::bus::BusInterface;
use crate::dma;
//use crate::fdc::Operation;
use crate::bus::IoDevice;
use crate::pic;
use crate::VirtualHardDisk;

// Public consts
pub const HDC_IRQ: u8 = 0x05;
pub const HDC_DMA: usize = 0x03;
pub const SECTOR_SIZE: usize = 512;
pub const DRIVE_TYPE2_DIP: u8 = 0b1010; // IBM Type 2, 20MB drive

pub const HDC_DATA_REGISTER: u16 =      0x320;
pub const HDC_STATUS_REGISTER: u16 =    0x321;
// 0x322 is Read DIP on READ,  Controller Select on WRITE
pub const HDC_READ_DIP_REGISTER: u16 =  0x322;
pub const HDC_CONTROLLER_SELECT: u16 =  0x322; 
pub const HDC_WRITE_MASK_REGISTER: u16 =  0x323;

// Private consts
const DBC_LEN: u32 = 5; // Length of Device Control Block, the 5 bytes that are sent after a command opcode
const IDC_LEN: u32 = 8; // The Initialize Drive Characteristics command is followed by 8 bytes after DCB

const ENABLE_DMA_MASK: u8 = 0x01;
const ENABLE_IRQ_MASK: u8 = 0x02;

const R1_STATUS_REQ: u8 =      0b0000_0001; // "Request Bit"
const R1_STATUS_IOMODE: u8 =   0b0000_0010; // "Mode Bit" -> Similar to DIO for FDC perhaps?
const R1_STATUS_BUS: u8 =      0b0000_0100; // "Command/Data Bit"
const R1_STATUS_BUSY: u8 =     0b0000_1000; // "Busy Bit"
const R1_STATUS_DREQ: u8 =     0b0001_0000;
const R1_STATUS_INT: u8 =      0b0010_0000;

// Controller error codes
const NO_ERROR_CODE: u8         = 0;
const ERR_NO_INDEX_SIGNAL: u8   = 0b00_0010;
const ERR_WRITE_FAULT: u8       = 0b00_0011;
const ERR_NO_READY_SIGNAL: u8   = 0b00_0100;
const ERR_SECTOR_NOT_FOUND: u8  = 0b01_0100;
const ERR_SEEK_ERROR: u8        = 0b01_0101;
const ERR_INVALID_COMMAND: u8   = 0b10_0000;
const ERR_ILLEGAL_ACCESS: u8    = 0b10_0001;

#[allow (dead_code)]
#[derive (Copy, Clone, Debug)]
pub enum OperationError {
    NoError,
    NoReadySignal,
    InvalidCommand,
    IllegalAccess
}

#[allow (dead_code)]
#[derive (Debug)]
pub enum ControllerError {
    NoError,
    InvalidDevice,
    UnsupportedVHD,
}
impl Error for ControllerError {}
impl Display for ControllerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ControllerError::NoError => write!(f, "No error."),
            ControllerError::InvalidDevice => write!(f, "The specified Device ID was out of range [0..1]"),
            ControllerError::UnsupportedVHD => write!(f, "The VHD file did not match the list of supported drive types.")
        }
    }
}

#[allow (dead_code)]
#[derive (Copy, Clone, Debug)]
pub enum State {
    Reset,
    WaitingForCommand,
    ReceivingCommand,
    ExecutingCommand,
    HaveCommandResult,
    HaveCommandStatus,
    HaveSenseBytes
}

#[allow(dead_code)]
#[derive (Copy, Clone, Debug)]
pub enum Command {
    None,
    TestDriveReady,
    Recalibrate,
    RequestSense,
    FormatDrive,
    ReadyVerify,
    FormatTrack,
    FormatBadTrack,
    Read,
    Write,
    Seek,
    Initialize,
    ReadEccBurstLength,
    ReadSectorBuffer,
    WriteSectorBuffer,
    RamDiagnostic,
    DriveDiagnostic,
    ControllerDiagnostic,
    ReadLongTrack,
    WriteLongTrack
}

type CommandDispatchFn = fn (&mut HardDiskController, &mut BusInterface) -> Continuation;

impl IoDevice for HardDiskController {
    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            HDC_DATA_REGISTER  => {
                self.handle_data_register_read()
            }
            HDC_STATUS_REGISTER => {
                self.handle_status_register_read()
            }
            HDC_READ_DIP_REGISTER => {
                self.handle_dip_register_read()
            }
            _ => {
                log::error!("Read from invalid port!");
                0
            }
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, bus: Option<&mut BusInterface>) {
        match port {
            HDC_DATA_REGISTER => {
                // Bus will always call us with Bus defined, so safe to unwrap
                self.handle_data_register_write(data, bus.unwrap());
            }
            HDC_STATUS_REGISTER => {
                // Write to the status register instructs the controller to reset
                self.reset();
            }
            HDC_CONTROLLER_SELECT => {
                self.handle_controller_select(data);
                //self.handle_dip_register_write()
            }
            HDC_WRITE_MASK_REGISTER => {
                self.handle_mask_register_write(data);
            }
            _ => log::error!("Write to invalid port: {:04X} : {:02X}!", port, data)
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![
            HDC_DATA_REGISTER,
            HDC_STATUS_REGISTER,
            HDC_READ_DIP_REGISTER,
            HDC_CONTROLLER_SELECT,
            HDC_WRITE_MASK_REGISTER,
        ]
    }
}

#[derive (Clone, Debug, Default)]
pub struct HardDiskFormat {
    pub max_cylinders: u16,
    pub max_heads: u8,
    pub max_sectors: u8,
    pub desc: String,
}

pub struct HardDisk {
    cylinder: u16,
    head: u8,
    sector: u8,
    max_cylinders: u16,
    max_heads: u8,
    max_sectors: u8,
    sector_buf: Vec<u8>,
    vhd: Option<VirtualHardDisk>
}

impl HardDisk {
    pub fn new() -> Self {
        Self {
            cylinder: 0,
            head: 0,
            sector: 0,
            max_cylinders: 0,
            max_heads: 0,
            max_sectors: 0,
            sector_buf: vec![0; SECTOR_SIZE],
            vhd: None
        }
    }

    pub fn get_next_sector(&self, cylinder: u16, head: u8, sector: u8) -> (u16, u8, u8) {

        if self.max_sectors == 0 || self.max_heads == 0 || self.max_cylinders == 0 {
            return (0,0,0)
        }

        if sector < self.max_sectors - 1 {
            // Not at last sector, just return next sector
            (cylinder, head, sector + 1)
        }
        else if head < self.max_heads - 1 {
            // At last sector, but not at last head, go to next head, same cylinder, sector 0
            (cylinder, head + 1, 0) 
        }
        else if cylinder < self.max_cylinders - 1 {
            // At last sector and last head, go to next cylinder, head 0, sector 0
            (cylinder + 1, 0, 0)
        }
        else {
            // At end of drive.
            (self.max_cylinders, 0, 0)
        }
    }
}

#[allow (dead_code)]
#[derive (Default)]
pub struct OperationStatus {
    drive_select: usize,
    buffer_idx: usize,
    block_ct: u8,
    block_n: u8,
    dma_bytes_left: usize,
    dma_byte_count: usize,
}

pub enum Continuation {
    CommandComplete,
    ContinueAsOperation
}

#[allow (dead_code)]
pub struct DeviceControlBlock {
    drive_select: usize,
    c: u16,
    h: u8,
    s: u8,
    interleave: u8,
    block_count: u8,
    step: u8,
    retry_on_ecc: bool,
    disable_retry: bool,
}

#[allow (dead_code)]
pub struct HardDiskController {
    drives: [HardDisk; 2],
    drive_select: usize,

    supported_formats: Vec<HardDiskFormat>,
    drive_type_dip: u8,
    state: State,
    last_error: OperationError,
    last_error_drive: usize,
    error_flag: bool,
    receiving_dcb: bool,
    command: Command,
    command_fn: Option<CommandDispatchFn>,
    last_command: Command,
    command_byte_n: u32,
    command_result_pending: bool,

    data_register_in: VecDeque<u8>,
    data_register_out: VecDeque<u8>,

    operation_status: OperationStatus,

    dma_enabled: bool,
    irq_enabled: bool,

    send_interrupt: bool,
    clear_interrupt: bool,
    interrupt_active: bool,
    send_dreq: bool,
    clear_dreq: bool,
    dreq_active: bool,
}

impl HardDiskController {
    pub fn new(drive_type_dip: u8) -> Self {
        Self {
            drives: [
                HardDisk::new(),
                HardDisk::new()
            ],
            drive_select: 0,
            supported_formats: vec![
                HardDiskFormat {
                    max_cylinders: 615,
                    max_heads: 4,
                    max_sectors: 17,
                    desc: "20MB, Type 2".to_string()
                }
            ],
            drive_type_dip,
            state: State::Reset,
            last_error: OperationError::NoError,
            last_error_drive: 0,
            error_flag: false,
            receiving_dcb: false,
            command: Command::None,
            command_fn: None,
            last_command: Command::None,
            command_byte_n: 0,
            command_result_pending: false,
            data_register_in: VecDeque::new(),
            data_register_out: VecDeque::new(),
            operation_status: Default::default(),
            dma_enabled: false,
            irq_enabled: false,
            send_interrupt: false,
            clear_interrupt: false,
            interrupt_active: false,   
            send_dreq: false,
            clear_dreq: false,
            dreq_active: false,         

        }
    }

    pub fn reset(&mut self) {

        log::trace!("Resetting Hard Disk Controller...");

        self.data_register_in.clear();
        self.data_register_out.clear();
        self.command_result_pending = false;
        self.command_byte_n = 0;

        self.interrupt_active = false;
        self.send_interrupt = false;
        self.send_dreq = false;
        self.state = State::Reset;
    }

    pub fn get_supported_formats(&self) -> Vec<HardDiskFormat> {

        self.supported_formats.clone()
    }

    pub fn set_vhd(&mut self, device_id: usize, vhd: VirtualHardDisk) -> Result<(), ControllerError> {

        if device_id > 1 {
            return Err(ControllerError::InvalidDevice)
        }
        
        // Check that the VHD geometry is in the list of supported formats
        // (Currently there is only one supported format but that might change)
        let mut supported = false;
        for format in &self.supported_formats {

            if vhd.max_cylinders as u16 == format.max_cylinders
                && vhd.max_heads as u8 == format.max_heads
                && vhd.max_sectors as u8 == format.max_sectors {
                    supported = true;
                    break;
                }
        }

        if supported {
            self.drives[device_id].max_cylinders = vhd.max_cylinders as u16;
            self.drives[device_id].max_heads = vhd.max_heads as u8;
            self.drives[device_id].max_sectors = vhd.max_sectors as u8;
            self.drives[device_id].vhd = Some(vhd);
        }
        else {
            return Err(ControllerError::UnsupportedVHD);
        }

        Ok(())
    }

    pub fn set_command(&mut self, command: Command, n_bytes: u32, command_fn: CommandDispatchFn ) {

        self.state = State::ReceivingCommand;
        self.receiving_dcb = true;
        self.command = command;
        self.command_fn = Some(command_fn);
        self.command_byte_n = n_bytes;
    }

    pub fn set_error(&mut self, error: OperationError, drive_select: usize ) {

        self.last_error = error;
        self.last_error_drive = drive_select;

        match error {
            OperationError::NoError => self.error_flag = false,
            _ => self.error_flag = true
        }
    }

    pub fn read_dcb(&mut self) -> DeviceControlBlock {

        let cmd_bytes = &self.data_register_in;

        // Byte 0: [ 0 0 drive-bit(1) head-bits(5) ]
        let drive_select: usize = ((cmd_bytes[0] >> 5) & 0x01).into();
        let h = cmd_bytes[0] & 0x1F;

        // Byte 1: [ cylinder high bits(2) sector bits(6)]
        let s = cmd_bytes[1] & 0x1F;
        
        // Byte 2: [ cylinder low bits (8) ]
        let c: u16 = ((cmd_bytes[1] & 0xC0) as u16) << 2 | cmd_bytes[2] as u16;
        
        // Byte 3: [ block count (8) ] -OR- [ 0 0 0 interleave(5) ]
        let block_count = cmd_bytes[3];
        let interleave = cmd_bytes[3] & 0x1F;
        
        // Byte 4: [ disable retry bit(1) retry_on_ecc_bit(1) 0 0 0 step_bits(3) ]
        let step = cmd_bytes[4] & 0x07;
        let retry_on_ecc = (cmd_bytes[4] >> 6 & 0x01) != 0;
        let disable_retry = (cmd_bytes[4] >> 7 & 0x01) != 0;
        
        DeviceControlBlock { 
            drive_select,
            c,
            h,
            s,
            block_count,
            interleave,
            step,
            disable_retry,
            retry_on_ecc,
        }
    }

    /// Handle a write to the Controller Select Pulse register
    pub fn handle_controller_select(&self, byte: u8) {
        // Byte written to pulse register ignored?
        // Not entirely sure the purpose of this register, but it may be used to coordinate multiple disk controllers
        log::trace!("Controller select: {:02X}", byte);
    }

    /// Read from the Data Register
    /// 
    /// Sense Bytes can be read after a Request Sense command, or the Status Byte otherwise
    pub fn handle_data_register_read(&mut self) -> u8 {

        let mut byte = 0;
        
        match self.state {
            State::HaveCommandStatus => {
                let mut error_flag = 1;
                if let OperationError::NoError = self.last_error {
                    error_flag = 0;
                }
                byte = (self.drive_select as u8 & 0x01 << 5) | (error_flag << 1);

                log::trace!("Status Byte read: {:02X}", 0);
                self.clear_interrupt = true;   
            }
            State::HaveSenseBytes => {

                if let Some(sense_byte) = self.data_register_out.pop_front() {
                    byte = sense_byte
                }
                
                if self.data_register_out.is_empty() {
                    // Sense status itself reports success/failure
                    self.state = State::HaveCommandStatus;
                }
            }
            _ => {
                log::warn!("Unexpected data register read, state: {:?}", self.state);
            }
        };

     
        byte
    }

    /// Handle a write to the DMA and interrupt mask register
    pub fn handle_mask_register_write(&mut self, byte: u8) {
        
        self.irq_enabled = byte & ENABLE_IRQ_MASK != 0;
        self.dma_enabled = byte & ENABLE_DMA_MASK != 0;
        log::trace!("Write to Mask Register. IRQ enabled: {} DMA enabled: {}", self.irq_enabled, self.dma_enabled );
        
        // Write to mask register puts us in Waiting For Command state
        self.state = State::WaitingForCommand;
    }

    /// Handle reading the Data Control Block and accepting the variable number of bytes 
    /// input for each command type. 
    /// 
    /// The 3 by 5 bit match statements match the 3 bit 'command class' and 5 bit command fields.
    pub fn handle_data_register_write(&mut self, byte: u8, bus: &mut BusInterface) {
        
        // Transition from other states. It's possible that we don't check the error code
        // after an operation
        if let State::HaveCommandStatus = self.state {
            log::warn!("Received command with pending unread status register");
            self.state = State::WaitingForCommand;
        }

        match self.state {

            /* Certain commands can be completed instantly - in the absence of emulated delays that the real hardware might have. 
               We distinguish between Commands and Operations, whereas some Commands are executed immediately and considered complete 
               by returning true, a Command may intiate an Operation by returning false. 

               An Operation is an ongoing command that may take some period of time to complete, such as a DMA transfer. 
               Operations are ticked during calls to run() on the HardDiskController device. Operations must be properly 
               terminated when complete, which usually results in an IRQ5.

               Here we match a command callback to a specified Command received in a DCB, it will be dispatched when all bytes of the DCB
               have been received, and in the case of Initialize DC, after the additional 8 DC bytes are received as well.
               */

            State::WaitingForCommand => {
                if self.interrupt_active {
                    log::warn!(" >>> Received command with interrupt active")
                }

                // TODO: Change this to a bitfield for clearer matching?
                #[allow(clippy::unusual_byte_groupings)]
                match byte {
                    0b000_00000 => {
                        // Test Drive
                        log::trace!("Received Test Drive Ready Command");
                        self.set_command(Command::TestDriveReady, DBC_LEN, HardDiskController::command_test_drive_ready);
                    }
                    0b000_00001 => {
                        // Recalibrate
                        log::trace!("Received Recalibrate Command");
                        self.set_command(Command::Recalibrate, DBC_LEN, HardDiskController::command_recalibrate);
                    }
                    0b000_00011 => {
                        // Request sense bytes
                        log::trace!("Received Request Sense Status Command");
                        self.set_command(Command::RequestSense, DBC_LEN, HardDiskController::command_sense_status);
                    }
                    0b000_00100 => {
                        // Format drive
                        log::trace!("Received Format Drive Command");
                    }
                    0b000_00101 => {
                        // Read Verify
                        log::trace!("Received Read Verify Command");
                        self.set_command(Command::ReadyVerify, DBC_LEN, HardDiskController::command_ready_verify);
                    }
                    0b000_00110 => {
                        // Format Track
                        log::trace!("Received Format Track Command");
                    }
                    0b000_00111 => {
                        // Format Bad Track
                        log::trace!("Received Format Bad Track Command");
                    }
                    0b000_01000 => {
                        // Read
                        log::trace!("Received Read Command");
                        self.set_command(Command::Read, DBC_LEN, HardDiskController::command_read);
                    }
                    0b000_01010 => {
                        // Write
                        log::trace!("Received Write Command");
                        self.set_command(Command::Write, DBC_LEN, HardDiskController::command_write);
                    }
                    0b000_01011 => {
                        // Seek
                        log::trace!("Received Seek Command");
                        self.set_command(Command::Seek, DBC_LEN, HardDiskController::command_seek);
                    }
                    0b000_01100 => {
                        // Iniitialize Drive Characteristics
                        log::trace!("Received Initialize DC Command");
                        self.set_command(Command::Initialize, DBC_LEN + IDC_LEN, HardDiskController::command_initialize_dc);
                    }
                    0b000_01101 => {
                        // Read ECC Burst Length
                        log::trace!("Received ECC Burst Length Command");
                    }
                    0b000_01110 => {
                        // Read Data From Sector Buffer
                        log::trace!("Received Read Sector Buffer Command");
                        self.set_command(Command::ReadSectorBuffer, DBC_LEN, HardDiskController::command_read_sector_buffer);
                    }
                    0b000_01111 => {
                        // Write Data to Sector Buffer
                        log::trace!("Received Write Sector Buffer Command");
                        self.set_command(Command::WriteSectorBuffer, DBC_LEN, HardDiskController::command_write_sector_buffer);
                    }
                    0b111_00000 => {
                        // RAM Diagnostic
                        log::trace!("Received RAM Diagnostic Command");
                        self.set_command(Command::RamDiagnostic, DBC_LEN, HardDiskController::command_ram_diagnostic);
                    }
                    0b111_00011 => {
                        // Drive Diagnostic
                        log::trace!("Received Drive Diagnostic Command");
                        self.set_command(Command::DriveDiagnostic, DBC_LEN, HardDiskController::command_drive_diagnostic);
                    }
                    0b111_00100 => {
                        // Controller Diagnostic
                        log::trace!("Received Controller Diagnostic Command");
                        self.set_command(Command::ControllerDiagnostic, DBC_LEN, HardDiskController::command_controller_diagnostic);
                    }
                    0b111_00101 => {
                        // Read Long Track
                        log::trace!("Received Read Long Track Command");
                    }
                    0b111_00110 => {
                        // Write Long Track
                        log::trace!("Received Write Long Track Command");
                    }
                    _ => {
                        log::error!("Unknown command received: {:02X}", byte);
                        // Unknown Command
                    }
                }
            }
            State::ReceivingCommand => {
                // If we are expecting another byte for this command, read it in.
                if self.command_byte_n > 0 {
                    self.data_register_in.push_back(byte);
                    //log::trace!("Remaining command bytes: {}", self.command_byte_n );
                    self.command_byte_n -= 1;
                }

                if self.command_byte_n == 0 {
                    // We read last byte expected for this command, so dispatch to the appropriate command handler
                    let mut result = Continuation::CommandComplete;
                    
                    match self.command_fn {
                        None => log::error!("No associated method for command: {:?}!", self.command),
                        Some(command_fn) => {
                            result = command_fn(self, bus);
                        }
                    }

                    // Clear command if complete
                    if let Continuation::CommandComplete = result {

                        if let Command::RequestSense = self.command {
                            // Present Sense Bytes after Sense Status command
                            self.state = State::HaveSenseBytes
                        }
                        else {
                            // Any other command, present status byte
                            self.state = State::HaveCommandStatus;
                        }
                        
                        // Allow commands to ignore unneeded bytes in DCB by clearing it now
                        self.data_register_in.clear();

                        self.last_command = self.command;
                        self.command = Command::None;
                        self.command_fn = None;
                    }
                }
            }
            _=> {
                log::error!("Unexpected write to data register.");
            }
        }
    }

    pub fn handle_status_register_read(&mut self) -> u8 {
        let mut out_byte;

        out_byte = match self.state {
            State::Reset => {
                0
            },
            State::HaveCommandStatus => {
                // Present mask 0b0000_1111
                R1_STATUS_REQ | R1_STATUS_IOMODE | R1_STATUS_BUS | R1_STATUS_BUSY
            },
            State::WaitingForCommand => {
                R1_STATUS_BUSY | R1_STATUS_BUS | R1_STATUS_REQ
            }
            State::ReceivingCommand => {
                // We are still receiving command bytes, so a status register read is generally unexpected.
                // There is one command that stops and checks the status register after sending the DBC,
                // Initialize Drive Characteristics. It looks for the 0b0000_1001 in the status register
                // before sending the 8 remaining bytes of the command.
                //
                // All other reads of the status register during the command receive phase should be invalid.
                match self.command {
                    Command::Initialize => {
                        // BIOS Specifies this mask (0b0000_1001)
                        R1_STATUS_REQ | R1_STATUS_BUSY
                    },
                    _ => {
                        log::error!("Unexpected status register read during incomplete command receive phase.");
                        R1_STATUS_REQ | R1_STATUS_BUSY
                    }
                }                
            }
            State::ExecutingCommand => {
                R1_STATUS_BUS | R1_STATUS_BUSY
            }
            State::HaveSenseBytes => {
                /* IBM BIOS waits for mask 1011 before reading Sense Bytes, Line 1322 */
                R1_STATUS_REQ | R1_STATUS_IOMODE | R1_STATUS_BUSY
            }
            _=> {
                panic!("Invalid HDC state!");
            }
        };

        if self.interrupt_active {
            //log::trace!(">>> Sending interrupt bit");
            out_byte |= R1_STATUS_INT; 
        }

        //log::trace!("Read status register: {:02}", out_byte);
        out_byte
    }

    fn handle_dip_register_read(&mut self) -> u8 {
        DRIVE_TYPE2_DIP
    }

    /// Return a boolean representing whether a virtual drive is mounted for the specified drive number
    fn drive_present(&mut self, drive_n: usize) -> bool {

        self.drives[drive_n].vhd.is_some()
    }

    /// Perform the Sensee Status command
    fn command_sense_status(&mut self, bus: &mut BusInterface) -> Continuation {

        let dcb = self.read_dcb();
        self.data_register_in.clear();

        let byte0 = match self.last_error {
            OperationError::NoError => 0,
            OperationError::NoReadySignal => ERR_NO_READY_SIGNAL,
            OperationError::InvalidCommand => ERR_INVALID_COMMAND,
            OperationError::IllegalAccess => ERR_ILLEGAL_ACCESS,
        };

        /* The controller BIOS source listing provides the following table for sense byte format        
            ;---------------------------------------------------;
            ;                 SENSE STATUS BYTES                ;
            ;                                                   ;
            ;       BYTE 0                                      ;
            ;           BIT     7   ADDRESS VALID, WHEN SET     ;
            ;           BIT     6   SPARE, SET TO ZERO          ;
            ;           BITS  5-4   ERROR TYPE                  ;
            ;           BITS  3-0   ERROR CODE                  ;
            ;                                                   ;
            ;      BYTE 1                                       ;
            ;           BITS  7-6   ZERO                        ;
            ;           BIT     5   DRIVE (0-1)                 ;
            ;           BITS  4-0   HEAD NUMBER                 ;
            ;                                                   ;
            ;      BYTE 2                                       ;
            ;           BITS  7-5   CYLINDER HIGH               ;
            ;           BITS  4-0   SECTOR NUMBER               ;
            ;                                                   ;
            ;      BYTE 3                                       ;
            ;           BITS  7-0   CYLINDER LOW                ;
            ;---------------------------------------------------;

            Certain fields like sector number vary in size compared to the equivalent fields in the DCB.
        */
        let byte1 = (dcb.drive_select << 5) as u8 | (self.drives[dcb.drive_select].head & 0x1F);
        let byte2 = (self.drives[dcb.drive_select].cylinder & 0x700 >> 3) as u8 | self.drives[dcb.drive_select].sector & 0x1F;
        let byte3 = (self.drives[dcb.drive_select].cylinder & 0xFF) as u8;
    
        self.data_register_out.push_back(byte0);
        self.data_register_out.push_back(byte1);
        self.data_register_out.push_back(byte2);
        self.data_register_out.push_back(byte3);
        
        self.set_error(OperationError::NoError, dcb.drive_select);
        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Perform the Read Sector command.
    fn command_read(&mut self, bus: &mut BusInterface) -> Continuation {

        let dcb = self.read_dcb();
        self.data_register_in.clear();

        let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        log::trace!("Command Read: drive: {} c: {} h: {} s: {}, xfer_size:{}", 
            dcb.drive_select, 
            dcb.c, 
            dcb.h,
            dcb.s,
            xfer_size);

        // Prime the Sector Buffer with an intitial sector read
        match &mut self.drives[dcb.drive_select].vhd {
            Some(vhd) => {
                if let Err(e) = 
                    vhd.read_sector(&mut self.drives[dcb.drive_select].sector_buf, 
                        dcb.c, 
                        dcb.h, 
                        dcb.s) 
                    {
                        log::error!("VHD read_sector() failed: c:{} h:{} s:{} Error: {}", dcb.c, dcb.h, dcb.s, e);
                    }
            }
            None => {
                // No VHD? Handle error stage for read command
            }
        }
        
        if xfer_size % SECTOR_SIZE != 0 {
            log::warn!("Command Read: DMA word count not multiple of sector size");
        }

        self.drive_select = dcb.drive_select;

        // Check drive status
        if self.drive_present(dcb.drive_select) {
            self.set_error(OperationError::NoError, dcb.drive_select);      
            
            // Set up Operation 
            self.operation_status.buffer_idx = 0;
            self.drives[self.drive_select].cylinder = dcb.c;
            self.drives[self.drive_select].head = dcb.h;
            self.drives[self.drive_select].sector = dcb.s;
            //self.command_status.block_ct = block_count;
            self.operation_status.block_n = 0;
            self.operation_status.dma_bytes_left = xfer_size;
            self.operation_status.dma_byte_count = 0;

            self.state = State::ExecutingCommand;
            self.send_dreq = true;

            // Keep running until DMA transfer is complete
            Continuation::ContinueAsOperation            
        }
        else {
            // No drive present - Fail immediately
            self.set_error(OperationError::NoReadySignal, dcb.drive_select);
            self.send_interrupt = true;
            Continuation::CommandComplete
        }

    }

    /// Perform the Write Sector command.
    fn command_write(&mut self, bus: &mut BusInterface) -> Continuation {

        let _cmd_bytes = &self.data_register_in;
        let dcb = self.read_dcb();
        self.data_register_in.clear();

        let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        log::trace!("Command Write: drive: {} c: {} h: {} s: {} bc: {}, xfer_size:{}", 
            dcb.drive_select, 
            dcb.c, 
            dcb.h,
            dcb.s,
            dcb.block_count,
            xfer_size);

        if xfer_size % SECTOR_SIZE != 0 {
            log::warn!("Command Write: DMA word count not multiple of sector size");
        }

        self.drive_select = dcb.drive_select;

        // Check drive status
        if self.drive_present(dcb.drive_select) {

            // Set up Operation 
            self.operation_status.buffer_idx = 0;
            self.drives[self.drive_select].cylinder = dcb.c;
            self.drives[self.drive_select].head = dcb.h;
            self.drives[self.drive_select].sector = dcb.s;
            
            self.operation_status.block_ct = dcb.block_count;
            self.operation_status.block_n = 0;

            self.operation_status.dma_bytes_left = xfer_size;
            self.operation_status.dma_byte_count = 0;

            self.state = State::ExecutingCommand;
            self.send_dreq = true;

            // Keep running until DMA transfer is complete'
            Continuation::ContinueAsOperation       
        }
        else {            
            // No drive present - Fail immediately
            self.set_error(OperationError::NoReadySignal, dcb.drive_select);
            self.send_interrupt = true;
            Continuation::CommandComplete            
        } 
    }

    /// Perform the Seek command.
    fn command_seek(&mut self, bus: &mut BusInterface) -> Continuation {

        let dcb = self.read_dcb();
        self.data_register_in.clear();

        log::trace!("Command Seek: drive: {} c: {} h: {}", 
            dcb.drive_select, 
            dcb.c, 
            dcb.h);

        self.drive_select = dcb.drive_select;

        // Check drive status
        if self.drive_present(dcb.drive_select) {
            
            self.drives[self.drive_select].cylinder = dcb.c;
            self.drives[self.drive_select].head = dcb.h;
            // Seek does not specify a sector - we can only seek to the first sector on a track
            self.drives[self.drive_select].sector = 0;

            self.set_error(OperationError::NoError, dcb.drive_select);      
        }
        else {
            // No drive present - Fail immediately
            self.set_error(OperationError::NoReadySignal, dcb.drive_select);    
        }

        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Perform the Ready Verify command.
    fn command_ready_verify(&mut self, bus: &mut BusInterface) -> Continuation {

        let _cmd_bytes = &self.data_register_in;
        let dcb = self.read_dcb();
        self.data_register_in.clear();

        log::trace!("Command Ready Verify: drive: {} c: {} h: {} s: {}, step: {} disable_retry: {}", 
            dcb.drive_select,
            dcb.c, 
            dcb.h, 
            dcb.s,
            dcb.step, 
            dcb.disable_retry);

        // Set failure status if no drive is present
        if self.drive_present(dcb.drive_select) {
            self.set_error(OperationError::NoError, dcb.drive_select);            
        }
        else {
            self.set_error(OperationError::NoReadySignal, dcb.drive_select);
        }

        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Perform the Read Sector Buffer command.
    /// 
    fn command_read_sector_buffer(&mut self, bus: &mut BusInterface) -> Continuation {
        // Don't care about DBC bytes

        let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        if xfer_size != SECTOR_SIZE {
            log::warn!("Command ReadSectorBuffer: DMA word count != sector size");
        }
        self.operation_status.dma_bytes_left = xfer_size;
        self.operation_status.dma_byte_count = 0;

        log::trace!("Command ReadSectorBuffer: DMA xfer size: {}", xfer_size);

        self.state = State::ExecutingCommand;
        self.send_dreq = true;

        // Keep running until DMA transfer is complete
        Continuation::ContinueAsOperation
    }

    /// Perform the Write Sector Buffer command.
    /// 
    fn command_write_sector_buffer(&mut self, bus: &mut BusInterface) -> Continuation {
        // Don't care about DBC bytes

        let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        if xfer_size != SECTOR_SIZE {
            log::warn!("Command WriteSectorBuffer: DMA word count != sector size");
        }
        self.operation_status.dma_bytes_left = xfer_size;
        self.operation_status.dma_byte_count = 0;

        log::trace!("Command WriteSectorBuffer: DMA xfer size: {}", xfer_size);

        self.state = State::ExecutingCommand;
        self.send_dreq = true;

        // Keep running until DMA transfer is complete
        Continuation::ContinueAsOperation
    }

    /// Perform the Initialize Drive Characteristics Command.
    /// This command will never produce an error code.
    fn command_initialize_dc(&mut self, _bus: &mut BusInterface) -> Continuation {
        
        let dcb = self.read_dcb();
        let data_bytes = &self.data_register_in;

        // Read Initialization Bytes
        let max_cylinders = (data_bytes[5] as u16) << 8 | data_bytes[6] as u16;
        let max_heads = data_bytes[7];
        let srwcc: u16 = (data_bytes[8] as u16) << 8 | data_bytes[9] as u16;
        let wpcc: u16 = (data_bytes[10] as u16) << 8 | data_bytes[11] as u16;
        let ecc = data_bytes[12];

        self.data_register_in.clear();

        log::trace!("Drive characteristics: drive: {}, ecc: {}, wpcc: {}, rwcc: {} max_heads: {} max_cylinders: {}", 
            dcb.drive_select, 
            ecc, 
            wpcc, 
            srwcc, 
            max_heads, 
            max_cylinders);

        // HDC BIOS seems to indicate it expects this command to succeed even on an unattached drive. After all
        // there is no jumper setting for "No Drive"
        log::trace!("Completed Initialize Drive Characteristics: Drive: {}", dcb.drive_select);
        self.set_error(OperationError::NoError, dcb.drive_select);            

        // int13h function 09h does NOT call WAIT_INT, implying this command does not send an interrupt.
        Continuation::CommandComplete
    }

    /// Perform the Test Drive Ready Command.
    fn command_test_drive_ready(&mut self, _bus: &mut BusInterface) -> Continuation {

        // Get the drive number from DCB
        let dcb = self.read_dcb();
        self.data_register_in.clear();
        
        // We should return failure if there is no VHD associated with this drive as no drive is present to test.
        if self.drive_present(dcb.drive_select) {
            self.set_error(OperationError::NoError, dcb.drive_select);
        }
        else {
            self.set_error(OperationError::NoReadySignal, dcb.drive_select);
        }
        self.last_error = OperationError::NoError;
        self.send_interrupt = true;

        // Normally we would fail if there is no VHD present, however, the HDC BIOS stubbornly retries 
        // a drive for 25 seconds during POST, which is an unacceptable delay for if someone just wants 
        // to boot a floppy...

        //if self.drive_present(dcb.drive_select) {
        //    log::trace!("Completed Test Drive Command: Drive: {}", dcb.drive_select);
        //    self.set_error(OperationError::NoError, dcb.drive_select);            
        //}               
        //else {
        //    log::trace!("Failed Test Drive Command: Drive Not Present: {}", dcb.drive_select);
        //    self.set_error(OperationError::NoReadySignal, dcb.drive_select);
        //}
        
        log::trace!("Completed Test Drive Command: Drive: {}", dcb.drive_select);
        self.set_error(OperationError::NoError, dcb.drive_select);       

        Continuation::CommandComplete
    }

    /// Perform the Recalibrate Command.
    /// This command will never produce an error code.
    fn command_recalibrate(&mut self, _bus: &mut BusInterface) -> Continuation {

        // Get the drive number from DCB
        let cmd_bytes = &self.data_register_in;
        let drive_select = (cmd_bytes[0] >> 5) & 0x01;

        self.last_error = OperationError::NoError;
        self.send_interrupt = true;

        log::trace!("Completed Recalibrate Command, Drive: {}", drive_select);
        Continuation::CommandComplete
    }

    /// Perform the Controller RAM Diagonstic Command. 
    /// This command will never produce an error code.
    fn command_ram_diagnostic(&mut self, _bus: &mut BusInterface) -> Continuation {

        self.last_error = OperationError::NoError;
        self.send_interrupt = true;
        log::trace!("Completed RAM Diagnostic Command");
        Continuation::CommandComplete
    }

    /// Perform the Drive Diagonstic Command. 
    /// Should this fail when a VHD is not attached?
    fn command_drive_diagnostic(&mut self, _bus: &mut BusInterface) -> Continuation {

        self.last_error = OperationError::NoError;
        self.send_interrupt = true;
        log::trace!("Completed Drive Diagnostic Command");
        Continuation::CommandComplete
    }

    /// Perform the Controller Diagonstic Command. 
    /// This command will never produce an error code.
    fn command_controller_diagnostic(&mut self, _bus: &mut BusInterface) -> Continuation {

        self.last_error = OperationError::NoError;
        self.send_interrupt = true;
        log::trace!("Completed Controller Diagnostic Command");
        Continuation::CommandComplete
    }

    /// End a Command that utilized DMA service.
    fn end_dma_command(&mut self, _drive: u32, error: bool ) {

        self.clear_dreq = true;
        self.operation_status.dma_byte_count = 0;
        self.operation_status.dma_bytes_left = 0;

        self.error_flag = error;
        self.send_interrupt = true;
        log::trace!("End of DMA command. Changing state to HaveCommandStatus");
        self.state = State::HaveCommandStatus;
    }

    /// Process the Write Sector Buffer operation.
    /// This operation continues until the DMA transfer is complete.
    fn opearation_write_sector_buffer(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {

            if self.operation_status.dma_bytes_left > 0 {
                // Bytes left to transfer
                let _byte = dma.do_dma_read_u8(bus, HDC_DMA);
                self.operation_status.dma_byte_count += 1;
                self.operation_status.dma_bytes_left -= 1;
                
                // See if we are done based on DMA controller
                let tc = dma.check_terminal_count(HDC_DMA);
                if tc {
                    log::trace!("DMA terminal count triggered end of WriteSectorBuffer command.");
                    if self.operation_status.dma_bytes_left != 0 {
                        log::warn!("Incomplete DMA transfer on terminal count!")
                    }

                    log::trace!("Completed WriteSectorBuffer command.");
                    self.end_dma_command(0, false);
                }
            }
            else {
                // No more bytes left to transfer. Finalize operation
                let tc = dma.check_terminal_count(HDC_DMA);
                if !tc {
                    log::warn!("WriteSectorBuffer complete without DMA terminal count.");
                }

                log::trace!("Completed WriteSectorBuffer command.");
                self.end_dma_command(0, false);                                
            }
        }
        else if !self.dreq_active {
            log::error!("Error: WriteSectorBuffer command without DMA active!")
        }   
    }

    /// Process the Read Sector operation. 
    /// This operation continues until the DMA transfer is complete.
    fn operation_read_sector(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {

            if self.operation_status.dma_bytes_left > 0 {
                // Bytes left to transfer

                let byte = self.drives[self.drive_select].sector_buf[self.operation_status.buffer_idx];
                dma.do_dma_write_u8(bus, HDC_DMA,byte);
                self.operation_status.buffer_idx += 1;
                self.operation_status.dma_byte_count += 1;
                self.operation_status.dma_bytes_left -= 1;

                // Exhausted the sector buffer, read more from disk
                if self.operation_status.buffer_idx == SECTOR_SIZE {

                    // Advance to next sector
                    //log::trace!("Command Read: Advancing to next sector...");
                    let(new_c, new_h, new_s) = self.drives[self.drive_select].get_next_sector(
                        self.drives[self.drive_select].cylinder,
                        self.drives[self.drive_select].head,
                        self.drives[self.drive_select].sector);

                    self.drives[self.drive_select].cylinder = new_c;
                    self.drives[self.drive_select].head = new_h;
                    self.drives[self.drive_select].sector = new_s;
                    self.operation_status.buffer_idx = 0;

                    match &mut self.drives[self.drive_select].vhd {
                        Some(vhd) => {
                            match vhd.read_sector(&mut self.drives[self.drive_select].sector_buf,
                                self.drives[self.drive_select].cylinder,
                                self.drives[self.drive_select].head,
                                self.drives[self.drive_select].sector) {

                                    Ok(_) => {
                                        // Sector read successful
                                    }
                                    Err(err) => {
                                        log::error!("Sector read failed: {}", err);
                                    }
                                };
                        }
                        None => {
                            log::error!("Read operation without VHD mounted.");
                        }
                    }
                }

                // See if we are done based on DMA controller
                let tc = dma.check_terminal_count(HDC_DMA);
                if tc {
                    log::trace!("DMA terminal count triggered end of Read command.");
                    if self.operation_status.dma_bytes_left != 0 {
                        log::warn!("Incomplete DMA transfer on terminal count!")
                    }

                    log::trace!("Completed Read Command");
                    self.end_dma_command(0, false);
                }
            }
            else {
                // No more bytes left to transfer. Finalize operation
                let tc = dma.check_terminal_count(HDC_DMA);
                if !tc {
                    log::warn!("Command Read complete without DMA terminal count.");
                }

                log::trace!("Completed Read Command");
                self.end_dma_command(0, false);                                
            }
        }
        else if !self.dreq_active {
            log::error!("Error: Read command without DMA active!")
        }   
    }

    fn operation_write_sector(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {

            if self.operation_status.dma_bytes_left > 0 {
                // Bytes left to transfer

                let byte = dma.do_dma_read_u8(bus, HDC_DMA);
                self.drives[self.drive_select].sector_buf[self.operation_status.buffer_idx] = byte;
                self.operation_status.buffer_idx += 1;
                self.operation_status.dma_byte_count += 1;
                self.operation_status.dma_bytes_left -= 1;

                // Filled the sector buffer, write it to disk
                if self.operation_status.buffer_idx == SECTOR_SIZE {
                    
                    match &mut self.drives[self.drive_select].vhd {
                        Some(vhd) => {
                            match vhd.write_sector(&self.drives[self.drive_select].sector_buf,
                                self.drives[self.drive_select].cylinder,
                                self.drives[self.drive_select].head,
                                self.drives[self.drive_select].sector) {

                                    Ok(_) => {
                                        // Sector write successful
                                    }
                                    Err(err) => {
                                        log::error!("Sector write failed: {}", err);
                                    }
                                };
                        }
                        None => {
                            log::error!("Write operation without VHD mounted.");
                        }
                    }

                    // Advance to next sector
                    log::trace!("Command Write: Advancing to next sector...");
                    let(new_c, new_h, new_s) = self.drives[self.drive_select].get_next_sector(
                        self.drives[self.drive_select].cylinder,
                        self.drives[self.drive_select].head,
                        self.drives[self.drive_select].sector);

                    self.drives[self.drive_select].cylinder = new_c;
                    self.drives[self.drive_select].head = new_h;
                    self.drives[self.drive_select].sector = new_s;
                    self.operation_status.buffer_idx = 0;
                }

                // See if we are done based on DMA controller
                let tc = dma.check_terminal_count(HDC_DMA);
                if tc {
                    log::trace!("DMA terminal count triggered end of Write command.");
                    if self.operation_status.dma_bytes_left != 0 {
                        log::warn!("Incomplete DMA transfer on terminal count!")
                    }

                    self.end_dma_command(0, false);
                }
            }
            else {
                // No more bytes left to transfer. Finalize operation
                let tc = dma.check_terminal_count(HDC_DMA);
                if !tc {
                    log::warn!("Command Write complete without DMA terminal count.");
                }

                self.end_dma_command(0, false);                                
            }
        }
        else if !self.dreq_active {
            log::error!("Error: Read command without DMA active!")
        }                       
    }

    /// Run the HDC device.
    pub fn run(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface, _us: f64 ) {

        // Handle interrupts
        if self.send_interrupt {
            if self.irq_enabled {
                //log::trace!(">>> Firing HDC IRQ 5");
                bus.pic_mut().as_mut().unwrap().request_interrupt(HDC_IRQ);
                self.send_interrupt = false;
                self.interrupt_active = true;

            }
            else {
                //log::trace!(">>> IRQ was masked");
                self.send_interrupt = false;
                self.interrupt_active = false;
            }

        }

        if self.clear_interrupt {
            bus.pic_mut().as_mut().unwrap().clear_interrupt(HDC_IRQ);
            self.clear_interrupt = false;
            self.interrupt_active = false;
        }
        
        if self.send_dreq {
            dma.request_service(HDC_DMA);
            self.send_dreq = false;
            self.dreq_active = true;
        }

        if self.clear_dreq {
            dma.clear_service(HDC_DMA);
            self.clear_dreq = false;
            self.dreq_active = false;
        }

        // Process any running Operations
        match self.state {

            State::ExecutingCommand => {
                match self.command {
                    Command::WriteSectorBuffer => {
                        self.opearation_write_sector_buffer(dma, bus);
                    }
                    Command::Read => {
                        self.operation_read_sector(dma, bus);
                    }
                    Command::Write => {
                        self.operation_write_sector(dma, bus);
                    }                    
                    _ => panic!("Unexpected command")
                }
            }
            _ => {
                // Unhandled state
            }
        }
    }

}