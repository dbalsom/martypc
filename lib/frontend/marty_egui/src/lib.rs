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
    time::{Duration, Instant},
};

use egui::{ClippedPrimitive, Color32, ColorImage, Context, TexturesDelta, ViewportId, Visuals};

//use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use frontend_common::{
    display_manager::{DisplayInfo, DisplayManagerGuiOptions},
    display_scaler::{ScalerMode, ScalerParams},
};

use egui_extras::install_image_loaders;
use egui_notify::{Anchor, Toasts};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};

use pixels::{wgpu, PixelsContext};
use winit::window::Window;

use frontend_common::display_scaler::ScalerPreset;
use regex::Regex;
use serialport::SerialPortInfo;

mod color;
mod constants;
mod image;

mod layouts;
mod menu;
mod theme;
mod token_listview;
mod ui;
mod widgets;
mod windows;

use crate::{
    theme::GuiTheme,
    // Use custom windows
    windows::about::AboutDialog,
    windows::composite_adjust::CompositeAdjustControl,
    windows::cpu_control::CpuControl,
    windows::cpu_state_viewer::CpuViewerControl,
    windows::cycle_trace_viewer::CycleTraceViewerControl,
    windows::delay_adjust::DelayAdjustControl,
    windows::device_control::DeviceControl,
    windows::disassembly_viewer::DisassemblyControl,
    windows::dma_viewer::DmaViewerControl,
    windows::instruction_history_viewer::InstructionHistoryControl,
    windows::ivr_viewer::IvrViewerControl,
    windows::memory_viewer::MemoryViewerControl,
    windows::performance_viewer::PerformanceViewerControl,
    windows::pic_viewer::PicViewerControl,
    windows::pit_viewer::PitViewerControl,
    windows::scaler_adjust::ScalerAdjustControl,
    windows::vhd_creator::VhdCreator,
};

use marty_core::{
    devices::{
        implementations::{hdc::HardDiskFormat, pic::PicStringState, pit::PitDisplayState, ppi::PpiStringState},
        traits::videocard::{DisplayApertureDesc, DisplayApertureType, VideoCardState, VideoCardStateEntry},
    },
    machine::{ExecutionControl, MachineState},
};

use crate::windows::text_mode_viewer::TextModeViewer;
use videocard_renderer::{CompositeParams};

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
    LoadFloppy(usize, OsString),
    SaveFloppy(usize, OsString),
    EjectFloppy(usize),
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

/// Manages all state required for rendering egui over `Pixels`.
pub struct GuiRenderContext {
    // State for egui.
    egui_ctx: Context,
    #[cfg(not(target_arch = "wasm32"))]
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,
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

pub struct GuiState {
    event_queue: GuiEventQueue,

    toasts: Toasts,
    media_tray: MediaTrayState,

    /// Only show the associated window when true.
    window_open_flags:   HashMap<GuiWindow, bool>,
    error_dialog_open:   bool,
    warning_dialog_open: bool,

    option_flags: HashMap<GuiBoolean, bool>,
    option_enums: GuiEnumMap,

    machine_state: MachineState,

    video_mem:  ColorImage,
    perf_stats: PerformanceStats,

    // Display stuff
    display_apertures: HashMap<usize, Vec<DisplayApertureDesc>>,
    scaler_modes: Vec<ScalerMode>,
    scaler_presets: Vec<String>,

    // Floppy Disk Images
    floppy_names: Vec<OsString>,
    floppy0_name: Option<OsString>,
    floppy1_name: Option<OsString>,

    // VHD Images
    vhd_names: Vec<OsString>,
    new_vhd_name0: Option<OsString>,
    vhd_name0: OsString,
    new_vhd_name1: Option<OsString>,
    vhd_name1: OsString,

    // Serial ports
    serial_ports: Vec<SerialPortInfo>,
    serial_port_name: String,

    exec_control: Rc<RefCell<ExecutionControl>>,

    error_string:   String,
    warning_string: String,

    pub about_dialog: AboutDialog,
    pub cpu_control: CpuControl,
    pub cpu_viewer: CpuViewerControl,
    pub cycle_trace_viewer: CycleTraceViewerControl,
    pub memory_viewer: MemoryViewerControl,

    pub perf_viewer:  PerformanceViewerControl,
    pub delay_adjust: DelayAdjustControl,

