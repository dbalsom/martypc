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

    ---------------------------------------------------------------------------

    This module implements a serial client for the arduino8088 serial CPU server.
    See https://github.com/dbalsom/arduino_8088 for more details.

    This client is utilized by the 'arduino8088_validator' cpu validator module
*/
#![allow(dead_code, unused_variables)]

use log;
use serialport::ClearBuffer;
use web_time::Duration;

pub const ARD8088_BAUD: u32 = 0; // Baud is inapplicable for SerialUSB

//pub const ARD8088_BAUD: u32 = 2000000;
//pub const ARD8088_BAUD: u32 = 460800;

#[derive(Copy, Clone)]
pub enum ServerCommand {
    CmdNull = 0x00,
    CmdVersion = 0x01,
    CmdReset = 0x02,
    CmdLoad = 0x03,
    CmdCycle = 0x04,
    CmdReadAddressLatch = 0x05,
    CmdReadStatus = 0x06,
    CmdRead8288Command = 0x07,
    CmdRead8288Control = 0x08,
    CmdReadDataBus = 0x09,
    CmdWriteDataBus = 0x0A,
    CmdFinalize = 0x0B,
    CmdBeginStore = 0x0C,
    CmdStore = 0x0D,
    CmdQueueLen = 0x0E,
    CmdQueueBytes = 0x0F,
    CmdWritePin = 0x10,
    CmdReadPin = 0x11,
    CmdGetProgramState = 0x12,
    CmdGetLastError = 0x13,
    CmdGetCycleState = 0x14,
    CmdCGetCycleState = 0x15,
    CmdPrefetchStore = 0x16,
    CmdReadAddressU = 0x17,
    CmdInvalid,
}

#[derive(Debug, PartialEq)]
pub enum ValidatorState {
    Reset = 0,
    JumpVector,
    Load,
    LoadDone,
    Execute,
    ExecuteFinalize,
    ExecuteDone,
    Store,
    StoreDone,
    Done,
}

#[derive(PartialEq)]
pub enum Segment {
    ES = 0,
    SS,
    CS,
    DS,
}

pub const REQUIRED_PROTOCOL_VER: u8 = 0x01;

pub const COMMAND_MRDC_BIT: u8 = 0b0000_0001;
pub const COMMAND_AMWC_BIT: u8 = 0b0000_0010;
pub const COMMAND_MWTC_BIT: u8 = 0b0000_0100;
pub const COMMAND_IORC_BIT: u8 = 0b0000_1000;
pub const COMMAND_AIOWC_BIT: u8 = 0b0001_0000;
pub const COMMAND_IOWC_BIT: u8 = 0b0010_0000;
pub const COMMAND_INTA_BIT: u8 = 0b0100_0000;
pub const COMMAND_ALE_BIT: u8 = 0b1000_0000;

pub const STATUS_SEG_BITS: u8 = 0b0001_1000;

macro_rules! get_segment {
    ($s:expr) => {
        match (($s >> 3) & 0x03) {
            0b00 => Segment::ES,
            0b01 => Segment::SS,
            0b10 => Segment::CS,
            _ => Segment::DS,
        }
    };
}

macro_rules! get_access_type {
    ($s:expr) => {
        match (($s >> 3) & 0x03) {
            0b00 => AccessType::AlternateData,
            0b01 => AccessType::Stack,
            0b10 => AccessType::CodeOrNone,
            _ => AccessType::Data,
        }
    };
}

macro_rules! get_bus_state {
    ($s:expr) => {
        match ($s & 0x07) {
            0 => BusState::INTA,
            1 => BusState::IOR,
            2 => BusState::IOW,
            3 => BusState::HALT,
            4 => BusState::CODE,
            5 => BusState::MEMR,
            6 => BusState::MEMW,
            _ => BusState::PASV,
        }
    };
}

macro_rules! get_queue_op {
    ($s:expr) => {
        match (($s >> 6) & 0x03) {
            0b00 => QueueOp::Idle,
            0b01 => QueueOp::First,
            0b10 => QueueOp::Flush,
            _ => QueueOp::Subsequent,
        }
    };
}

macro_rules! is_reading {
    ($s:expr) => {
        match ((!($s) & 0b0000_1001) != 0) {
            true => true,
            false => false,
        }
    };
}

macro_rules! is_writing {
    ($s:expr) => {
        match ((!($s) & 0b0011_0110) != 0) {
            true => true,
            false => false,
        }
    };
}

use std::{cell::RefCell, error::Error, fmt::Display, rc::Rc, str};

#[derive(Debug)]
pub enum CpuClientError {
    ReadFailure,
    WriteFailure,
    BadValue,
    ReadTimeout,
    EnumerationError,
    DiscoveryError,
    CommandFailed,
}

