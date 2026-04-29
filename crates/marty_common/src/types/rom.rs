/*
   MartyPC
   https://github.com/dbalsom/martypc

   Copyright 2022-2025 Daniel Balsom

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

   ---------------------------------------------------------------------------
*/

//! Shared ROM manifest types.
//!
//! This module defines the data structures used to pass resolved ROM selections from frontend ROM
//! discovery into the emulator core. The types live in `marty_common` so frontends can build
//! manifests without depending on `marty_core`, while the core can consume the same manifest type
//! directly.

use std::{collections::HashMap, path::PathBuf};

/// A single ROM image load requested by a machine ROM manifest.
#[derive(Clone, Default, Debug)]
pub struct MachineRomEntry {
    /// Display name or source filename for the ROM image.
    pub name:   String,
    /// Source path for the ROM file.
    pub path:   PathBuf,
    /// MD5 digest used to identify the ROM image.
    pub md5:    String,
    /// Physical memory address where the ROM image should be loaded.
    pub addr:   u32,
    /// Number of times the ROM image should be repeated at consecutive addresses.
    pub repeat: u32,
    /// ROM image bytes after any manifest-side transformations such as offset, size, or byte order
    /// handling.
    pub data:   Vec<u8>,
}

/// A CPU execution checkpoint associated with a ROM.
///
/// Checkpoints allow the core to report when execution reaches known ROM locations, such as
/// initialization routines or diagnostic entry points.
#[derive(Clone, Default, Debug)]
pub struct MachineCheckpoint {
    /// Physical memory address that triggers the checkpoint.
    pub addr: u32,
    /// Logging or diagnostic level associated with this checkpoint.
    pub lvl:  u32,
    /// Human-readable checkpoint description.
    pub desc: String,
}

/// A byte patch that may be applied when execution reaches a trigger address.
#[derive(Clone, Default, Debug)]
pub struct MachinePatch {
    /// Human-readable patch description.
    pub desc: String,
    /// Physical memory address that triggers patch installation.
    pub trigger: u32,
    /// Physical memory address where patch bytes should be written.
    pub addr: u32,
    /// Patch payload bytes.
    pub bytes: Vec<u8>,
    /// Whether the patch has already been installed.
    pub installed: bool,
}

/// Resolved ROM manifest used to initialize or reload a machine.
///
/// The manifest is produced by the frontend ROM manager and consumed by the core.
/// It contains the exact ROM images selected for a machine configuration, plus optional checkpoints
/// and patches associated with those ROMs.
#[derive(Clone, Default, Debug)]
pub struct MachineRomManifest {
    /// Execution checkpoints associated with the selected ROMs.
    pub checkpoints: Vec<MachineCheckpoint>,
    /// Patches associated with the selected ROMs.
    pub patches: Vec<MachinePatch>,
    /// ROM image entries to load into machine memory.
    pub roms: Vec<MachineRomEntry>,
}

impl MachineRomManifest {
    /// Create an empty ROM manifest.
    pub fn new() -> Self {
        Default::default()
    }

    /// Return true if the specified address range is not covered by any ROM in the manifest.
    /// Return false if the specified address range conflicts with an existing ROM.
    pub fn check_load(&self, addr: usize, len: usize) -> bool {
        let check_end = addr + len;

        for rom in self.roms.iter() {
            let rom_start = rom.addr as usize;
            let rom_end = rom_start + rom.data.len();

            if (check_end > rom_start) && (check_end < rom_end) {
                return false;
            }
        }
        true
    }

    /// Build a map from checkpoint address to checkpoint index.
    pub fn checkpoint_map(&self) -> HashMap<u32, usize> {
        let mut map = HashMap::new();
        for (idx, cp) in self.checkpoints.iter().enumerate() {
            map.insert(cp.addr, idx);
        }
        map
    }

    /// Build a map from patch trigger address to patch index.
    pub fn patch_map(&self) -> HashMap<u32, usize> {
        let mut map = HashMap::new();
        for (idx, patch) in self.patches.iter().enumerate() {
            map.insert(patch.trigger, idx);
        }
        map
    }
}
