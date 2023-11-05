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

    ---------------------------------------------------------------------------

    backend_common::lib.rs

    Defines a MartyBackend trait that can be implemented for various backends.
*/

use std::fmt;

// Define your custom error types as an enum
pub enum DisplayBackendError {
    InitializationError(String),
    ValidationError(String),
    RenderError(String),
    // Add as many variants as needed for different error cases
}

// Implement the Display trait for your error types to describe the errors
impl fmt::Display for DisplayBackendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DisplayBackendError::InitializationError(ref msg) => write!(f, "Initialization Error: {}", msg),
            DisplayBackendError::ValidationError(ref msg) => write!(f, "Validation Error: {}", msg),
            DisplayBackendError::RenderError(ref msg) => write!(f, "Validation Error: {}", msg),
            // Handle other cases
        }
    }
}

// Implement the Error trait for your error types
impl std::error::Error for DisplayBackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Return the underlying error that caused this, if applicable
        match *self {
            DisplayBackendError::InitializationError(ref e) => Some(e),
            DisplayBackendError::ValidationError(_) => None,
            DisplayBackendError::RenderError(ref e) => Some(e),
            // Handle other cases
        }
    }
}

pub struct BufferDimensions {
    x: u32,
    y: u32,
    pitch: u32,
}

pub trait DisplayBackend {

    fn resize_buf(&mut self, new: BufferDimensions);
    fn resize_surface(&mut self, new: BufferDimensions);

    fn buf_dimensions(&self) -> BufferDimensions;
    fn surface_dimensions(&self) -> BufferDimensions;

    fn buf(&self) -> &[u8];
    fn buf_mut(&mut self) -> &mut [u8];

    fn get_backend_raw<T: Any>(&mut self) -> Option<&mut T>;

    fn render(&mut self) -> Result<(), DisplayBackendError>;
}