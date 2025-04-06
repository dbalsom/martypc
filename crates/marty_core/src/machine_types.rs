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

    machine_types.rs

    This module manages machine-related type definitions.

*/

use core::fmt;
use fluxfox::StandardFormat;
use serde::{self, Deserializer};
use serde_derive::Deserialize;
use std::{fmt::Display, str::FromStr};

#[derive(Copy, Clone, Debug, Default, Deserialize, Hash, Eq, PartialEq)]
pub enum MachineType {
    Default,
    Ibm5150v64K,
    #[default]
    Ibm5150v256K,
    Ibm5160,
    IbmPCJr,
    Tandy1000,
    Tandy1000SL,
    CompaqPortable,
    CompaqDeskpro,
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
            "tandy1000sl" => Ok(MachineType::Tandy1000SL),
            "compaq_deskpro" => Ok(MachineType::CompaqDeskpro),
            _ => Err("Bad value for model".to_string()),
        }
    }
}

impl MachineType {
    pub fn has_ppi_turbo_bit(&self) -> bool {
        match self {
            MachineType::Ibm5150v64K => false,
            MachineType::Ibm5150v256K => false,
            MachineType::Ibm5160 => true,
            MachineType::IbmPCJr => false,
            MachineType::Tandy1000 => false,
            MachineType::Tandy1000SL => false,
            MachineType::CompaqPortable => false,
            MachineType::CompaqDeskpro => false,
            _ => false,
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

impl FloppyDriveType {
    pub fn get_compatible_formats(&self) -> Vec<StandardFormat> {
        match self {
            FloppyDriveType::Floppy360K => vec![
                StandardFormat::PcFloppy160,
                StandardFormat::PcFloppy180,
                StandardFormat::PcFloppy320,
                StandardFormat::PcFloppy360,
            ],
            FloppyDriveType::Floppy720K => vec![StandardFormat::PcFloppy720],
            FloppyDriveType::Floppy12M => vec![
                StandardFormat::PcFloppy160,
                StandardFormat::PcFloppy180,
                StandardFormat::PcFloppy320,
                StandardFormat::PcFloppy360,
                StandardFormat::PcFloppy1200,
            ],
            FloppyDriveType::Floppy144M => vec![StandardFormat::PcFloppy720, StandardFormat::PcFloppy1440],
        }
    }
}

/// Convert MartyPC's FloppyDriveType to fluxfox's StandardFormat
impl From<FloppyDriveType> for StandardFormat {
    fn from(val: FloppyDriveType) -> Self {
        match val {
            FloppyDriveType::Floppy360K => StandardFormat::PcFloppy360,
            FloppyDriveType::Floppy720K => StandardFormat::PcFloppy720,
            FloppyDriveType::Floppy12M => StandardFormat::PcFloppy1200,
            FloppyDriveType::Floppy144M => StandardFormat::PcFloppy2880,
        }
    }
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

        impl serde::de::Visitor<'_> for FloppyTypeVisitor {
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
    XtIde,
    JrIde,
}

impl FromStr for HardDiskControllerType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "ibmxebec" => Ok(HardDiskControllerType::IbmXebec),
            "xtide" => Ok(HardDiskControllerType::XtIde),
            "jride" => Ok(HardDiskControllerType::JrIde),
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
pub enum ParallelControllerType {
    Standard,
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum EmsType {
    LoTech2MB,
}
