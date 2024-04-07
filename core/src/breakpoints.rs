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

    ---------------------------------------------------------------------------

    breakpoints.rs

    Implement enum for breakpoint definitions.

*/

#[allow(dead_code)]
pub enum BreakPointType {
    StepOver(u32),       // Breakpoint on next decoded instruction
    Execute(u16, u16),   // Breakpoint on CS:IP
    ExecuteOffset(u16),  // Breakpoint on *::IP
    ExecuteFlat(u32),    // Breakpoint on CS<<4+IP
    MemAccess(u16, u16), // Breakpoint on memory access, seg::offset
    MemAccessFlat(u32),  // Breakpoint on memory access, seg<<4+offset
    Interrupt(u8),       // Breakpoint on interrupt #
}
