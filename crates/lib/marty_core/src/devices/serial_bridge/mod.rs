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

//! # `serial_bridge` Module

mod host_serial;
mod tcp_stream;

use std::{
    collections::VecDeque,
    fmt,
    io,
    time::{Duration, Instant},
};

use crossbeam_channel::{bounded, Receiver, TryRecvError};
use serde_derive::Deserialize;

use self::{host_serial::open_host_serial_stream, tcp_stream::open_tcp_stream};

const BRIDGE_READ_BUFFER_SIZE: usize = 1024;
const BRIDGE_READ_BUDGET: usize = 4096;
const BRIDGE_TX_QUEUE_LIMIT: usize = 16 * 1024;

const fn default_true() -> bool {
    true
}

const fn default_baud_rate() -> u32 {
    9600
}

const fn default_stop_bits() -> u32 {
    1
}

const fn default_data_bits() -> u32 {
    8
}

const fn default_reconnect_interval_ms() -> u64 {
    1000
}

const fn default_connect_timeout_ms() -> u64 {
    3000
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, strum_macros::EnumString, Deserialize)]
pub enum ParityType {
    Even,
    Odd,
    #[default]
    None,
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, strum_macros::EnumString, Deserialize)]
pub enum FlowControlType {
    #[default]
    None,
    Hardware,
    Software,
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, Deserialize)]
pub enum SerialPortBridgeConnectTrigger {
    /// Connect as soon as the bridge is created.
    #[default]
    OnStartup,
    /// Connect when the guest transmits its first byte.
    OnOutput,
    /// Connect only when explicitly requested.
    OnRequest,
}

/// A complete bridge definition for one emulated serial port.
#[derive(Clone, Debug, Deserialize)]
pub struct SerialPortBridgeConfiguration {
    pub guest_port: usize,
    #[serde(default)]
    pub connect_trigger: SerialPortBridgeConnectTrigger,
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
    #[serde(default = "default_reconnect_interval_ms")]
    pub reconnect_interval_ms: u64,
    #[serde(flatten)]
    pub target: SerialPortBridgeTarget,
}

impl SerialPortBridgeConfiguration {
    pub fn serial(guest_port: usize, port_name: String) -> Self {
        Self {
            guest_port,
            connect_trigger: SerialPortBridgeConnectTrigger::default(),
            auto_reconnect: default_true(),
            reconnect_interval_ms: default_reconnect_interval_ms(),
            target: SerialPortBridgeTarget::Serial {
                port_name,
                baud_rate: default_baud_rate(),
                stop_bits: default_stop_bits(),
                data_bits: default_data_bits(),
                parity: ParityType::default(),
                flow_control: FlowControlType::default(),
            },
        }
    }

    pub fn tcp_client(guest_port: usize, address: String) -> Self {
        Self {
            guest_port,
            connect_trigger: SerialPortBridgeConnectTrigger::default(),
            auto_reconnect: default_true(),
            reconnect_interval_ms: default_reconnect_interval_ms(),
            target: SerialPortBridgeTarget::TcpClient {
                port_name: address.clone(),
                address,
                connect_timeout_ms: default_connect_timeout_ms(),
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.auto_reconnect && self.reconnect_interval_ms == 0 {
            return Err("reconnect_interval_ms must be greater than zero".to_string());
        }

        match &self.target {
            SerialPortBridgeTarget::Serial {
                port_name,
                baud_rate,
                stop_bits,
                data_bits,
                ..
            } => {
                if port_name.trim().is_empty() {
                    return Err("serial port_name must not be empty".to_string());
                }
                if *baud_rate == 0 {
                    return Err("serial baud_rate must be greater than zero".to_string());
                }
                if !matches!(stop_bits, 1 | 2) {
                    return Err("serial stop_bits must be 1 or 2".to_string());
                }
                if !matches!(data_bits, 5..=8) {
                    return Err("serial data_bits must be between 5 and 8".to_string());
                }
            }
            SerialPortBridgeTarget::TcpClient {
                port_name,
                address,
                connect_timeout_ms,
            } => {
                if port_name.trim().is_empty() {
                    return Err("TCP port_name must not be empty".to_string());
                }
                if address.trim().is_empty() {
                    return Err("TCP address must not be empty".to_string());
                }
                if *connect_timeout_ms == 0 {
                    return Err("connect_timeout_ms must be greater than zero".to_string());
                }
            }
        }

        Ok(())
    }
}

/// The host-side endpoint for a serial bridge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum SerialPortBridgeTarget {
    Serial {
        port_name: String,
        #[serde(default = "default_baud_rate")]
        baud_rate: u32,
        #[serde(default = "default_stop_bits")]
        stop_bits: u32,
        #[serde(default = "default_data_bits")]
        data_bits: u32,
        #[serde(default)]
        parity: ParityType,
        #[serde(default)]
        flow_control: FlowControlType,
    },
    #[serde(rename = "tcp")]
    TcpClient {
        port_name: String,
        address: String,
        #[serde(default = "default_connect_timeout_ms")]
        connect_timeout_ms: u64,
    },
}

impl SerialPortBridgeTarget {
    pub fn label(&self) -> &str {
        match self {
            Self::Serial { port_name, .. } => port_name,
            Self::TcpClient { port_name, .. } => port_name,
        }
    }

