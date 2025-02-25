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

    egui::src::lib.rs

    MartyPC's implementation of an egui-based GUI.
*/

extern crate core;

use egui::{Color32, Context, Visuals};
use fluxfox::{DiskImageFileFormat, StandardFormat};
use lazy_static::lazy_static;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    ffi::OsString,
    hash::Hash,
    mem::Discriminant,
    path::PathBuf,
    time::Duration,
};

use marty_frontend_common::{
    display_manager::DisplayTargetInfo,
    display_scaler::{ScalerMode, ScalerParams},
};

mod color;
mod constants;
mod image;

mod glyphs;
mod layouts;
mod menu;
pub mod modal;
pub mod state;
pub mod themes;
mod token_listview;
mod ui;
mod widgets;
mod windows;
mod workspace;

use marty_core::{
    device_traits::videocard::DisplayApertureType,
    device_types::hdc::HardDiskFormat,
    devices::pic::PicStringState,
    machine::MachineState,
};

use marty_frontend_common::display_manager::{DisplayTargetType, DtHandle};
use marty_videocard_renderer::CompositeParams;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Clone, EnumIter, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd, Debug)]
pub enum GuiWindow {
    About,
    CpuControl,
    PerfViewer,
    MemoryViewer,
    CompositeAdjust,
    ScalerAdjust,
    CpuStateViewer,
    InstructionHistoryViewer,
    IvtViewer,
    IoStatsViewer,
    DelayAdjust,
    DeviceControl,
    DisassemblyViewer,
    PitViewer,
    SerialViewer,
    PicViewer,
    PpiViewer,
    DmaViewer,
    VideoCardViewer,
    DataVisualizer,
    CallStack,
    VHDCreator,
    CycleTraceViewer,
    TextModeViewer,
    FdcViewer,
    FloppyViewer,
}

#[derive(Copy, Clone, Debug)]
pub enum InputFieldChangeSource {
    None,
    ScrollEvent,
    UserInput,
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
    ShowBackBuffer,
    ShowRasterPosition,
}