impl Error for CpuClientError {}
impl Display for CpuClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            CpuClientError::ReadFailure => {
                write!(f, "Failed to read from serial port.")
            }
            CpuClientError::WriteFailure => {
                write!(f, "Failed to write to serial port.")
            }
            CpuClientError::BadValue => {
                write!(f, "Received invalid value from command.")
            }
            CpuClientError::ReadTimeout => {
                write!(f, "Response timeout.")
            }
            CpuClientError::EnumerationError => {
                write!(f, "Failed to find a valid serial port.")
            }
            CpuClientError::DiscoveryError => {
                write!(f, "Failed to find a listening Arduino8088 server.")
            }
            CpuClientError::CommandFailed => {
                write!(f, "Server command returned failure code.")
            }
        }
    }
}

pub struct CpuClient {
    port: Rc<RefCell<Box<dyn serialport::SerialPort>>>,
}

impl CpuClient {
    pub fn init(baud_rate: u32) -> Result<CpuClient, CpuClientError> {
        match serialport::available_ports() {
            Ok(ports) => {
                for port in ports {
                    log::trace!("Found serial port: {}", port.port_name);
                    if let Some(rtk_port) = CpuClient::try_port(port, baud_rate) {
                        return Ok(CpuClient {
                            port: Rc::new(RefCell::new(rtk_port)),
                        });
                    }
                }
            }
            Err(e) => {
                log::error!("Didn't find any serial ports: {:?}", e);
                return Err(CpuClientError::EnumerationError);
            }
        };
        Err(CpuClientError::DiscoveryError)
    }

