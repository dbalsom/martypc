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

    render::mod.rs

    This module implements various Video rendering functions. Video card devices
    render in either Direct or Indirect mode.

    In Direct mode, the video device draws directly to a framebuffer in an
    intermediate representation, which the render module converts and displays.

    In Indirect mode, the render module draws the video device's VRAM directly. 
    This is fast, but not always accurate if register writes happen mid-frame.
*/

#![allow(dead_code)]
#![allow(clippy::identity_op)] // Adding 0 lines things up nicely for formatting.

use std::path::Path;
use std::mem::size_of;
use std::sync::{Arc, Mutex};

pub mod consts;
pub mod color;
pub mod resize;
pub mod draw;
pub mod composite;
// Reenigne composite
pub mod composite_new;

// Re-export submodules
pub use self::resize::*;
pub use self::composite::*;
pub use self::color::*;
pub use self::consts::*;

use composite_new::{ReCompositeContext, ReCompositeBuffers};

use marty_core::{
    videocard::{VideoType, CGAColor, CGAPalette, DisplayExtents, DisplayMode},
    devices::cga,
    file_util
};

pub use display_scaler::ScalerMode;

use image;
use log;
use display_scaler::{ScalerEffect, ScalerOption, ScanlineMode};

#[derive (Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScalingMode {
    Integer = 0,
    Scale = 1,
    Stretch = 2
}

/*
impl Default for ScalingMode {
    fn default() -> Self {
        ScalingMode::Integer
    }
}

pub const SCALING_MODES: [ScalingMode; 3] = [
    ScalingMode::Integer,
    ScalingMode::Scale,
    ScalingMode::Stretch,
];
*/

pub const SCALING_MODES: [ScalerMode; 4] = [
    ScalerMode::None,
    ScalerMode::Integer,
    ScalerMode::Fit,
    ScalerMode::Stretch,
];

#[derive (Copy, Clone, Default)]
pub struct VideoDimensions {
    pub w: u32,
    pub h: u32
}

impl From<(u32, u32)> for VideoDimensions {
    fn from(t: (u32, u32)) -> Self {
        VideoDimensions { w: t.0, h: t.1 }
    }
}

#[derive (Copy, Clone)]
pub struct VideoParams {
    pub render_w: u32,
    pub render_h: u32,
    pub aspect_w: u32,
    pub aspect_h: u32,
    pub surface_w: u32,
    pub surface_h: u32,
    pub aspect_correction_enabled: bool,
    pub composite_params: CompositeParams,
}

impl Default for VideoParams {
    fn default() -> VideoParams {
        VideoParams {
            render_w: DEFAULT_RENDER_WIDTH,
            render_h: DEFAULT_RENDER_HEIGHT,
            aspect_w: 640,
            aspect_h: 480,
            surface_w: 640,
            surface_h: 480,
            aspect_correction_enabled: false,
            composite_params: Default::default(),
        }
    }
}

#[derive (Copy, Clone, PartialEq)]
pub struct AspectRatio {
    pub h: u32,
    pub v: u32,
}

impl Default for AspectRatio {
    fn default() -> Self {
        Self {
            h: 4,
            v: 3
        }
    }
}

impl AspectRatio {
    pub fn is_square(&self) -> bool {
        self.h == 1 && self.v == 1
    }
}

#[derive (Copy, Clone, Debug)]
pub struct CompositeParams {
    pub phase: usize,
    pub contrast: f64,
    pub hue: f64,
    pub sat: f64,
    pub luma: f64,
    pub new_cga: bool
}

impl Default for CompositeParams {
    fn default() -> Self {
        Self {
            phase: 0,
            contrast: 1.0,
            hue: 0.0,
            sat: 1.0,
            luma: 1.0,
            new_cga: false,
        }
    }
}

#[derive (Copy, Clone, Debug)]
pub struct ScalerParams {
    pub crt_effect: bool,
    pub crt_hcurvature: f32,
    pub crt_vcurvature: f32,
    pub crt_cornerradius: f32,
    pub crt_scanlines: bool,
    pub crt_phosphor_type: PhosphorType,
    pub gamma: f32,
}

