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

    cpu_808x::queue.rs

    Implements the data structure for the processor instruction queue.

*/

use crate::cpu_808x::*;

pub struct InstructionQueue {
    size: usize,
    fetch_size: usize,
    policy_len0: usize,
    policy_len1: usize,
    len: usize,
    back: usize,
    front: usize,
    q: [u8; QUEUE_MAX],
    preload: Option<u8>,
    // Whether to discard the low order byte of the next fetch
    discard: bool,
}

impl Default for InstructionQueue {
    fn default() -> Self {
        Self {
            size: QUEUE_MAX,
            fetch_size: 2,
            policy_len0: QUEUE_MAX - 1,
            policy_len1: QUEUE_MAX - 2,
            len: 0,
            back: 0,
            front: 0,
            q: [0; QUEUE_MAX],
            preload: None,
            discard: false,
        }
    }
}

impl Display for InstructionQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut base_str = "".to_string();

        if let Some(preload) = self.preload {
            base_str.push_str(&format!("{:02X}", preload));
        }

        for i in 0..self.len {
            base_str.push_str(&format!("{:02X}", self.q[(self.back + i) % self.size]));
        }
        write!(f, "{}", base_str)
    }
}

impl InstructionQueue {
    pub fn new(size: usize, fetch_size: usize) -> Self {
        Self {
            size,
            fetch_size,
            policy_len0: if fetch_size == 1 { size - 1 } else { size - 2 },
            policy_len1: if fetch_size == 1 { size - 1 } else { size - 3 },
            ..Self::default()
        }
    }

    pub fn set_size(&mut self, size: usize, fetch_size: usize) {
        assert!(size <= QUEUE_MAX);
        self.size = size;
        self.fetch_size = fetch_size;
        self.policy_len0 = if fetch_size == 1 { size - 1 } else { size - 2 };
        self.policy_len1 = if fetch_size == 1 { size - 1 } else { size - 3 };
    }

    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
    pub fn at_policy_len(&self) -> bool {
        self.len == self.policy_len0 || self.len == self.policy_len1
    }

    #[inline]
    pub fn at_policy_threshold(&self) -> bool {
        self.len == self.policy_len1
    }

    #[inline]
    pub fn has_room_for_fetch(&self) -> bool {
        self.len <= (self.size - self.fetch_size)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn len_p(&self) -> usize {
        self.len + if self.preload.is_some() { 1 } else { 0 }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len == self.size
    }

    #[inline]
    pub fn get_preload(&mut self) -> Option<u8> {
        let preload = self.preload;
        self.preload = None;
        preload
    }

    #[inline]
    pub fn has_preload(&self) -> bool {
        self.preload.is_some()
    }

    #[inline]
    pub fn set_preload(&mut self) {
        if self.len > 0 {
            let byte = self.pop();
            self.preload = Some(byte);
        }
        else {
            panic!("Tried to preload with empty queue.")
        }
    }

    #[inline]
    pub fn set_discard(&mut self) {
        self.discard = true;
    }

    #[inline]
    pub fn push8(&mut self, byte: u8) -> u16 {
        if self.len < self.size {
            self.q[self.front] = byte;
            self.front = (self.front + 1) % self.size;
            self.len += 1;
            1
        }
        else {
            panic!("Queue overrun!");
        }
    }

    #[inline]
    pub fn push16(&mut self, word: u16, a0: bool) -> u16 {
        assert_eq!(self.fetch_size, 2);

        if a0 {
            self.push8((word >> 8) as u8);
            1
        }
        else {
            self.push8((word & 0xFF) as u8);
            self.push8((word >> 8) as u8);
            2
        }
    }

    #[inline]
    pub fn pop(&mut self) -> u8 {
        if self.len > 0 {
            let byte = self.q[self.back];
            self.back = (self.back + 1) % self.size;
            self.len -= 1;

            return byte;
        }
        panic!("Queue underrun!");
    }

    /// Flush the processor queue. This resets the queue to an empty state
    pub fn flush(&mut self) {
        log::trace!("flushing queue!");
        self.len = 0;
        self.back = 0;
        self.front = 0;
        self.preload = None;
        self.discard = false;
    }

    /// Write the contents of the processor instruction queue in order to the
    /// provided slice of u8. The slice must be the same size as the current piq
    /// length for the given cpu type.
    #[allow(dead_code)]
    pub fn to_slice(&self, slice: &mut [u8]) {
        for i in 0..self.len {
            slice[i] = self.q[(self.back + i) % self.size];
        }
    }
}