    /// Try to access an Arduino8088 on the specified port. Return the port if successful, otherwise None.
    pub fn try_port(port_info: serialport::SerialPortInfo, baud_rate: u32) -> Option<Box<dyn serialport::SerialPort>> {
        let port_result = serialport::new(port_info.port_name.clone(), baud_rate)
            .baud_rate(baud_rate)
            .timeout(Duration::from_millis(100))
            .stop_bits(serialport::StopBits::One)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .open();

        match port_result {
            Ok(mut new_port) => {
                //log::trace!("Successfully opened host port {}", port_info.port_name);

                // Flush
                new_port.clear(ClearBuffer::Input).unwrap();
                new_port.clear(ClearBuffer::Output).unwrap();

                let cmd: [u8; 1] = [1];
                let mut buf: [u8; 100] = [0; 100];
                match new_port.write(&cmd) {
                    Ok(_) => {
                        log::debug!("try_port: Sent discovery command.");
                    }
                    Err(_) => {
                        log::error!("try_port: Write error");
                        return None;
                    }
                }

                let bytes_read = match new_port.read(&mut buf) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        log::error!("try_port: Read error");
                        return None;
                    }
                };

                new_port.clear(serialport::ClearBuffer::Input).unwrap();
                if bytes_read == 9 {
                    let ver_text = str::from_utf8(&buf).unwrap();
                    if ver_text.contains("ard8088") {
                        let proto_ver = buf[7];
                        log::info!(
                            "Found an Arduino8088 server, protocol verison: {} on port {}",
                            proto_ver,
                            port_info.port_name
                        );

                        if proto_ver != REQUIRED_PROTOCOL_VER {
                            log::error!("Unsupported protocol version.");
                            return None;
                        }
                        return Some(new_port);
                    }
                    log::warn!("Invalid response from discovery command.",);
                    return None;
                }
                else {
                    log::warn!(
                        "Invalid response from discovery command. Read {} bytes (Expected 9).",
                        bytes_read
                    );
                }
                None
            }
            Err(e) => {
                log::error!("try_port: Error opening host port: {}", e);
                None
            }
        }
    }

    /// Send a command byte to the Arduino8088 CPU server.
    pub fn send_command_byte(&mut self, cmd: ServerCommand) -> Result<(), CpuClientError> {
        let cmd: [u8; 1] = [cmd as u8];

        self.port.borrow_mut().clear(ClearBuffer::Input).unwrap();
        match self.port.borrow_mut().write_all(&cmd) {
            Ok(_) => Ok(()),
            Err(_) => Err(CpuClientError::WriteFailure),
        }
    }

    /// Read the result code from the Arduino8088 CPU server after issuing a command.
    pub fn read_result_code(&mut self) -> Result<bool, CpuClientError> {
        let mut buf: [u8; 1] = [0; 1];

        match self.port.borrow_mut().read(&mut buf) {
            Ok(bytes) => {
                if bytes == 0 {
                    Err(CpuClientError::ReadFailure)
                }
                else if (buf[0] & 0x01) != 0 {
                    // LSB set in return code == success
                    Ok(true)
                }
                else {
                    Err(CpuClientError::CommandFailed)
                }
            }
            Err(_) => Err(CpuClientError::ReadFailure),
        }
    }

    /// Send a slice of u8 to the CPU Server.
    pub fn send_buf(&mut self, buf: &[u8]) -> Result<bool, CpuClientError> {
        match self.port.borrow_mut().write(&buf) {
            Ok(bytes) => {
                if bytes != buf.len() {
                    Err(CpuClientError::WriteFailure)
                }
                else {
                    Ok(true)
                }
            }
            Err(_) => Err(CpuClientError::WriteFailure),
        }
    }

    /// Receive a fixed number of bytes from the CPU Server.
    pub fn recv_buf(&mut self, buf: &mut [u8]) -> Result<bool, CpuClientError> {
        match self.port.borrow_mut().read(buf) {
            Ok(bytes) => {
                if bytes != buf.len() {
                    // We didn't read entire buffer worth of data, fail
                    log::error!("Only read {} bytes of {}.", bytes, buf.len());
                    Err(CpuClientError::ReadFailure)
                }
                else {
                    Ok(true)
                }
            }
            Err(e) => {
                log::error!("Read Error: {}", e);
                Err(CpuClientError::ReadFailure)
            }
        }
    }

    /// Receive a buffer of dynamic size (don't expect the entire buffer read like recv_buf does)
    /// Returns the number of bytes read.
    /// Primarily used for get_last_error
    pub fn recv_dyn_buf(&mut self, buf: &mut [u8]) -> Result<usize, CpuClientError> {
        match self.port.borrow_mut().read(buf) {
            Ok(bytes) => Ok(bytes),
            Err(_) => Err(CpuClientError::ReadFailure),
        }
    }

    /// Server command - Load
    /// Load the specified register state into the CPU.
    /// This command takes 28 bytes, which correspond to the word values of each of the 14
    /// CPU registers.
    /// Registers should be loaded in the following order, little-endian:
    ///
    /// AX, BX, CX, DX, SS, SP, FLAGS, IP, CS, DS, ES, BP, SI, DI
    pub fn load_registers_from_buf(&mut self, reg_data: &[u8]) -> Result<bool, CpuClientError> {
        self.send_command_byte(ServerCommand::CmdLoad)?;
        self.send_buf(reg_data)?;
        self.read_result_code()
    }

    pub fn begin_store(&mut self) -> Result<bool, CpuClientError> {
        self.send_command_byte(ServerCommand::CmdBeginStore)?;
        self.read_result_code()
    }

    pub fn store_registers_to_buf(&mut self, reg_data: &mut [u8]) -> Result<bool, CpuClientError> {
        self.send_command_byte(ServerCommand::CmdStore)?;
        self.recv_buf(reg_data)?;
        self.read_result_code()
    }

    /// Server command - Cycle
    /// Cycle the CPU once.
    pub fn cycle(&mut self) -> Result<bool, CpuClientError> {
        self.send_command_byte(ServerCommand::CmdCycle)?;
        self.read_result_code()
    }

    /// Server command - ReadAddress
    /// Return the value of the address latches (Latched on ALE)
    pub fn read_address_latch(&mut self) -> Result<u32, CpuClientError> {
        let mut buf: [u8; 3] = [0; 3];
        self.send_command_byte(ServerCommand::CmdReadAddressLatch)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        let address = buf[0] as u32 | (buf[1] as u32) << 8 | (buf[2] as u32) << 16;

        Ok(address)
    }

    /// Server command - ReadAddress
    /// Return the current value of the address bus
    pub fn read_address(&mut self) -> Result<u32, CpuClientError> {
        let mut buf: [u8; 3] = [0; 3];
        self.send_command_byte(ServerCommand::CmdReadAddressU)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        let address = buf[0] as u32 | (buf[1] as u32) << 8 | (buf[2] as u32) << 16;

        Ok(address)
    }

    pub fn read_status(&mut self) -> Result<u8, CpuClientError> {
        let mut buf: [u8; 1] = [0; 1];
        self.send_command_byte(ServerCommand::CmdReadStatus)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        Ok(buf[0])
    }

    pub fn read_8288_command(&mut self) -> Result<u8, CpuClientError> {
        let mut buf: [u8; 1] = [0; 1];
        self.send_command_byte(ServerCommand::CmdRead8288Command)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        Ok(buf[0])
    }

    pub fn read_8288_control(&mut self) -> Result<u8, CpuClientError> {
        let mut buf: [u8; 1] = [0; 1];
        self.send_command_byte(ServerCommand::CmdRead8288Control)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        Ok(buf[0])
    }

    pub fn read_data_bus(&mut self) -> Result<u8, CpuClientError> {
        let mut buf: [u8; 1] = [0; 1];
        self.send_command_byte(ServerCommand::CmdReadDataBus)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        Ok(buf[0])
    }

    pub fn write_data_bus(&mut self, data: u8) -> Result<bool, CpuClientError> {
        let mut buf: [u8; 1] = [0; 1];
        self.send_command_byte(ServerCommand::CmdWriteDataBus)?;
        buf[0] = data;
        self.send_buf(&mut buf)?;
        self.read_result_code()?;

        Ok(true)
    }

    pub fn write_store_pgm(&mut self) -> Result<bool, CpuClientError> {
        self.send_command_byte(ServerCommand::CmdPrefetchStore)?;
        self.read_result_code()
    }

    pub fn finalize(&mut self) -> Result<bool, CpuClientError> {
        self.send_command_byte(ServerCommand::CmdFinalize)?;
        self.read_result_code()
    }

    pub fn get_program_state(&mut self) -> Result<ValidatorState, CpuClientError> {
        let mut buf: [u8; 1] = [0; 1];
        self.send_command_byte(ServerCommand::CmdGetProgramState)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        match buf[0] {
            0x00 => Ok(ValidatorState::Reset),
            0x01 => Ok(ValidatorState::JumpVector),
            0x02 => Ok(ValidatorState::Load),
            0x03 => Ok(ValidatorState::LoadDone),
            0x04 => Ok(ValidatorState::Execute),
            0x05 => Ok(ValidatorState::ExecuteFinalize),
            0x06 => Ok(ValidatorState::ExecuteDone),
            0x07 => Ok(ValidatorState::Store),
            0x08 => Ok(ValidatorState::StoreDone),
            0x09 => Ok(ValidatorState::Done),
            _ => Err(CpuClientError::BadValue),
        }
    }

    pub fn get_last_error(&mut self) -> Result<String, CpuClientError> {
        //let mut cmdbuf: [u8; 1] = [0; 1];
        let mut errbuf: [u8; 50] = [0; 50];
        self.send_command_byte(ServerCommand::CmdGetLastError)?;
        let bytes = self.recv_dyn_buf(&mut errbuf)?;
        let err_string = str::from_utf8(&errbuf[..bytes - 1]).unwrap();

        Ok(err_string.to_string())
    }

    pub fn get_cycle_state(&mut self) -> Result<(ValidatorState, u8, u8, u8, u8), CpuClientError> {
        let mut buf: [u8; 4] = [0; 4];
        self.send_command_byte(ServerCommand::CmdGetCycleState)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        let state_bits: u8 = buf[0] >> 4;
        let state: ValidatorState = match state_bits {
            0x00 => ValidatorState::Reset,
            0x01 => ValidatorState::JumpVector,
            0x02 => ValidatorState::Load,
            0x03 => ValidatorState::LoadDone,
            0x04 => ValidatorState::Execute,
            0x05 => ValidatorState::ExecuteFinalize,
            0x06 => ValidatorState::ExecuteDone,
            0x07 => ValidatorState::Store,
            0x08 => ValidatorState::StoreDone,
            0x09 => ValidatorState::Done,
            _ => {
                return Err(CpuClientError::BadValue);
            }
        };

        let control_bits = buf[0] & 0x0F;

        Ok((state, control_bits, buf[1], buf[2], buf[3]))
    }

    pub fn cycle_get_cycle_state(&mut self) -> Result<(ValidatorState, u8, u8, u8, u8), CpuClientError> {
        let mut buf: [u8; 4] = [0; 4];
        self.send_command_byte(ServerCommand::CmdCGetCycleState)?;
        self.recv_buf(&mut buf)?;
        self.read_result_code()?;

        let state_bits: u8 = buf[0] >> 4;
        let state: ValidatorState = match state_bits {
            0x00 => ValidatorState::Reset,
            0x01 => ValidatorState::JumpVector,
            0x02 => ValidatorState::Load,
            0x03 => ValidatorState::LoadDone,
            0x04 => ValidatorState::Execute,
            0x05 => ValidatorState::ExecuteFinalize,
            0x06 => ValidatorState::ExecuteDone,
            0x07 => ValidatorState::Store,
            0x08 => ValidatorState::StoreDone,
            0x09 => ValidatorState::Done,
            _ => {
                return Err(CpuClientError::BadValue);
            }
        };

        let control_bits = buf[0] & 0x0F;

        Ok((state, control_bits, buf[1], buf[2], buf[3]))
    }
}
