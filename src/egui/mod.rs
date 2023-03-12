/*
    gui.rs

    Handle drawing the egui interface

*/
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    ffi::OsString,
    rc::Rc
};

use egui::{
    ClippedPrimitive, 
    Context, 
    ColorImage, 
    //ImageData, 
    TexturesDelta,
};

use egui::{
    Visuals, 
    Color32, 
    //FontDefinitions,
    //Style
};
//use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use pixels::{wgpu, PixelsContext};
use regex::Regex;
use winit::{window::Window, event_loop::EventLoopWindowTarget};
use super::VideoData;

use serialport::SerialPortInfo;

mod constants;
mod color;
mod image;
mod menu;
mod color_swatch;
mod token_listview;
mod disassembly_viewer;
mod videocard_viewer;
mod memory_viewer;
mod pit_viewer;

use crate::{

    egui::image::{UiImage, get_ui_image},
    egui::color::{darken_c32, lighten_c32, add_c32},

    // Use custom windows
    egui::memory_viewer::MemoryViewerControl,
    egui::disassembly_viewer::DisassemblyControl,
    egui::pit_viewer::PitViewerControl,

    machine::{ExecutionControl, ExecutionState, ExecutionOperation},
    cpu_808x::CpuStringState, 
    dma::DMAControllerStringState,
    hdc::HardDiskFormat,
    pit::PitDisplayState, 
    pic::PicStringState,
    ppi::PpiStringState, 
    videocard::{VideoCardState, VideoCardStateEntry}
    
};

const VHD_REGEX: &str = r"[\w_]*.vhd$";

#[derive(PartialEq, Eq, Hash)]
pub(crate) enum GuiWindow {
    About,
    CpuControl,
    PerfViewer,
    MemoryViewer,
    CpuStateViewer,
    RegisterViewer,
    TraceViewer,
    DiassemblyViewer,
    PitViewer,
    PicViewer,
    PpiViewer,
    DmaViewer,
    VideoCardViewer,
    VideoMemViewer,
    CallStack,
    VHDCreator,
}

pub enum GuiEvent {
    #[allow (dead_code)]
    LoadVHD(u32,OsString),
    CreateVHD(OsString, HardDiskFormat),
    LoadFloppy(usize, OsString),
    EjectFloppy(usize),
    BridgeSerialPort(String),
    DumpVRAM,
    DumpCS,
    EditBreakpoint,
    MemoryUpdate,
    TokenHover(usize)
}

/// Manages all state required for rendering egui over `Pixels`.
pub(crate) struct Framework {
    // State for egui.
    egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,

    // State for the GUI
    pub gui: GuiState,
}

/// Example application state. A real application will need a lot more state than this.
pub(crate) struct GuiState {

    texture: Option<egui::TextureHandle>,
    event_queue: VecDeque<GuiEvent>,

    /// Only show the associated window when true.
    window_open_flags: HashMap::<GuiWindow, bool>,
    error_dialog_open: bool,
    
    video_mem: ColorImage,

    video_data: VideoData,
    current_fps: u32,
    emulated_fps: u32,
    current_cps: u64,
    current_ips: u64,
    emulation_ms: u32,
    render_ms: u32,

    // Floppy Disk Images
    floppy_names: Vec<OsString>,
    new_floppy_name0: Option<OsString>,
    new_floppy_name1: Option<OsString>,
    
    // VHD Images
    vhd_names: Vec<OsString>,
    new_vhd_name0: Option<OsString>,
    vhd_name0: OsString,
    new_vhd_name1: Option<OsString>,
    vhd_name1: OsString,

    vhd_formats: Vec<HardDiskFormat>,
    selected_format_idx: usize,
    new_vhd_filename: String,
    vhd_regex: Regex,

    // Serial ports
    serial_ports: Vec<SerialPortInfo>,
    serial_port_name: String,

    exec_control: Rc<RefCell<ExecutionControl>>,

    error_string: String,

    pub memory_viewer: MemoryViewerControl,
    pub cpu_state: CpuStringState,
    
    pub breakpoint: String,
    pub mem_breakpoint: String,
    
    pub pit_viewer: PitViewerControl,
    pub pic_state: PicStringState,
    pub ppi_state: PpiStringState,
    pub dma_state: DMAControllerStringState,

