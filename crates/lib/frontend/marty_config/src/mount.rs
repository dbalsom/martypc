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
use marty_common::MartyHashMap;
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, PartialEq)]
pub enum MountableDeviceType {
    Cartridge,
    Floppy,
    HardDisk,
}

#[derive(Debug, PartialEq)]
pub struct MountSpec {
    pub device:  MountableDeviceType,
    pub index:   usize,
    pub path:    PathBuf,
    pub options: MartyHashMap<String, String>,
}

impl FromStr for MountSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Split once for optional options
        let (main, opt_str) = match s.split_once('?') {
            Some((left, right)) => (left, Some(right)),
            None => (s, None),
        };

        let mut parts = main.splitn(3, ':');

        let device_str = parts.next().ok_or("Missing device type")?;
        let index_str = parts.next().ok_or("Missing device unit")?;
        let path_str = parts.next().ok_or("Missing file path")?;

        let device = match device_str {
            "fd" => MountableDeviceType::Floppy,
            "hd" => MountableDeviceType::HardDisk,
            "cart" => MountableDeviceType::Cartridge,
            other => return Err(format!("Unknown device type: {other}")),
        };

        let index: usize = index_str
            .parse()
            .map_err(|_| format!("Invalid device index: {index_str}"))?;

        let path = PathBuf::from(path_str);
        let mut options = MartyHashMap::default();

        if let Some(opts) = opt_str {
            for entry in opts.split('&') {
                let (k, v) = entry.split_once('=').unwrap_or((entry, "true"));
                options.insert(k.to_string(), v.to_string());
            }
        }

        Ok(MountSpec {
            device,
            index,
            path,
            options,
        })
    }
}