impl Default for ScalerParams {
    fn default() -> Self {
        Self {
            crt_effect: false,
            crt_hcurvature: 0.0,
            crt_vcurvature: 0.0,
            crt_cornerradius: 0.0,
            crt_scanlines: false,
            crt_phosphor_type: PhosphorType::Color,
            gamma: 1.0,
        }
    }
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum PhosphorType {
    Color,
    White,
    Green,
    Amber,
}

#[derive (Copy, Clone)]
pub enum RenderColor {
    CgaIndex(u8),
    Rgb(u8, u8, u8)
}

#[derive (Copy, Clone)]
pub struct DebugRenderParams {
    pub draw_scanline: Option<u32>,
    pub draw_scanline_color: Option<RenderColor>
}

pub struct VideoRenderer<T,U> {
    video_type: VideoType,
    scaler_mode: ScalerMode,
    mode: DisplayMode,
    params: VideoParams,

    buf: Vec<u8>,
    aspect_ratio: Option<AspectRatio>,
    software_aspect: bool,

    mode_byte: u8,

    // Legacy composite stuff
    composite_buf: Option<Vec<u8>>,
    sync_table_w: u32,
    sync_table: Vec<(f32, f32, f32)>,

    // Reenigne composite stuff
    composite_ctx: ReCompositeContext,
    composite_bufs: ReCompositeBuffers,
    last_cga_mode: u8,

    // Composite adjustments    
    composite_params: CompositeParams,
    resample_context: ResampleContext,

    // Callback closures
    on_resize: Option<Box<dyn FnMut(&mut T, u32, u32) + Send>>,
    on_resize_surface: Option<Box<dyn FnMut(&mut T, u32, u32) + Send>>,
    on_resize_scaler: Option<Box<dyn FnMut(&mut T, &mut U, u32, u32, u32, u32) + Send>>,
    on_margin: Option<Box<dyn FnMut(&mut U, u32, u32, u32, u32) + Send>>,
    on_scalermode: Option<Box<dyn FnMut(&mut T, &mut U, ScalerMode) + Send>>,
    on_scaler_options: Option<Box<dyn FnMut(&mut T, &mut U, Vec<ScalerOption>) + Send>>,
    on_get_buffer: Option<Box<dyn FnMut(&mut T) -> &mut [u8]>>,
    on_with_buffer: Option<Box<dyn FnMut(&mut dyn FnMut(&mut [u8])) + Send>>,
    backend: Arc<Mutex<T>>,
    scaler: Arc<Mutex<U>>,
}

impl<T,U> VideoRenderer<T,U> {

    pub fn new(
        video_type: VideoType, 
        scaler_mode: ScalerMode,
        backend: T,
        scaler: U,
    ) -> Self {

        // Create a buffer to hold composite conversion of CGA graphics.
        // This buffer will need to be twice as large as the largest possible
        // CGA screen (CGA_MAX_CLOCK * 4) to account for half-hdots used in the 
        // composite conversion process.
        let composite_vec_opt = match video_type {
            VideoType::CGA => {
                Some(vec![0; cga::CGA_MAX_CLOCK * 4])
            }
            _ => {
                None
            }
        };

        Self {
            video_type,
            scaler_mode,
            mode: DisplayMode::Mode3TextCo80,
            params: Default::default(),
            
            buf: vec![0; (DEFAULT_RENDER_WIDTH * DEFAULT_RENDER_HEIGHT * 4) as usize],
            aspect_ratio: None,
            software_aspect: true,

            mode_byte: 0,

            // Legacy composite stuff
            composite_buf: composite_vec_opt,
            sync_table_w: 0,
            sync_table: Vec::new(),

            // Reenigne composite stuff
            composite_ctx: ReCompositeContext::new(),
            composite_bufs: ReCompositeBuffers::new(),
            last_cga_mode: 0,

            composite_params: Default::default(),
            resample_context: ResampleContext::new(),

            on_resize: None,
            on_resize_surface: None,
            on_resize_scaler: None,
            on_margin: None,
            on_scalermode: None,
            on_scaler_options: None,
            on_get_buffer: None,
            on_with_buffer: None,
            backend: Arc::new(Mutex::new(backend)),
            scaler: Arc::new(Mutex::new(scaler)),
        }
    }

    pub fn set_scaler_mode(&mut self, new_mode: ScalerMode) {
        self.scaler_mode = new_mode;

        if let Some(ref mut scaler_callback) = self.on_scalermode {
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            let mut scaler = self.scaler.lock().expect("Failed to lock scaler");
            scaler_callback(&mut *backend, &mut *scaler, new_mode)
        }
    }



