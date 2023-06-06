/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    --------------------------------------------------------------------------

    memerror.rs

    Defines the Memory Error enum.
*/


#![allow(dead_code)]
use std::error::Error;
use core::fmt::Display;

#[derive(Debug)]
pub enum MemError {
    ReadOutOfBoundsError,
    SeekOutOfBoundsError,
    FileReadError,
    MmioError,
}
impl Error for MemError {}
impl Display for MemError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            MemError::ReadOutOfBoundsError => write!(f, "An attempt was made to read out of buffer bounds."),
            MemError::SeekOutOfBoundsError => write!(f, "An attempt was made to move the buffer cursor out of bounds."),
            MemError::FileReadError => write!(f, "Error reading file into MemBuf."),
            MemError::MmioError => write!(f, "Error accessing map for memory mapped device.")
        }
    }
}