    pub videocard_state: VideoCardState,
    videocard_set_select: String,
    dma_channel_select: u32,
    dma_channel_select_str: String,
    memory_viewer_dump: String,

    disassembly_viewer_string: String,
    disassembly_viewer_address: String,
    pub disassembly_viewer: DisassemblyControl,
    
    trace_string: String,
    call_stack_string: String,

    aspect_correct: bool,
    composite: bool
}

impl Framework {
    /// Create egui.
    pub(crate) fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        width: u32, 
        height: u32, 
        scale_factor: f32, 
        pixels: &pixels::Pixels,
        exec_control: Rc<RefCell<ExecutionControl>>) -> Self {
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;

        let egui_ctx = Context::default();
        let mut egui_state = egui_winit::State::new(event_loop);
        egui_state.set_max_texture_side(max_texture_size);
        egui_state.set_pixels_per_point(scale_factor);

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer = Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        let textures = TexturesDelta::default();
        let gui = GuiState::new(exec_control);

        let visuals = egui::Visuals::dark();
        let visuals = Framework::create_theme(&visuals, Color32::from_rgb(56,45,89));

        //let mut style: egui::Style = (*egui_ctx.style()).clone();
        egui_ctx.set_visuals(visuals);
        //egui_ctx.set_debug_on_hover(true);

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,
            gui,
        }
    }

    fn create_theme(base: &egui::Visuals, color: Color32) -> Visuals {
        
        let mut new_visuals = base.clone();

        new_visuals.window_fill = color;
        new_visuals.extreme_bg_color = darken_c32(color, 0.50);
        new_visuals.faint_bg_color = darken_c32(color, 0.15);

        new_visuals.widgets.noninteractive.bg_fill = lighten_c32(color, 0.10);
        new_visuals.widgets.noninteractive.bg_stroke.color = lighten_c32(color, 0.75);
        new_visuals.widgets.noninteractive.fg_stroke.color = add_c32(color, 128);

        new_visuals.widgets.active.bg_fill = lighten_c32(color, 0.20);
        new_visuals.widgets.active.bg_stroke.color = lighten_c32(color, 0.35);

        new_visuals.widgets.inactive.bg_fill = lighten_c32(color, 0.35);
        new_visuals.widgets.inactive.bg_stroke.color = lighten_c32(color, 0.50);

        new_visuals.widgets.hovered.bg_fill = lighten_c32(color, 0.75);
        new_visuals.widgets.hovered.bg_stroke.color = lighten_c32(color, 0.75);

        new_visuals
    }

    pub(crate) fn has_focus(&self) -> bool {
        match self.egui_ctx.memory().focus() {
            Some(_) => true,
            None => false
        }
    }

    /// Handle input events from the window manager.
    pub(crate) fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        let _ = self.egui_state.on_event(&self.egui_ctx, event);
    }

    /// Resize egui.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    /// Update scaling factor.
    pub(crate) fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    /// Prepare egui.
    pub(crate) fn prepare(&mut self, window: &Window) {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        let raw_input = self.egui_state.take_egui_input(window);

        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            // Draw the application.
            self.gui.ui(egui_ctx);
        });

        self.textures.append(output.textures_delta);
        self.egui_state
            .handle_platform_output(window, &self.egui_ctx, output.platform_output);
        self.paint_jobs = self.egui_ctx.tessellate(output.shapes);
    }

    /// Render egui.
    pub(crate) fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext,
    ) {

        // Upload all resources to the GPU.
        for (id, image_delta) in &self.textures.set {
            self.renderer.update_texture(
                &context.device,
                &context.queue, 
                *id,
                image_delta
            );
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
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer.render(&mut rpass, &self.paint_jobs, &self.screen_descriptor);
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
    fn new(exec_control: Rc<RefCell<ExecutionControl>>) -> Self {

        // Set default values for window open flags
        let window_open_flags: HashMap<GuiWindow, bool> = [
            (GuiWindow::About, false),
            (GuiWindow::CpuControl, true),
            (GuiWindow::PerfViewer, false),
            (GuiWindow::MemoryViewer, false),
            (GuiWindow::CpuStateViewer, false),
            (GuiWindow::RegisterViewer, false),
            (GuiWindow::TraceViewer, false),
            (GuiWindow::DiassemblyViewer, true),
            (GuiWindow::PitViewer, false),
            (GuiWindow::PicViewer, false),
            (GuiWindow::PpiViewer, false),
            (GuiWindow::DmaViewer, false),
            (GuiWindow::VideoCardViewer, false),
            (GuiWindow::VideoMemViewer, false),
            (GuiWindow::CallStack, false),
            (GuiWindow::VHDCreator, false),
        ].into();

        Self { 

            texture: None,
            event_queue: VecDeque::new(),
            window_open_flags: window_open_flags,
            error_dialog_open: false,

            video_mem: ColorImage::new([320,200], egui::Color32::BLACK),

            video_data: Default::default(),
            current_fps: 0,
            emulated_fps: 0,
            current_cps: 0,
            current_ips: 0,
            emulation_ms: 0,
            render_ms: 0,
        
            floppy_names: Vec::new(),
            new_floppy_name0: Option::None,
            new_floppy_name1: Option::None,

            vhd_names: Vec::new(),
            new_vhd_name0: Option::None,
            vhd_name0: OsString::new(),
            new_vhd_name1: Option::None,
            vhd_name1: OsString::new(),

            vhd_formats: Vec::new(),
            selected_format_idx: 0,
            new_vhd_filename: String::new(),
            vhd_regex: Regex::new(VHD_REGEX).unwrap(),

            serial_ports: Vec::new(),
            serial_port_name: String::new(),

            exec_control: exec_control,

            error_string: String::new(),
            memory_viewer_dump: String::new(),
            memory_viewer: MemoryViewerControl::new(),
            cpu_state: Default::default(),
            breakpoint: String::new(),
            mem_breakpoint: String::new(),
            pit_viewer: PitViewerControl::new(),
            pic_state: Default::default(),
            ppi_state: Default::default(),
            dma_state: Default::default(),
            dma_channel_select: 0,
            dma_channel_select_str: String::new(),

            videocard_state: Default::default(),
            videocard_set_select: String::new(),
            disassembly_viewer_string: String::new(),
            disassembly_viewer_address: "cs:ip".to_string(),
            disassembly_viewer: DisassemblyControl::new(),
            trace_string: String::new(),
            call_stack_string: String::new(),

            // Options menu items
            aspect_correct: false,
            composite: false
        }
    }

    pub fn get_event(&mut self) -> Option<GuiEvent> {
        self.event_queue.pop_front()
    }

    #[allow (dead_code)]
    pub fn send_event(&mut self, event: GuiEvent) {
        self.event_queue.push_back(event);
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

    pub fn show_error(&mut self, err_str: &str) {
        self.error_dialog_open = true;
        self.error_string = err_str.to_string();
    }

    pub fn set_floppy_names(&mut self, names: Vec<OsString>) {
        self.floppy_names = names;
    }

    pub fn set_vhd_names(&mut self, names: Vec<OsString>) {
        self.vhd_names = names;
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
                self.new_vhd_name0 = None;
                got_str
            }
            _ => {
                None
            }
        }
    }    

    pub fn show_window(&mut self, window: GuiWindow) {
        *self.window_open_flags.get_mut(&window).unwrap() = true;
    }

    pub fn get_disassembly_view_address(&mut self) -> &str {
        &self.disassembly_viewer_address
    }

    pub fn get_aspect_correct_enabled(&self) -> bool {
        self.aspect_correct
    }   

    pub fn get_composite_enabled(&self) -> bool {
        self.composite
    }

    pub fn update_cpu_state(&mut self, state: CpuStringState) {
        self.cpu_state = state.clone();
    }

    pub fn update_pic_state(&mut self, state: PicStringState) {
        self.pic_state = state;
    }

    pub fn get_breakpoints(&mut self) -> (&str, &str) {
        (&self.breakpoint, &self.mem_breakpoint)
    }

    pub fn update_pit_state(&mut self, state: &PitDisplayState) {
        self.pit_viewer.update_state(state);
    }

    pub fn update_trace_state(&mut self, trace_string: String) {
        self.trace_string = trace_string;
    }

    pub fn update_call_stack_state(&mut self, call_stack_string: String) {
        self.call_stack_string = call_stack_string;
    }

    pub fn update_ppi_state(&mut self, state: PpiStringState) {
        self.ppi_state = state;
    }

    pub fn update_dma_state(&mut self, state: DMAControllerStringState) {
        self.dma_state = state;
    }

    pub fn update_vhd_formats(&mut self, formats: Vec<HardDiskFormat>) {
        self.vhd_formats = formats.clone()
    }

    pub fn update_serial_ports(&mut self, ports: Vec<SerialPortInfo>) {
        self.serial_ports = ports;
    }

    pub fn update_perf_view(&mut self, current_fps: u32, emulated_fps: u32, current_cps: u64, current_ips: u64, emulation_ms: u32, render_ms: u32) {
        self.current_fps = current_fps;
        self.emulated_fps = emulated_fps;
        self.current_cps = current_cps;
        self.current_ips = current_ips;
        self.emulation_ms = emulation_ms;
        self.render_ms = render_ms;
    }

    pub fn update_video_data(&mut self, video_data: VideoData) {
        self.video_data = video_data;
    }

    pub fn update_videocard_state(&mut self, state: HashMap<String,Vec<(String, VideoCardStateEntry)>>) {
        self.videocard_state = state;
    }

    #[allow (dead_code)]
    pub fn update_videomem_state(&mut self, mem: Vec<u8>, w: u32, h: u32) {

        self.video_mem = ColorImage::from_rgba_unmultiplied([w as usize, h as usize],&mem);
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &Context) {

        // Draw top menu bar
        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
            self.draw_menu(ui);
        });
        
        egui::Window::new("About")
            .open(self.window_open_flags.get_mut(&GuiWindow::About).unwrap())
            .show(ctx, |ui| {

                let about_texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
                    ctx.load_texture(
                        "logo",
                        get_ui_image(UiImage::Logo),
                        Default::default()
                    )
                });

                ui.image(about_texture, about_texture.size_vec2());
                ui.separator();

                ui.label("Marty is free software.");

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x /= 2.0;
                    ui.label("Github:");
                    ui.hyperlink("https://github.com/dbalsom/marty");
                });

                ui.separator();

            });

        //let video_texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
        //        ctx.load_texture(
        //            "video_mem",
        //            self.video_mem,
        //        )
        //    });

        egui::Window::new("Video Mem")
            .open(self.window_open_flags.get_mut(&GuiWindow::VideoMemViewer).unwrap())
            .show(ctx, |_ui| {

            });            

        egui::Window::new("Error")
            .open(&mut self.error_dialog_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("❎").color(egui::Color32::RED).font(egui::FontId::proportional(40.0)));
                    ui.label(&self.error_string);
                });
            });

        egui::Window::new("Performance")
            .open(self.window_open_flags.get_mut(&GuiWindow::PerfViewer).unwrap())
            .show(ctx, |ui| {

                egui::Grid::new("perf")
                    .striped(true)
                    .min_col_width(100.0)
                    .show(ui, |ui| {
                        ui.label("Internal resolution: ");
                        ui.label(egui::RichText::new(format!("{}, {}", 
                            self.video_data.render_w, 
                            self.video_data.render_h))
                            .background_color(egui::Color32::BLACK));
                        ui.end_row();
                        ui.label("Display buffer resolution: ");
                        ui.label(egui::RichText::new(format!("{}, {}", 
                            self.video_data.aspect_w, 
                            self.video_data.aspect_h))
                            .background_color(egui::Color32::BLACK));
                        ui.end_row();

                        ui.label("FPS: ");
                        ui.label(egui::RichText::new(format!("{}", self.current_fps)).background_color(egui::Color32::BLACK));
                        ui.end_row();
                        ui.label("Emulated FPS: ");
                        ui.label(egui::RichText::new(format!("{}", self.emulated_fps)).background_color(egui::Color32::BLACK));
                        ui.end_row();                        
                        ui.label("IPS: ");
                        ui.label(egui::RichText::new(format!("{}", self.current_ips)).background_color(egui::Color32::BLACK));
                        ui.end_row();
                        ui.label("CPS: ");
                        ui.label(egui::RichText::new(format!("{}", self.current_cps)).background_color(egui::Color32::BLACK));
                        ui.end_row();                         
                        ui.label("Emulation time: ");
                        ui.label(egui::RichText::new(format!("{}", self.emulation_ms)).background_color(egui::Color32::BLACK));
                        ui.end_row();
                        ui.label("Render time: ");
                        ui.label(egui::RichText::new(format!("{}", self.render_ms)).background_color(egui::Color32::BLACK));
                        ui.end_row();
                    });      
            });

        egui::Window::new("CPU Control")
            .open(self.window_open_flags.get_mut(&GuiWindow::CpuControl).unwrap())
            .show(ctx, |ui| {

                let mut exec_control = self.exec_control.borrow_mut();

                let (pause_enabled, step_enabled, run_enabled) = match exec_control.state {
                    ExecutionState::Paused | ExecutionState::BreakpointHit => (false, true, true),
                    ExecutionState::Running => (true, false, false),
                    ExecutionState::Halted => (false, false, false),
                };

                ui.horizontal(|ui|{

                    ui.add_enabled_ui(pause_enabled, |ui| {
                        if ui.button(egui::RichText::new("⏸").font(egui::FontId::proportional(20.0))).clicked() {
                            exec_control.set_state(ExecutionState::Paused);
                        };
                    });

                    ui.add_enabled_ui(step_enabled, |ui| {
                        if ui.button(egui::RichText::new("⤵").font(egui::FontId::proportional(20.0))).clicked() {
                           exec_control.set_op(ExecutionOperation::StepOver);
                        };

                        if ui.input().key_pressed(egui::Key::F10) {
                            exec_control.set_op(ExecutionOperation::StepOver);
                        }                             
                    });   

                    ui.add_enabled_ui(step_enabled, |ui| {
                        if ui.button(egui::RichText::new("➡").font(egui::FontId::proportional(20.0))).clicked() {
                           exec_control.set_op(ExecutionOperation::Step);
                        };

                        if ui.input().key_pressed(egui::Key::F11) {
                            log::debug!("f11 pressed!");
                            exec_control.set_op(ExecutionOperation::Step);
                        }                             
                    });                 

                    ui.add_enabled_ui(run_enabled, |ui| {
                        if ui.button(egui::RichText::new("▶").font(egui::FontId::proportional(20.0))).clicked() {
                            exec_control.set_op(ExecutionOperation::Run);
                        };

                        if ui.input().key_pressed(egui::Key::F5) {
                            exec_control.set_op(ExecutionOperation::Run);
                        }                        
                    });

                    if ui.button(egui::RichText::new("⟲").font(egui::FontId::proportional(20.0))).clicked() {
                        exec_control.set_op(ExecutionOperation::Reset);
                    };
                });

                let state_str = format!("{:?}", exec_control.get_state());
                ui.separator();
                ui.horizontal(|ui|{
                    ui.label("Run state: ");
                    ui.label(&state_str);
                });
                ui.separator();
                ui.horizontal(|ui|{
                    ui.label("Exec Breakpoint: ");
                    if ui.text_edit_singleline(&mut self.breakpoint).changed() {
                        self.event_queue.push_back(GuiEvent::EditBreakpoint);
                    };
                });
                ui.separator();
                ui.horizontal(|ui|{
                    ui.label("Mem Breakpoint: ");
                    if ui.text_edit_singleline(&mut self.mem_breakpoint).changed() {
                        self.event_queue.push_back(GuiEvent::EditBreakpoint);
                    }
                });                
            });

        egui::Window::new("Memory View")
            .open(self.window_open_flags.get_mut(&GuiWindow::MemoryViewer).unwrap())
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {
                self.memory_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Instruction Trace")
            .open(self.window_open_flags.get_mut(&GuiWindow::TraceViewer).unwrap())
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {

                ui.horizontal(|ui| {
                    ui.add_sized(ui.available_size(), 
                        egui::TextEdit::multiline(&mut self.trace_string)
                            .font(egui::TextStyle::Monospace));
                    ui.end_row()
                });
            });       


        egui::Window::new("Call Stack")
            .open(self.window_open_flags.get_mut(&GuiWindow::CallStack).unwrap())
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {

                ui.horizontal(|ui| {
                    ui.add_sized(ui.available_size(), 
                        egui::TextEdit::multiline(&mut self.call_stack_string)
                            .font(egui::TextStyle::Monospace));
                    ui.end_row()
                });
            });              

        egui::Window::new("Disassembly View")
            .open(self.window_open_flags.get_mut(&GuiWindow::DiassemblyViewer).unwrap())
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {
                self.disassembly_viewer.draw(ui, &mut self.event_queue);
            });             

        egui::Window::new("Register View")
            .open(self.window_open_flags.get_mut(&GuiWindow::RegisterViewer).unwrap())
            .resizable(false)
            .default_width(220.0)
            .show(ctx, |ui| {
                egui::Grid::new("reg_general")
                    .striped(true)
                    .min_col_width(100.0)
                    .show(ui, |ui| {

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("AH:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ah).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("AL:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.al).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("AX:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ax).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("BH:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bh).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("BL:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bl).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("BX:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bx).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("CH:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ch).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("CL:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cl).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("CX:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cx).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("DH:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dh).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("DL:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dl).font(egui::TextStyle::Monospace));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("DX:").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dx).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();         
                });

                ui.separator();

                egui::Grid::new("reg_segment")
                    .striped(true)
                    .min_col_width(100.0)
                    .show(ui, |ui| {

                        ui.horizontal( |ui| {
                            //ui.add(egui::Label::new("SP:"));
                            ui.label(egui::RichText::new("SP:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.sp).font(egui::TextStyle::Monospace));
                        });
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("ES:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.es).font(egui::TextStyle::Monospace));
                        });                        
                        ui.end_row();  
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("BP:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bp).font(egui::TextStyle::Monospace));
                        });
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("CS:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cs).font(egui::TextStyle::Monospace));
                        });                         
                        ui.end_row();  
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("SI:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.si).font(egui::TextStyle::Monospace));
                        });
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("SS:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ss).font(egui::TextStyle::Monospace));
                        });                         
                        ui.end_row();  
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("DI:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.di).font(egui::TextStyle::Monospace));
                        });
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("DS:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ds).font(egui::TextStyle::Monospace));
                        });                         
                        ui.end_row();  
                        ui.label("");
                        ui.horizontal( |ui| {
                            ui.label(egui::RichText::new("IP:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ip).font(egui::TextStyle::Monospace));
                            //ui.text_edit_singleline(&mut self.memory_viewer_address);
                        }); 
                        ui.end_row();  
                    });

                ui.separator();

                egui::Grid::new("reg_flags")
                    .striped(true)
                    .max_col_width(15.0)
                    .show(ui, |ui| {
                        //const CPU_FLAG_CARRY: u16      = 0b0000_0000_0001;
                        //const CPU_FLAG_RESERVED1: u16  = 0b0000_0000_0010;
                        //const CPU_FLAG_PARITY: u16     = 0b0000_0000_0100;
                        //const CPU_FLAG_AUX_CARRY: u16  = 0b0000_0001_0000;
                        //const CPU_FLAG_ZERO: u16       = 0b0000_0100_0000;
                        //const CPU_FLAG_SIGN: u16       = 0b0000_1000_0000;
                        //const CPU_FLAG_TRAP: u16       = 0b0001_0000_0000;
                        //const CPU_FLAG_INT_ENABLE: u16 = 0b0010_0000_0000;
                        //const CPU_FLAG_DIRECTION: u16  = 0b0100_0000_0000;
                        //const CPU_FLAG_OVERFLOW: u16   = 0b1000_0000_0000;

                        ui.horizontal( |ui| {
                            //ui.add(egui::Label::new("SP:"));
                            ui.label(egui::RichText::new("O:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.o_fl).font(egui::TextStyle::Monospace));
                            ui.label(egui::RichText::new("D:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.d_fl).font(egui::TextStyle::Monospace)); 
                            ui.label(egui::RichText::new("I:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.i_fl).font(egui::TextStyle::Monospace));  
                            ui.label(egui::RichText::new("T:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.t_fl).font(egui::TextStyle::Monospace));
                            ui.label(egui::RichText::new("S:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.s_fl).font(egui::TextStyle::Monospace));
                            ui.label(egui::RichText::new("Z:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.z_fl).font(egui::TextStyle::Monospace));      
                            ui.label(egui::RichText::new("A:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.a_fl).font(egui::TextStyle::Monospace));  
                            ui.label(egui::RichText::new("P:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.p_fl).font(egui::TextStyle::Monospace));             
                            ui.label(egui::RichText::new("C:").text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.c_fl).font(egui::TextStyle::Monospace));                                        
                        });

                        ui.end_row();  
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Instruction #:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.instruction_count).font(egui::TextStyle::Monospace));
                }); 
            });        
            
        egui::Window::new("PIT View")
            .open(self.window_open_flags.get_mut(&GuiWindow::PitViewer).unwrap())
            .resizable(false)
            .min_width(600.0)
            .default_width(600.0)
            .show(ctx, |ui| {

                self.pit_viewer.draw(ui, &mut self.event_queue);

            });               

            egui::Window::new("PIC View")
            .open(self.window_open_flags.get_mut(&GuiWindow::PicViewer).unwrap())
            .resizable(true)
            .default_width(600.0)
            .show(ctx, |ui| {
                egui::Grid::new("pic_view")
                    .striped(true)
                    .min_col_width(300.0)
                    .show(ui, |ui| {

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("IMR Register: ").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.pic_state.imr).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("IRR Register: ").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.pic_state.irr).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("ISR Register: ").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.pic_state.isr).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Auto-EOI: ").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.pic_state.autoeoi).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Trigger Mode: ").text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut self.pic_state.trigger_mode).font(egui::TextStyle::Monospace));
                    });
                    ui.end_row();                    

                    for i in 0..self.pic_state.interrupt_stats.len() {
                        ui.horizontal(|ui| {
                            let label_str = format!("IRQ {} IMR Masked: ", i );
                            ui.label(egui::RichText::new(label_str).text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.pic_state.interrupt_stats[i].0).font(egui::TextStyle::Monospace));
                        });
                        ui.end_row();
                        ui.horizontal(|ui| {
                            let label_str = format!("IRQ {} ISR Masked: ", i );
                            ui.label(egui::RichText::new(label_str).text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.pic_state.interrupt_stats[i].1).font(egui::TextStyle::Monospace));
                        });
                        ui.end_row();
                        ui.horizontal(|ui| {
                            let label_str = format!("IRQ {} Serviced:   ", i );
                            ui.label(egui::RichText::new(label_str).text_style(egui::TextStyle::Monospace));
                            ui.add(egui::TextEdit::singleline(&mut self.pic_state.interrupt_stats[i].2).font(egui::TextStyle::Monospace));
                        });
                        ui.end_row();                                                
                    }
                      
                });
            });           
            
            egui::Window::new("PPI View")
            .open(self.window_open_flags.get_mut(&GuiWindow::PpiViewer).unwrap())
            .resizable(true)
            .default_width(600.0)
            .show(ctx, |ui| {
                egui::Grid::new("ppi_view")
                    .num_columns(2)
                    .striped(true)
                    .spacing([40.0, 4.0])
                    .show(ui, |ui| {
                        
                    ui.label(egui::RichText::new("Port A Mode:  ").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_a_mode).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new("Port A Value: ").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_a_value_bin).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new("Port A Value: ").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_a_value_hex).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new("Port B Value: ").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_b_value_bin).font(egui::TextStyle::Monospace));
                    ui.end_row();                    

                    ui.label(egui::RichText::new("Keyboard byte:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.kb_byte_value_hex).font(egui::TextStyle::Monospace));
                    ui.end_row();
                    
                    ui.label(egui::RichText::new("Keyboard resets:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.kb_resets_counter).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new("Port C Mode:  ").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_c_mode).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new("Port C Value: ").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_c_value).font(egui::TextStyle::Monospace));
                    ui.end_row();
                });
            });

            egui::Window::new("DMA View")
            .open(self.window_open_flags.get_mut(&GuiWindow::DmaViewer).unwrap())
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                egui::Grid::new("dma_view")
                    .num_columns(2)
                    .striped(true)
                    .min_col_width(50.0)
                    .show(ui, |ui| {

                    ui.label(egui::RichText::new(format!("Enabled:")).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.dma_state.enabled).font(egui::TextStyle::Monospace));
                    ui.end_row();     

                    //ui.horizontal(|ui| {
                    //    ui.separator();
                    //});
                    ui.separator();
                    ui.separator();
                    ui.end_row();    

                    ui.horizontal(|ui| {
                        egui::ComboBox::from_label("Channel #")
                            .selected_text(format!("Channel #{}", self.dma_channel_select))
                            .show_ui(ui, |ui| {
                                for (i, _chan) in self.dma_state.dma_channel_state.iter_mut().enumerate() {
                                    ui.selectable_value(&mut self.dma_channel_select, i as u32, format!("Channel #{}",i));
                                }
                            });
                    });                        
                    ui.end_row();   

                    if (self.dma_channel_select as usize) < self.dma_state.dma_channel_state.len() {
                        let chan = &mut self.dma_state.dma_channel_state[self.dma_channel_select as usize];
                        
                        ui.label(egui::RichText::new(format!("#{} CAR:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.current_address_reg).font(egui::TextStyle::Monospace));
                        ui.end_row();

                        ui.label(egui::RichText::new(format!("#{} Page:        ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.page).font(egui::TextStyle::Monospace));
                        ui.end_row();                      

                        ui.label(egui::RichText::new(format!("#{} CWC:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.current_word_count_reg).font(egui::TextStyle::Monospace));
                        ui.end_row();

                        ui.label(egui::RichText::new(format!("#{} BAR:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.base_address_reg).font(egui::TextStyle::Monospace));
                        ui.end_row();

                        ui.label(egui::RichText::new(format!("#{} BWC:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.base_word_count_reg).font(egui::TextStyle::Monospace));
                        ui.end_row();    

                        ui.label(egui::RichText::new(format!("#{} Service Mode:", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.service_mode).font(egui::TextStyle::Monospace));
                        ui.end_row();

                        ui.label(egui::RichText::new(format!("#{} Address Mode:", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.address_mode).font(egui::TextStyle::Monospace));
                        ui.end_row();

                        ui.label(egui::RichText::new(format!("#{} Xfer Type:   ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.transfer_type).font(egui::TextStyle::Monospace));
                        ui.end_row();

                        ui.label(egui::RichText::new(format!("#{} Auto Init:   ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.auto_init).font(egui::TextStyle::Monospace));
                        ui.end_row();   

                        ui.label(egui::RichText::new(format!("#{} Terminal Ct: ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.terminal_count).font(egui::TextStyle::Monospace));
                        ui.end_row();  

                        ui.label(egui::RichText::new(format!("#{} TC Reached:  ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.terminal_count_reached).font(egui::TextStyle::Monospace));
                        ui.end_row();

                        ui.label(egui::RichText::new(format!("#{} Masked:      ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                        ui.add(egui::TextEdit::singleline(&mut chan.masked).font(egui::TextStyle::Monospace));
                        ui.end_row();
                    }


                });
            });                       

            egui::Window::new("Video Card View")
                .open(self.window_open_flags.get_mut(&GuiWindow::VideoCardViewer).unwrap())
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                   GuiState::draw_video_card_panel(ui, &self.videocard_state);
                });         

            egui::Window::new("Create VHD")
                .open(self.window_open_flags.get_mut(&GuiWindow::VHDCreator).unwrap())
                .resizable(false)
                .default_width(400.0)
                .show(ctx, |ui| {

                    if self.vhd_formats.len() > 0 {
                        egui::ComboBox::from_label("Format")
                        .selected_text(format!("{}", self.vhd_formats[self.selected_format_idx].desc))
                        .show_ui(ui, |ui| {
                            for (i, fmt) in self.vhd_formats.iter_mut().enumerate() {
                                ui.selectable_value(&mut self.selected_format_idx, i, format!("{}", fmt.desc));
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Filename: ");
                            ui.text_edit_singleline(&mut self.new_vhd_filename);
                        });               

                        let enabled = self.vhd_regex.is_match(&self.new_vhd_filename.to_lowercase());

                        if ui.add_enabled(enabled, egui::Button::new("Create"))
                            .clicked() {
                            self.event_queue.push_back(GuiEvent::CreateVHD(OsString::from(&self.new_vhd_filename), self.vhd_formats[self.selected_format_idx].clone()))
                        };                        
                    }
            });

        }
    }


