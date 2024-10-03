/*
   MartyPC
   https://github.com/dbalsom/martypc

   Copyright 2022-2024 Daniel Balsom

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

   common::types::history_buffer.rs

   A HistoryBuffer is a fixed size ring buffer that overwrites the oldest entry
   on push when full.

*/

pub struct HistoryBuffer<T>
where
    T: Clone,
{
    buffer: Vec<T>,
    capacity: usize,
    start: usize,
    end: usize,
    full: bool,
}

impl<T> HistoryBuffer<T>
where
    T: Clone,
{
    pub fn new(capacity: usize) -> Self {
        HistoryBuffer {
            buffer: Vec::with_capacity(capacity),
            capacity,
            start: 0,
            end: 0,
            full: false,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.full {
            // If the buffer is full, overwrite the oldest element
            self.buffer[self.start] = item;
            self.start = (self.start + 1) % self.capacity;
            self.end = self.start;
        }
        else {
            // Add new item and update end
            if self.buffer.len() < self.capacity {
                self.buffer.push(item);
            }
            else {
                self.buffer[self.end] = item;
            }
            self.end = (self.end + 1) % self.capacity;
            self.full = self.end == self.start;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer[self.start..]
            .iter()
            .chain(self.buffer[..self.start].iter())
            .take(self.len())
    }

    pub fn as_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        if self.full {
            self.capacity
        }
        else {
            self.end
        }
    }

    pub fn clear(&mut self) {
        self.start = 0;
        self.end = 0;
        self.full = false;
        self.buffer.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.end == 0 && !self.full
    }
}
