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

    marty_egui::state.rs

    EGUI State management
*/

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    mem::discriminant,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

use crate::{
    modal::ModalState,
    widgets::file_tree_menu::FileTreeMenu,
    windows::{
        about::AboutDialog,
        call_stack_viewer::CallStackViewer,
        composite_adjust::CompositeAdjustControl,
        cpu_control::{BreakpointSet, CpuControl},
        cpu_state_viewer::CpuViewerControl,
        cycle_trace_viewer::CycleTraceViewerControl,
        data_visualizer::DataVisualizerControl,
        delay_adjust::DelayAdjustControl,
        device_control::DeviceControl,
        disassembly_viewer::DisassemblyControl,
        dma_viewer::DmaViewerControl,
        fdc_viewer::FdcViewerControl,
        floppy_viewer::FloppyViewerControl,
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
        sn_viewer::SnViewerControl,
        text_mode_viewer::TextModeViewer,
        vhd_creator::VhdCreator,
    },
    DialogProvider,
    GuiBoolean,
    GuiEnum,
    GuiEnumMap,
    GuiEvent,
    GuiEventQueue,
    GuiFloat,
    GuiVariable,
    GuiVariableContext,
    GuiWindow,
    MediaTrayState,
    PerformanceStats,
};

#[cfg(feature = "markdown")]
use crate::windows::info_viewer::InfoViewer;

use marty_core::{
    device_traits::videocard::{DisplayApertureDesc, VideoCardState, VideoCardStateEntry},
    devices::{pit::PitDisplayState, serial::SerialPortDescriptor},
    machine::{ExecutionControl, MachineState},
    machine_types::FloppyDriveType,
};
use marty_display_common::{
    display_manager::{DisplayTargetInfo, DtHandle},
    display_scaler::{ScalerMode, ScalerPreset},
};
use marty_frontend_common::{
    resource_manager::PathTreeNode,
    thread_events::FrontendThreadEvent,
    types::sound::SoundSourceInfo,
    RelativeDirectory,
};

use egui::ColorImage;
use egui_notify::{Anchor, Toasts};
use fluxfox::{DiskImage, DiskImageFileFormat, StandardFormat};
use marty_common::types::{joystick::ControllerLayout, ui::MouseCaptureMode};
use marty_frontend_common::types::gamepad::{GamepadId, GamepadInfo};
use serde::{Deserialize, Serialize};
#[cfg(feature = "use_serialport")]
use serialport::SerialPortInfo;
use strum::IntoEnumIterator;

pub enum FloppyDriveSelection {
    None,
    NewImage(StandardFormat),
    Image(PathBuf),
    ZipArchive(PathBuf),
    Directory(PathBuf),
}

pub struct GuiFloppyDriveInfo {
    pub(crate) idx: usize,
    pub(crate) selection_new: Option<StandardFormat>,
    pub(crate) selected_idx: Option<usize>,
    pub(crate) selected_path: FloppyDriveSelection,
    pub(crate) write_protected: bool,
    pub(crate) read_only: bool,
    pub(crate) drive_type: FloppyDriveType,
    pub(crate) supported_formats: Vec<(DiskImageFileFormat, Vec<String>)>,
    pub(crate) source_format: Option<DiskImageFileFormat>,
    pub(crate) source_writeback: bool,
    write_ct: u64,
}

impl GuiFloppyDriveInfo {
    pub fn filename(&self) -> Option<String> {
        match &self.selected_path {
            FloppyDriveSelection::NewImage(_) => None,
            FloppyDriveSelection::Image(path) => Some(path.file_name()?.to_string_lossy().to_string()),
            FloppyDriveSelection::Directory(path) => Some(path.file_name()?.to_string_lossy().to_string()),
            FloppyDriveSelection::ZipArchive(path) => Some(path.to_string_lossy().to_string()),
            FloppyDriveSelection::None => None,
        }
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        match &self.selected_path {
            FloppyDriveSelection::NewImage(_) => None,
            FloppyDriveSelection::Image(path) => Some(path),
            FloppyDriveSelection::Directory(path) => Some(path),
            FloppyDriveSelection::ZipArchive(path) => Some(path),
            FloppyDriveSelection::None => None,
        }
    }

