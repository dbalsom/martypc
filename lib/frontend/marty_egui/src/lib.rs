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

    egui::src::lib.rs

    MartyPC's implementation of an egui-based GUI.
*/

extern crate core;

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    ffi::OsString,
    hash::Hash,
    mem::{discriminant, Discriminant},
    rc::Rc,
    time::Duration,
};

use egui::{Color32, ColorImage, Context, Visuals};
use egui_notify::{Anchor, Toasts};

use frontend_common::{
    display_manager::DisplayInfo,
    display_scaler::{ScalerMode, ScalerParams, ScalerPreset},
};

use serialport::SerialPortInfo;

mod color;
mod constants;
mod image;

pub mod context;
mod layouts;
mod menu;
pub mod state;
mod theme;
mod token_listview;
mod ui;
mod widgets;
mod windows;

use marty_core::{
    devices::{
        implementations::{hdc::HardDiskFormat, pic::PicStringState, pit::PitDisplayState, ppi::PpiStringState},
        traits::videocard::{DisplayApertureDesc, DisplayApertureType, VideoCardState, VideoCardStateEntry},
    },
    machine::{ExecutionControl, MachineState},
};

use crate::windows::text_mode_viewer::TextModeViewer;
use videocard_renderer::CompositeParams;

#[derive(PartialEq, Eq, Hash)]
pub enum GuiWindow {
    About,
    CpuControl,
    PerfViewer,
    MemoryViewer,
    CompositeAdjust,
    ScalerAdjust,
    CpuStateViewer,
    HistoryViewer,
    IvrViewer,
    DelayAdjust,
    DeviceControl,
    DisassemblyViewer,
    PitViewer,
    PicViewer,
    PpiViewer,
    DmaViewer,
    VideoCardViewer,
    VideoMemViewer,
    CallStack,
    VHDCreator,
    CycleTraceViewer,
    TextModeViewer,
}

pub enum GuiVariable {
    Bool(GuiBoolean, bool),
    Enum(GuiEnum),
}

#[derive(PartialEq, Eq, Hash)]
pub enum GuiBoolean {
    // Boolean options
    CpuEnableWaitStates,
    CpuInstructionHistory,
    CpuTraceLoggingEnabled,
    TurboButton,
}

// Enums are hashed with with a tuple of GuiEnumContext and their base discriminant.
// This allows the same enum to be stored in different contexts, ie, a DisplayAperture can be
// stored for each Display context.  The Global context can be used if no specific context is
// required.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum GuiVariableContext {
    Global,
    Display(usize),
}
impl Default for GuiVariableContext {
    fn default() -> Self {
        GuiVariableContext::Global
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiEnum {
    DisplayAspectCorrect(bool),
    DisplayAperture(DisplayApertureType),
    DisplayScalerMode(ScalerMode),
    DisplayScalerPreset(String),
    DisplayComposite(bool),
}

fn create_default_variant(ge: GuiEnum) -> GuiEnum {
    match ge {
        GuiEnum::DisplayAspectCorrect(_) => GuiEnum::DisplayAspectCorrect(Default::default()),
        GuiEnum::DisplayAperture(_) => GuiEnum::DisplayAperture(Default::default()),
        GuiEnum::DisplayScalerMode(_) => GuiEnum::DisplayAperture(Default::default()),
        GuiEnum::DisplayScalerPreset(_) => GuiEnum::DisplayScalerPreset(String::new()),
        GuiEnum::DisplayComposite(_) => GuiEnum::DisplayComposite(Default::default()),
    }
}

type GuiEnumMap = HashMap<(GuiVariableContext, Discriminant<GuiEnum>), GuiEnum>;

#[allow(dead_code)]
pub enum GuiEvent {
    LoadVHD(usize, OsString),
    CreateVHD(OsString, HardDiskFormat),
    LoadFloppy(usize, usize),
    SaveFloppy(usize, usize),
    EjectFloppy(usize),
    SetFloppyWriteProtect(usize, bool),
    BridgeSerialPort(String),
    DumpVRAM,
    DumpCS,
    DumpAllMem,
    EditBreakpoint,
    MemoryUpdate,
    TokenHover(usize),
    VariableChanged(GuiVariableContext, GuiVariable),
    CompositeAdjust(CompositeParams),
    ScalerAdjust(ScalerParams),
    FlushLogs,
    DelayAdjust,
    TickDevice(DeviceSelection, u32),
    MachineStateChange(MachineState),
    TakeScreenshot(usize),
    Exit,
    SetNMI(bool),
    TriggerParity,
    RescanMediaFolders,
    CtrlAltDel,
    ZoomChanged(f32),
}

pub enum DeviceSelection {
    Timer(u8),
    VideoCard,
}

#[derive(Clone, Default)]
pub struct PerformanceStats {
    pub adapter: String,
    pub backend: String,
    pub dti: Vec<DisplayInfo>,
    pub current_ups: u32,
    pub current_fps: u32,
    pub emulated_fps: u32,
    pub cycle_target: u32,
    pub current_cps: u64,
    pub current_tps: u64,
    pub current_ips: u64,
    pub emulation_time: Duration,
    pub render_time: Duration,
    pub gui_time: Duration,
}

pub struct GuiEventQueue(VecDeque<GuiEvent>);

impl GuiEventQueue {
    fn new() -> Self {
        GuiEventQueue(VecDeque::new())
    }

    // Send a GuiEvent to the queue
    fn send(&mut self, event: GuiEvent) {
        self.0.push_back(event);
    }

    // Send a GuiEvent to the queue
    fn pop(&mut self) -> Option<GuiEvent> {
        self.0.pop_front()
    }
}

#[derive(Copy, Clone, Default)]
pub struct MediaTrayState {
    pub floppy: u8,
    pub hdd:    u8,
    pub turtle: u8,
}
