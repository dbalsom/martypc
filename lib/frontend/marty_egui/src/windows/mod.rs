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

    egui::src::windows::mod.rs

    Various gui windows
*/

pub mod composite_adjust;
pub mod cpu_control;
pub mod disassembly_viewer;
// Bring in submodules
pub mod about;
pub mod call_stack_viewer;
pub mod cpu_state_viewer;
pub mod cycle_trace_viewer;
pub mod data_visualizer;
pub mod delay_adjust;
pub mod device_control;
pub mod dma_viewer;
pub mod instruction_history_viewer;
pub mod io_stats_viewer;
pub mod ivt_viewer;
pub mod memory_viewer;
pub mod performance_viewer;
pub mod pic_viewer;
pub mod pit_viewer;
pub mod ppi_viewer;
pub mod scaler_adjust;
pub mod serial_viewer;
pub mod text_mode_viewer;
pub mod vhd_creator;
pub mod videocard_viewer;
