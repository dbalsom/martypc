/*
    Raspberry Pi 8088 CPU Validator

    Original code Copyright (c) 2019-2022 Andreas T Jonsson <mail@andreasjonsson.se>
    Ported to Rust for the Marty emulator by Daniel Balsom

    Original Copyright notice follows.
*/

// Copyright (c) 2019-2022 Andreas T Jonsson <mail@andreasjonsson.se>
//
// This software is provided 'as-is', without any express or implied
// warranty. In no event will the authors be held liable for any damages
// arising from the use of this software.
//
// Permission is granted to anyone to use this software for any purpose,
// including commercial applications, and to alter it and redistribute it
// freely, subject to the following restrictions:
//
// 1. The origin of this software must not be misrepresented; you must not
//    claim that you wrote the original software. If you use this software in
//    a product, an acknowledgment (see the following) in the product
//    documentation is required.
//
//    Portions Copyright (c) 2019-2022 Andreas T Jonsson <mail@andreasjonsson.se>
//
// 2. Altered source versions must be plainly marked as such, and must not be
//    misrepresented as being the original software.
//
// 3. This notice may not be removed or altered from any source distribution.


use crate::validator::{
    VFLAG_CARRY,
    VFLAG_PARITY,
    VFLAG_AUXILIARY,
    VFLAG_ZERO,
    VFLAG_SIGN,
    VFLAG_TRAP,
    VFLAG_INTERRUPT,
    VFLAG_DIRECTION,
    VFLAG_OVERFLOW
};

pub struct FlagMask {
    pub opcode: i16,
    pub ext: i16,
    pub mask: u16
}

pub const FLAG_MASK_LOOKUP: [FlagMask; 97] =  [
	FlagMask { opcode: 0x08, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x09, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x0A, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x0B, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x0C, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x0D, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x20, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x21, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x22, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x23, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x24, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x25, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x27, ext: -1, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0x2F, ext: -1, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0x30, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x31, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x32, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x33, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x34, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x35, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x37, ext: -1, mask: 0|VFLAG_PARITY|VFLAG_ZERO|VFLAG_SIGN|VFLAG_OVERFLOW },
	FlagMask { opcode: 0x3F, ext: -1, mask: 0|VFLAG_PARITY|VFLAG_ZERO|VFLAG_SIGN|VFLAG_OVERFLOW },
	FlagMask { opcode: 0x69, ext: -1, mask: 0|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN },
	FlagMask { opcode: 0x6B, ext: -1, mask: 0|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN },
	FlagMask { opcode: 0x80, ext: 1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x80, ext: 4, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x80, ext: 6, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x81, ext: 1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x81, ext: 4, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x81, ext: 6, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x82, ext: 1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x82, ext: 4, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x82, ext: 6, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x83, ext: 1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x83, ext: 4, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x83, ext: 6, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x84, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0x85, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xA8, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xA9, ext: -1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xC0, ext: 0, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC0, ext: 1, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC0, ext: 2, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC0, ext: 3, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC0, ext: 4, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC0, ext: 5, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC0, ext: 6, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC0, ext: 7, mask: 0|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 0, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 1, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 2, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 3, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 4, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 5, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 6, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xC1, ext: 7, mask: 0|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD0, ext: 4, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD0, ext: 5, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD0, ext: 6, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD0, ext: 7, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD1, ext: 4, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD1, ext: 5, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD1, ext: 6, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD1, ext: 7, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD2, ext: 0, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD2, ext: 1, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD2, ext: 2, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD2, ext: 3, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD2, ext: 4, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD2, ext: 5, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD2, ext: 6, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD2, ext: 7, mask: 0|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 0, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 1, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 2, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 3, mask: 0|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 4, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 5, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 6, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD3, ext: 7, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xD4, ext: -1, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD4, ext: -1, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD5, ext: -1, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xD5, ext: -1, mask: 0|VFLAG_CARRY|VFLAG_AUXILIARY|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xF6, ext: 0, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xF6, ext: 1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xF6, ext: 4, mask: 0|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN },
	FlagMask { opcode: 0xF6, ext: 5, mask: 0|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN },
	FlagMask { opcode: 0xF6, ext: 6, mask: 0|VFLAG_CARRY|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xF6, ext: 7, mask: 0|VFLAG_CARRY|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xF7, ext: 0, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xF7, ext: 1, mask: 0|VFLAG_AUXILIARY },
	FlagMask { opcode: 0xF7, ext: 4, mask: 0|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN },
	FlagMask { opcode: 0xF7, ext: 5, mask: 0|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN },
	FlagMask { opcode: 0xF7, ext: 6, mask: 0|VFLAG_CARRY|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN|VFLAG_OVERFLOW },
	FlagMask { opcode: 0xF7, ext: 7, mask: 0|VFLAG_CARRY|VFLAG_PARITY|VFLAG_AUXILIARY|VFLAG_ZERO|VFLAG_SIGN|VFLAG_OVERFLOW },
	FlagMask { opcode: -1, ext: -1, mask: 0xFF},
];
