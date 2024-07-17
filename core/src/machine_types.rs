/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    machine_types.rs

    This module manages machine-related type definitions.

*/

use core::fmt;
use serde::{self, Deserializer};
use serde_derive::Deserialize;
use std::{fmt::Display, str::FromStr};

#[derive(Copy, Clone, Debug, Deserialize, Hash, Eq, PartialEq)]
pub enum MachineType {
    Default,
    Ibm5150v64K,
    Ibm5150v256K,
    Ibm5160,
    IbmPCJr,
    Tandy1000,
}

impl FromStr for MachineType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "default" => Ok(MachineType::Default),
            "ibm5150v64k" => Ok(MachineType::Ibm5150v64K),
            "ibm5150v256k" => Ok(MachineType::Ibm5150v64K),
            "ibm5160" => Ok(MachineType::Ibm5160),
            "ibm_pcjr" => Ok(MachineType::IbmPCJr),
            "tandy1000" => Ok(MachineType::Tandy1000),
            _ => Err("Bad value for model".to_string()),
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Hash, Eq, PartialEq)]
pub enum SoundType {
    AdLib,
}

impl FromStr for SoundType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "adlib" => Ok(SoundType::AdLib),
            _ => Err("Bad value for SoundType".to_string()),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize)]
pub enum OnHaltBehavior {
    #[default]
    Continue,
    Warn,
    Stop,
}

impl FromStr for OnHaltBehavior {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "continue" => Ok(OnHaltBehavior::Continue),
            "warn" => Ok(OnHaltBehavior::Warn),
            "stop" => Ok(OnHaltBehavior::Stop),
            _ => Err("Bad value for OnHaltBehavior".to_string()),
        }
    }
}

#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq)]
pub enum FloppyDriveType {
    #[default]
    Floppy360K,
    Floppy720K,
    Floppy12M,
    Floppy144M,
}

impl Display for FloppyDriveType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FloppyDriveType::Floppy360K => write!(f, "360K"),
            FloppyDriveType::Floppy720K => write!(f, "720K"),
            FloppyDriveType::Floppy12M => write!(f, "1.2M"),
            FloppyDriveType::Floppy144M => write!(f, "1.44M"),
        }
    }
}

impl FromStr for FloppyDriveType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "floppy360k" => Ok(FloppyDriveType::Floppy360K),
            "floppy720k" => Ok(FloppyDriveType::Floppy720K),
            "floppy12m" => Ok(FloppyDriveType::Floppy12M),
            "floppy144m" => Ok(FloppyDriveType::Floppy144M),
            _ => Err("Bad value for floppy drive type".to_string()),
        }
    }
}

// Implement Deserialize for FloppyType
impl<'de> serde::Deserialize<'de> for FloppyDriveType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FloppyTypeVisitor;

        impl<'de> serde::de::Visitor<'de> for FloppyTypeVisitor {
            type Value = FloppyDriveType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("`360k`, `720k`, `1.2m` or `1.44m`")
            }

            fn visit_str<E>(self, value: &str) -> Result<FloppyDriveType, E>
            where
                E: serde::de::Error,
            {
                match value.to_lowercase().as_str() {
                    "360k" => Ok(FloppyDriveType::Floppy360K),
                    "720k" => Ok(FloppyDriveType::Floppy720K),
                    "1.2m" => Ok(FloppyDriveType::Floppy12M),
                    "1.44m" => Ok(FloppyDriveType::Floppy144M),
                    _ => Err(E::custom(format!("invalid floppy type: {}", value))),
                }
            }
        }

        deserializer.deserialize_str(FloppyTypeVisitor)
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum FdcType {
    IbmNec,
    IbmPCJrNec,
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum HardDiskControllerType {
    IbmXebec,
}

impl FromStr for HardDiskControllerType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "ibmxebec" => Ok(HardDiskControllerType::IbmXebec),
            _ => Err("Bad value for HardDiskControllerType".to_string()),
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum HardDriveFormat {
    Mfm,
    Rll,
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum SerialControllerType {
    IbmAsync,
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum SerialMouseType {
    Microsoft,
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum EmsType {
    LoTech2MB,
}