    pub pit_viewer: PitViewerControl,
    pub pic_viewer: PicViewerControl,
    pub ppi_state:  PpiStringState,

    pub videocard_state: VideoCardState,
    pub display_info:    Vec<DisplayInfo>,

    pub disassembly_viewer: DisassemblyControl,
    pub dma_viewer: DmaViewerControl,
    pub trace_viewer: InstructionHistoryControl,
    pub composite_adjust: CompositeAdjustControl,
    pub scaler_adjust: ScalerAdjustControl,
    pub ivr_viewer: IvrViewerControl,
    pub device_control: DeviceControl,
    pub vhd_creator: VhdCreator,
    pub text_mode_viewer: TextModeViewer,

    call_stack_string: String,
    global_zoom: f32,
}

impl GuiRenderContext {
    /// Create egui.
    pub fn new(
        dt_idx: usize,
        width: u32,
        height: u32,
        scale_factor: f64,
        pixels: &pixels::Pixels,
        window: &Window,
        gui_options: &DisplayManagerGuiOptions,
    ) -> Self {
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;
        let egui_ctx = Context::default();

        log::debug!(
            "GuiRenderContext::new(): {}x{} (scale_factor: {} native_scale_factor: {})",
            width,
            height,
            scale_factor,
            egui_ctx.native_pixels_per_point().unwrap_or(1.0)
        );

        // Load image loaders so we can use images in ui (0.24)
        install_image_loaders(&egui_ctx);

        let _id_string = format!("display{}", dt_idx);

        #[cfg(not(target_arch = "wasm32"))]
        let mut egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            //egui::ViewportId::from_hash_of(id_string.as_str()),
            ViewportId::ROOT,
            //&event_loop,
            window as &dyn pixels::raw_window_handle::HasRawDisplayHandle,
            Some(scale_factor as f32),
            None,
        );
        #[cfg(not(target_arch = "wasm32"))]
        {
            egui_ctx.set_zoom_factor(gui_options.zoom.min(1.0).max(0.1));
            // DO NOT SET THIS. Let State::new() handle it.
            //egui_ctx.set_pixels_per_point(scale_factor as f32);
            egui_state.set_max_texture_side(max_texture_size);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels:   [width, height],
            pixels_per_point: scale_factor as f32,
        };

        let renderer = Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        let textures = TexturesDelta::default();

        let visuals = match gui_options.theme_dark {
            true => Visuals::dark(),
            false => Visuals::light(),
        };

        // Make header smaller.
        use egui::{FontFamily::Proportional, FontId, TextStyle::*};
        let mut style = (*egui_ctx.style()).clone();

        style.text_styles.entry(Heading).and_modify(|text_style| {
            *text_style = FontId::new(14.0, Proportional);
        });

        egui_ctx.set_style(style);

        if let Some(color) = gui_options.theme_color {
            let theme = GuiTheme::new(&visuals, crate::color::hex_to_c32(color));
            egui_ctx.set_visuals(theme.visuals().clone());
        }
        else {
            egui_ctx.set_visuals(visuals);
        }

        #[cfg(debug_assertions)]
        if gui_options.debug_drawing {
            egui_ctx.set_debug_on_hover(true);
        }

        let slf = Self {
            egui_ctx,
            #[cfg(not(target_arch = "wasm32"))]
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,
        };

        //slf.resize(width, height);
        slf
    }

    pub fn set_zoom_factor(&mut self, zoom: f32) {
        self.egui_ctx.set_zoom_factor(zoom);
    }

    pub fn has_focus(&self) -> bool {
        match self.egui_ctx.memory(|m| m.focus()) {
            Some(_) => true,
            None => false,
        }
    }

    /// Handle input events from the window manager.
    pub fn handle_event(&mut self, window: &Window, event: &winit::event::WindowEvent) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            //log::debug!("Handling event: {:?}", event);