// Enums are hashed with a tuple of GuiEnumContext and their base discriminant.
// This allows the same enum to be stored in different contexts, ie, a DisplayAperture can be
// stored for each Display context.  The Global context can be used if no specific context is
// required.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum GuiVariableContext {
    Global,
    Display(DtHandle),
    SerialPort(usize),
}
impl Default for GuiVariableContext {
    fn default() -> Self {
        GuiVariableContext::Global
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiEnum {
    DisplayType(DisplayTargetType),
    DisplayAspectCorrect(bool),
    DisplayAperture(DisplayApertureType),
    DisplayScalerMode(ScalerMode),
    DisplayScalerPreset(String),
    DisplayComposite(bool),
    WindowBezel(bool),
    SerialPortBridge(usize),
}

fn create_default_variant(ge: GuiEnum) -> GuiEnum {
    match ge {
        GuiEnum::DisplayType(_) => GuiEnum::DisplayType(Default::default()),
        GuiEnum::DisplayAspectCorrect(_) => GuiEnum::DisplayAspectCorrect(Default::default()),
        GuiEnum::DisplayAperture(_) => GuiEnum::DisplayAperture(Default::default()),
        GuiEnum::DisplayScalerMode(_) => GuiEnum::DisplayAperture(Default::default()),
        GuiEnum::DisplayScalerPreset(_) => GuiEnum::DisplayScalerPreset(String::new()),
        GuiEnum::DisplayComposite(_) => GuiEnum::DisplayComposite(Default::default()),
        GuiEnum::WindowBezel(_) => GuiEnum::WindowBezel(Default::default()),
        GuiEnum::SerialPortBridge(_) => GuiEnum::SerialPortBridge(Default::default()),
    }
}

type GuiEnumMap = HashMap<(GuiVariableContext, Discriminant<GuiEnum>), GuiEnum>;

#[allow(dead_code)]
pub enum GuiEvent {
    LoadVHD(usize, usize),
    DetachVHD(usize),
    CreateVHD(OsString, HardDiskFormat),
    LoadQuickFloppy(usize, usize),
    RequestLoadFloppyDialog(usize),
    LoadFloppyAs(usize, PathBuf),
    LoadAutoFloppy(usize, PathBuf),
    SaveFloppy(usize, usize),                          // Drive index, disk index
    SaveFloppyAs(usize, DiskImageFileFormat, PathBuf), // Drive image, format, requested path
    EjectFloppy(usize),
    CreateNewFloppy(usize, StandardFormat, bool),
    QueryCompatibleFloppyFormats(usize),
    SetFloppyWriteProtect(usize, bool),
    BridgeSerialPort(usize, String, usize),
    DumpVRAM,
    DumpCS,
    DumpAllMem,
    EditBreakpoint,
    MemoryUpdate,
    TokenHover(usize),
    VariableChanged(GuiVariableContext, GuiVariable),
    CompositeAdjust(DtHandle, CompositeParams),
    ScalerAdjust(usize, ScalerParams),
    FlushLogs,
    DelayAdjust,
    TickDevice(DeviceSelection, u32),
    MachineStateChange(MachineState),
    TakeScreenshot(usize),
    ToggleFullscreen(usize),
    Exit,
    SetNMI(bool),
    TriggerParity,
    RescanMediaFolders,
    CtrlAltDel,
    ZoomChanged(f32),
    ResetIOStats,
    StartRecordingDisassembly,
    StopRecordingDisassembly,
    InsertCartridge(usize, usize),
    RemoveCartridge(usize),
}

pub enum DeviceSelection {
    Timer(u8),
    VideoCard,
}

#[derive(Clone, Default)]
pub struct PerformanceStats {
    pub adapter: String,
    pub backend: String,
    pub dti: Vec<DisplayTargetInfo>,
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

pub struct WorkspaceWindowDef {
    pub id: GuiWindow,
    pub title: &'static str,
    pub menu: &'static str,
    pub width: f32,
    pub resizable: bool,
}

lazy_static! {
    static ref WORKSPACE_WINDOWS: BTreeMap<GuiWindow, WorkspaceWindowDef> = [
        (
            GuiWindow::About,
            WorkspaceWindowDef {
                id: GuiWindow::About,
                title: "About",
                menu: "About",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::PerfViewer,
            WorkspaceWindowDef {
                id: GuiWindow::PerfViewer,
                title: "Performance",
                menu: "Performance Viewer",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::CpuControl,
            WorkspaceWindowDef {
                id: GuiWindow::CpuControl,
                title: "CPU Control",
                menu: "CPU Control",
                width: 300.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::MemoryViewer,
            WorkspaceWindowDef {
                id: GuiWindow::MemoryViewer,
                title: "Memory Viewer",
                menu: "Memory Viewer",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::CompositeAdjust,
            WorkspaceWindowDef {
                id: GuiWindow::CompositeAdjust,
                title: "Composite Adjustment",
                menu: "Composite Adjustment",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::ScalerAdjust,
            WorkspaceWindowDef {
                id: GuiWindow::ScalerAdjust,
                title: "Scaler Adjustment",
                menu: "Scaler Adjustment",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::CpuStateViewer,
            WorkspaceWindowDef {
                id: GuiWindow::CpuStateViewer,
                title: "CPU State Viewer",
                menu: "CPU State",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::InstructionHistoryViewer,
            WorkspaceWindowDef {
                id: GuiWindow::InstructionHistoryViewer,
                title: "Instruction History",
                menu: "Instruction History",
                width: 540.0,
                resizable: true,
            },
        ),
        (
            GuiWindow::CycleTraceViewer,
            WorkspaceWindowDef {
                id: GuiWindow::CycleTraceViewer,
                title: "Cycle Trace",
                menu: "Cycle Trace",
                width: 600.0,
                resizable: true,
            },
        ),
        (
            GuiWindow::CallStack,
            WorkspaceWindowDef {
                id: GuiWindow::CallStack,
                title: "Call Stack",
                menu: "Call Stack",
                width: 540.0,
                resizable: true,
            },
        ),
        (
            GuiWindow::IvtViewer,
            WorkspaceWindowDef {
                id: GuiWindow::IvtViewer,
                title: "IVT Viewer",
                menu: "IVT",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::IoStatsViewer,
            WorkspaceWindowDef {
                id: GuiWindow::IoStatsViewer,
                title: "IO Stats Viewer",
                menu: "IO Stats Viewer",
                width: 500.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::DelayAdjust,
            WorkspaceWindowDef {
                id: GuiWindow::DelayAdjust,
                title: "Delay Adjust",
                menu: "Delay Adjust",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::DeviceControl,
            WorkspaceWindowDef {
                id: GuiWindow::DeviceControl,
                title: "Device Control",
                menu: "Device Control",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::DisassemblyViewer,
            WorkspaceWindowDef {
                id: GuiWindow::DisassemblyViewer,
                title: "Disassembly Viewer",
                menu: "Disassembly",
                width: 540.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::PitViewer,
            WorkspaceWindowDef {
                id: GuiWindow::PitViewer,
                title: "PIT Viewer",
                menu: "PIT",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::SerialViewer,
            WorkspaceWindowDef {
                id: GuiWindow::SerialViewer,
                title: "Serial Port Viewer",
                menu: "Serial Ports",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::PicViewer,
            WorkspaceWindowDef {
                id: GuiWindow::PicViewer,
                title: "PIC Viewer",
                menu: "PIC",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::PpiViewer,
            WorkspaceWindowDef {
                id: GuiWindow::PpiViewer,
                title: "PPI Viewer",
                menu: "PPI",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::DmaViewer,
            WorkspaceWindowDef {
                id: GuiWindow::DmaViewer,
                title: "DMA Viewer",
                menu: "DMA",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::VideoCardViewer,
            WorkspaceWindowDef {
                id: GuiWindow::VideoCardViewer,
                title: "Video Card Viewer",
                menu: "Video Card",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::VHDCreator,
            WorkspaceWindowDef {
                id: GuiWindow::VHDCreator,
                title: "VHD Creator",
                menu: "Create VHD",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::DataVisualizer,
            WorkspaceWindowDef {
                id: GuiWindow::DataVisualizer,
                title: "Data Visualizer",
                menu: "Data Visualizer",
                width: 400.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::TextModeViewer,
            WorkspaceWindowDef {
                id: GuiWindow::TextModeViewer,
                title: "Text Mode Viewer",
                menu: "Text Mode Viewer",
                width: 700.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::FdcViewer,
            WorkspaceWindowDef {
                id: GuiWindow::FdcViewer,
                title: "FDC Viewer",
                menu: "FDC",
                width: 800.0,
                resizable: false,
            },
        ),
        (
            GuiWindow::FloppyViewer,
            WorkspaceWindowDef {
                id: GuiWindow::FloppyViewer,
                title: "Floppy Viewer",
                menu: "Floppy Viewer",
                width: 700.0,
                resizable: false,
            },
        ),
    ]
    .into();
}
