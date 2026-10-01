/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a copy
    of this software and associated documentation files (the "Software"), to deal
    in the Software without restriction, including without limitation the rights
    to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
    copies of the Software, and to permit persons to whom the Software is
    furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
    OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
    SOFTWARE.
*/

use std::io::Cursor;

use crossbeam_channel::Sender;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use serde_derive::Deserialize;

use crate::{
    channel::BidirectionalChannel,
    devices::lpt_port::{ParallelControl, ParallelMessage, ParallelStatus},
};

const BUSY_TIME_US: f64 = 5.0;
const ACK_TIME_US: f64 = 5.0;
const CGA_FONT: &[u8] = include_bytes!("../../../../../assets/cga_8by8.bin");

fn default_job_timeout_ms() -> u64 {
    2_000
}

fn default_dpi() -> u16 {
    180
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PrinterOutputFormat {
    #[default]
    Raw,
    Text,
    EscP2,
    Postscript,
    PostscriptPdf,
    Pcl5e,
    Pcl5ePdf,
    Pcl5c,
    Pcl5cPdf,
    HpRtl,
    HpRtlPdf,
    Pcl6,
    Pcl6Pdf,
}

impl PrinterOutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Raw => "prn",
            Self::Text => "txt",
            Self::EscP2 => "png",
            Self::Postscript | Self::PostscriptPdf => "ps",
            Self::Pcl6 | Self::Pcl6Pdf => "pxl",
            _ => "pcl",
        }
    }

    pub fn conversion(self) -> Option<PrinterConversion> {
        match self {
            Self::PostscriptPdf => Some(PrinterConversion::PostscriptToPdf),
            Self::Pcl5ePdf | Self::Pcl5cPdf | Self::HpRtlPdf | Self::Pcl6Pdf => Some(PrinterConversion::PclToPdf),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PrinterPaperSize {
    #[default]
    Letter,
    A4,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PrinterQuality {
    #[default]
    Draft,
    Letter,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VirtualPrinterConfig {
    #[serde(default)]
    pub port: usize,
    #[serde(default)]
    pub output: PrinterOutputFormat,
    #[serde(default)]
    pub paper_size: PrinterPaperSize,
    #[serde(default)]
    pub quality: PrinterQuality,
    #[serde(default = "default_dpi")]
    pub dpi: u16,
    #[serde(default = "default_job_timeout_ms")]
    pub job_timeout_ms: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PrinterConversion {
    PostscriptToPdf,
    PclToPdf,
}

#[derive(Clone, Debug)]
pub struct PrinterArtifact {
    pub job_id: u64,
    pub page: Option<usize>,
    pub extension: &'static str,
    pub data: Vec<u8>,
    pub conversion: Option<PrinterConversion>,
}

#[derive(Clone, Debug)]
pub enum PrinterEvent {
    JobComplete(Vec<PrinterArtifact>),
}

#[derive(Copy, Clone)]
enum HandshakeState {
    Idle,
    Busy(f64),
    AckLow(f64),
}

pub struct VirtualPrinter {
    config: VirtualPrinterConfig,
    channel: BidirectionalChannel<ParallelMessage>,
    event_sender: Sender<PrinterEvent>,
    status: ParallelStatus,
    control: ParallelControl,
    data: u8,
    handshake: HandshakeState,
    idle_us: f64,
    have_job: bool,
    job_id: u64,
    raw_data: Vec<u8>,
    text_data: TextPrinter,
    escp: EscP2Printer,
}

impl VirtualPrinter {
    pub fn new(
        config: VirtualPrinterConfig,
        channel: BidirectionalChannel<ParallelMessage>,
        event_sender: Sender<PrinterEvent>,
    ) -> Self {
        let mut status = ParallelStatus::default();
        status.set_error(true);
        status.set_select(true);
        status.set_ack(true);
        status.set_busy(true);
        let escp = EscP2Printer::new(config.paper_size, config.quality, config.dpi);
        let printer = Self {
            config,
            channel,
            event_sender,
            status,
            control: ParallelControl::default(),
            data: 0,
            handshake: HandshakeState::Idle,
            idle_us: 0.0,
            have_job: false,
            job_id: 0,
            raw_data: Vec::new(),
            text_data: TextPrinter::default(),
            escp,
        };
        let _ = printer.channel.send(ParallelMessage::Status(status));
        printer
    }

    pub fn run(&mut self, usec: f64) {
        while let Ok(message) = self.channel.try_recv() {
            match message {
                ParallelMessage::Data(data) => self.data = data,
                ParallelMessage::Control(control) => self.update_control(control),
                ParallelMessage::Status(_) => {}
            }
        }
        self.advance_handshake(usec);
        if self.have_job {
            self.idle_us += usec;
            if self.idle_us >= self.config.job_timeout_ms as f64 * 1_000.0 {
                self.finish_job();
            }
        }
    }

    pub fn finish_job(&mut self) {
        if !self.have_job {
            return;
        }
        self.job_id += 1;
        let artifacts = match self.config.output {
            PrinterOutputFormat::Text => {
                let data = self.text_data.take();
                vec![self.artifact(data, None)]
            }
            PrinterOutputFormat::EscP2 => self.escp.take_pages(self.job_id),
            output => vec![PrinterArtifact {
                job_id: self.job_id,
                page: None,
                extension: output.extension(),
                data: std::mem::take(&mut self.raw_data),
                conversion: output.conversion(),
            }],
        };
        if !artifacts.is_empty() {
            let _ = self.event_sender.send(PrinterEvent::JobComplete(artifacts));
        }
        self.have_job = false;
        self.idle_us = 0.0;
    }

    fn artifact(&self, data: Vec<u8>, page: Option<usize>) -> PrinterArtifact {
        PrinterArtifact {
            job_id: self.job_id,
            page,
            extension: self.config.output.extension(),
            data,
            conversion: self.config.output.conversion(),
        }
    }

    fn update_control(&mut self, control: ParallelControl) {
        let strobe_rising = !self.control.strobe() && control.strobe();
        if !control.initialize() {
            self.reset_peripheral();
        }
        self.control = control;
        if control.initialize() && strobe_rising && matches!(self.handshake, HandshakeState::Idle) {
            self.accept_byte(self.data);
        }
    }

    fn accept_byte(&mut self, data: u8) {
        self.have_job = true;
        self.idle_us = 0.0;
        match self.config.output {
            PrinterOutputFormat::Text => {
                self.text_data.push(data);
                if data == b'\r' && self.control.auto_line_feed() {
                    self.text_data.push(b'\n');
                }
            }
            PrinterOutputFormat::EscP2 => {
                self.escp.push(data);
                if data == b'\r' && self.control.auto_line_feed() {
                    self.escp.push(b'\n');
                }
            }
            _ => self.raw_data.push(data),
        }
        self.status.set_busy(false);
        self.send_status();
        self.handshake = HandshakeState::Busy(0.0);
    }

    fn advance_handshake(&mut self, usec: f64) {
        self.handshake = match self.handshake {
            HandshakeState::Idle => HandshakeState::Idle,
            HandshakeState::Busy(elapsed) if elapsed + usec >= BUSY_TIME_US => {
                self.status.set_ack(false);
                self.send_status();
                HandshakeState::AckLow(0.0)
            }
            HandshakeState::Busy(elapsed) => HandshakeState::Busy(elapsed + usec),
            HandshakeState::AckLow(elapsed) if elapsed + usec >= ACK_TIME_US => {
                self.status.set_busy(true);
                self.status.set_ack(true);
                self.send_status();
                HandshakeState::Idle
            }
            HandshakeState::AckLow(elapsed) => HandshakeState::AckLow(elapsed + usec),
        };
    }

    fn reset_peripheral(&mut self) {
        self.handshake = HandshakeState::Idle;
        self.status.set_busy(true);
        self.status.set_ack(true);
        self.status.set_error(true);
        self.status.set_select(true);
        self.status.set_paper_out(false);
        self.send_status();
    }

    fn send_status(&self) {
        let _ = self.channel.send(ParallelMessage::Status(self.status));
    }
}

#[derive(Default)]
struct TextPrinter {
    lines:  Vec<Vec<u8>>,
    line:   Vec<u8>,
    column: usize,
}

impl TextPrinter {
    fn push(&mut self, byte: u8) {
        match byte {
            0x08 => self.column = self.column.saturating_sub(1),
            0x09 => self.column = (self.column + 8) & !7,
            0x0A => self.newline(),
            0x0C => {
                self.newline();
                self.lines.push(Vec::new());
            }
            0x0D => self.column = 0,
            0x20..=0xFF => {
                if self.line.len() <= self.column {
                    self.line.resize(self.column + 1, b' ');
                }
                self.line[self.column] = byte;
                self.column += 1;
            }
            _ => {}
        }
    }

    fn newline(&mut self) {
        self.lines.push(std::mem::take(&mut self.line));
        self.column = 0;
    }

    fn take(&mut self) -> Vec<u8> {
        if !self.line.is_empty() {
            self.newline();
        }
        let mut output = String::new();
        for line in self.lines.drain(..) {
            for byte in line {
                output.push(cp437_char(byte));
            }
            output.push_str("\r\n");
        }
        output.into_bytes()
    }
}

fn cp437_char(byte: u8) -> char {
    if byte < 0x80 {
        return byte as char;
    }
    const HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■ ";
    HIGH.chars().nth((byte - 0x80) as usize).unwrap_or('�')
}

enum EscState {
    Normal,
    Escape,
    Param(u8, usize, Vec<u8>),
    Graphics(usize, usize, Vec<u8>),
}

struct EscP2Printer {
    dpi: u16,
    width: u32,
    height: u32,
    page: Vec<u8>,
    pages: Vec<Vec<u8>>,
    x: u32,
    y: u32,
    line_spacing: u32,
    char_width: u32,
    bold: bool,
    underline: bool,
    double_width: bool,
    letter_quality: bool,
    state: EscState,
    page_dirty: bool,
}

impl EscP2Printer {
    fn new(paper: PrinterPaperSize, quality: PrinterQuality, dpi: u16) -> Self {
        let dpi = dpi.clamp(72, 360);
        let (width, height) = match paper {
            PrinterPaperSize::Letter => (dpi as u32 * 17 / 2, dpi as u32 * 11),
            PrinterPaperSize::A4 => (dpi as u32 * 827 / 100, dpi as u32 * 1169 / 100),
        };
        Self {
            dpi,
            width,
            height,
            page: vec![255; (width * height) as usize],
            pages: Vec::new(),
            x: dpi as u32 / 2,
            y: dpi as u32 / 2,
            line_spacing: dpi as u32 / 6,
            char_width: dpi as u32 / 10,
            bold: false,
            underline: false,
            double_width: false,
            letter_quality: quality == PrinterQuality::Letter,
            state: EscState::Normal,
            page_dirty: false,
        }
    }

    fn push(&mut self, byte: u8) {
        match std::mem::replace(&mut self.state, EscState::Normal) {
            EscState::Normal => self.normal(byte),
            EscState::Escape => self.escape(byte),
            EscState::Param(command, needed, mut data) => {
                data.push(byte);
                if data.len() == needed {
                    self.command(command, &data);
                }
                else {
                    self.state = EscState::Param(command, needed, data);
                }
            }
            EscState::Graphics(remaining, bytes_per_column, mut data) => {
                data.push(byte);
                if remaining == 1 {
                    self.draw_graphics(&data, bytes_per_column);
                }
                else {
                    self.state = EscState::Graphics(remaining - 1, bytes_per_column, data);
                }
            }
        }
    }

    fn normal(&mut self, byte: u8) {
        match byte {
            8 => self.x = self.x.saturating_sub(self.effective_width()),
            9 => {
                let tab = self.effective_width() * 8;
                self.x = (self.x / tab + 1) * tab;
            }
            10 => self.line_feed(),
            12 => self.finish_page(),
            13 => self.x = self.dpi as u32 / 2,
            27 => self.state = EscState::Escape,
            0x20..=0xFF => self.draw_char(byte),
            _ => {}
        }
    }

    fn escape(&mut self, command: u8) {
        match command {
            b'@' => self.reset_modes(),
            b'E' => self.bold = true,
            b'F' => self.bold = false,
            b'0' => self.line_spacing = self.dpi as u32 / 8,
            b'2' => self.line_spacing = self.dpi as u32 / 6,
            b'P' => self.char_width = self.dpi as u32 / 10,
            b'M' => self.char_width = self.dpi as u32 / 12,
            b'-' | b'W' | b'A' | b'3' | b'!' => self.state = EscState::Param(command, 1, Vec::new()),
            b'*' => self.state = EscState::Param(command, 3, Vec::new()),
            b'K' | b'L' | b'Y' | b'Z' => self.state = EscState::Param(command, 2, Vec::new()),
            _ => {}
        }
    }

    fn command(&mut self, command: u8, data: &[u8]) {
        match command {
            b'-' => self.underline = data[0] != 0,
            b'W' => self.double_width = data[0] != 0,
            b'A' => self.line_spacing = self.dpi as u32 * data[0] as u32 / 72,
            b'3' => self.line_spacing = self.dpi as u32 * data[0] as u32 / 216,
            b'!' => {
                self.char_width = self.dpi as u32 / if data[0] & 1 != 0 { 12 } else { 10 };
                self.bold = data[0] & 8 != 0;
                self.double_width = data[0] & 0x20 != 0;
                self.underline = data[0] & 0x80 != 0;
            }
            b'*' => {
                let columns = data[1] as usize | (data[2] as usize) << 8;
                let groups = if data[0] >= 32 { 3 } else { 1 };
                if columns > 0 {
                    self.state = EscState::Graphics(columns * groups, groups, Vec::new());
                }
            }
            b'K' | b'L' | b'Y' | b'Z' => {
                let count = data[0] as usize | (data[1] as usize) << 8;
                if count > 0 {
                    self.state = EscState::Graphics(count, 1, Vec::new());
                }
            }
            _ => {}
        }
    }

    fn draw_char(&mut self, byte: u8) {
        let scale = (self.dpi as u32 / 72).max(1);
        let xscale = if self.double_width { scale * 2 } else { scale };
        for row in 0..8 {
            let bits = CGA_FONT[byte as usize * 8 + row];
            for col in 0..8 {
                if bits & (0x80 >> col) != 0 {
                    self.rect(self.x + col as u32 * xscale, self.y + row as u32 * scale, xscale, scale);
                    if self.bold || self.letter_quality {
                        self.rect(
                            self.x + col as u32 * xscale + 1,
                            self.y + row as u32 * scale,
                            xscale,
                            scale,
                        );
                    }
                }
            }
        }
        if self.underline {
            self.rect(self.x, self.y + 8 * scale, self.effective_width(), scale);
        }
        self.page_dirty = true;
        self.x += self.effective_width();
        if self.x + self.effective_width() >= self.width - self.dpi as u32 / 2 {
            self.x = self.dpi as u32 / 2;
            self.line_feed();
        }
    }

    fn draw_graphics(&mut self, data: &[u8], groups: usize) {
        let dot = (self.dpi as u32 / 180).max(1);
        for column in data.chunks(groups) {
            for (group, byte) in column.iter().enumerate() {
                for bit in 0..8 {
                    if byte & (0x80 >> bit) != 0 {
                        self.rect(self.x, self.y + (group as u32 * 8 + bit) * dot, dot, dot);
                    }
                }
            }
            self.x += dot;
        }
        self.page_dirty = true;
    }

    fn rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        for py in y..(y + height).min(self.height) {
            for px in x..(x + width).min(self.width) {
                self.page[(py * self.width + px) as usize] = 0;
            }
        }
    }

    fn effective_width(&self) -> u32 {
        self.char_width * if self.double_width { 2 } else { 1 }
    }

    fn line_feed(&mut self) {
        self.y += self.line_spacing.max(1);
        if self.y + self.line_spacing >= self.height - self.dpi as u32 / 2 {
            self.finish_page();
        }
    }

    fn finish_page(&mut self) {
        if self.page_dirty {
            let mut png = Vec::new();
            let encoder = PngEncoder::new(Cursor::new(&mut png));
            if encoder
                .write_image(&self.page, self.width, self.height, ColorType::L8.into())
                .is_ok()
            {
                self.pages.push(png);
            }
        }
        self.page.fill(255);
        self.page_dirty = false;
        self.x = self.dpi as u32 / 2;
        self.y = self.dpi as u32 / 2;
    }

    fn reset_modes(&mut self) {
        self.bold = false;
        self.underline = false;
        self.double_width = false;
        self.char_width = self.dpi as u32 / 10;
        self.line_spacing = self.dpi as u32 / 6;
    }

    fn take_pages(&mut self, job_id: u64) -> Vec<PrinterArtifact> {
        self.finish_page();
        self.pages
            .drain(..)
            .enumerate()
            .map(|(page, data)| PrinterArtifact {
                job_id,
                page: Some(page + 1),
                extension: "png",
                data,
                conversion: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    fn test_config(output: PrinterOutputFormat) -> VirtualPrinterConfig {
        VirtualPrinterConfig {
            port: 0,
            output,
            paper_size: PrinterPaperSize::Letter,
            quality: PrinterQuality::Draft,
            dpi: 72,
            job_timeout_ms: 1,
        }
    }

    #[test]
    fn centronics_handshake_pulses_ack_and_returns_ready() {
        let (port, device) = BidirectionalChannel::new_pair();
        let (sender, receiver) = unbounded();
        let mut printer = VirtualPrinter::new(test_config(PrinterOutputFormat::Raw), device, sender);
        let mut control = ParallelControl::default();
        control.set_initialize(true);
        port.send(ParallelMessage::Data(b'A')).unwrap();
        port.send(ParallelMessage::Control(control)).unwrap();
        control.set_strobe(true);
        port.send(ParallelMessage::Control(control)).unwrap();
        printer.run(1.0);
        printer.run(BUSY_TIME_US);
        printer.run(ACK_TIME_US);
        let statuses: Vec<_> = port
            .receiver()
            .try_iter()
            .filter_map(|message| match message {
                ParallelMessage::Status(status) => Some(status),
                _ => None,
            })
            .collect();
        assert!(statuses.iter().any(|status| !status.ack()));
        assert!(statuses.last().unwrap().ack());
        assert!(statuses.last().unwrap().busy());

        printer.finish_job();
        let PrinterEvent::JobComplete(artifacts) = receiver.try_recv().unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].data, b"A");
    }

    #[test]
    fn text_output_decodes_cp437() {
        let mut text = TextPrinter::default();
        for byte in [b'A', 0x82, b'\r', b'\n'] {
            text.push(byte);
        }
        assert_eq!(String::from_utf8(text.take()).unwrap(), "Aé\r\n");
    }

    #[test]
    fn escp_output_produces_png() {
        let mut printer = EscP2Printer::new(PrinterPaperSize::Letter, PrinterQuality::Draft, 72);
        for byte in b"TEST\x0C" {
            printer.push(*byte);
        }
        let pages = printer.take_pages(1);
        assert_eq!(pages.len(), 1);
        assert_eq!(&pages[0].data[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn printer_config_deserializes_pdf_output() {
        let config: VirtualPrinterConfig = toml::from_str(
            r#"
                port = 1
                output = "pcl6_pdf"
                paper_size = "a4"
                quality = "letter"
                dpi = 300
                job_timeout_ms = 750
            "#,
        )
        .unwrap();

        assert_eq!(config.port, 1);
        assert_eq!(config.output, PrinterOutputFormat::Pcl6Pdf);
        assert_eq!(config.paper_size, PrinterPaperSize::A4);
        assert_eq!(config.quality, PrinterQuality::Letter);
        assert_eq!(config.dpi, 300);
        assert_eq!(config.job_timeout_ms, 750);
    }

    #[test]
    fn parallel_device_config_deserializes_printer() {
        let config: crate::machine_config::ParallelDeviceConfig = toml::from_str(
            r#"
                type = "printer"
                output = "postscript"
            "#,
        )
        .unwrap();

        let crate::machine_config::ParallelDeviceConfig::Printer(config) = config;
        assert_eq!(config.port, 0);
        assert_eq!(config.output, PrinterOutputFormat::Postscript);
    }
}