    pub fn transport(&self) -> SerialPortBridgeTransport {
        match self {
            Self::Serial { .. } => SerialPortBridgeTransport::Serial,
            Self::TcpClient { .. } => SerialPortBridgeTransport::TcpClient,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SerialPortBridgeTransport {
    Serial,
    TcpClient,
}

impl fmt::Display for SerialPortBridgeTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serial => write!(f, "serial"),
            Self::TcpClient => write!(f, "TCP client"),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SerialPortBridgeState {
    WaitingForOutput,
    WaitingForRequest,
    Connecting,
    Connected,
    ReconnectPending,
    Suspended,
}

impl fmt::Display for SerialPortBridgeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WaitingForOutput => write!(f, "waiting for output"),
            Self::WaitingForRequest => write!(f, "waiting for connection request"),
            Self::Connecting => write!(f, "connecting"),
            Self::Connected => write!(f, "connected"),
            Self::ReconnectPending => write!(f, "waiting to reconnect"),
            Self::Suspended => write!(f, "disconnected"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerialPortBridgeInfo {
    pub target: String,
    pub transport: SerialPortBridgeTransport,
    pub state: SerialPortBridgeState,
    pub last_error: Option<String>,
}

/// Result of one nonblocking stream operation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BridgeIo {
    Idle,
    Transferred(usize),
    Disconnected,
}

/// A [SerialPortBridgeStream] is a byte stream backing a serial port bridge.
///
/// Implementations normalize their native nonblocking behavior into `BridgeIo`.
/// Connection creation and reconnection are intentionally owned by
/// [SerialPortBridge], since reconnecting requires constructing a fresh stream.
pub trait SerialPortBridgeStream: Send {
    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<BridgeIo>;
    fn try_write(&mut self, buf: &[u8]) -> io::Result<BridgeIo>;

    fn set_modem_control(&mut self, _dtr: bool, _rts: bool) -> io::Result<()> {
        Ok(())
    }
}

type ConnectResult = Result<Box<dyn SerialPortBridgeStream>, String>;

#[derive(Default)]
pub struct SerialPortBridgeUpdate {
    pub received: Vec<u8>,
    pub connection_changed: Option<bool>,
}

/// A [SerialPortBridge] represents a persistent bridge definition and owns a replaceable,
/// dynamic implementation of [SerialPortBridgeStream] as `stream`.
pub struct SerialPortBridge {
    target: SerialPortBridgeTarget,
    connect_trigger: SerialPortBridgeConnectTrigger,
    auto_reconnect: bool,
    reconnect_interval: Duration,
    state: SerialPortBridgeState,
    stream: Option<Box<dyn SerialPortBridgeStream>>,
    connect_receiver: Option<Receiver<ConnectResult>>,
    reconnect_at: Option<Instant>,
    tx_queue: VecDeque<u8>,
    read_buffer: Vec<u8>,
    last_modem_control: Option<(bool, bool)>,
    last_reported_connected: bool,
    last_error: Option<String>,
    tx_overflow_reported: bool,
}

impl SerialPortBridge {
    pub fn new(configuration: SerialPortBridgeConfiguration) -> Result<Self, String> {
        configuration.validate()?;
        let connect_trigger = configuration.connect_trigger;
        let mut bridge = Self {
            target: configuration.target,
            connect_trigger,
            auto_reconnect: configuration.auto_reconnect,
            reconnect_interval: Duration::from_millis(configuration.reconnect_interval_ms),
            state: match connect_trigger {
                SerialPortBridgeConnectTrigger::OnStartup => SerialPortBridgeState::ReconnectPending,
                SerialPortBridgeConnectTrigger::OnOutput => SerialPortBridgeState::WaitingForOutput,
                SerialPortBridgeConnectTrigger::OnRequest => SerialPortBridgeState::WaitingForRequest,
            },
            stream: None,
            connect_receiver: None,
            reconnect_at: match connect_trigger {
                SerialPortBridgeConnectTrigger::OnStartup => Some(Instant::now()),
                SerialPortBridgeConnectTrigger::OnOutput | SerialPortBridgeConnectTrigger::OnRequest => None,
            },
            tx_queue: VecDeque::new(),
            read_buffer: vec![0; BRIDGE_READ_BUFFER_SIZE],
            last_modem_control: None,
            last_reported_connected: false,
            last_error: None,
            tx_overflow_reported: false,
        };
        if connect_trigger == SerialPortBridgeConnectTrigger::OnStartup {
            bridge.start_connect();
        }
        Ok(bridge)
    }

    #[cfg(test)]
    fn with_stream(configuration: SerialPortBridgeConfiguration, stream: Box<dyn SerialPortBridgeStream>) -> Self {
        Self {
            target: configuration.target,
            connect_trigger: configuration.connect_trigger,
            auto_reconnect: configuration.auto_reconnect,
            reconnect_interval: Duration::from_millis(configuration.reconnect_interval_ms),
            state: SerialPortBridgeState::Connected,
            stream: Some(stream),
            connect_receiver: None,
            reconnect_at: None,
            tx_queue: VecDeque::new(),
            read_buffer: vec![0; BRIDGE_READ_BUFFER_SIZE],
            last_modem_control: None,
            last_reported_connected: true,
            last_error: None,
            tx_overflow_reported: false,
        }
    }

    pub fn info(&self) -> SerialPortBridgeInfo {
        SerialPortBridgeInfo {
            target: self.target.label().to_string(),
            transport: self.target.transport(),
            state: self.state,
            last_error: self.last_error.clone(),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.state == SerialPortBridgeState::Connected
    }

    pub fn target(&self) -> &SerialPortBridgeTarget {
        &self.target
    }

    pub fn enqueue(&mut self, byte: u8) {
        let connect_on_output = self.connect_trigger == SerialPortBridgeConnectTrigger::OnOutput;
        let waiting_for_output = self.state == SerialPortBridgeState::WaitingForOutput;
        if !(self.is_connected()
            || connect_on_output
                && matches!(
                    self.state,
                    SerialPortBridgeState::WaitingForOutput | SerialPortBridgeState::Connecting
                ))
        {
            return;
        }

        if self.tx_queue.len() < BRIDGE_TX_QUEUE_LIMIT {
            self.tx_queue.push_back(byte);
            self.tx_overflow_reported = false;
        }
        else if !self.tx_overflow_reported {
            log::warn!(
                "Serial bridge {} transmit queue is full; dropping bytes",
                self.target.label()
            );
            self.tx_overflow_reported = true;
        }

        if waiting_for_output {
            self.start_connect();
        }
    }

    /// Disconnect the current stream and suppress automatic reconnection.
    pub fn disconnect(&mut self) {
        self.stream = None;
        self.connect_receiver = None;
        self.reconnect_at = None;
        self.tx_queue.clear();
        self.last_modem_control = None;
        self.last_error = None;
        self.state = SerialPortBridgeState::Suspended;
    }

    /// Immediately create a fresh connection attempt.
    pub fn reconnect(&mut self) {
        self.stream = None;
        self.connect_receiver = None;
        self.reconnect_at = None;
        self.tx_queue.clear();
        self.last_modem_control = None;
        self.last_error = None;
        self.start_connect();
    }

    /// Preserve the cable/socket across a guest reset while discarding bytes that the guest had
    /// not finished transmitting and forcing its reset modem-control state to be reapplied.
    pub fn on_guest_reset(&mut self) {
        self.tx_queue.clear();
        self.last_modem_control = None;
        self.last_reported_connected = false;
    }

    pub fn update(&mut self, dtr: bool, rts: bool) -> SerialPortBridgeUpdate {
        self.poll_connection();

        if self.is_connected() {
            if let Err(error) = self.apply_modem_control(dtr, rts) {
                self.connection_lost(format!("modem-control update failed: {error}"));
            }
        }

        if self.is_connected() {
            self.flush_tx();
        }

        let received = if self.is_connected() {
            self.read_available()
        }
        else {
            Vec::new()
        };

        let connected = self.is_connected();
        let connection_changed = if connected != self.last_reported_connected {
            self.last_reported_connected = connected;
            Some(connected)
        }
        else {
            None
        };

        SerialPortBridgeUpdate {
            received,
            connection_changed,
        }
    }

    fn start_connect(&mut self) {
        let target = self.target.clone();
        let (sender, receiver) = bounded(1);
        self.stream = None;
        self.connect_receiver = Some(receiver);
        self.reconnect_at = None;
        self.state = SerialPortBridgeState::Connecting;

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let _ = sender.send(open_target(target));
        });

        #[cfg(target_arch = "wasm32")]
        {
            let _ = target;
            let _ = sender.send(Err("serial bridges are not supported in web builds".to_string()));
        }
    }

    fn poll_connection(&mut self) {
        if self.state == SerialPortBridgeState::ReconnectPending {
            if self.reconnect_at.is_some_and(|deadline| Instant::now() >= deadline) {
                self.start_connect();
            }
            return;
        }

        if self.state != SerialPortBridgeState::Connecting {
            return;
        }

        let result = self.connect_receiver.as_ref().map(Receiver::try_recv);
        match result {
            Some(Ok(Ok(stream))) => {
                log::info!(
                    "Serial bridge connected via {} to {}",
                    self.target.transport(),
                    self.target.label()
                );
                self.stream = Some(stream);
                self.connect_receiver = None;
                self.state = SerialPortBridgeState::Connected;
                self.last_error = None;
                self.last_modem_control = None;
            }
            Some(Ok(Err(error))) => self.connection_lost(error),
            Some(Err(TryRecvError::Disconnected)) => {
                self.connection_lost("bridge connector exited without a result".to_string())
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
    }

    fn apply_modem_control(&mut self, dtr: bool, rts: bool) -> io::Result<()> {
        if self.last_modem_control == Some((dtr, rts)) {
            return Ok(());
        }

        if let Some(stream) = self.stream.as_mut() {
            stream.set_modem_control(dtr, rts)?;
            self.last_modem_control = Some((dtr, rts));
        }
        Ok(())
    }

    fn flush_tx(&mut self) {
        if self.tx_queue.is_empty() {
            return;
        }

        self.tx_queue.make_contiguous();
        let result = {
            let (bytes, _) = self.tx_queue.as_slices();
            self.stream.as_mut().unwrap().try_write(bytes)
        };

        match result {
            Ok(BridgeIo::Transferred(count)) => {
                let count = count.min(self.tx_queue.len());
                self.tx_queue.drain(..count);
            }
            Ok(BridgeIo::Idle) => {}
            Ok(BridgeIo::Disconnected) => self.connection_lost("remote endpoint disconnected".to_string()),
            Err(error) => self.connection_lost(format!("write failed: {error}")),
        }
    }

    fn read_available(&mut self) -> Vec<u8> {
        let mut received = Vec::new();
        let mut remaining = BRIDGE_READ_BUDGET;

        while remaining > 0 && self.is_connected() {
            let read_len = remaining.min(self.read_buffer.len());
            let result = self
                .stream
                .as_mut()
                .unwrap()
                .try_read(&mut self.read_buffer[..read_len]);

            match result {
                Ok(BridgeIo::Transferred(count)) => {
                    let count = count.min(read_len);
                    received.extend_from_slice(&self.read_buffer[..count]);
                    remaining -= count;
                    if count == 0 {
                        break;
                    }
                }
                Ok(BridgeIo::Idle) => break,
                Ok(BridgeIo::Disconnected) => {
                    self.connection_lost("remote endpoint disconnected".to_string());
                    break;
                }
                Err(error) => {
                    self.connection_lost(format!("read failed: {error}"));
                    break;
                }
            }
        }

        received
    }

    fn connection_lost(&mut self, error: String) {
        log::warn!("Serial bridge {}: {}", self.target.label(), error);
        self.stream = None;
        self.connect_receiver = None;
        self.tx_queue.clear();
        self.last_modem_control = None;
        self.last_error = Some(error);

        if self.auto_reconnect {
            self.state = SerialPortBridgeState::ReconnectPending;
            self.reconnect_at = Some(Instant::now() + self.reconnect_interval);
        }
        else {
            self.state = SerialPortBridgeState::Suspended;
            self.reconnect_at = None;
        }
    }
}

fn open_target(target: SerialPortBridgeTarget) -> ConnectResult {
    match target {
        SerialPortBridgeTarget::TcpClient {
            address,
            connect_timeout_ms,
            ..
        } => open_tcp_stream(&address, Duration::from_millis(connect_timeout_ms)),
        SerialPortBridgeTarget::Serial {
            port_name,
            baud_rate,
            stop_bits,
            data_bits,
            parity,
            flow_control,
        } => open_host_serial_stream(&port_name, baud_rate, stop_bits, data_bits, parity, flow_control),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BridgeIo,
        FlowControlType,
        ParityType,
        SerialPortBridge,
        SerialPortBridgeConfiguration,
        SerialPortBridgeConnectTrigger,
        SerialPortBridgeState,
        SerialPortBridgeStream,
        SerialPortBridgeTarget,
    };
    use serde_derive::Deserialize;
    use std::{
        io::{self, Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    struct PartialWriteStream {
        written:   Arc<Mutex<Vec<u8>>>,
        max_write: usize,
    }

    impl SerialPortBridgeStream for PartialWriteStream {
        fn try_read(&mut self, _buf: &mut [u8]) -> io::Result<BridgeIo> {
            Ok(BridgeIo::Idle)
        }

        fn try_write(&mut self, buf: &[u8]) -> io::Result<BridgeIo> {
            let count = buf.len().min(self.max_write);
            self.written.lock().unwrap().extend_from_slice(&buf[..count]);
            Ok(BridgeIo::Transferred(count))
        }
    }

    #[derive(Deserialize)]
    struct BridgeList {
        connection: Vec<SerialPortBridgeConfiguration>,
    }

    #[test]
    fn config_loads_serial_and_tcp_connections() {
        let config: BridgeList = toml::from_str(
            r#"
                [[connection]]
                guest_port = 0
                transport = "serial"
                port_name = "COM11"
                baud_rate = 1200
                data_bits = 7

                [[connection]]
                guest_port = 1
                transport = "tcp"
                port_name = "TCP test port"
                address = "127.0.0.1:7000"
                connect_trigger = "OnOutput"
                reconnect_interval_ms = 250

                [[connection]]
                guest_port = 2
                transport = "tcp"
                port_name = "On-request test port"
                address = "127.0.0.1:7001"
                connect_trigger = "OnRequest"
            "#,
        )
        .unwrap();

        assert_eq!(config.connection.len(), 3);
        assert_eq!(
            config.connection[0].connect_trigger,
            SerialPortBridgeConnectTrigger::OnStartup
        );
        assert_eq!(
            config.connection[1].connect_trigger,
            SerialPortBridgeConnectTrigger::OnOutput
        );
        assert_eq!(
            config.connection[2].connect_trigger,
            SerialPortBridgeConnectTrigger::OnRequest
        );
        assert_eq!(config.connection[1].reconnect_interval_ms, 250);
        assert!(matches!(
            &config.connection[0].target,
            SerialPortBridgeTarget::Serial {
                port_name,
                baud_rate: 1200,
                stop_bits: 1,
                data_bits: 7,
                parity: ParityType::None,
                flow_control: FlowControlType::None,
            } if port_name == "COM11"
        ));
        assert!(matches!(
            &config.connection[1].target,
            SerialPortBridgeTarget::TcpClient {
                port_name,
                address,
                connect_timeout_ms: 3000,
            } if port_name == "TCP test port" && address == "127.0.0.1:7000"
        ));
    }

    #[test]
    fn on_request_stays_idle_until_a_connection_is_requested() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let mut configuration = SerialPortBridgeConfiguration::tcp_client(0, address.to_string());
        configuration.connect_trigger = SerialPortBridgeConnectTrigger::OnRequest;
        let mut bridge = SerialPortBridge::new(configuration).unwrap();

        assert_eq!(bridge.info().state, SerialPortBridgeState::WaitingForRequest);
        bridge.enqueue(b'X');
        assert_eq!(bridge.info().state, SerialPortBridgeState::WaitingForRequest);
        assert!(bridge.tx_queue.is_empty());
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock
        ));

        bridge.reconnect();
        assert_eq!(bridge.info().state, SerialPortBridgeState::Connecting);

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !bridge.is_connected() {
            bridge.update(false, false);
            thread::sleep(Duration::from_millis(2));
        }
        assert!(bridge.is_connected());
        assert!(listener.accept().is_ok());
    }

    #[test]
    fn first_output_connects_without_losing_the_byte() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let mut configuration = SerialPortBridgeConfiguration::tcp_client(0, address.to_string());
        configuration.connect_trigger = SerialPortBridgeConnectTrigger::OnOutput;
        let mut bridge = SerialPortBridge::new(configuration).unwrap();

        assert_eq!(bridge.info().state, SerialPortBridgeState::WaitingForOutput);
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock
        ));

        bridge.enqueue(b'X');
        assert_eq!(bridge.info().state, SerialPortBridgeState::Connecting);
        assert_eq!(bridge.tx_queue.iter().copied().collect::<Vec<_>>(), vec![b'X']);

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !bridge.is_connected() {
            bridge.update(false, false);
            thread::sleep(Duration::from_millis(2));
        }
        assert!(bridge.is_connected());

        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("failed to accept deferred bridge connection: {error}"),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut received = [0];
        stream.read_exact(&mut received).unwrap();

        assert_eq!(received[0], b'X');
    }

