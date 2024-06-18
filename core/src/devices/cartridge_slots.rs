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

    --------------------------------------------------------------------------

    devices::cartridge_slots.rs

    Implement the IBM PCJr's cartridge slots.

*/

use anyhow::{anyhow, Error};

use crate::bus::{MemRangeDescriptor, MemoryMappedDevice};

use marty_common::types::cartridge::CartImage;

pub const CARTRIDGE_SLOT_ADDRESS: usize = 0xD0000;
pub const CARTRIDGE_SLOT_SIZE: usize = 0x20000;

pub struct CartridgeSlot {
    pub carts: [Option<CartImage>; 2],
}

impl CartridgeSlot {
    pub fn new() -> Self {
        CartridgeSlot { carts: [None, None] }
    }

    pub fn insert_cart(&mut self, slot: usize, cart: CartImage) -> Result<(), Error> {
        if slot > 1 {
            return Err(anyhow!("Invalid cartridge slot"));
        }

        log::error!(
            "Loaded cartridge into slot {}. Segment: {:04X} Mask: {:04X} Size: {} Comment: {}",
            slot,
            cart.address_seg,
            cart.address_mask,
            cart.image.len(),
            cart.comment
        );

        self.carts[slot] = Some(cart);
        Ok(())
    }

    pub fn remove_cart(&mut self, slot: usize) {
        self.carts[slot] = None;
    }
}

impl MemoryMappedDevice for CartridgeSlot {
    fn get_read_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        0
    }

    fn mmio_read_u8(&mut self, address: usize, _cycles: u32, _cpumem: Option<&[u8]>) -> (u8, u32) {
        //log::debug!("Cartridge slot address space read at {:X}", address);

        for cart in self.carts.iter() {
            if let Some(cart) = cart {
                let cart_address = (cart.address_seg as usize) << 4;

                let masked_address = address & !(cart.address_mask as usize);
                if (address >= cart_address) && (address < (cart_address + cart.image.len())) {
                    //log::debug!("Cartridge read at {:X}", address);
                    return (cart.image[address - cart_address], 0);
                }
            }
        }
        (0xFF, 0)
    }

    fn mmio_read_u16(&mut self, address: usize, _cycles: u32, cpumem: Option<&[u8]>) -> (u16, u32) {
        (0xFFFF, 0)
    }

    fn mmio_peek_u8(&self, address: usize, _cpumem: Option<&[u8]>) -> u8 {
        for cart in self.carts.iter() {
            if let Some(cart) = cart {
                let cart_address = (cart.address_seg as usize) << 4;
                if address >= cart_address && address < (cart_address + cart.image.len()) {
                    return cart.image[address - cart_address];
                }
            }
        }
        0xFF
    }

    fn mmio_peek_u16(&self, address: usize, _cpumem: Option<&[u8]>) -> u16 {
        0xFFFF
    }

    fn get_write_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        0
    }

    fn mmio_write_u8(&mut self, address: usize, byte: u8, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        0
    }

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        0
    }

    fn get_mapping(&self) -> Vec<MemRangeDescriptor> {
        let mut mapping = Vec::new();

        mapping.push(MemRangeDescriptor {
            address: CARTRIDGE_SLOT_ADDRESS,
            size: CARTRIDGE_SLOT_SIZE,
            cycle_cost: 0,
            read_only: false,
            priority: 0,
        });

        mapping
    }
}
