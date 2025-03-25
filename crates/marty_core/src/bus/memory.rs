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

use std::path::Path;

use crate::{
    bus::{
        BusInterface,
        MemRangeDescriptor,
        MemoryMappedDevice,
        MmioDeviceType,
        DEFAULT_WAIT_STATES,
        MEM_MMIO_BIT,
        MEM_ROM_BIT,
        MMIO_MAP_LEN,
        MMIO_MAP_SHIFT,
    },
    memerror::MemError,
    syntax_token::SyntaxToken,
};

impl BusInterface {
    pub fn copy_from(&mut self, src: &[u8], location: usize, cycle_cost: u32, read_only: bool) -> Result<(), bool> {
        let src_size = src.len();
        if location + src_size > self.memory.len() {
            // copy request goes out of bounds
            log::error!("copy out of range: {} len: {}", location, src_size);
            return Err(false);
        }

        let mem_slice: &mut [u8] = &mut self.memory[location..location + src_size];
        let mask_slice: &mut [u8] = &mut self.memory_mask[location..location + src_size];

        for (dst, src) in mem_slice.iter_mut().zip(src) {
            *dst = *src;
        }

        // Write access mask
        let access_bit = match read_only {
            true => MEM_ROM_BIT,
            false => 0x00,
        };
        for dst in mask_slice.iter_mut() {
            *dst |= access_bit;
        }

        self.desc_vec.push({
            MemRangeDescriptor {
                address: location,
                size: src_size,
                cycle_cost,
                read_only,
                priority: 1,
            }
        });

        Ok(())
    }

    /// Write the specified bytes from src_vec into memory at location 'location'
    ///
    /// Does not obey memory mapping
    pub fn patch_from(&mut self, src_vec: &Vec<u8>, location: usize) -> Result<(), bool> {
        let src_size = src_vec.len();
        if location + src_size > self.memory.len() {
            // copy request goes out of bounds
            return Err(false);
        }

        let mem_slice: &mut [u8] = &mut self.memory[location..location + src_size];

        for (dst, src) in mem_slice.iter_mut().zip(src_vec.as_slice()) {
            *dst = *src;
        }
        Ok(())
    }

    /// Return a slice of memory at the specified location and length.
    /// Does not resolve mmio addresses.
    pub fn get_slice_at(&self, start: usize, len: usize) -> &[u8] {
        if start >= self.memory.len() {
            return &[];
        }

        &self.memory[start..std::cmp::min(start + len, self.memory.len())]
    }

    /// Return a vector of memory at the specified location and length.
    /// Does not resolve mmio addresses.
    pub fn get_vec_at(&self, start: usize, len: usize) -> Vec<u8> {
        if start >= self.memory.len() {
            return Vec::new();
        }

        self.memory[start..std::cmp::min(start + len, self.memory.len())].to_vec()
    }

    /// Return a vector representing the contents of memory starting from the specified location,
    /// and continuing for the specified length. This function resolves mmio addresses.
    pub fn get_vec_at_ex(&self, start: usize, len: usize) -> Vec<u8> {
        if start >= self.memory.len() {
            return Vec::new();
        }

        let start_mmio_block = start >> MMIO_MAP_SHIFT;
        let mut end_mmio_block = (start + len) >> MMIO_MAP_SHIFT;

        if end_mmio_block >= MMIO_MAP_LEN {
            // If the end block is out of range, just return a slice of conventional memory.
            end_mmio_block = MMIO_MAP_LEN - 1;
        }

        // First, scan the mmio map to see if the range contains any mmio mapped devices.
        // If one is found, set a flag to fall back to the slow path.
        let mut range_has_mmio = false;
        for i in start_mmio_block..=end_mmio_block {
            if self.mmio_map_fast[i] != MmioDeviceType::None {
                range_has_mmio = true;
            }
        }

        if !range_has_mmio {
            // Fast path: No mmio. Just return a slice of conventional.
            self.memory[start..std::cmp::min(start + len, self.memory.len())].to_vec()
        }
        else {
            // Slow path: Slice has mmio. Build a vector from bus peeks.
            let mut result = Vec::with_capacity(len);
            for addr in start..std::cmp::min(start + len, self.memory.len()) {
                match self.peek_u8(addr) {
                    Ok(byte) => result.push(byte),
                    Err(_) => result.push(0xFF),
                }
            }
            result
        }
    }

