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

//! # TcpSerialPortBridgeStream
//!
//! Implements [SerialPortBridgeStream] for raw TCP sockets.

use std::{
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use super::{BridgeIo, ConnectResult, SerialPortBridgeStream};

struct TcpSerialPortBridgeStream {
    stream: TcpStream,
}

impl SerialPortBridgeStream for TcpSerialPortBridgeStream {
    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<BridgeIo> {
        match self.stream.read(buf) {
            Ok(0) => Ok(BridgeIo::Disconnected),
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
        match self.stream.write(buf) {
            Ok(0) => Ok(BridgeIo::Disconnected),
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
}

pub(super) fn open_tcp_stream(address: &str, timeout: Duration) -> ConnectResult {
    let addresses = address
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve TCP address '{address}': {error}"))?;
    let mut last_error = None;

    for socket_address in addresses {
        match TcpStream::connect_timeout(&socket_address, timeout) {
            Ok(stream) => {
                stream
                    .set_nonblocking(true)
                    .map_err(|error| format!("failed to make TCP stream nonblocking: {error}"))?;
                stream
                    .set_nodelay(true)
                    .map_err(|error| format!("failed to disable Nagle's algorithm: {error}"))?;
                return Ok(Box::new(TcpSerialPortBridgeStream { stream }));
            }
            Err(error) => last_error = Some(error),
        }
    }

    Err(match last_error {
        Some(error) => format!("failed to connect to TCP endpoint '{address}': {error}"),
        None => format!("TCP address '{address}' resolved to no socket addresses"),
    })
}
