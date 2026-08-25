use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::arduino8088_validator::{code_stream::CodeStream, queue::QueueDataType, OPCODE_NOP};
use ard808x_client::CpuWidth;

pub struct RemoteProgram {
    pub(crate) bytes: Cursor<Vec<u8>>,
    fill_byte: u8,
    next_fetch: u8,
    width: CpuWidth,
    used_fill: bool,
    pad_start_odd: bool, // Pad initial odd fetch to this program with a NOP to get us in alignment.
}

impl RemoteProgram {
    pub fn new(data: &[u8], fill_byte: u8, width: CpuWidth, pad_start_odd: bool) -> Self {
        Self {
            bytes: Cursor::new(data.to_vec()),
            fill_byte,
            next_fetch: OPCODE_NOP,
            width,
            used_fill: false,
            pad_start_odd,
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

    #[inline]
    pub fn set_next_fetch(&mut self, byte: u8) {
        self.next_fetch = byte;
    }

    // Read the program into a CodeStream.
    pub fn read_program(&mut self, a0: bool, stream: &mut CodeStream, data_type: QueueDataType) -> usize {
        if self.program_remaining() == 0 {
            log::trace!("read_program(): no more bytes to read!");
            return 0;
        }
        match self.width {
            CpuWidth::Eight => {
                let mut buf = [0u8; 1];
                self.bytes
                    .read_exact(&mut buf)
                    .expect("read_program(): failed to read byte!");
                stream.push_byte(buf[0], data_type);
                1
            }
            CpuWidth::Sixteen => {
                let mut buf = [0u8; 2];

                if !a0 {
                    // Even address. Read normally.
                    if self.program_remaining() < 2 {
                        // Only one byte left, read it.
                        self.bytes
                            .read_exact(&mut buf[..1])
                            .expect("read_program(): last byte failed to read!");
                        stream.push_byte(buf[0], data_type);
                        // Push the next-fetch byte.
                        stream.push_byte(self.next_fetch, QueueDataType::Program);

                        1
                    }
                    else {
                        // At least two bytes left
                        self.bytes
                            .read_exact(&mut buf)
                            .expect("read_program(): failed to read two bytes!");
                        stream.push_word(u16::from_le_bytes(buf), data_type);
                        2
                    }
                }
                // else if (self.bytes.position() == 0) && self.pad_start_odd {
                //     // Odd address, with pad at start of program. Provide one NOP for the floating
                //     // low side of the bus, and one NOP of padding for the high side of the bus.
                //     // This should place us back in even alignment for the rest of the program.
                //     stream.push_byte(OPCODE_NOP, QueueDataType::Fill);
                //     stream.push_byte(OPCODE_NOP, QueueDataType::Fill);
                //     0
                // }
                else {
                    // Odd address... provide a dummy byte if at start of program
                    if self.bytes.position() == 0 {
                        log::trace!("read_program(): providing fill byte at start of program");
                        stream.push_byte(OPCODE_NOP, QueueDataType::Fill);
                    }
                    else {
                        // If we're not at the start, we can rewind by one and give a more accurate
                        // representation of the low byte of the bus in a real fetch.
                        _ = self.bytes.seek(SeekFrom::Current(-1));
                        self.bytes
                            .read_exact(&mut buf[..1])
                            .expect("read_program(): failed to read byte!");
                        stream.push_byte(buf[0], data_type);
                    }
                    // Read the second byte
                    self.bytes
                        .read_exact(&mut buf[..1])
                        .expect("read_program(): failed to read byte!");
                    stream.push_byte(buf[0], data_type);
                    1
                }
            }
        }
    }

    pub fn pos(&self) -> usize {
        self.bytes.position() as usize
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
        self.next_fetch = 0x90; // Reset next fetch to NOP
    }

    /// Returns the total length of the program in bytes.
    pub fn len(&self) -> usize {
        self.bytes.get_ref().len()
    }

    /// Return a value to use for adjusting IP for the length of the program. This is mostly used
    /// for the prefetch program.
    pub fn ip_adjustment(&self, start_ip: u16) -> usize {
        self.bytes.get_ref().len()
            + if start_ip & 1 != 0 && self.pad_start_odd {
                // If the start IP is odd, and we are padding the start of the program, we need to
                // adjust the IP by 1 to account for the NOP we added.
                0
            }
            else {
                0
            }
    }
}
