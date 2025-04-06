use std::{error::Error, fmt::Display};

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum AtaError {
    NoError,
    InvalidDevice,
    UnsupportedVHD,
}
impl Error for AtaError {}
impl Display for AtaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            AtaError::NoError => write!(f, "No error."),
            AtaError::InvalidDevice => {
                write!(f, "The specified Device ID was out of range [0..1]")
            }
            AtaError::UnsupportedVHD => {
                write!(f, "The VHD file did not match the list of supported drive types.")
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum AtaOperationError {
    NoError,
    NoReadySignal,
    InvalidCommand,
    IllegalAccess,
}

impl Error for AtaOperationError {}
impl Display for AtaOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            AtaOperationError::NoError => write!(f, "No error."),
            AtaOperationError::NoReadySignal => write!(f, "No ready signal."),
            AtaOperationError::InvalidCommand => write!(f, "Invalid command."),
            AtaOperationError::IllegalAccess => write!(f, "Illegal access."),
        }
    }
}