            let _ = self.egui_state.on_window_event(window, event);
        }
    }

    /// Resize egui.
    pub fn resize(&mut self, window: &Window, w: u32, h: u32) {
        if w > 0 && h > 0 {
            //let scale_factor = self.egui_ctx.pixels_per_point();
            let scale_factor = egui_winit::pixels_per_point(&self.egui_ctx, window);
            //let w = (w as f32 * scale_factor as f32).floor() as u32;
            //let h = (h as f32 * scale_factor as f32).floor() as u32;

            log::debug!("GuiRenderContext::resize: {}x{} (scale_factor: {})", w, h, scale_factor);
            self.screen_descriptor = ScreenDescriptor {
                size_in_pixels:   [w, h],
                pixels_per_point: scale_factor as f32,
            };

            //self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    /// Update scaling factor.
    pub fn scale_factor(&mut self, scale_factor: f64) {
        log::debug!("Setting scale factor: {}", scale_factor);
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    pub fn viewport_mut(&mut self) -> &mut egui::ViewportInfo {
        /* Eventually this should get the viewport created by State::new(), but for the moment
           that is just the root viewport.
        let vpi = self.egui_state.get_viewport_id();
        self.egui_state
            .egui_input_mut()
            .viewports
            .get_mut(&vpi)
            .expect(&format!("Failed to get viewport: {:?}", &vpi))
         */

        self.egui_state
            .egui_input_mut()
            .viewports
            .get_mut(&ViewportId::ROOT)
            .expect(&format!("Failed to get ROOT viewport!"))
    }

    /// Prepare egui.
    pub fn prepare(&mut self, window: &Window, state: &mut GuiState) {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let ctx = self.egui_ctx.clone();
            let vpi = self.viewport_mut();
            egui_winit::update_viewport_info(vpi, &ctx, window);
            let raw_input = self.egui_state.take_egui_input(window);
            let gui_start = Instant::now();

            let mut ran = false;
            let output = self.egui_ctx.run(raw_input, |egui_ctx| {
                // Draw the application.
                state.ui(egui_ctx);
                ran = true;
            });

            if ran {
                self.textures.append(output.textures_delta);
                self.egui_state.handle_platform_output(window, output.platform_output);

                //let ppp = output.pixels_per_point;
                let ppp = egui_winit::pixels_per_point(&ctx, window);
                //log::debug!("Tesselate with ppp: {}", ppp);
                self.paint_jobs = self.egui_ctx.tessellate(output.shapes, ppp);
            }
            state.perf_stats.gui_time = Instant::now() - gui_start;
        }
    }

    /// Render egui.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext,
    ) {
        // Upload all resources to the GPU.
        for (id, image_delta) in &self.textures.set {
            self.renderer
                .update_texture(&context.device, &context.queue, *id, image_delta);
        }

        self.renderer.update_buffers(
            &context.device,
            &context.queue,
            encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        // Render egui with WGPU
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.renderer
                .render(&mut rpass, &self.paint_jobs, &self.screen_descriptor);
        }

        // Cleanup
        let textures = std::mem::take(&mut self.textures);
        for id in &textures.free {
            self.renderer.free_texture(id);
        }
    }
}