    pub fn get_read_wait(&mut self, address: usize, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                // Handle memory-mapped devices
                let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let syswait = card_dispatch.mmio_read_wait(address, system_ticks);
                            return Ok(self.system_ticks_to_cpu_cycles(syswait));
                        }
                    }
                    MmioDeviceType::Cart => {
                        return Ok(0);
                    }
                    _ => {}
                }
                // We didn't match any mmio devices, return raw memory
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn get_write_wait(&mut self, address: usize, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                // Handle memory-mapped devices
                let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let syswait = card_dispatch.mmio_write_wait(address, system_ticks);
                            return Ok(self.system_ticks_to_cpu_cycles(syswait));
                        }
                    }
                    MmioDeviceType::Cart => {
                        return Ok(0);
                    }
                    _ => {}
                }
                // We didn't match any mmio devices, return raw memory
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_u8(&mut self, address: usize, cycles: u32) -> Result<(u8, u32), MemError> {
        if address < self.memory.len() {
            return if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                let data: u8 = self.memory[address];
                Ok((data, 0))
            }
            else {
                // Handle memory-mapped devices
                let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            return Ok(card_dispatch.mmio_read_u8(address, system_ticks, Some(&self.memory)));
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            let (data, _waits) = MemoryMappedDevice::mmio_read_u8(ems, address, system_ticks, None);
                            return Ok((data, 0));
                        }
                    }
                    MmioDeviceType::Cart => {
                        if let Some(cart_slot) = &mut self.cart_slot {
                            let (data, _waits) =
                                MemoryMappedDevice::mmio_read_u8(cart_slot, address, system_ticks, None);
                            return Ok((data, 0));
                        }
                    }
                    _ => {}
                }
                Err(MemError::MmioError)
            };
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn peek_range(&self, address: usize, len: usize) -> Result<&[u8], MemError> {
        if address + len < self.memory.len() {
            Ok(&self.memory[address..address + len])
        }
        else {
            Err(MemError::ReadOutOfBoundsError)
        }
    }

    pub fn peek_u8(&self, address: usize) -> Result<u8, MemError> {
        if address < self.memory.len() {
            return if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                let b: u8 = self.memory[address];
                Ok(b)
            }
            else {
                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get(&vid) {
                            return Ok(card_dispatch.mmio_peek_u8(address, Some(&self.memory)));
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &self.ems {
                            let data = MemoryMappedDevice::mmio_peek_u8(ems, address, None);
                            return Ok(data);
                        }
                    }
                    MmioDeviceType::Cart => {
                        if let Some(cart_slot) = &self.cart_slot {
                            let data = MemoryMappedDevice::mmio_peek_u8(cart_slot, address, None);
                            return Ok(data);
                        }
                    }
                    _ => {}
                }
                Err(MemError::MmioError)
            };
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_u16(&mut self, address: usize, cycles: u32) -> Result<(u16, u32), MemError> {
        if address < self.memory.len() - 1 {
            return if self.memory_mask[address] & MEM_MMIO_BIT == 0 {
                // Address is not mapped.
                let w: u16 = self.memory[address] as u16 | (self.memory[address + 1] as u16) << 8;
                Ok((w, DEFAULT_WAIT_STATES))
            }
            else {
                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let system_ticks = self.cycles_to_ticks[cycles as usize];
                            return Ok(card_dispatch.mmio_read_u16(address, system_ticks, Some(&self.memory)));
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            let (data, syswait) = MemoryMappedDevice::mmio_read_u16(ems, address, 0, None);
                            return Ok((data, self.system_ticks_to_cpu_cycles(syswait)));
                        }
                    }
                    _ => {}
                }
                Ok((0xFFFF, 0))
                //return Err(MemError::MmioError);
            };
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn write_u8(&mut self, address: usize, data: u8, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            return if self.memory_mask[address] & (MEM_MMIO_BIT | MEM_ROM_BIT) == 0 {
                // Address is not mapped and not ROM, write to it if it is within conventional memory.
                if address < self.conventional_size {
                    self.memory[address] = data;
                }
                Ok(DEFAULT_WAIT_STATES)
            }
            else {
                // Handle memory-mapped devices.
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let system_ticks = self.cycles_to_ticks[cycles as usize];
                            return Ok(card_dispatch.mmio_write_u8(
                                address,
                                data,
                                system_ticks,
                                Some(&mut self.memory),
                            ));
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            MemoryMappedDevice::mmio_write_u8(ems, address, data, 0, None);
                        }
                    }
                    _ => {}
                }
                Ok(DEFAULT_WAIT_STATES)
            };
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn write_u16(&mut self, address: usize, data: u16, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {
            return if self.memory_mask[address] & (MEM_MMIO_BIT | MEM_ROM_BIT) == 0 {
                // Address is not mapped. Write to memory if within conventional memory size.
                if address < self.conventional_size - 1 {
                    self.memory[address] = (data & 0xFF) as u8;
                    self.memory[address + 1] = (data >> 8) as u8;
                }
                else if address < self.conventional_size {
                    self.memory[address] = (data & 0xFF) as u8;
                }
                Ok(DEFAULT_WAIT_STATES)
            }
            else {
                // Handle memory-mapped devices
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video(vid) => {
                        if let Some(card_dispatch) = self.videocards.get_mut(&vid) {
                            let system_ticks = self.cycles_to_ticks[cycles as usize];
                            return Ok(card_dispatch.mmio_write_u16(
                                address,
                                data,
                                system_ticks,
                                Some(&mut self.memory),
                            ));
                        }
                    }
                    MmioDeviceType::Ems => {
                        if let Some(ems) = &mut self.ems {
                            MemoryMappedDevice::mmio_write_u16(ems, address, data, 0, None);
                        }
                    }
                    _ => {}
                }
                Ok(0)
            };
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    /// Get bit flags for the specified byte at address
    #[inline]
    pub fn get_flags(&self, address: usize) -> u8 {
        if address < self.memory.len() - 1 {
            self.memory_mask[address]
        }
        else {
            0
        }
    }

    /// Set bit flags for the specified byte at address
    pub fn set_flags(&mut self, address: usize, flags: u8) {
        if address < self.memory.len() - 1 {
            //log::trace!("set flag for address: {:05X}: {:02X}", address, flags);
            self.memory_mask[address] |= flags;
        }
    }

    /// Clear the specified flags for the specified byte at address
    /// Do not allow ROM bit to be cleared
    pub fn clear_flags(&mut self, address: usize, flags: u8) {
        if address < self.memory.len() - 1 {
            self.memory_mask[address] &= !(flags & 0x7F);
        }
    }

    /// Dump memory to a string representation.
    ///
    /// Does not honor memory mappings.
    pub fn dump_flat(&self, address: usize, size: usize) -> String {
        if address + size >= self.memory.len() {
            "REQUEST OUT OF BOUNDS".to_string()
        }
        else {
            let mut dump_str = String::new();
            let dump_slice = &self.memory[address..address + size];
            let mut display_address = address;

            for dump_row in dump_slice.chunks_exact(16) {
                let mut dump_line = String::new();
                let mut ascii_line = String::new();

                for byte in dump_row {
                    dump_line.push_str(&format!("{:02x} ", byte));

                    let char_str = match byte {
                        00..=31 => ".".to_string(),
                        32..=127 => format!("{}", *byte as char),
                        128.. => ".".to_string(),
                    };
                    ascii_line.push_str(&char_str)
                }
                dump_str.push_str(&format!("{:05X} {} {}\n", display_address, dump_line, ascii_line));
                display_address += 16;
            }
            dump_str
        }
    }

    /// Dump memory to a vector of string representations.
    ///
    /// Does not honor memory mappings.
    pub fn dump_flat_vec(&self, address: usize, size: usize) -> Vec<String> {
        let mut vec = Vec::new();

        if address + size >= self.memory.len() {
            vec.push("REQUEST OUT OF BOUNDS".to_string());
            return vec;
        }
        else {
            let dump_slice = &self.memory[address..address + size];
            let mut display_address = address;

            for dump_row in dump_slice.chunks_exact(16) {
                let mut dump_line = String::new();
                let mut ascii_line = String::new();

                for byte in dump_row {
                    dump_line.push_str(&format!("{:02x} ", byte));

                    let char_str = match byte {
                        00..=31 => ".".to_string(),
                        32..=127 => format!("{}", *byte as char),
                        128.. => ".".to_string(),
                    };
                    ascii_line.push_str(&char_str)
                }

                vec.push(format!("{:05X} {} {}\n", display_address, dump_line, ascii_line));

                display_address += 16;
            }
        }
        vec
    }

    /// Dump memory to a vector of vectors of SyntaxTokens.
    ///
    /// Does not honor memory mappings.
    pub fn dump_flat_tokens(&self, address: usize, cursor: usize, mut size: usize) -> Vec<Vec<SyntaxToken>> {
        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        if address >= self.memory.len() {
            // Start address is invalid. Send only an error token.
            let mut linevec = Vec::new();

            linevec.push(SyntaxToken::ErrorString("REQUEST OUT OF BOUNDS".to_string()));
            vec.push(linevec);

            return vec;
        }
        else if address + size >= self.memory.len() {
            // Request size invalid. Send truncated result.
            let new_size = size - ((address + size) - self.memory.len());
            size = new_size
        }

        let dump_slice = &self.memory[address..address + size];
        let mut display_address = address;

        for dump_row in dump_slice.chunks_exact(16) {
            let mut line_vec = Vec::new();

            // Push memory flat address tokens
            line_vec.push(SyntaxToken::MemoryAddressFlat(
                display_address as u32,
                format!("{:05X}", display_address),
            ));

            // Build hex byte value tokens
            let mut i = 0;
            for byte in dump_row {
                if (display_address + i) == cursor {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        *byte,
                        format!("{:02X}", *byte),
                        true, // Set cursor on this byte
                        0,
                    ));
                }
                else {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        *byte,
                        format!("{:02X}", *byte),
                        false,
                        0,
                    ));
                }
                i += 1;
            }

            // Build ASCII representation tokens
            let mut i = 0;
            for byte in dump_row {
                let char_str = match byte {
                    00..=31 => ".".to_string(),
                    32..=127 => format!("{}", *byte as char),
                    128.. => ".".to_string(),
                };
                line_vec.push(SyntaxToken::MemoryByteAsciiValue(
                    (display_address + i) as u32,
                    *byte,
                    char_str,
                    0,
                ));
                i += 1;
            }

            vec.push(line_vec);
            display_address += 16;
        }

        vec
    }

    /// Dump memory to a vector of vectors of SyntaxTokens.
    ///
    /// Uses bus peek functions to resolve MMIO addresses.
    pub fn dump_flat_tokens_ex(&self, address: usize, cursor: usize, mut size: usize) -> Vec<Vec<SyntaxToken>> {
        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        if address >= self.memory.len() {
            // Start address is invalid. Send only an error token.
            let mut linevec = Vec::new();

            linevec.push(SyntaxToken::ErrorString("REQUEST OUT OF BOUNDS".to_string()));
            vec.push(linevec);

            return vec;
        }
        else if address + size >= self.memory.len() {
            // Request size invalid. Send truncated result.
            let new_size = size - ((address + size) - self.memory.len());
            size = new_size
        }

        let addr_vec = Vec::from_iter(address..address + size);
        let mut display_address = address;

        for dump_addr_row in addr_vec.chunks_exact(16) {
            let mut line_vec = Vec::new();

            // Push memory flat address tokens
            line_vec.push(SyntaxToken::MemoryAddressFlat(
                display_address as u32,
                format!("{:05X}", display_address),
            ));

            // Build hex byte value tokens
            let mut i = 0;
            for addr in dump_addr_row {
                let byte = self.peek_u8(*addr).unwrap();

                if (display_address + i) == cursor {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        byte,
                        format!("{:02X}", byte),
                        true, // Set cursor on this byte
                        0,
                    ));
                }
                else {
                    line_vec.push(SyntaxToken::MemoryByteHexValue(
                        (display_address + i) as u32,
                        byte,
                        format!("{:02X}", byte),
                        false,
                        0,
                    ));
                }
                i += 1;
            }

            // Build ASCII representation tokens
            let mut i = 0;
            for addr in dump_addr_row {
                let byte = self.peek_u8(*addr).unwrap();

                let char_str = match byte {
                    00..=31 => ".".to_string(),
                    32..=127 => format!("{}", byte as char),
                    128.. => ".".to_string(),
                };
                line_vec.push(SyntaxToken::MemoryByteAsciiValue(
                    (display_address + i) as u32,
                    byte,
                    char_str,
                    0,
                ));
                i += 1;
            }

            vec.push(line_vec);
            display_address += 16;
        }

        vec
    }

    pub fn dump_mem(&self, path: &Path) {
        let filename = path.to_path_buf();

        let len = 0x100000;
        let address = 0;
        log::debug!("Dumping {} bytes at address {:05X}", len, address);

        // TODO: replace with Writer
        match std::fs::write(filename.clone(), &self.memory) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename.display())
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename.display(), e)
            }
        }
    }

    pub fn dump_mem_range(&self, start: u32, end: u32, path: &Path) {
        let filename = path.to_path_buf();

        let len = end.saturating_sub(start) as usize;
        let end = (start as usize + len) & 0xFFFFF;
        log::debug!("Dumping {} bytes at address {:05X}", len, start);

        // TODO: replace with Writer
        match std::fs::write(filename.clone(), &self.memory[(start as usize)..=end]) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename.display())
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename.display(), e)
            }
        }
    }
}