    pub fn get_scaler_mode(&mut self) -> ScalerMode {
        self.scaler_mode
    }

    /// Return a mutable reference to the render buffer
    pub fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    /// Return a reference to the video paramters
    pub fn params(&self) -> &VideoParams {
        &self.params
    }

    pub fn set_on_resize<F>(&mut self, resize_closure: F)
    where
        F: 'static + FnMut(&mut T, u32, u32) + Send,
    {
        self.on_resize = Some(Box::new(resize_closure));
    }    

    pub fn set_on_resize_scaler<F>(&mut self, resize_closure: F)
    where
        F: 'static + FnMut(&mut T, &mut U, u32, u32, u32, u32) + Send,
    {
        self.on_resize_scaler = Some(Box::new(resize_closure));
    }

    pub fn set_on_margin<F>(&mut self, scaler_closure: F)
    where
        F: 'static + FnMut(&mut U, u32, u32, u32, u32) + Send,
    {
        self.on_margin = Some(Box::new(scaler_closure));
    }

    pub fn set_on_scalemode<F>(&mut self, scaler_closure: F)
    where
        F: 'static + FnMut(&mut T, &mut U, ScalerMode) + Send,
    {
        self.on_scalermode = Some(Box::new(scaler_closure));
    }

    pub fn set_on_scaler_options<F>(&mut self, scaler_closure: F)
        where
            F: 'static + FnMut(&mut T, &mut U, Vec<ScalerOption>) + Send,
    {
        self.on_scaler_options = Some(Box::new(scaler_closure));
    }

    pub fn set_on_resize_surface<F>(&mut self, resize_closure: F)
    where
        F: 'static + FnMut(&mut T, u32, u32) + Send,
    {
        self.on_resize_surface = Some(Box::new(resize_closure));
    }

    pub fn set_get_buffer<F>(&mut self, get_buffer_closure: F)
    where
        F: 'static + FnMut(&mut T) -> &mut [u8] + Send,
    {
        self.on_get_buffer = Some(Box::new(get_buffer_closure));
    }

    pub fn set_with_buffer<F>(&mut self, with_buffer_closure: F)
    where
        F: 'static + FnMut(&mut dyn FnMut(&mut [u8])) + Send,
    {
        self.on_with_buffer = Some(Box::new(with_buffer_closure));
    }

    pub fn with_backend<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T, &mut U) -> R,
    {
        let mut backend = self.backend.lock().expect("Failed to lock backend");
        let mut scaler = self.scaler.lock().expect("Failed to lock scaler");
        f(&mut *backend, &mut *scaler)
    }

    pub fn backend_resize(&mut self) {
        if let Some(ref mut resize_callback) = self.on_resize {
            // Lock the backend and pass the mutable reference to the closure
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            resize_callback(&mut *backend, self.params.aspect_w, self.params.aspect_h);
        }
    }

    pub fn backend_resize_surface(&mut self, new: VideoDimensions) {
        if let Some(ref mut resize_callback) = self.on_resize_surface {
            // Lock the backend and pass the mutable reference to the closure
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            resize_callback(&mut *backend, new.w, new.h);
        }

        self.params.surface_w = new.w;
        self.params.surface_h = new.h;

        self.backend_resize_scaler((self.params.aspect_w, self.params.aspect_h).into(), (new.w, new.h).into());
    }

    pub fn backend_resize_scaler(&mut self, buf: VideoDimensions, screen: VideoDimensions) {

        if let Some(ref mut resize_callback) = self.on_resize_scaler {
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            let mut scaler = self.scaler.lock().expect("Failed to lock scaler");
            resize_callback(&mut *backend, &mut *scaler, buf.w, buf.h, screen.w, screen.h);
        }        
    }

    pub fn set_scaler_margin(&mut self, l: u32, r: u32, t: u32, b: u32) {
        if let Some(ref mut margin_callback) = self.on_margin {
            let mut scaler = self.scaler.lock().expect("Failed to lock scaler");
            margin_callback(&mut *scaler, l, r, t, b);
        }     
    }

    /*
    pub  fn get_buffer<F>(&mut self) -> Option<&mut [u8]>
    where
        F: 'static + FnMut(&mut T) -> &mut [u8] + Send,
    {
        if let Some(ref mut get_buffer_callback) = self.on_get_buffer {
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            return Some(get_buffer_callback(&mut *backend))
        }
        None
    }
    */

