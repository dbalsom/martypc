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

use marty_core::devices::cga;
use std::{collections::VecDeque, mem::size_of, path::Path};

use web_time::Duration;

use image;
use log;

use composite_new::{ReCompositeBuffers, ReCompositeContext};
pub use display_backend_trait::DisplayBackend;
use marty_common::VideoDimensions;
use marty_core::device_traits::videocard::{
    BufferSelect,
    CGAColor,
    CGAPalette,
    DisplayApertureType,
    DisplayExtents,
    DisplayMode,
    RenderBpp,
    VideoType,
};
use serde::Deserialize;

// Re-export submodules
pub use self::{color::*, composite::*, consts::*, resize::*};

pub mod color;
pub mod composite;
pub mod consts;
pub mod draw;
pub mod resize;
// Reenigne composite
pub mod composite_new;

/// Events that the renderer can return. These must be read and handled every frame to avoid
/// memory leaks.
#[derive(Copy, Clone, Debug)]
pub enum RendererEvent {
    ScreenshotSaved,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub enum AspectCorrectionMode {
    #[default]
    None,
    Software,
    Hardware,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct RendererConfigParams {
    #[serde(default)]
    pub aspect_correction: bool,
    pub aspect_ratio: Option<AspectRatio>,
    pub display_aperture: Option<DisplayApertureType>,
    #[serde(default)]
    pub composite: bool,
}

#[derive(Copy, Clone)]
pub struct VideoParams {
    pub render: VideoDimensions, // The size of the internal marty_render buffer before aspect correction.
    pub aspect_corrected: VideoDimensions, // The size of the internal marty_render buffer after aspect correction.
    pub backend: VideoDimensions, // The size of the backend buffer.
    pub surface: VideoDimensions, // The size of the backend surface (window client area)
    pub line_double: bool,       // Whether to double rows when rendering into the internal buffer.
    pub aspect_correction: AspectCorrectionMode, // Determines how to handle aspect correction.
    pub aperture: DisplayApertureType, // Selected display aperture for renderer
    pub debug_aperture: bool,
    pub composite_params: CompositeParams, // Parameters used for composite emulation.
    pub bpp: RenderBpp,
}

impl Default for VideoParams {
    fn default() -> VideoParams {
        VideoParams {
            render: (DEFAULT_RENDER_WIDTH, DEFAULT_RENDER_HEIGHT).into(),
            aspect_corrected: (640, 480).into(),
            backend: (640, 480).into(),
            surface: (640, 480).into(),
            line_double: false,
            aspect_correction: AspectCorrectionMode::None,
            aperture: DisplayApertureType::Cropped,
            debug_aperture: false,
            composite_params: Default::default(),
            bpp: Default::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize)]
pub struct AspectRatio {
    pub h: u32,
    pub v: u32,
}

impl Default for AspectRatio {
    fn default() -> Self {
        Self { h: 4, v: 3 }
    }
}

impl AspectRatio {
    pub fn is_square(&self) -> bool {
        self.h == 1 && self.v == 1
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CompositeParams {
    pub phase: usize,
    pub contrast: f64,
    pub hue: f64,
    pub sat: f64,
    pub luma: f64,
    pub new_cga: bool,
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

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum PhosphorType {
    #[default]
    Color,
    White,
    Green,
    Amber,
}

#[derive(Copy, Clone)]
pub enum RenderColor {
    CgaIndex(u8),
    Rgb(u8, u8, u8),
}

#[derive(Copy, Clone)]
pub struct DebugRenderParams {
    pub draw_scanline: Option<u32>,
    pub draw_scanline_color: Option<RenderColor>,
}

pub struct VideoRenderer {
    video_type: VideoType,
    mode: DisplayMode,
    params: VideoParams,
    initialized: bool, // Has the renderer received a resize event?

    buf: Vec<u8>,
    aspect_ratio: Option<AspectRatio>,
    aspect_dirty: bool,
    aperture_dirty: bool,
    mode_byte: u8,

    // Legacy composite stuff
    composite_buf: Option<Vec<u8>>,
    sync_table_w:  u32,
    sync_table:    Vec<(f32, f32, f32)>,

    // Reenigne composite stuff
    composite_ctx:  ReCompositeContext,
    composite_bufs: ReCompositeBuffers,
    last_cga_mode:  u8,

    // Composite adjustments
    composite_enabled: bool,
    composite_params:  CompositeParams,
    resample_context:  ResampleContext,

    buffer_select: BufferSelect,

    screenshot_buf: Vec<u8>,
    screenshot_path: Option<std::path::PathBuf>,
    screenshot_requested: bool,

    last_render_time: Duration,
    event_queue: VecDeque<RendererEvent>,
}

impl VideoRenderer {
    pub fn new(video_type: VideoType) -> Self {
        // Create a buffer to hold composite conversion of CGA graphics.
        // This buffer will need to be twice as large as the largest possible
        // CGA screen (CGA_MAX_CLOCK * 4) to account for half-hdots used in the
        // composite conversion process.
        //
        // TODO: This is only used by the legacy composite code. Remove?
        let composite_vec_opt = match video_type {
            VideoType::CGA => Some(vec![0; cga::CGA_MAX_CLOCK * 4]),
            _ => None,
        };

        Self {
            video_type,
            mode: DisplayMode::Mode3TextCo80,
            params: Default::default(),
            initialized: false,

            buf: vec![0; (DEFAULT_RENDER_WIDTH * DEFAULT_RENDER_HEIGHT * 4) as usize],
            aspect_ratio: None,
            aspect_dirty: false,
            aperture_dirty: false,
            mode_byte: 0,

            // Legacy composite stuff
            composite_buf: composite_vec_opt,
            sync_table_w: 0,
            sync_table: Vec::new(),

            // Reenigne composite stuff
            composite_ctx: ReCompositeContext::new(),
            composite_bufs: ReCompositeBuffers::new(),
            last_cga_mode: 0,

            composite_enabled: false,
            composite_params: Default::default(),
            resample_context: ResampleContext::new(),

            buffer_select: BufferSelect::Front,

            screenshot_buf: Vec::new(),
            screenshot_path: None,
            screenshot_requested: false,

            last_render_time: Duration::from_secs(0),
            event_queue: VecDeque::new(),
        }
    }

    pub fn get_event(&mut self) -> Option<RendererEvent> {
        self.event_queue.pop_front()
    }

    pub fn send_event(&mut self, event: RendererEvent) {
        self.event_queue.push_back(event);
    }

    pub fn get_last_render_time(&self) -> Duration {
        self.last_render_time
    }

    pub fn set_config_params(&mut self, cfg: &RendererConfigParams) {
        self.composite_enabled = cfg.composite;

        if cfg.aspect_correction {
            self.set_aspect_ratio(cfg.aspect_ratio, Some(AspectCorrectionMode::Hardware));
        }
        else {
            self.set_aspect_ratio(None, Some(AspectCorrectionMode::Hardware));
        }

        self.set_aperture(cfg.display_aperture.unwrap_or(DisplayApertureType::Cropped));
    }

    pub fn get_config_params(&self) -> RendererConfigParams {
        RendererConfigParams {
            aspect_correction: if self.aspect_ratio.is_some() { true } else { false },
            aspect_ratio: self.aspect_ratio,
            display_aperture: Some(self.params.aperture),
            composite: self.composite_enabled,
        }
    }
    pub fn get_params(&self) -> &VideoParams {
        &self.params
    }

    pub fn select_buffer(&mut self, selection: BufferSelect) {
        self.buffer_select = selection;
    }

    #[inline]
    pub fn get_selected_buffer(&self) -> BufferSelect {
        self.buffer_select
    }

    pub fn set_composite(&mut self, state: bool) {
        log::debug!("Setting composite rendering to {}", state);
        self.composite_enabled = state;
    }

    pub fn set_aperture(&mut self, aperture: DisplayApertureType) {
        log::debug!("Setting renderer aperture to {:?}", aperture);
        self.params.aperture = aperture;
        self.aperture_dirty = true;
    }

    pub fn set_debug(&mut self, state: bool) {
        self.params.debug_aperture = state;
    }

    pub fn set_line_double(&mut self, state: bool) {
        self.params.line_double = state;
    }

    /// Resizes the internal rendering buffer to the specified dimensions, before aspect correction.
    pub fn resize(&mut self, new_dims: VideoDimensions) {
        self.initialized = true;

        let mut new_aspect_corrected_dims = self.params.render;
        if let Some(_) = self.aspect_ratio {
            new_aspect_corrected_dims = VideoRenderer::get_aspect_corrected_res(new_dims, self.aspect_ratio);
        }

        match self.params.aspect_correction {
            AspectCorrectionMode::None => {
                self.params.aspect_corrected = new_dims;
                self.params.backend = self.params.render;
            }
            AspectCorrectionMode::Software => {
                // For software aspect correction, we must ensure the backend buffer is large enough
                // to receive the aspect-corrected image.
                self.params.aspect_corrected = new_aspect_corrected_dims;
                self.params.backend = new_aspect_corrected_dims;
            }
            AspectCorrectionMode::Hardware => {
                // For hardware aspect correction, the backend dimensions remain the same as native
                // marty_render resolution and the aspect corrected dimensions are used as a target for
                // the vertex shader only.
                self.params.aspect_corrected = new_aspect_corrected_dims;
                self.params.backend = new_dims;
            }
        }

        // Only resize the internal render buffer if size has changed.
        if self.params.render != new_dims {
            self.buf.resize((new_dims.w * new_dims.h * 4) as usize, 0);
            self.buf.fill(0);
            self.params.render = new_dims;
        }
    }

    // Given the new specified dimensions, returns a bool if the dimensions require resizing
    // the internal buffer. This should be called before actually resizing the renderer.
    pub fn would_resize(&mut self, new: VideoDimensions) -> bool {
        if !self.initialized {
            return true;
        }
        if self.aspect_dirty {
            log::debug!("would_resize(): aspect ratio change detected.");
            self.aspect_dirty = false;
            return true;
        }
        if self.aperture_dirty {
            log::debug!("would_resize(): aperture change detected.");
            self.aperture_dirty = false;
            return true;
        }

        match self.params.aspect_correction {
            AspectCorrectionMode::None | AspectCorrectionMode::Hardware => {
                if self.params.render != new {
                    return true;
                }
            }
            AspectCorrectionMode::Software => {
                let new_aspect = VideoRenderer::get_aspect_corrected_res(new, self.aspect_ratio);
                if self.params.aspect_corrected != new_aspect {
                    return true;
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
            AspectCorrectionMode::None => self.params.render,
            AspectCorrectionMode::Hardware | AspectCorrectionMode::Software => self.params.aspect_corrected,
        }
    }

    pub fn set_aspect_ratio(&mut self, new_aspect: Option<AspectRatio>, new_mode: Option<AspectCorrectionMode>) {
        if let Some(aspect) = new_aspect {
            if self.aspect_ratio != new_aspect {
                // Aspect ratio is changing.
                let desired_ratio: f64 = aspect.h as f64 / aspect.v as f64;
                let adjusted_h = (self.params.render.w as f64 / desired_ratio) as u32;
                log::debug!(
                    "set_aspect_ratio(): Updating aspect ratio to {:?} aspect dim: {}x{}",
                    aspect,
                    self.params.render.w,
                    adjusted_h
                );
                self.params.aspect_corrected.w = self.params.render.w;
                self.params.aspect_corrected.h = adjusted_h;
                self.aspect_ratio = Some(aspect);
                self.aspect_dirty = true;
            }
        }
        else {
            // Disable aspect correction
            log::debug!("set_aspect_ratio(): Disabling aspect correction.");
            self.params.aspect_corrected = self.params.render;
            self.aspect_ratio = None;
            self.aspect_dirty = true;
        }

        if let Some(mode) = new_mode {
            self.params.aspect_correction = mode;
            self.aspect_dirty = true;
        }
    }

    /// Given the specified resolution and desired aspect ratio, return an aspect corrected resolution
    /// by adjusting the vertical resolution (Horizontal resolution will never be changed)
    pub fn get_aspect_corrected_res(res: VideoDimensions, aspect: Option<AspectRatio>) -> VideoDimensions {
        if let Some(aspect) = aspect {
            let desired_ratio: f64 = aspect.h as f64 / aspect.v as f64;

            let adjusted_h = (res.w as f64 / desired_ratio) as u32; // Result should be slightly larger than integer, ok to cast

            return (res.w, adjusted_h).into();
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

    pub fn screenshot(&self, frame: &[u8], path: &Path) {
        let frame_slice =
            &frame[0..(self.params.backend.w as usize * self.params.backend.h as usize * std::mem::size_of::<u32>())];

        match image::save_buffer(
            path,
            frame_slice,
            self.params.backend.w,
            self.params.backend.h,
            image::ColorType::Rgba8,
        ) {
            Ok(_) => println!("Saved screenshot: {}", path.display()),
            Err(e) => {
                println!("Error writing screenshot: {}: {}", path.display(), e)
            }
        }
    }

    /// Request a screenshot be taken on next render pass. The screenshot will be saved to the specified path.
    /// This is deferred to the next rendering pass for simplicity so we don't have to retrieve backend
    /// or card buffers when requesting a screenshot.
    pub fn request_screenshot(&mut self, path: &Path) {
        self.screenshot_buf = vec![
            0;
            (self.params.backend.w as usize * self.params.backend.h as usize * std::mem::size_of::<u32>())
                as usize
        ];
        self.screenshot_path = Some(path.to_path_buf());
        self.screenshot_requested = true;
    }

    pub fn render_screenshot(&self, frame: &[u8], path: &Path) {
        let frame_slice =
            &frame[0..(self.params.backend.w as usize * self.params.backend.h as usize * std::mem::size_of::<u32>())];

        /*
        self.draw(
            &mut self,
            input_buf: &[u8],
            output_buf: &mut [u8],
            extents: &DisplayExtents,
            beam_pos: Option<(u32, u32)>,
        )
         */

        match image::save_buffer(
            path,
            frame_slice,
            self.params.backend.w,
            self.params.backend.h,
            image::ColorType::Rgba8,
        ) {
            Ok(_) => println!("Saved screenshot: {}", path.display()),
            Err(e) => {
                println!("Error writing screenshot: {}: {}", path.display(), e)
            }
        }
    }
}
