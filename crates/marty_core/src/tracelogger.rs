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

    ---------------------------------------------------------------------------

    tracelogger.rs

    This module implements a logging enum, designed to be passed to devices
    that may wish to implement logging.

    Thanks to Bigbass for the suggestion that avoids references.
*/

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

#[derive(Debug)]
pub enum TraceLogger {
    FileWriter(BufWriter<File>),
    Console,
    None,
}

impl Default for TraceLogger {
    fn default() -> TraceLogger {
        TraceLogger::None
    }
}

impl TraceLogger {
    pub fn from_filename<S: AsRef<Path>>(filename: S) -> Self {
        match File::create(filename) {
            Ok(file) => TraceLogger::FileWriter(BufWriter::new(file)),
            Err(e) => {
                eprintln!("Couldn't create specified video tracelog file: {}", e);
                TraceLogger::None
            }
        }
    }

    #[inline(always)]
    pub fn print<S: AsRef<str> + std::fmt::Display>(&mut self, msg: S) {
        match self {
            TraceLogger::FileWriter(buf) => {
                _ = buf.write_all(msg.as_ref().as_bytes());
            }
            TraceLogger::Console => println!("{}", msg),
            TraceLogger::None => (),
        }
    }

    #[inline(always)]
    pub fn println<S: AsRef<str> + std::fmt::Display>(&mut self, msg: S) {
        match self {
            TraceLogger::FileWriter(buf) => {
                _ = buf.write_all(msg.as_ref().as_bytes());
                _ = buf.write_all("\n".as_bytes());
            }
            TraceLogger::Console => println!("{}", msg),
            TraceLogger::None => (),
        }
    }

    pub fn flush(&mut self) {
        if let TraceLogger::FileWriter(file) = self {
            if let Err(e) = file.flush() {
                log::error!("Failed to flush trace log: {}", e);
            }
        }
    }

    #[inline(always)]
    pub fn is_some(&self) -> bool {
        matches!(*self, TraceLogger::FileWriter(_) | TraceLogger::Console)
    }
}
