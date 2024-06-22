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

    marty_egui::state.rs

    EGUI State management
*/

use crate::{
    GuiBoolean,
    GuiEnum,
    GuiEnumMap,
    GuiEvent,
    GuiEventQueue,
    GuiVariableContext,
    GuiWindow,
    MediaTrayState,
    PerformanceStats,
};
use egui::ColorImage;
use egui_notify::{Anchor, Toasts};
use frontend_common::{
    display_manager::DisplayInfo,
    display_scaler::{ScalerMode, ScalerPreset},
    resource_manager::PathTreeNode,
};
use marty_core::{
    device_traits::videocard::{DisplayApertureDesc, VideoCardState, VideoCardStateEntry},
    devices::{pit::PitDisplayState, serial::SerialPortDescriptor},
    machine::{ExecutionControl, MachineState},
};
use serde::{Deserialize, Serialize};
use serialport::SerialPortInfo;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    mem::discriminant,
    path::PathBuf,
    rc::Rc,
};
use strum::IntoEnumIterator;

use crate::{
    widgets::file_tree_menu::FileTreeMenu,
    windows::{
        about::AboutDialog,
        call_stack_viewer::CallStackViewer,
        composite_adjust::CompositeAdjustControl,
        cpu_control::{BreakpointSet, CpuControl},
        cpu_state_viewer::CpuViewerControl,
        cycle_trace_viewer::CycleTraceViewerControl,
        delay_adjust::DelayAdjustControl,
        device_control::DeviceControl,
        disassembly_viewer::DisassemblyControl,
        dma_viewer::DmaViewerControl,
        instruction_history_viewer::InstructionHistoryControl,
        io_stats_viewer::IoStatsViewerControl,
        ivt_viewer::IvtViewerControl,
        memory_viewer::MemoryViewerControl,
        performance_viewer::PerformanceViewerControl,
        pic_viewer::PicViewerControl,
        pit_viewer::PitViewerControl,
        ppi_viewer::PpiViewerControl,
        scaler_adjust::ScalerAdjustControl,
        serial_viewer::SerialViewerControl,
        text_mode_viewer::TextModeViewer,
        vhd_creator::VhdCreator,
    },
};

pub struct GuiFloppyDriveInfo {
    pub(crate) idx: usize,
    pub(crate) selected_idx: Option<usize>,
    pub(crate) selected_path: Option<PathBuf>,
    pub(crate) write_protected: bool,
}

impl GuiFloppyDriveInfo {
    pub fn filename(&self) -> Option<String> {
        match &self.selected_path {
            Some(path) => Some(path.to_string_lossy().to_string()),
            None => None,
        }
    }
}

pub struct GuiHddInfo {
    pub(crate) idx: usize,
    pub(crate) selected_idx: Option<usize>,
    pub(crate) selected_path: Option<PathBuf>,
    pub(crate) write_protected: bool,
}

impl GuiHddInfo {
    pub fn filename(&self) -> Option<String> {
        match &self.selected_path {
            Some(path) => Some(path.to_string_lossy().to_string()),
            None => None,
        }
    }
}

pub struct GuiCartInfo {
    pub(crate) idx: usize,
    pub(crate) selected_idx: Option<usize>,
    pub(crate) selected_path: Option<PathBuf>,
}