    pub fn get_backend(&self) -> Arc<Mutex<T>> {
        self.backend.clone()
    }

    pub fn with_backend_buffer<F>(&mut self, mut action: F)
    where
        F: FnMut(&mut [u8]),
    {
        if let Some(ref mut with_buffer) = self.on_with_buffer {
            // Call the stored closure with the provided action as an argument.
            with_buffer(&mut action);
        }
    }

    pub fn resize(&mut self, new: VideoDimensions ) {

        self.params.render_w = new.w;
        self.params.render_h = new.h;

        if let Some(_) = self.aspect_ratio {
            let new_aspect = VideoRenderer::<T,U>::get_aspect_corrected_res(new, self.aspect_ratio);

            self.params.aspect_w = new_aspect.w;
            self.params.aspect_h = new_aspect.h;
        }
        else {
            self.params.aspect_w = self.params.render_w;
            self.params.aspect_h = self.params.render_h;
        }

        self.buf.resize((new.w * new.h * 4) as usize, 0);
        self.buf.fill(0);

        // Resize backend via closure
        if let Some(ref mut resize_callback) = self.on_resize {
            // Lock the backend and pass the mutable reference to the closure
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            resize_callback(&mut *backend, self.params.aspect_w, self.params.aspect_h);

            // Resize scaler via closure
            if let Some(ref mut resize_scaler_callback) = self.on_resize_scaler {
                let mut scaler = self.scaler.lock().expect("Failed to lock scaler");

                resize_scaler_callback(
                    &mut *backend, 
                    &mut *scaler, 
                    self.params.aspect_w, 
                    self.params.aspect_h, 
                    self.params.surface_w, 
                    self.params.surface_h
                );           
            }            
        }
    }

    // Given the new specified dimensions, returns a bool if the dimensions require resizing
    // the internal buffer.
    pub fn would_resize(&self, new: VideoDimensions) -> bool {

        if self.software_aspect {
            let new_aspect = VideoRenderer::<T,U>::get_aspect_corrected_res(new, self.aspect_ratio);
            if self.params.aspect_w != new_aspect.w || self.params.aspect_h != new_aspect.h {
                return true
            }
        }   
        else {
            if self.params.render_w != new.w || self.params.render_h != new.h {
                return true
            }
        }     
        false
    }
    
    pub fn get_buf_dimensions(&mut self) -> VideoDimensions {
        (self.params.render_w, self.params.render_h).into()
    }

    pub fn get_display_dimensions(&mut self) -> VideoDimensions {
        if self.software_aspect {
            (self.params.aspect_w, self.params.aspect_h).into()
        }
        else {
            (self.params.render_w, self.params.render_h).into()
        }
    }

    pub fn set_aspect_ratio(&mut self, new_aspect: Option<AspectRatio>) {

        if let Some(aspect) = new_aspect {
            if self.aspect_ratio != new_aspect {
                // Aspect ratio is changing.
    
                let desired_ratio: f64 = aspect.h as f64 / aspect.v as f64;
                let adjusted_h = (self.params.render_w as f64 / desired_ratio) as u32;
    
                self.params.aspect_h = adjusted_h;
                log::debug!(
                    "VideoRenderer: Adjusting backend dimensions due to aspect ratio change. New dimensions: {}x{}",
                    self.params.aspect_w,
                    self.params.aspect_h
                );
                
                self.aspect_ratio = Some(aspect);
            }
        }
        else {
            // Disable aspect correction
            self.params.aspect_w = self.params.render_w;
            self.params.aspect_h = self.params.render_h;
            self.aspect_ratio = None;
        }

        // Resize backend via closure
        if let Some(ref mut resize_callback) = self.on_resize {
            // Lock the backend and pass the mutable reference to the closure
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            resize_callback(&mut *backend, self.params.aspect_w, self.params.aspect_h);
        }               

        // Resize scaler
        self.backend_resize_scaler(
            (self.params.aspect_w, self.params.aspect_h).into(), 
            (self.params.surface_w, self.params.surface_h).into()
        );
    }

