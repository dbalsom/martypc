/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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
use std::{
    collections::HashSet,
    fmt,
    iter::FromIterator,
    slice::{Iter, IterMut},
};

pub const TOKEN_MAX_AGE: u8 = 255;

pub trait SyntaxTokenize {
    fn tokenize(&self) -> SyntaxTokenStream;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum HighlightType {
    Alert,
    Warning,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum SyntaxFormatType {
    Space,
    Tab,
    HighlightLine(HighlightType),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Default)]
pub enum SyntaxToken {
    #[default]
    NullToken,
    // Generic display tokens
    ColorText(String, u8, u8, u8),

    // State string has a 'dirty' flag for displaying state data as new, and a
    // u8 frame age counter for tracking age of value.
    StateString(String, bool, u8),

    // Memory viewer tokens
    ErrorString(String),
    MemoryAddressSeg16(u16, u16, String),
    StateMemoryAddressSeg16(u16, u16, String, u8),
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

    Formatter(SyntaxFormatType),
}

impl fmt::Display for SyntaxToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyntaxToken::NullToken => write!(f, ""),
            SyntaxToken::ColorText(s, ..) => write!(f, "{}", s),
            SyntaxToken::StateString(s, ..) => write!(f, "{}", s),
            SyntaxToken::ErrorString(s) => write!(f, "{}", s),
            SyntaxToken::MemoryAddressSeg16(_, _, s) => write!(f, "{}", s),
            SyntaxToken::StateMemoryAddressSeg16(_, _, s, _) => write!(f, "{}", s),
            SyntaxToken::MemoryAddressFlat(_, s) => write!(f, "{}", s),
            SyntaxToken::MemoryByteHexValue(_, _, s, ..) => write!(f, "{}", s),
            SyntaxToken::MemoryByteAsciiValue(_, _, s, _) => write!(f, "{}", s),
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
                SyntaxFormatType::HighlightLine(_) => write!(f, ">>> "),
                SyntaxFormatType::Space => write!(f, " "),
                SyntaxFormatType::Tab => write!(f, "\t"),
            },
        }
    }
}

/// A sequence of syntax tokens representing one logical line of output.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct SyntaxTokenStream {
    tokens: Vec<SyntaxToken>,
}

impl fmt::Display for SyntaxTokenStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tok in &self.tokens {
            write!(f, "{}", tok)?;
        }
        Ok(())
    }
}

impl SyntaxTokenStream {
    pub const fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            tokens: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, token: SyntaxToken) {
        self.tokens.push(token);
    }

    pub fn iter(&self) -> Iter<'_, SyntaxToken> {
        self.tokens.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, SyntaxToken> {
        self.tokens.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn as_slice(&self) -> &[SyntaxToken] {
        &self.tokens
    }

    pub fn as_mut_slice(&mut self) -> &mut [SyntaxToken] {
        &mut self.tokens
    }

    pub fn into_inner(self) -> Vec<SyntaxToken> {
        self.tokens
    }

    #[inline]
    pub fn push_space(&mut self, t: SyntaxToken) {
        self.tokens.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        self.tokens.push(t);
    }

    #[inline]
    pub fn push_comma(&mut self, t: SyntaxToken) {
        self.tokens.push(SyntaxToken::Comma);
        self.tokens.push(t);
    }

    #[inline]
    pub fn push_comma_space(&mut self, t: SyntaxToken) {
        self.tokens.push(SyntaxToken::Comma);
        self.tokens.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        self.tokens.push(t);
    }

    #[inline]
    pub fn push_brackets(&mut self, t: SyntaxToken) {
        self.tokens.push(SyntaxToken::OpenBracket);
        self.tokens.push(t);
        self.tokens.push(SyntaxToken::CloseBracket);
    }

    pub fn strip_whitespace(&mut self) {
        self.tokens
            .retain(|item| !matches!(item, SyntaxToken::Formatter(SyntaxFormatType::Space)));
    }

    pub fn retain(&mut self, list: &[SyntaxToken]) {
        let list = list.iter().cloned().collect::<HashSet<_>>();
        self.tokens.retain(|item| list.contains(item));
    }

    pub fn append(
        &mut self,
        items: impl IntoIterator<Item = SyntaxToken>,
        start_tok: Option<SyntaxToken>,
        separator: Option<SyntaxToken>,
    ) {
        if let Some(start) = start_tok {
            self.tokens.push(start);
        }
        if let Some(sep) = separator {
            // If a separator is provided, join with separator and then append
            let mut iter = items.into_iter();
            if let Some(first) = iter.next() {
                self.tokens.push(first);
                for item in iter {
                    self.tokens.push(sep.clone());
                    self.tokens.push(item);
                }
            }
        }
        else {
            self.tokens.extend(items);
        }
    }
}