impl GuiCartInfo {
    pub fn filename(&self) -> Option<String> {
        match &self.selected_path {
            Some(path) => Some(path.to_string_lossy().to_string()),
            None => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceWindowState {
    pub open: bool,
    pub resizable: bool,
    pub initial_pos: Option<egui::Pos2>,
    pub pos: egui::Pos2,
    pub initial_size: Option<egui::Vec2>,
    pub size: egui::Vec2,
}

impl Default for WorkspaceWindowState {
    fn default() -> Self {
        Self {
            open: false,
            resizable: false,
            initial_pos: None,
            pos: egui::Pos2::new(0.0, 0.0),
            initial_size: None,
            size: egui::Vec2::new(0.0, 0.0),
        }
    }
}

pub struct GuiState {
    pub(crate) event_queue: GuiEventQueue,

    pub(crate) toasts: Toasts,
    media_tray: MediaTrayState,

    /// Only show the associated window when true.
    pub(crate) window_open_flags: HashMap<GuiWindow, bool>,
    pub(crate) window_state: BTreeMap<GuiWindow, WorkspaceWindowState>,
    pub(crate) error_dialog_open: bool,
    pub(crate) warning_dialog_open: bool,

    pub(crate) option_flags: HashMap<GuiBoolean, bool>,
    pub(crate) option_enums: GuiEnumMap,

    pub(crate) machine_state: MachineState,

    video_mem: ColorImage,
    pub(crate) perf_stats: PerformanceStats,

    // Display stuff
    pub(crate) display_apertures: HashMap<usize, Vec<DisplayApertureDesc>>,
    pub(crate) scaler_modes: Vec<ScalerMode>,
    pub(crate) scaler_presets: Vec<String>,

    // Media Images
    pub(crate) floppy_drives: Vec<GuiFloppyDriveInfo>,
    pub(crate) hdds: Vec<GuiHddInfo>,
    pub(crate) carts: Vec<GuiCartInfo>,

    // VHD Images
    pub(crate) vhd_names: Vec<OsString>,

    // Serial ports
    pub(crate) serial_ports: Vec<SerialPortDescriptor>,
    pub(crate) host_serial_ports: Vec<SerialPortInfo>,
    pub(crate) serial_port_name: String,

    pub(crate) exec_control: Rc<RefCell<ExecutionControl>>,

    pub(crate) error_string:   String,
    pub(crate) warning_string: String,

    pub about_dialog: AboutDialog,
    pub cpu_control: CpuControl,
    pub cpu_viewer: CpuViewerControl,
    pub cycle_trace_viewer: CycleTraceViewerControl,
    pub memory_viewer: MemoryViewerControl,

    pub perf_viewer:  PerformanceViewerControl,
    pub delay_adjust: DelayAdjustControl,

    pub pit_viewer:    PitViewerControl,
    pub serial_viewer: SerialViewerControl,
    pub pic_viewer:    PicViewerControl,
    pub ppi_viewer:    PpiViewerControl,

    pub videocard_state: VideoCardState,
    pub display_info:    Vec<DisplayInfo>,

    pub disassembly_viewer: DisassemblyControl,
    pub dma_viewer: DmaViewerControl,
    pub trace_viewer: InstructionHistoryControl,
    pub composite_adjust: CompositeAdjustControl,
    pub scaler_adjust: ScalerAdjustControl,
    pub ivt_viewer: IvtViewerControl,
    pub io_stats_viewer: IoStatsViewerControl,
    pub device_control: DeviceControl,
    pub vhd_creator: VhdCreator,
    pub text_mode_viewer: TextModeViewer,
    pub call_stack_viewer: CallStackViewer,

    pub floppy_tree_menu: FileTreeMenu,
    pub hdd_tree_menu:    FileTreeMenu,
    pub cart_tree_menu:   FileTreeMenu,
    //pub(crate) global_zoom: f32,
}

impl GuiState {
    /// Create a struct representing the state of the GUI.
    pub fn new(exec_control: Rc<RefCell<ExecutionControl>>) -> Self {
        // Set default values for window open flags

        let mut window_open_flags = HashMap::new();
        for window in GuiWindow::iter() {
            window_open_flags.insert(window, false);
        }

        let mut window_state = BTreeMap::new();
        for window in GuiWindow::iter() {
            window_state.insert(window, WorkspaceWindowState::default());
        }

        /*        let window_open_flags: HashMap<GuiWindow, bool> = [
            (GuiWindow::About, false),
            (GuiWindow::CpuControl, false),
            (GuiWindow::PerfViewer, false),
            (GuiWindow::MemoryViewer, false),
            (GuiWindow::CompositeAdjust, false),
            (GuiWindow::ScalerAdjust, false),
            (GuiWindow::CpuStateViewer, false),
            (GuiWindow::HistoryViewer, false),
            (GuiWindow::IvtViewer, false),
            (GuiWindow::DelayAdjust, false),
            (GuiWindow::DeviceControl, false),
            (GuiWindow::DisassemblyViewer, false),
            (GuiWindow::PitViewer, false),
            (GuiWindow::PicViewer, false),
            (GuiWindow::PpiViewer, false),
            (GuiWindow::DmaViewer, false),
            (GuiWindow::VideoCardViewer, false),
            (GuiWindow::VideoMemViewer, false),
            (GuiWindow::CallStack, false),
            (GuiWindow::VHDCreator, false),
            (GuiWindow::CycleTraceViewer, false),
            (GuiWindow::TextModeViewer, false),
        ]
        .into();*/

        let option_flags: HashMap<GuiBoolean, bool> = [
            //(GuiBoolean::CompositeDisplay, false),
            //(GuiBoolean::CorrectAspect, false),
            (GuiBoolean::CpuEnableWaitStates, true),
            (GuiBoolean::CpuInstructionHistory, false),
            (GuiBoolean::CpuTraceLoggingEnabled, false),
            (GuiBoolean::TurboButton, false),
            (GuiBoolean::ShowBackBuffer, false),
            (GuiBoolean::ShowRasterPosition, true),
            //(GuiBoolean::EnableSnow, true),
        ]
        .into();

        let option_enums = HashMap::new();

        Self {
            event_queue: GuiEventQueue::new(),
            toasts: Toasts::new().with_anchor(Anchor::BottomRight),
            media_tray: Default::default(),

            window_open_flags,
            window_state,
            error_dialog_open: false,
            warning_dialog_open: false,

            option_flags,
            option_enums,

            machine_state: MachineState::Off,
            video_mem: ColorImage::new([320, 200], egui::Color32::BLACK),

            perf_stats: Default::default(),

            display_apertures: Default::default(),
            scaler_modes: Vec::new(),
            scaler_presets: Vec::new(),

            floppy_drives: Vec::new(),
            hdds: Vec::new(),
            carts: Vec::new(),
            vhd_names: Vec::new(),

            serial_ports: Vec::new(),
            host_serial_ports: Vec::new(),
            serial_port_name: String::new(),

            exec_control: exec_control.clone(),

            error_string: String::new(),
            warning_string: String::new(),

            about_dialog: AboutDialog::new(),
            cpu_control: CpuControl::new(exec_control.clone()),
            cpu_viewer: CpuViewerControl::new(),
            cycle_trace_viewer: CycleTraceViewerControl::new(),
            memory_viewer: MemoryViewerControl::new(),

            perf_viewer: PerformanceViewerControl::new(),
            delay_adjust: DelayAdjustControl::new(),
            pit_viewer: PitViewerControl::new(),
            serial_viewer: SerialViewerControl::new(),
            pic_viewer: PicViewerControl::new(),
            ppi_viewer: PpiViewerControl::new(),

            videocard_state: Default::default(),
            display_info: Vec::new(),
            disassembly_viewer: DisassemblyControl::new(),
            dma_viewer: DmaViewerControl::new(),
            trace_viewer: InstructionHistoryControl::new(),
            composite_adjust: CompositeAdjustControl::new(),
            scaler_adjust: ScalerAdjustControl::new(),
            ivt_viewer: IvtViewerControl::new(),
            io_stats_viewer: IoStatsViewerControl::new(),
            device_control: DeviceControl::new(),
            vhd_creator: VhdCreator::new(),
            text_mode_viewer: TextModeViewer::new(),
            call_stack_viewer: CallStackViewer::new(),

            floppy_tree_menu: FileTreeMenu::new(),
            hdd_tree_menu: FileTreeMenu::new(),
            cart_tree_menu: FileTreeMenu::new(),
            //global_zoom: 1.0,
        }
    }

    /// Allow the GUI to send events to the frontend to request initialization.
    pub fn initialize(&mut self) {
        self.event_queue.send(GuiEvent::RescanMediaFolders);
    }

    pub fn toasts(&mut self) -> &mut Toasts {
        &mut self.toasts
    }

    pub fn get_event(&mut self) -> Option<GuiEvent> {
        self.event_queue.pop()
    }

    pub fn set_option(&mut self, option: GuiBoolean, state: bool) {
        if let Some(opt) = self.option_flags.get_mut(&option) {
            *opt = state
        }
    }

    pub fn set_option_enum(&mut self, option: GuiEnum, idx: Option<GuiVariableContext>) {
        let ctx = idx.unwrap_or_default();

        if let Some(opt) = self.option_enums.get_mut(&(ctx, discriminant(&option))) {
            //log::debug!("Updating GuiEnum: {:?}", option);
            *opt = option
        }
        else {
            log::debug!("Creating GuiEnum: {:?}", option);
            self.option_enums.insert((ctx, discriminant(&option)), option);
        }
    }

    pub fn get_option(&mut self, option: GuiBoolean) -> Option<bool> {
        self.option_flags.get(&option).copied()
    }

    #[allow(dead_code)]
    pub fn get_option_enum(&self, option: GuiEnum, ctx: Option<GuiVariableContext>) -> Option<&GuiEnum> {
        let ctx = ctx.unwrap_or_default();
        self.option_enums.get(&(ctx, discriminant(&option)))
    }

    pub fn get_option_enum_mut(&mut self, option: GuiEnum, ctx: Option<GuiVariableContext>) -> Option<&mut GuiEnum> {
        let ctx = ctx.unwrap_or_default();
        self.option_enums.get_mut(&(ctx, discriminant(&option)))
    }

    pub fn get_option_mut(&mut self, option: GuiBoolean) -> &mut bool {
        self.option_flags.get_mut(&option).unwrap()
    }

    pub fn show_error(&mut self, err_str: &String) {
        self.error_dialog_open = true;
        self.error_string = err_str.clone();
    }

    pub fn clear_error(&mut self) {
        self.error_dialog_open = false;
        self.error_string = String::new();
    }

    #[allow(dead_code)]
    pub fn show_warning(&mut self, warn_str: &String) {
        self.warning_dialog_open = true;
        self.warning_string = warn_str.clone();
    }

    #[allow(dead_code)]
    pub fn clear_warning(&mut self) {
        self.warning_dialog_open = false;
        self.warning_string = String::new();
    }

    pub fn set_machine_state(&mut self, state: MachineState) {
        self.machine_state = state;
    }

    pub fn set_floppy_drives(&mut self, drive_ct: usize) {
        self.floppy_drives.clear();
        for idx in 0..drive_ct {
            self.floppy_drives.push(GuiFloppyDriveInfo {
                idx,
                selected_idx: None,
                selected_path: None,
                write_protected: true,
            });
        }
    }

    pub fn set_floppy_write_protected(&mut self, drive: usize, state: bool) {
        self.floppy_drives[drive].write_protected = state;
    }

    pub fn set_floppy_tree(&mut self, tree: PathTreeNode) {
        self.floppy_tree_menu.set_root(tree);
    }

    pub fn set_floppy_selection(&mut self, drive: usize, idx: Option<usize>, name: Option<PathBuf>) {
        self.floppy_drives[drive].selected_idx = idx;
        self.floppy_drives[drive].selected_path = name;
    }

    pub fn set_hdds(&mut self, drivect: usize) {
        self.hdds.clear();
        for idx in 0..drivect {
            self.hdds.push(GuiHddInfo {
                idx,
                selected_idx: None,
                selected_path: None,
                write_protected: true,
            });
        }
    }

    pub fn set_hdd_tree(&mut self, tree: PathTreeNode) {
        self.hdd_tree_menu.set_root(tree);
    }

    pub fn set_hdd_selection(&mut self, drive: usize, idx: Option<usize>, name: Option<PathBuf>) {
        self.hdds[drive].selected_idx = idx;
        self.hdds[drive].selected_path = name;
    }

    pub fn set_cart_slots(&mut self, slotct: usize) {
        self.carts.clear();
        for idx in 0..slotct {
            self.carts.push(GuiCartInfo {
                idx,
                selected_idx: None,
                selected_path: None,
            });
        }
    }

    pub fn set_cart_selection(&mut self, slot: usize, idx: Option<usize>, name: Option<PathBuf>) {
        self.carts[slot].selected_idx = idx;
        self.carts[slot].selected_path = name;
    }

    pub fn set_cart_tree(&mut self, tree: PathTreeNode) {
        self.cart_tree_menu.set_root(tree);
    }

    /// Set display apertures for the specified display. Should be called in a loop for each display
    /// target.
    pub fn set_display_apertures(&mut self, display: usize, apertures: Vec<DisplayApertureDesc>) {
        self.display_apertures.insert(display, apertures);
    }

    /// Set list of available scaler modes
    pub fn set_scaler_modes(&mut self, modes: Vec<ScalerMode>) {
        self.scaler_modes = modes;
    }

    /// Provide the list of graphics cards to all windows that need them.
    /// TODO: We can create this from update_display_info, no need for a separate method..
    pub fn set_card_list(&mut self, cards: Vec<String>) {
        self.text_mode_viewer.set_cards(cards.clone());
    }

    pub fn set_scaler_presets(&mut self, presets: &Vec<ScalerPreset>) {
        self.scaler_presets = presets.iter().map(|p| p.name.clone()).collect();
        log::debug!("installed scaler presets: {:?}", self.scaler_presets);
    }

    pub fn show_window(&mut self, window: GuiWindow) {
        *self.window_open_flags.get_mut(&window).unwrap() = true;
    }

    pub fn get_breakpoints(&mut self) -> BreakpointSet {
        self.cpu_control.get_breakpoints()
    }

    pub fn update_pit_state(&mut self, state: &PitDisplayState) {
        self.pit_viewer.update_state(state);
    }

    pub fn set_serial_ports(&mut self, ports: Vec<SerialPortDescriptor>) {
        self.serial_ports = ports;
    }

    pub fn set_host_serial_ports(&mut self, ports: Vec<SerialPortInfo>) {
        self.host_serial_ports = ports;
    }

    pub fn update_videocard_state(&mut self, state: HashMap<String, Vec<(String, VideoCardStateEntry)>>) {
        self.videocard_state = state;
    }

    /// Initialize GUI Display enum state given a vector of DisplayInfo fields.  
    pub fn init_display_info(&mut self, vci: Vec<DisplayInfo>) {
        self.display_info = vci.clone();

        // Build a vector of enums to set to avoid borrowing twice.
        let mut enum_vec = Vec::new();

        // Create a list of display target strings to give to the composite and scaler adjustment windows.
        let mut dt_descs = Vec::new();
        for (idx, display) in self.display_info.iter().enumerate() {
            let mut dt_str = format!("Display {}", idx);
            if let Some(vid) = display.vid {
                dt_str.push_str(&format!(" Card: {} [{:?}]", vid.idx, vid.vtype));
            }
            dt_descs.push(dt_str);
        }
        self.scaler_adjust.set_dt_list(dt_descs.clone());
        self.composite_adjust.set_dt_list(dt_descs.clone());

        for (idx, display) in self.display_info.iter().enumerate() {
            if let Some(renderer) = &display.renderer {
                enum_vec.push((
                    GuiEnum::DisplayAspectCorrect(renderer.aspect_correction),
                    Some(GuiVariableContext::Display(idx)),
                ));
                enum_vec.push((
                    // Fairly certain that if we have a renderer, we have an aperture...
                    GuiEnum::DisplayAperture(renderer.display_aperture.unwrap()),
                    Some(GuiVariableContext::Display(idx)),
                ));
                enum_vec.push((
                    GuiEnum::DisplayComposite(renderer.composite),
                    Some(GuiVariableContext::Display(idx)),
                ));
            }

            // Create GuiEnums for each display scaler mode.
            if let Some(scaler_mode) = &display.scaler_mode {
                enum_vec.push((
                    GuiEnum::DisplayScalerMode(*scaler_mode),
                    Some(GuiVariableContext::Display(idx)),
                ));
            }

            // Set the initial scaler params for the Scaler Adjustments window if we have them.
            if let Some(scaler_params) = &display.scaler_params {
                self.scaler_adjust.set_params(idx, scaler_params.clone());
            }
        }

        // Set all enums.
        for enum_item in enum_vec.iter() {
            self.set_option_enum(enum_item.0.clone(), enum_item.1);
        }
    }

    #[allow(dead_code)]
    pub fn update_videomem_state(&mut self, mem: Vec<u8>, w: u32, h: u32) {
        self.video_mem = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &mem);
    }
}
