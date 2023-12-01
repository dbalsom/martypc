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

    videocard_renderer::mod.rs

    This module implements various Video rendering functions. Video card devices
    marty_render in either Direct or Indirect mode.

    In Direct mode, the video device draws directly to a framebuffer in an
    intermediate representation, which the marty_render module converts and displays.

    In Indirect mode, the marty_render module draws the video device's VRAM directly.
    This is fast, but not always accurate if register writes happen mid-frame.
*/

#![allow(dead_code)]
#![allow(clippy::identity_op)] // Adding 0 lines things up nicely for formatting.

use std::path::Path;
use std::mem::size_of;
use std::sync::{Arc, Mutex};
use std::marker::PhantomData;

pub use display_backend_trait::DisplayBackend;
use marty_core::videocard::RenderBpp;

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

use image;
use log;
pub use marty_pixels_scaler::{ScalerMode, ScalerOption, ScalerFilter, DisplayScaler};

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

#[derive (Copy, Clone, Default, PartialEq)]
pub struct VideoDimensions {
    pub w: u32,
    pub h: u32
}

impl From<(u32, u32)> for VideoDimensions {
    fn from(t: (u32, u32)) -> Self {
        VideoDimensions { w: t.0, h: t.1 }
    }
}

impl VideoDimensions {
    pub fn has_some_size(&self) -> bool {
        self.w > 0 && self.h > 0
    }
}



#[derive (Copy, Clone, Default)]
#[derive(PartialEq)]
pub enum AspectCorrectionMode {
    #[default]
    None,
    Software,
    Hardware
}

#[derive (Copy, Clone)]
pub struct VideoParams {
    pub render: VideoDimensions,                    // The size of the internal marty_render buffer before aspect correction.
    pub aspect_corrected: VideoDimensions,          // The size of the internal marty_render buffer after aspect correction.
    pub backend: VideoDimensions,                   // The size of the backend buffer.
    pub surface: VideoDimensions,                   // The size of the backend surface (window client area)
    pub double_scan: bool,                          // Whether to double rows when rendering into the internal buffer.
    pub aspect_correction: AspectCorrectionMode,    // Determines how to handle aspect correction.
    pub composite_params: CompositeParams,          // Parameters used for composite emulation.
    pub bpp: RenderBpp,
}