impl GuiState {
    /// Create a struct representing the state of the GUI.
    pub fn new(exec_control: Rc<RefCell<ExecutionControl>>) -> Self {
        // Set default values for window open flags
        let window_open_flags: HashMap<GuiWindow, bool> = [
            (GuiWindow::About, false),
            (GuiWindow::CpuControl, false),
            (GuiWindow::PerfViewer, false),
            (GuiWindow::MemoryViewer, false),
            (GuiWindow::CompositeAdjust, false),
            (GuiWindow::ScalerAdjust, false),
            (GuiWindow::CpuStateViewer, false),
            (GuiWindow::HistoryViewer, false),
            (GuiWindow::IvrViewer, false),
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
        .into();

        let option_flags: HashMap<GuiBoolean, bool> = [
            //(GuiBoolean::CompositeDisplay, false),
            //(GuiBoolean::CorrectAspect, false),
            (GuiBoolean::CpuEnableWaitStates, true),
            (GuiBoolean::CpuInstructionHistory, false),
            (GuiBoolean::CpuTraceLoggingEnabled, false),
            (GuiBoolean::TurboButton, false),
            //(GuiBoolean::ShowBackBuffer, true),
            //(GuiBoolean::EnableSnow, true),
        ]
        .into();

        let option_enums = HashMap::new();

        Self {
            event_queue: GuiEventQueue::new(),
            toasts: Toasts::new().with_anchor(Anchor::BottomRight),
            media_tray: Default::default(),

            window_open_flags,
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

            floppy_names: Vec::new(),
            floppy0_name: Option::None,
            floppy1_name: Option::None,

            vhd_names: Vec::new(),
            new_vhd_name0: Option::None,
            vhd_name0: OsString::new(),
            new_vhd_name1: Option::None,
            vhd_name1: OsString::new(),

            serial_ports: Vec::new(),
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
            pic_viewer: PicViewerControl::new(),
            ppi_state: Default::default(),

            videocard_state: Default::default(),
            display_info: Vec::new(),
            disassembly_viewer: DisassemblyControl::new(),
            dma_viewer: DmaViewerControl::new(),
            trace_viewer: InstructionHistoryControl::new(),
            composite_adjust: CompositeAdjustControl::new(),
            scaler_adjust: ScalerAdjustControl::new(),
            ivr_viewer: IvrViewerControl::new(),
            device_control: DeviceControl::new(),
            vhd_creator: VhdCreator::new(),
            text_mode_viewer: TextModeViewer::new(),
            call_stack_string: String::new(),

            global_zoom: 1.0,
        }
    }

    pub fn toasts(&mut self) -> &mut Toasts {
        &mut self.toasts
    }

    pub fn get_event(&mut self) -> Option<GuiEvent> {
        self.event_queue.pop()
    }

    pub fn window_flag(&mut self, window: GuiWindow) -> &mut bool {
        self.window_open_flags.get_mut(&window).unwrap()
    }

    pub fn is_window_open(&self, window: GuiWindow) -> bool {
        if let Some(status) = self.window_open_flags.get(&window) {
            *status
        }
        else {
            false
        }
    }

    pub fn set_window_open(&mut self, window: GuiWindow, state: bool) {
        *self.window_open_flags.get_mut(&window).unwrap() = state;
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

    pub fn set_floppy_names(&mut self, names: Vec<OsString>) {
        self.floppy_names = names;
    }

    pub fn set_vhd_names(&mut self, names: Vec<OsString>) {
        self.vhd_names = names;
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
    pub fn set_card_list(&mut self, cards: Vec<String>) {
        self.text_mode_viewer.set_cards(cards.clone());
    }

    pub fn set_scaler_presets(&mut self, presets: &Vec<ScalerPreset>) {
        self.scaler_presets = presets.iter().map(|p| p.name.clone()).collect();
        log::debug!("installed scaler presets: {:?}", self.scaler_presets);
    }

    /// Retrieve a newly selected VHD image name for the specified device slot.
    ///
    /// If a VHD image was selected from the UI then we return it as an Option.
    /// A return value of None indicates no selection change.
    pub fn get_new_vhd_name(&mut self, dev: u32) -> Option<OsString> {
        match dev {
            0 => {
                let got_str = self.new_vhd_name0.clone();
                self.new_vhd_name0 = None;
                got_str
            }
            1 => {
                let got_str = self.new_vhd_name1.clone();
                self.new_vhd_name1 = None;
                got_str
            }
            _ => None,
        }
    }

    pub fn show_window(&mut self, window: GuiWindow) {
        *self.window_open_flags.get_mut(&window).unwrap() = true;
    }

    pub fn get_breakpoints(&mut self) -> (&str, &str, &str) {
        self.cpu_control.get_breakpoints()
    }

    pub fn update_pit_state(&mut self, state: &PitDisplayState) {
        self.pit_viewer.update_state(state);
    }

    pub fn update_call_stack_state(&mut self, call_stack_string: String) {
        self.call_stack_string = call_stack_string;
    }

    pub fn update_ppi_state(&mut self, state: PpiStringState) {
        self.ppi_state = state;
    }

    pub fn update_serial_ports(&mut self, ports: Vec<SerialPortInfo>) {
        self.serial_ports = ports;
    }

    pub fn update_videocard_state(&mut self, state: HashMap<String, Vec<(String, VideoCardStateEntry)>>) {
        self.videocard_state = state;
    }

    /// Initialize GUI Display enum state given a vector of DisplayInfo fields.  
    pub fn update_display_info(&mut self, vci: Vec<DisplayInfo>) {
        self.display_info = vci.clone();

        // Build a vector of enums to set to avoid borrowing twice.
        let mut enum_vec = Vec::new();

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

            if let Some(scaler_mode) = &display.scaler_mode {
                enum_vec.push((
                    GuiEnum::DisplayScalerMode(*scaler_mode),
                    Some(GuiVariableContext::Display(idx)),
                ));
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
