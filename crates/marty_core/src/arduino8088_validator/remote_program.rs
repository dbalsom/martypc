use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::arduino8088_validator::{code_stream::CodeStream, queue::QueueDataType, OPCODE_NOP};
use ard808x_client::CpuWidth;

pub struct RemoteProgram {
    pub(crate) bytes: Cursor<Vec<u8>>,
    fill_byte: u8,
    width: CpuWidth,
    used_fill: bool,
}

impl RemoteProgram {
    pub fn new(data: &[u8], fill_byte: u8, width: CpuWidth) -> Self {
        Self {
            bytes: Cursor::new(data.to_vec()),
            fill_byte,
            width,
            used_fill: false,
        }
    }

    pub fn program_remaining(&self) -> usize {
        let pos = self.bytes.position() as usize;
        let len = self.bytes.get_ref().len();
        len.saturating_sub(pos)
    }

    #[inline]
    pub fn is_finished(&self) -> bool {
        self.program_remaining() == 0
    }

    #[inline]
    pub fn set_fill(&mut self, byte: u8) {
        self.fill_byte = byte;
    }

    // Read the program into a CodeStream.
    pub fn read_program(&mut self, a0: bool, stream: &mut CodeStream, data_type: QueueDataType) -> usize {
        match self.width {
            CpuWidth::Eight => {
                let mut buf = [0u8; 1];
                if let Ok(_) = self.bytes.read_exact(&mut buf) {
                    stream.push_byte(buf[0], data_type);
                }
                1
            }
            CpuWidth::Sixteen => {
                let mut buf = [0u8; 2];

                if a0 == false {
                    // Even address. Read normally.
                    match self.bytes.read_exact(&mut buf) {
                        Ok(_) => {
                            // There were two bytes left, push the word.
                            stream.push_word(u16::from_le_bytes(buf), data_type);
                            2
                        }
                        Err(_) => {
                            // Fewer than two bytes remaining...
                            if self.program_remaining() == 1 {
                                // Only one byte left, read it.
                                if let Ok(_) = self.bytes.read_exact(&mut buf[..1]) {
                                    stream.push_byte(buf[0], data_type);
                                    1
                                }
                                else {
                                    0
                                }
                            }
                            else {
                                // No bytes left!
                                log::trace!("read_program(): no more bytes!");
                                0
                            }
                        }
                    }
                }
                else {
                    // Odd address... provide a dummy byte if at start of program
                    // We must have at least 1 byte in the program to do this
                    if self.program_remaining() == 1 {
                        if self.bytes.position() == 0 {
                            stream.push_byte(OPCODE_NOP, QueueDataType::Fill);
                        }
                        else {
                            // Seek backwards 1
                            _ = self.bytes.seek(SeekFrom::Current(-1));
                            let mut buf = [0u8; 1];
                            if let Ok(_) = self.bytes.read_exact(&mut buf) {
                                stream.push_byte(buf[0], data_type);
                            }
                        }
                        // Read the second byte
                        let mut buf = [0u8; 1];
                        if let Ok(_) = self.bytes.read_exact(&mut buf) {
                            stream.push_byte(buf[0], data_type);
                            1
                        }
                        else {
                            0
                        }
                    }
                    else {
                        // No bytes left!
                        log::trace!("read_program(): no more bytes!");
                        0
                    }
                }
            }
        }
    }

    /// Get the fill count - this will either be 0 or 1. This can be used to instruct
    /// the CPU server to adjust IP in the store program
    pub fn get_fill_ct(&self) -> usize {
        if self.used_fill {
            1
        }
        else {
            0
        }
    }

    /// Rewinds the program to the start.
    pub fn reset(&mut self) {
        self.bytes.set_position(0);
        self.used_fill = false;
    }

    /// Returns the total length of the program in bytes.
    pub fn len(&self) -> usize {
        self.bytes.get_ref().len()
    }
}