    pub fn type_string(&self) -> String {
        match &self.selected_path {
            FloppyDriveSelection::NewImage(_) => "New Image: ".to_string(),
            FloppyDriveSelection::Image(_) => "Image: ".to_string(),
            FloppyDriveSelection::Directory(_) => "Directory: ".to_string(),
            FloppyDriveSelection::ZipArchive(_) => "Zip Archive: ".to_string(),
            FloppyDriveSelection::None => "".to_string(),
        }
    }

    pub fn is_new(&self) -> Option<StandardFormat> {
        match &self.selected_path {
            FloppyDriveSelection::NewImage(sf) => Some(*sf),
            _ => None,
        }
    }

    pub fn is_writeable(&self) -> bool {
        !self.read_only & self.source_writeback
    }

    pub fn write_protect(&mut self, state: bool) {
        self.write_protected = state;
    }

    pub fn read_only(&mut self, state: bool) {
        self.read_only = state;
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

pub struct GuiAutofloppyPath {
    pub(crate) full_path: PathBuf,
    pub(crate) name: OsString,
    pub(crate) mounted: bool,
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
    pub(crate) thread_sender: crossbeam_channel::Sender<FrontendThreadEvent<Arc<DiskImage>>>,
    pub(crate) dialog_provider: DialogProvider,

    pub(crate) toasts: Toasts,
    media_tray: MediaTrayState,

    pub(crate) default_floppy_path: Option<PathBuf>,

    /// Only show the associated window when true.
    pub(crate) window_open_flags: HashMap<GuiWindow, bool>,
    pub(crate) window_state: BTreeMap<GuiWindow, WorkspaceWindowState>,
    pub(crate) error_dialog_open: bool,
    pub(crate) warning_dialog_open: bool,

    pub(crate) option_flags:  HashMap<GuiBoolean, bool>,
    pub(crate) option_floats: HashMap<GuiFloat, f32>,
    pub(crate) option_enums:  GuiEnumMap,

    pub(crate) machine_state: MachineState,

    video_mem: ColorImage,
    pub(crate) perf_stats: PerformanceStats,

    // Audio stuff
    pub(crate) sound_sources: Vec<SoundSourceInfo>,

    // Display stuff
    pub(crate) display_apertures: HashMap<usize, Vec<DisplayApertureDesc>>,
    pub(crate) scaler_modes: Vec<ScalerMode>,
    pub(crate) scaler_presets: Vec<String>,

    // Media Images
    pub(crate) floppy_drives: Vec<GuiFloppyDriveInfo>,
    pub(crate) hdds: Vec<GuiHddInfo>,
    pub(crate) carts: Vec<GuiCartInfo>,
    pub(crate) autofloppy_paths: Vec<GuiAutofloppyPath>,

    // VHD Images
    pub(crate) vhd_names: Vec<OsString>,

    // Gamepads
    pub(crate) gamepads: Vec<GamepadInfo>,
    pub(crate) gameport: bool,
    pub(crate) controller_layout: ControllerLayout,
    pub(crate) selected_gamepad: [Option<GamepadId>; 2],

    // Serial ports
    pub(crate) serial_ports: Vec<SerialPortDescriptor>,
    #[cfg(feature = "use_serialport")]
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
    pub data_visualizer: DataVisualizerControl,

    pub perf_viewer:  PerformanceViewerControl,
    pub delay_adjust: DelayAdjustControl,

    pub pit_viewer:    PitViewerControl,
    pub serial_viewer: SerialViewerControl,
    pub pic_viewer:    PicViewerControl,
    pub ppi_viewer:    PpiViewerControl,

    pub videocard_state: VideoCardState,
    pub display_info:    Vec<DisplayTargetInfo>,

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
    pub fdc_viewer: FdcViewerControl,
    pub floppy_viewer: FloppyViewerControl,
    pub call_stack_viewer: CallStackViewer,
    #[cfg(feature = "markdown")]
    pub info_viewer: InfoViewer,
    pub sn_viewer: SnViewerControl,

    pub floppy_tree_menu: FileTreeMenu,
    pub hdd_tree_menu:    FileTreeMenu,
    pub cart_tree_menu:   FileTreeMenu,

    //pub(crate) global_zoom: f32,
    pub modal: ModalState,
}

impl GuiState {
    /// Create a struct representing the state of the GUI.
    pub fn new(
        exec_control: Rc<RefCell<ExecutionControl>>,
        thread_sender: crossbeam_channel::Sender<FrontendThreadEvent<Arc<DiskImage>>>,
    ) -> Self {
        // Set default values for window open flags

        let mut window_open_flags = HashMap::new();
        for window in GuiWindow::iter() {
            window_open_flags.insert(window, false);
        }

        let mut window_state = BTreeMap::new();
        for window in GuiWindow::iter() {
            window_state.insert(window, WorkspaceWindowState::default());
        }

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

        let option_floats: HashMap<GuiFloat, f32> =
            [(GuiFloat::EmulationSpeed, 1.0f32), (GuiFloat::MouseSpeed, 0.5f32)].into();

        let mut option_enums = HashMap::new();

        let capture_option = GuiEnum::MouseCaptureMode(MouseCaptureMode::Mouse);
        option_enums.insert(
            (GuiVariableContext::default(), discriminant(&capture_option)),
            capture_option,
        );

        let gamepad_mapping = GuiEnum::GamepadMapping((None, None));
        option_enums.insert(
            (GuiVariableContext::default(), discriminant(&gamepad_mapping)),
            gamepad_mapping,
        );

        Self {
            event_queue: GuiEventQueue::new(),
            thread_sender,
            dialog_provider: DialogProvider::default(),
            toasts: Toasts::new().with_anchor(Anchor::BottomRight),
            media_tray: Default::default(),

            default_floppy_path: None,

            window_open_flags,
            window_state,
            error_dialog_open: false,
            warning_dialog_open: false,

            option_flags,
            option_floats,
            option_enums,

            machine_state: MachineState::Off,
            video_mem: ColorImage::new([320, 200], egui::Color32::BLACK),

            perf_stats: Default::default(),

            sound_sources: Vec::new(),

            display_apertures: Default::default(),
            scaler_modes: Vec::new(),
            scaler_presets: Vec::new(),

            floppy_drives: Vec::new(),
            hdds: Vec::new(),
            carts: Vec::new(),
            vhd_names: Vec::new(),
            autofloppy_paths: Vec::new(),

            gamepads: Vec::new(),
            gameport: false,
            controller_layout: Default::default(),
            selected_gamepad: [None, None],

            serial_ports: Vec::new(),
            #[cfg(feature = "use_serialport")]
            host_serial_ports: Vec::new(),
            serial_port_name: String::new(),

            exec_control: exec_control.clone(),

            error_string: String::new(),
            warning_string: String::new(),

            about_dialog: AboutDialog::new(),
            cpu_control: CpuControl::new(exec_control.clone()),
            cpu_viewer: CpuViewerControl::new(exec_control.clone()),
            cycle_trace_viewer: CycleTraceViewerControl::new(),
            memory_viewer: MemoryViewerControl::new(),
            data_visualizer: DataVisualizerControl::new(),

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
            fdc_viewer: FdcViewerControl::new(),
            floppy_viewer: FloppyViewerControl::new(),
            call_stack_viewer: CallStackViewer::new(),
            #[cfg(feature = "markdown")]
            info_viewer: InfoViewer::new(),
            sn_viewer: SnViewerControl::new(),

            floppy_tree_menu: FileTreeMenu::new().with_file_icon("💾"),
            hdd_tree_menu: FileTreeMenu::new().with_file_icon("🖴"),
            cart_tree_menu: FileTreeMenu::new(),
            //global_zoom: 1.0,
            modal: ModalState::new(),
        }
    }

    /// Allow the GUI to send events to the frontend to request initialization.
    pub fn initialize(&mut self) {
        self.event_queue.send(GuiEvent::RescanMediaFolders);
    }

    pub fn set_paths(&mut self, default_floppy_path: PathBuf) {
        self.default_floppy_path = Some(default_floppy_path.clone());
        self.modal.set_paths(default_floppy_path.clone());
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

    pub fn set_dump_path(&mut self, path: PathBuf) {
        self.data_visualizer.set_dump_path(path);
    }

    pub fn set_machine_state(&mut self, state: MachineState) {
        self.machine_state = state;
    }

    pub fn set_floppy_drives(&mut self, drives: Vec<FloppyDriveType>) {
        self.floppy_drives.clear();

        for (idx, drive_type) in drives.iter().enumerate() {
            self.floppy_drives.push(GuiFloppyDriveInfo {
                idx,
                selection_new: None,
                selected_idx: None,
                selected_path: FloppyDriveSelection::None,
                write_protected: true,
                read_only: false,
                drive_type: *drive_type,
                supported_formats: Vec::new(),
                source_format: None,
                source_writeback: false,
                write_ct: 0,
            });
        }
    }

    pub fn set_floppy_write_protected(&mut self, drive: usize, state: bool) {
        self.floppy_drives[drive].write_protect(state);
    }

    pub fn set_floppy_tree(&mut self, tree: PathTreeNode) {
        self.floppy_tree_menu.set_root(tree);
    }

    pub fn set_autofloppy_paths(&mut self, paths: Vec<RelativeDirectory>) {
        let paths = paths
            .iter()
            .map(|rd| GuiAutofloppyPath {
                full_path: rd.full.clone(),
                name: rd.name.clone(),
                mounted: false,
            })
            .collect();
        self.autofloppy_paths = paths;
    }

    pub fn set_floppy_selection(
        &mut self,
        drive: usize,
        idx: Option<usize>,
        name: FloppyDriveSelection,
        source_format: Option<DiskImageFileFormat>,
        supported_formats: Vec<(DiskImageFileFormat, Vec<String>)>,
        read_only: Option<bool>,
    ) {
        self.floppy_drives[drive].selected_idx = idx;

        if matches!(name, FloppyDriveSelection::None) {
            // Disk has been ejected - update viewer
            self.floppy_viewer.clear_visualization(drive);
        }
        self.floppy_drives[drive].selected_path = name;

        if let Some(read_only) = read_only {
            self.floppy_drives[drive].read_only = read_only;
        }

        let fmts_alone = supported_formats.iter().map(|(fmt, _)| *fmt).collect::<Vec<_>>();

        log::warn!(
            "Source format: {:?} Supported formats: {:?}",
            source_format,
            supported_formats
        );
        if let Some(source_format) = source_format {
            self.floppy_drives[drive].source_writeback = fmts_alone.contains(&source_format);
            self.floppy_drives[drive].source_format = Some(source_format);
        }
        else {
            self.floppy_drives[drive].source_writeback = false;
            self.floppy_drives[drive].source_format = None;
        }
        self.floppy_drives[drive].supported_formats = supported_formats;
        self.floppy_viewer.reset();
    }

    pub fn set_floppy_supported_formats(
        &mut self,
        drive: usize,
        supported_formats: Vec<(DiskImageFileFormat, Vec<String>)>,
    ) {
        self.floppy_drives[drive].supported_formats = supported_formats;
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
        log::debug!("set_scaler_modes(): Installed {} scaler modes", modes.len());
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

    /// Update the GUI state with a list of gamepads, represented by GamepadInfo.
    pub fn set_gamepads(&mut self, gamepads: Vec<GamepadInfo>) {
        self.gamepads = gamepads;

        for mapping in &mut self.selected_gamepad {
            if let Some(gamepad) = mapping {
                if self.gamepads.iter().find(|g| g.internal_id == *gamepad).is_none() {
                    // Gamepad is no longer available, so clear the mapping.
                    log::debug!("Gamepad id {} is no longer valid. Clearing mapping.", gamepad);
                    *mapping = None;
                }
            }
        }
    }

    /// Specify the presence of a gameport. This will enable the gameport submenu under Input.
    #[inline]
    pub fn set_gameport(&mut self, state: bool, layout: ControllerLayout) {
        self.gameport = state;
    }

    pub fn set_serial_ports(&mut self, ports: Vec<SerialPortDescriptor>) {
        self.serial_ports = ports;
    }

    #[cfg(feature = "use_serialport")]
    pub fn set_host_serial_ports(&mut self, ports: Vec<SerialPortInfo>) {
        self.host_serial_ports = ports;
    }

    pub fn update_videocard_state(&mut self, state: HashMap<String, Vec<(String, VideoCardStateEntry)>>) {
        self.videocard_state = state;
    }

    pub fn set_sound_state(&mut self, info: Vec<SoundSourceInfo>) {
        for (snd_idx, source) in info.iter().enumerate() {
            let sctx = GuiVariableContext::SoundSource(snd_idx);
            if let Some(GuiEnum::AudioMuted(state)) =
                self.get_option_enum_mut(GuiEnum::AudioMuted(Default::default()), Some(sctx))
            {
                *state = source.muted;
            }

            if let Some(GuiEnum::AudioVolume(vol)) =
                self.get_option_enum_mut(GuiEnum::AudioVolume(Default::default()), Some(sctx))
            {
                *vol = source.volume;
            }
        }
        self.sound_sources = info;
    }

    /// Initialize the Sound enum state given a vector of SoundSourceInfo fields.
    pub fn init_sound_info(&mut self, info: Vec<SoundSourceInfo>) {
        self.sound_sources = info;

        // Build a vector of enums to set to avoid borrowing twice.
        let mut enum_vec = Vec::new();

        for (idx, sound_source) in self.sound_sources.iter().enumerate() {
            enum_vec.push((
                GuiEnum::AudioMuted(sound_source.muted),
                Some(GuiVariableContext::SoundSource(idx)),
            ));

            enum_vec.push((
                GuiEnum::AudioVolume(sound_source.volume),
                Some(GuiVariableContext::SoundSource(idx)),
            ));
        }

        // Set all enums.
        for enum_item in enum_vec.iter() {
            self.set_option_enum(enum_item.0.clone(), enum_item.1);
        }
    }

    pub fn has_sn76489(&self) -> bool {
        self.sound_sources.iter().any(|source| source.name.contains("SN76489"))
    }

    /// Initialize GUI Display enum state given a vector of DisplayInfo fields.  
    pub fn init_display_info(&mut self, vci: Vec<DisplayTargetInfo>) {
        self.display_info = vci;

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
            log::debug!("init_display_info(): Initializing Display {:?}", display.handle);

            if let Some(renderer) = &display.renderer {
                log::debug!("init_display_info(): Initializing Renderer...");
                enum_vec.push((
                    GuiEnum::DisplayAspectCorrect(renderer.aspect_correction),
                    Some(GuiVariableContext::Display(display.handle)),
                ));
                enum_vec.push((
                    // Fairly certain that if we have a renderer, we have an aperture...
                    GuiEnum::DisplayAperture(renderer.display_aperture.unwrap()),
                    Some(GuiVariableContext::Display(display.handle)),
                ));
                enum_vec.push((
                    GuiEnum::DisplayComposite(renderer.composite),
                    Some(GuiVariableContext::Display(display.handle)),
                ));
            }

            // Create GuiEnums for each display scaler mode.
            if let Some(scaler_mode) = &display.scaler_mode {
                log::debug!("init_display_info(): Creating scaler mode enum {:?}", scaler_mode);
                enum_vec.push((
                    GuiEnum::DisplayScalerMode(*scaler_mode),
                    Some(GuiVariableContext::Display(display.handle)),
                ));
            }

            // Create GuiEnum for Display Type (windowed or background)
            enum_vec.push((
                GuiEnum::DisplayType(display.dtype),
                Some(GuiVariableContext::Display(display.handle)),
            ));

            // Create GuiEnum for Bezel status (true if bezel is enabled)
            enum_vec.push((
                GuiEnum::WindowBezel(false),
                Some(GuiVariableContext::Display(display.handle)),
            ));

            // Set the initial scaler params for the Scaler Adjustments window if we have them.
            if let Some(scaler_params) = &display.scaler_params {
                self.scaler_adjust.set_params(DtHandle(idx), scaler_params.clone());
            }
        }

        // Set all enums.
        for enum_item in enum_vec.iter() {
            self.set_option_enum(enum_item.0.clone(), enum_item.1);
        }
    }

    // This is a hack interface - figure out where to better expose this state
    pub fn primary_video_has_bezel(&mut self) -> bool {
        let vctx = GuiVariableContext::Display(DtHandle::MAIN);
        if !self.display_info.is_empty() {
            if let Some(enum_mut) = self.get_option_enum_mut(GuiEnum::WindowBezel(Default::default()), Some(vctx)) {
                let mut checked = *enum_mut == GuiEnum::WindowBezel(true);

                return checked;
            }
        }
        false
    }

    #[allow(dead_code)]
    pub fn update_videomem_state(&mut self, mem: Vec<u8>, w: u32, h: u32) {
        self.video_mem = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &mem);
    }
}