    #[test]
    fn slow_writes_do_not_drop_queued_bytes() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let stream = PartialWriteStream {
            written:   written.clone(),
            max_write: 2,
        };
        let configuration = SerialPortBridgeConfiguration::tcp_client(0, "127.0.0.1:7000".to_string());
        let mut bridge = SerialPortBridge::with_stream(configuration, Box::new(stream));

        for byte in 0..5 {
            bridge.enqueue(byte);
        }
        bridge.update(false, false);
        bridge.update(false, false);
        bridge.update(false, false);

        assert_eq!(*written.lock().unwrap(), vec![0, 1, 2, 3, 4]);
        assert!(bridge.tx_queue.is_empty());
    }

    #[test]
    fn tcp_bridge_recovers_when_the_remote_side_disconnects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            let accept_before_deadline = || loop {
                match listener.accept() {
                    Ok((stream, _)) => return Some(stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return None;
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("TCP accept failed: {error}"),
                }
            };

            let Some(mut first) = accept_before_deadline()
            else {
                return false;
            };
            first.write_all(&[0x11]).unwrap();
            drop(first);

            let Some(mut second) = accept_before_deadline()
            else {
                return false;
            };
            second.write_all(&[0x22]).unwrap();
            true
        });

        let mut configuration = SerialPortBridgeConfiguration::tcp_client(0, address.to_string());
        configuration.reconnect_interval_ms = 10;
        configuration.target = SerialPortBridgeTarget::TcpClient {
            port_name: "TCP test port".to_string(),
            address: address.to_string(),
            connect_timeout_ms: 250,
        };
        let mut bridge = SerialPortBridge::new(configuration).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut received = Vec::new();

        while Instant::now() < deadline && !received.contains(&0x22) {
            let update = bridge.update(false, false);
            received.extend(update.received);
            thread::sleep(Duration::from_millis(2));
        }

        assert!(server.join().unwrap());
        assert!(received.contains(&0x11));
        assert!(received.contains(&0x22));
        assert!(matches!(
            bridge.info().state,
            SerialPortBridgeState::Connected
                | SerialPortBridgeState::ReconnectPending
                | SerialPortBridgeState::Connecting
        ));
    }
}
