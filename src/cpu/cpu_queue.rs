use crate::cpu::*;
use crate::bytequeue::*;

pub struct InstructionQueue {
    size: usize,
    len: usize,
    back: usize,
    front: usize,
    q: [u8; QUEUE_MAX],
    dt: [QueueType; QUEUE_MAX],
    preload: Option<u8>
}

impl Default for InstructionQueue {
    fn default() -> Self {
        Self::new(4)
    }
}

impl InstructionQueue {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            len: 0,
            back: 0,
            front: 0,
            q: [0; QUEUE_MAX],
            dt: [QueueType::First; QUEUE_MAX],
            preload: None,
        }
    }

    pub fn set_size(&mut self, size: usize) {
        assert!(size <= QUEUE_MAX);
        self.size = size;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_full(&self) -> bool {
        self.len == self.size
    }

    pub fn get_preload(&mut self) -> Option<u8> {
        let preload = self.preload;
        self.preload = None;
        preload
    }

    pub fn has_preload(&self) -> bool {
        if let Some(_) = self.preload {
            true
        }
        else {
            false
        }
    }

    pub fn set_preload(&mut self) {
        if self.len > 0 {

            let byte = self.pop();
            self.preload = Some(byte);
        }
    }

    pub fn push8(&mut self, byte: u8) {
        if self.len < self.size {

            self.q[self.front] = byte;
            //self.dt[self.front] = dtype;

            self.front = (self.front + 1) % self.size;
            self.len += 1;
        }
        else {
            panic!("Queue overrun!");
        }
    }

    pub fn push16(&mut self, word: u16) {

        self.push8((word & 0xFF) as u8);
        self.push8(((word >> 8) & 0xFF) as u8);
    }

    pub fn pop(&mut self) -> u8 {
        if self.len > 0 {
            let byte = self.q[self.back];
            //let dt = self.dt[self.back];

            self.back = (self.back + 1) % self.size;
            self.len -= 1;

            return byte
        }
        panic!("Queue underrun!");
    }

    pub fn flush(&mut self) {
        self.len = 0;
        self.back = 0;
        self.front = 0;
        self.preload = None;
    }

    pub fn to_string(&self) -> String {

        let mut base_str = "".to_string();

        for i in 0..self.len {
            base_str.push_str(&format!("{:02X}", self.q[(self.back + i) % self.size]));
        }

        base_str
    }
}