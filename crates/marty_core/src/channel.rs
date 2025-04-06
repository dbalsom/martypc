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

    --------------------------------------------------------------------------
*/

//! Bidirectional crossbeam channel.

use crossbeam_channel;

#[derive(Clone)]
pub struct BidirectionalChannel<T> {
    sender:   crossbeam_channel::Sender<T>,
    receiver: crossbeam_channel::Receiver<T>,
}

impl<T> BidirectionalChannel<T> {
    pub fn new_pair() -> (Self, Self) {
        let (sender_a, receiver_a) = crossbeam_channel::unbounded();
        let (sender_b, receiver_b) = crossbeam_channel::unbounded();
        (
            Self {
                sender:   sender_a,
                receiver: receiver_b,
            },
            Self {
                sender:   sender_b,
                receiver: receiver_a,
            },
        )
    }

    pub fn sender(&self) -> crossbeam_channel::Sender<T> {
        self.sender.clone()
    }

    pub fn receiver(&self) -> crossbeam_channel::Receiver<T> {
        self.receiver.clone()
    }

    pub fn send(&self, value: T) -> Result<(), crossbeam_channel::SendError<T>> {
        self.sender.send(value)
    }

    pub fn recv(&self) -> Result<T, crossbeam_channel::RecvError> {
        self.receiver.recv()
    }

    pub fn try_recv(&self) -> Result<T, crossbeam_channel::TryRecvError> {
        self.receiver.try_recv()
    }
}