impl From<Vec<SyntaxToken>> for SyntaxTokenStream {
    fn from(tokens: Vec<SyntaxToken>) -> Self {
        Self { tokens }
    }
}

impl From<SyntaxTokenStream> for Vec<SyntaxToken> {
    fn from(stream: SyntaxTokenStream) -> Self {
        stream.tokens
    }
}

impl FromIterator<SyntaxToken> for SyntaxTokenStream {
    fn from_iter<T: IntoIterator<Item = SyntaxToken>>(iter: T) -> Self {
        Self {
            tokens: iter.into_iter().collect(),
        }
    }
}

impl Extend<SyntaxToken> for SyntaxTokenStream {
    fn extend<T: IntoIterator<Item = SyntaxToken>>(&mut self, iter: T) {
        self.tokens.extend(iter);
    }
}

impl AsRef<[SyntaxToken]> for SyntaxTokenStream {
    fn as_ref(&self) -> &[SyntaxToken] {
        self.as_slice()
    }
}

impl AsMut<[SyntaxToken]> for SyntaxTokenStream {
    fn as_mut(&mut self) -> &mut [SyntaxToken] {
        self.as_mut_slice()
    }
}

impl IntoIterator for SyntaxTokenStream {
    type Item = SyntaxToken;
    type IntoIter = std::vec::IntoIter<SyntaxToken>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens.into_iter()
    }
}

impl<'a> IntoIterator for &'a SyntaxTokenStream {
    type Item = &'a SyntaxToken;
    type IntoIter = Iter<'a, SyntaxToken>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut SyntaxTokenStream {
    type Item = &'a mut SyntaxToken;
    type IntoIter = IterMut<'a, SyntaxToken>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_display_concatenates_token_text() {
        let stream = SyntaxTokenStream::from(vec![
            SyntaxToken::Mnemonic("mov".into()),
            SyntaxToken::Formatter(SyntaxFormatType::Space),
            SyntaxToken::Register("ax".into()),
            SyntaxToken::Comma,
            SyntaxToken::Formatter(SyntaxFormatType::Space),
            SyntaxToken::HexValue("1234h".into()),
        ]);

        assert_eq!(stream.to_string(), "mov ax, 1234h");
    }

    #[test]
    fn memory_token_display_uses_preformatted_text() {
        assert_eq!(
            SyntaxToken::MemoryByteHexValue(0x1234, 0xAB, "AB".into(), false, 0).to_string(),
            "AB"
        );
        assert_eq!(
            SyntaxToken::MemoryByteAsciiValue(0x1234, b'A', "A".into(), 0).to_string(),
            "A"
        );
    }

    #[test]
    fn stream_helpers_build_expected_tokens() {
        let mut stream = SyntaxTokenStream::new();
        stream.push(SyntaxToken::Text("value".into()));
        stream.push_comma_space(SyntaxToken::Text("next".into()));
        stream.push_brackets(SyntaxToken::Register("bx".into()));

        assert_eq!(stream.to_string(), "value, next[bx]");
    }

    #[test]
    fn stream_supports_collection_conversions_and_iteration() {
        let mut stream: SyntaxTokenStream = [SyntaxToken::Text("a".into())].into_iter().collect();
        stream.extend([SyntaxToken::Text("b".into())]);

        assert_eq!(stream.len(), 2);
        assert_eq!(stream.iter().map(ToString::to_string).collect::<String>(), "ab");
        assert_eq!(Vec::<SyntaxToken>::from(stream).len(), 2);
    }
}
