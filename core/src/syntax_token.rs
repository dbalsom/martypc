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
use std::fmt;

pub const TOKEN_MAX_AGE: u8 = 255;

pub trait SyntaxTokenize {
    fn tokenize(&self) -> Vec<SyntaxToken>;
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum SyntaxFormatType {
    Space
}

#[derive(Clone, Eq, PartialEq, Hash)]
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

    Formatter(SyntaxFormatType)
}

impl Default for SyntaxToken {
    fn default() -> Self { SyntaxToken::NullToken }
}

impl fmt::Display for SyntaxToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyntaxToken::NullToken => write!(f, ""),
            SyntaxToken::StateString(s,..) => write!(f, "{}", s),
            SyntaxToken::ErrorString(s) => write!(f, "{}", s),
            SyntaxToken::MemoryAddressSeg16(seg, off, _) => write!(f, "{:04X}:{:04X}", seg, off),
            SyntaxToken::MemoryAddressFlat(addr, _) => write!(f, "{:05X}", addr),
            SyntaxToken::MemoryByteHexValue(_, val, ..) => write!(f, "{:02}", val),
            SyntaxToken::MemoryByteAsciiValue(_, val, ..) => write!(f, "{:02}", val),
            SyntaxToken::ErrorText(s) => write!(f, "{}", s),
            SyntaxToken::InstructionBytes(bytes) => write!(f, "{}", bytes),
            SyntaxToken::Prefix(prefix) => write!(f, "{}", prefix),
            SyntaxToken::Mnemonic(mnemonic) => write!(f, "{}", mnemonic),
            SyntaxToken::Text(text) => write!(f, "{}", text),
            SyntaxToken::Segment(segment) => write!(f, "{}", segment),
            SyntaxToken::Colon => write!(f, ":"),
            SyntaxToken::Comma => write!(f, ","),
            SyntaxToken::PlusSign => write!(f, "+"),
            SyntaxToken::OpenBracket => write!(f, "["),
            SyntaxToken::CloseBracket => write!(f, "]"),
            SyntaxToken::HexValue(value) => write!(f, "{}", value),
            SyntaxToken::Register(register) => write!(f, "{}", register),
            SyntaxToken::Displacement(displacement) => write!(f, "{}", displacement),

            SyntaxToken::Formatter(fmt_type) => match fmt_type {
                SyntaxFormatType::Space => write!(f, " "),
            }
        }
    }
}

/// NewType to implement Display on a vec of SyntaxToken.
pub struct SyntaxTokenVec(pub Vec<SyntaxToken>);

impl fmt::Display for SyntaxTokenVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tok in &self.0 {
            write!(f, "{}", tok)?;
        }
        Ok(())
    }
}

/// Implement some common operations when building vecs of SyntaxToken.
impl SyntaxTokenVec {
    #[inline]
    pub fn push_space(&mut self, t: SyntaxToken) {
        self.0.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        self.0.push(t);
    }
    #[inline]
    pub fn push_comma(&mut self, t: SyntaxToken) {
        self.0.push(SyntaxToken::Comma);
        self.0.push(t);
    }
    #[inline]
    pub fn push_comma_space(&mut self, t: SyntaxToken) {
        self.0.push(SyntaxToken::Comma);
        self.0.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        self.0.push(t);
    }
    #[inline]
    pub fn push_brackets(&mut self, t: SyntaxToken) {
        self.0.push(SyntaxToken::Comma);
        self.0.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        self.0.push(t);
    }    

    pub fn strip_whitespace(&mut self) {
        self.0.retain(|item| match item {
            SyntaxToken::Formatter(SyntaxFormatType::Space) => false,
            _ => true,
        });        
    }

    pub fn retain(&mut self, list: &[SyntaxToken]) {
        let list = list.iter().cloned().collect::<std::collections::HashSet<_>>();
        self.0.retain(|item| list.contains(&item));
    }

    pub fn append(
        &mut self, 
        items: Vec<SyntaxToken>, 
        start_tok: Option<SyntaxToken>,
        separator: Option<SyntaxToken>) 
    {
        if let Some(start) = start_tok {
            self.0.push(start);
        }
        if let Some(sep) = separator {
            // If a separator is provided, join with separator and then append
            let mut iter = items.into_iter();
            if let Some(first) = iter.next() {
                self.0.push(first);
                for item in iter {
                    self.0.push(sep.clone());
                    self.0.push(item);
                }
            }
        } 
        else {
            self.0.extend(items);
        }
    }
    
}