    pub fn set_scaler_params(&mut self, params: &ScalerParams) {
        /*
        pub enum ScalerOption {
            Mode(ScalerMode),
            Margins { l: u32, r: u32, t: u32, b: u32 },
            Filtering(ScalerFilter),
            FillColor { r: u8, g: u8, b: u8, a: u8 },
            Effect(ScalerEffect),
        }*/

        let mut scaler_update = Vec::new();
        scaler_update.push(
            ScalerOption::Geometry{
                h_curvature: params.crt_hcurvature,
                v_curvature: params.crt_vcurvature,
                corner_radius: params.crt_cornerradius,
            }
        );

        scaler_update.push(
            ScalerOption::Adjustment {
                h: 1.0,
                s: 1.0,
                c: 1.0,
                b: 1.0,
                g: params.gamma
            }
        );

        scaler_update.push(
            ScalerOption::Scanlines {
                enabled: params.crt_scanlines,
                intensity: 0.3,
            }
        );
        
        match params.crt_phosphor_type {
            PhosphorType::Color => {
                scaler_update.push(
                    ScalerOption::Mono {
                        enabled: false,
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    })
            },
            PhosphorType::White => {
                scaler_update.push(
                ScalerOption::Mono {
                    enabled: true,
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                })
            }
            PhosphorType::Green => {
                scaler_update.push(
                ScalerOption::Mono {
                    enabled: true,
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                })
            }
            PhosphorType::Amber => {
                scaler_update.push(
                    ScalerOption::Mono {
                        enabled: true,
                        r: 1.0,
                        g: 0.75,
                        b: 0.0,
                        a: 1.0,
                    })
            }
        }

        if let Some(ref mut scaler_callback) = self.on_scaler_options {
            let mut backend = self.backend.lock().expect("Failed to lock backend");
            let mut scaler = self.scaler.lock().expect("Failed to lock scaler");
            log::debug!("Sending scaler params to callback...");
            scaler_callback(&mut *backend, &mut *scaler, scaler_update);
        }
    }

    /// Given the specified resolution and desired aspect ratio, return an aspect corrected resolution
    /// by adjusting the vertical resolution (Horizontal resolution will never be changed)
    pub fn get_aspect_corrected_res(res: VideoDimensions, aspect: Option<AspectRatio>) -> VideoDimensions {

        if let Some(aspect) = aspect {
            let desired_ratio: f64 = aspect.h as f64 / aspect.v as f64;

            let adjusted_h = (res.w as f64 / desired_ratio) as u32; // Result should be slightly larger than integer, ok to cast
    
            return (res.w, adjusted_h).into()
        }
        
        (res.w, res.h).into()
    }

    pub fn get_mode_byte(&self) -> u8 {
        self.mode_byte
    }

    pub fn set_mode_byte(&mut self, byte: u8) {
        self.mode_byte = byte;
    }

    pub fn screenshot_with_backend(&mut self, path: &Path) {
        // Find first unique filename in screenshot dir
        let filename = file_util::find_unique_filename(path, "screenshot", ".png");

        // Take the buffer closure out of self so we can call a closure that binds self
        if let Some(mut with_buffer) = self.on_with_buffer.take() {
            //let mut backend = self.backend.lock().expect("Failed to lock backend");
            with_buffer(&mut |buffer: &mut [u8]| {
                let frame_slice = &buffer[0..(self.params.aspect_w as usize * self.params.aspect_h as usize * std::mem::size_of::<u32>())];
                match image::save_buffer(
                    filename.clone(),
                    frame_slice,
                    self.params.aspect_w,
                    self.params.aspect_h, 
                    image::ColorType::Rgba8) 
                {
                    Ok(_) => println!("Saved screenshot: {}", filename.display()),
                    Err(e) => {
                        println!("Error writing screenshot: {}: {}", filename.display(), e)
                    }
                }
            });
            // Put closure back
            self.on_with_buffer.replace(with_buffer);
        }        
    }

    pub fn screenshot(
        &self,
        frame: &[u8],
        path: &Path) 
    {

        // Find first unique filename in screenshot dir
        let filename = file_util::find_unique_filename(path, "screenshot", ".png");

        let frame_slice = &frame[0..(self.params.aspect_w as usize * self.params.aspect_h as usize * std::mem::size_of::<u32>())];

        match image::save_buffer(
            filename.clone(),
            frame_slice,
            self.params.aspect_w,
            self.params.aspect_h, 
            image::ColorType::Rgba8) 
        {
            Ok(_) => println!("Saved screenshot: {}", filename.display()),
            Err(e) => {
                println!("Error writing screenshot: {}: {}", filename.display(), e)
            }
        }
    }
}