impl Default for VideoParams {
    fn default() -> VideoParams {
        VideoParams {
            render: (DEFAULT_RENDER_WIDTH, DEFAULT_RENDER_HEIGHT).into(),
            aspect_corrected: (640, 480).into(),
            backend: (640, 480).into(),
            surface: (640, 480).into(),
            double_scan: false,
            aspect_correction: AspectCorrectionMode::None,
            composite_params: Default::default(),
            bpp: Default::default(),
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
    pub filter: ScalerFilter,
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
            filter: ScalerFilter::Linear,
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

#[derive (Copy, Clone, Debug, Default, PartialEq)]
pub enum PhosphorType {
    #[default]
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

pub struct VideoRenderer {
    video_type: VideoType,
    mode: DisplayMode,
    params: VideoParams,

    buf: Vec<u8>,
    aspect_ratio: Option<AspectRatio>,
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
}

impl VideoRenderer {
    pub fn new(
        video_type: VideoType,
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
            mode: DisplayMode::Mode3TextCo80,
            params: Default::default(),

            buf: vec![0; (DEFAULT_RENDER_WIDTH * DEFAULT_RENDER_HEIGHT * 4) as usize],
            aspect_ratio: None,

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
        }
    }

    pub fn get_params(&self) -> &VideoParams {
        &self.params
    }

    /*
    pub fn resize_scaler(
        &mut self,
        backend: &mut Backend,
        buf: VideoDimensions,
        target: VideoDimensions,
        surface: VideoDimensions) {

        if buf.has_some_size() && surface.has_some_size() {
            log::debug!(
                "Resizing scaler to texture {}x{}, target: {}x{}, surface: {}x{}...",
                buf.w, buf.h,
                target.w, target.h,
                surface.w, surface.h,
            );

            self.scaler.resize(
                backend.get_backend_raw().unwrap(),
                buf.w, buf.h,
                target.w, target.h,
                surface.w, surface.h
            );
        }
    }
     */

    /*
    pub fn set_scaler_margin(&mut self, l: u32, r: u32, t: u32, b: u32) {
        if let Some(ref mut margin_callback) = self.on_margin {
            let mut scaler = self.scaler.lock().expect("Failed to lock scaler");
            margin_callback(&mut *scaler, l, r, t, b);
        }     
    }

     */

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

    /*

    pub fn get_backend(&self) -> Arc<Mutex<T>> {
        self.backend.clone()
    }
     */

    /*
    pub fn has_gui(&self) -> bool {
        self.has_gui
    }

     */

    /// Resizes the internal rendering buffer to the specified dimensions, before aspect correction.
    pub fn resize(&mut self, new: VideoDimensions ) {

        self.params.render = new;

        let mut new_aspect = self.params.render;
        if let Some(_) = self.aspect_ratio {
            new_aspect = VideoRenderer::get_aspect_corrected_res(new, self.aspect_ratio);
        }

        match self.params.aspect_correction {
            AspectCorrectionMode::None => {
                self.params.aspect_corrected = self.params.render;
                self.params.backend = self.params.render;
            }
            AspectCorrectionMode::Software => {
                // For software aspect correction, we must ensure the backend buffer is large enough
                // to receive the aspect-corrected image.
                self.params.aspect_corrected = new_aspect;
                self.params.backend = new_aspect;
            }
            AspectCorrectionMode::Hardware => {
                // For hardware aspect correction, the backend dimensions remain the same as native
                // marty_render resolution and the aspect corrected dimensions are used as a target for
                // the vertex shader only.
                self.params.aspect_corrected = new_aspect;
                self.params.backend = self.params.render;
            }
        }

        // Only resize the internal render buffer if size has changed.
        if self.params.render != new {
            self.buf.resize((new.w * new.h * 4) as usize, 0);
            self.buf.fill(0);
        }
    }

    // Given the new specified dimensions, returns a bool if the dimensions require resizing
    // the internal buffer.
    pub fn would_resize(&self, new: VideoDimensions) -> bool {

        match self.params.aspect_correction {
            AspectCorrectionMode::None | AspectCorrectionMode::Hardware => {
                if self.params.render != new {
                    return true
                }
            }
            AspectCorrectionMode::Software => {
                let new_aspect = VideoRenderer::get_aspect_corrected_res(new, self.aspect_ratio);
                if self.params.aspect_corrected != new_aspect {
                    return true
                }
            }
        }
        false
    }
    
    pub fn get_buf_dimensions(&mut self) -> VideoDimensions {
        self.params.render
    }

    pub fn get_display_dimensions(&mut self) -> VideoDimensions {
        match self.params.aspect_correction {
            AspectCorrectionMode::None | AspectCorrectionMode::Hardware => {
                self.params.render
            }
            AspectCorrectionMode::Software => {
                self.params.aspect_corrected
            }
        }
    }

    pub fn set_aspect_ratio(&mut self, new_aspect: Option<AspectRatio>) {

        if let Some(aspect) = new_aspect {
            if self.aspect_ratio != new_aspect {
                // Aspect ratio is changing.
                log::debug!("set_aspect_ratio(): Updating aspect ratio.");
                let desired_ratio: f64 = aspect.h as f64 / aspect.v as f64;
                let adjusted_h = (self.params.render.w as f64 / desired_ratio) as u32;
    
                self.params.aspect_corrected.h = adjusted_h;
                self.aspect_ratio = Some(aspect);
            }
        }
        else {
            // Disable aspect correction
            self.params.aspect_corrected = self.params.render;
            self.aspect_ratio = None;
        }
        /*
        let new_backend;

        match self.params.aspect_correction {
            AspectCorrectionMode::None | AspectCorrectionMode::Hardware => {
                new_backend = self.params.render;
            }
            AspectCorrectionMode::Software => {
                new_backend = self.params.aspect_corrected;
            }
        }


        // Are backend dimensions changing due to aspect change? If so, resize the backend.
        if self.params.backend != new_backend {
            self.params.backend = new_backend;

            log::debug!(
                    "VideoRenderer: Resizing backend due to aspect ratio change. New dimensions: {}x{}",
                    self.params.backend.w,
                    self.params.backend.h,
                );

            // Resize backend via closure
            if let Some(ref mut resize_callback) = self.on_resize {
                // Lock the backend and pass the mutable reference to the closure
                let mut backend = self.backend.lock().expect("Failed to lock backend");
                resize_callback(&mut *backend, self.params.backend);
            }
        }

        // Resize scaler
        self.backend_resize_scaler(
            self.params.render,
            self.params.aspect_corrected,
            self.params.surface
        );
         */
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

    pub fn screenshot_with_backend(&mut self, _path: &Path) {
        // Find first unique filename in screenshot dir

        /*
        let filename = file_util::find_unique_filename(path, "screenshot", ".png");


        // Take the buffer closure out of self so we can call a closure that binds self
        if let Some(mut with_buffer) = self.on_with_buffer.take() {
            with_buffer(&mut |buffer: &mut [u8]| {
                let frame_slice = &buffer[0..(self.params.backend.w as usize * self.params.backend.h as usize * std::mem::size_of::<u32>())];
                match image::save_buffer(
                    filename.clone(),
                    frame_slice,
                    self.params.backend.w,
                    self.params.backend.h,
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
        */
    }

    pub fn screenshot(
        &self,
        frame: &[u8],
        path: &Path) 
    {

        // Find first unique filename in screenshot dir
        let filename = file_util::find_unique_filename(path, "screenshot", ".png");

        let frame_slice = &frame[0..(self.params.backend.w as usize * self.params.backend.h as usize * std::mem::size_of::<u32>())];

        match image::save_buffer(
            filename.clone(),
            frame_slice,
            self.params.backend.w,
            self.params.backend.h,
            image::ColorType::Rgba8) 
        {
            Ok(_) => println!("Saved screenshot: {}", filename.display()),
            Err(e) => {
                println!("Error writing screenshot: {}: {}", filename.display(), e)
            }
        }
    }
}