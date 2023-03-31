/*
    Marty PC Emulator 
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

    ---------------------------------------------------------------------------

    This module implements a logging enum, designed to be passed to devices
    that may wish to implement logging. 

    Thanks to BigBass for the suggestion that avoids references.
*/

use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;

pub enum TraceLogger {
    FileWriter(BufWriter<File>),
    Console,
    None,
}

impl TraceLogger {

    pub fn from_filename<S: AsRef<Path>>(filename: S) -> Self {
        match File::create(filename) {
            Ok(file) => {
                TraceLogger::FileWriter(BufWriter::new(file))

            },
            Err(e) => {
                eprintln!("Couldn't create specified video tracelog file: {}", e);
                TraceLogger::None
            }
        }        
    }

    #[inline(always)]
    pub fn print<S: AsRef<str> + std::fmt::Display>(&mut self, msg: S) {
        match self {
            TraceLogger::FileWriter(buf) => { _ = buf.write_all(msg.as_ref().as_bytes()); },
            TraceLogger::Console => println!("{}", msg),
            TraceLogger::None => (),
        }
    }

    pub fn flush(&mut self) {
        if let TraceLogger::FileWriter(file) = self {
            _ = file.flush()
        }
    }
}