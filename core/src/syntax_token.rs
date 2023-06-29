/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    syntax_token.rs

    Defines token enums for visual formatting of debugging output 
    including disassembly and memory views. A corresponding egui control
    TokenListView can use these tokens to format output with syntax coloring.
*/

pub const TOKEN_MAX_AGE: u8 = 255;

pub trait SyntaxTokenize {
    fn tokenize(&self) -> Vec<SyntaxToken>;
}

#[derive(Clone)]
pub enum SyntaxToken {

    NullToken,
    // Generic display tokens

    // State string has a 'dirty' flag for displaying state data as new, and a 
    // u8 frame age counter for tracking age of value.
    StateString(String, bool, u8), 

    // Memory viewer tokens
    ErrorString(String),
    MemoryAddressSeg16(u16, u16, String),
    MemoryAddressFlat(u32, String),
    MemoryByteHexValue(u32, u8, String, bool, u8),
    MemoryByteAsciiValue(u32, u8, String, u8),

    // Disassembly tokens
    ErrorText(String),
    InstructionBytes(String),
    Prefix(String),
    Mnemonic(String),
    Text(String),
    Segment(String),
    Colon,
    Comma,
    PlusSign,
    OpenBracket,
    CloseBracket,
    HexValue(String),
    Register(String),
    Displacement(String),
}

impl Default for SyntaxToken {
    fn default() -> Self { SyntaxToken::NullToken }
}
