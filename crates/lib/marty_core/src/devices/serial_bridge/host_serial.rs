/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
*/

//! # HostSerialPortBridgeStream
//!
//! Implements [SerialPortBridgeStream] for host serial ports.

#[cfg(feature = "serial")]
use std::{
    io::{self, Read, Write},
    time::Duration,
};

#[cfg(feature = "serial")]
use super::{BridgeIo, SerialPortBridgeStream};
use super::{ConnectResult, FlowControlType, ParityType};

#[cfg(feature = "serial")]
struct HostSerialPortBridgeStream {
    port: Box<dyn serialport::SerialPort>,
}

#[cfg(feature = "serial")]
impl SerialPortBridgeStream for HostSerialPortBridgeStream {
    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<BridgeIo> {
        let available = self.port.bytes_to_read().map_err(io::Error::other)?;
        if available == 0 {
            return Ok(BridgeIo::Idle);
        }

        match self.port.read(buf) {
            Ok(0) => Ok(BridgeIo::Idle),
            Ok(count) => Ok(BridgeIo::Transferred(count)),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(BridgeIo::Idle)
            }
            Err(error) => Err(error),
        }
    }

    fn try_write(&mut self, buf: &[u8]) -> io::Result<BridgeIo> {
        match self.port.write(buf) {
            Ok(0) => Ok(BridgeIo::Idle),
            Ok(count) => Ok(BridgeIo::Transferred(count)),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(BridgeIo::Idle)
            }
            Err(error) => Err(error),
        }
    }

    fn set_modem_control(&mut self, dtr: bool, rts: bool) -> io::Result<()> {
        self.port.write_data_terminal_ready(dtr).map_err(io::Error::other)?;
        self.port.write_request_to_send(rts).map_err(io::Error::other)
    }
}

#[cfg(feature = "serial")]
pub(super) fn open_host_serial_stream(
    port_name: &str,
    baud_rate: u32,
    stop_bits: u32,
    data_bits: u32,
    parity: ParityType,
    flow_control: FlowControlType,
) -> ConnectResult {
    let stop_bits = match stop_bits {
        1 => serialport::StopBits::One,
        2 => serialport::StopBits::Two,
        value => return Err(format!("invalid stop-bit count {value}; expected 1 or 2")),
    };
    let data_bits = match data_bits {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        8 => serialport::DataBits::Eight,
        value => return Err(format!("invalid data-bit count {value}; expected 5, 6, 7, or 8")),
    };
    let parity = match parity {
        ParityType::Even => serialport::Parity::Even,
        ParityType::Odd => serialport::Parity::Odd,
        ParityType::None => serialport::Parity::None,
    };
    let flow_control = match flow_control {
        FlowControlType::Hardware => serialport::FlowControl::Hardware,
        FlowControlType::Software => serialport::FlowControl::Software,
        FlowControlType::None => serialport::FlowControl::None,
    };

    let mut port = serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(1))
        .dtr_on_open(false)
        .stop_bits(stop_bits)
        .data_bits(data_bits)
        .parity(parity)
        .flow_control(flow_control)
        .open()
        .map_err(|error| format!("failed to open host serial port '{port_name}': {error}"))?;

    let _ = port.write_data_terminal_ready(false);
    Ok(Box::new(HostSerialPortBridgeStream { port }))
}

#[cfg(not(feature = "serial"))]
pub(super) fn open_host_serial_stream(
    port_name: &str,
    _baud_rate: u32,
    _stop_bits: u32,
    _data_bits: u32,
    _parity: ParityType,
    _flow_control: FlowControlType,
) -> ConnectResult {
    Err(format!(
        "host serial bridge '{port_name}' requested, but MartyPC was built without serial-port feature"
    ))
}
