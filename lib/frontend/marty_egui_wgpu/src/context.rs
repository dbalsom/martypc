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

    marty_egui::context.rs

    EGUI Render context
*/

use egui::{ClippedPrimitive, Context, TexturesDelta, ViewportId};
use egui_extras::install_image_loaders;
use egui_wgpu::{Renderer, ScreenDescriptor};
use frontend_common::display_manager::DmGuiOptions;
use marty_egui::{
    state::GuiState,
    themes::{make_theme, GuiTheme},
};
use web_time::{Duration, Instant};
use wgpu_wrapper::{wgpu, wrapper::raw_window_handle, PixelsContext};
use winit::window::Window;

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
    main_theme: Box<dyn GuiTheme>,
    menu_theme: Box<dyn GuiTheme>,
    render_time: Duration,
}

impl GuiRenderContext {
    /// Create egui.
    pub fn new(
        dt_idx: usize,
        width: u32,
        height: u32,
        scale_factor: f64,
        pixels: &wgpu_wrapper::Pixels,
        window: &Window,
        gui_options: &DmGuiOptions,
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
            window as &dyn raw_window_handle::HasDisplayHandle,
            Some(scale_factor as f32),
            None,
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

        let renderer = Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1, false);
        let textures = TexturesDelta::default();

        // Resolve themes.
        let gui_theme_enum = gui_options.theme.unwrap_or_default();
        let menu_theme_enum = gui_options.menu_theme.unwrap_or(gui_theme_enum);
        let main_theme = make_theme(gui_theme_enum);
        let menu_theme = make_theme(menu_theme_enum);

        // Make header smaller.
        use egui::{FontFamily::Proportional, FontId, TextStyle::*};
        let mut style = (*egui_ctx.style()).clone();

        style.text_styles.entry(Heading).and_modify(|text_style| {
            *text_style = FontId::new(14.0, Proportional);
        });

        egui_ctx.set_style(style);

        // if let Some(color) = gui_options.theme_color {
        //     let theme = GuiTheme::new(&visuals, crate::color::hex_to_c32(color));
        //     egui_ctx.set_visuals(theme.visuals().clone());
        // }
        // else {
        //     egui_ctx.set_visuals(visuals);
        // }

        egui_ctx.set_visuals(main_theme.visuals());

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
            main_theme,
            menu_theme,
            render_time: Duration::ZERO,
        };

        //slf.resize(width, height);
        slf
    }

    pub fn get_render_time(&self) -> Duration {
        self.render_time
    }

    pub fn set_zoom_factor(&mut self, zoom: f32) {
        self.egui_ctx.set_zoom_factor(zoom);
    }

    pub fn has_focus(&self) -> bool {
        match self.egui_ctx.memory(|m| m.focused()) {
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
            .expect("Failed to get ROOT viewport!")
    }

    /// Prepare egui.
    pub fn prepare(&mut self, window: &Window, state: &mut GuiState) {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let gui_start = Instant::now();

            let ctx = self.egui_ctx.clone();
            let vpi = self.viewport_mut();
            egui_winit::update_viewport_info(vpi, &ctx, window, true);
            let raw_input = self.egui_state.take_egui_input(window);

            let mut ran = false;
            let output = self.egui_ctx.run(raw_input, |egui_ctx| {
                // Draw the application.
                self.egui_ctx.set_visuals(self.menu_theme.visuals());
                state.menu_ui(egui_ctx);
                self.egui_ctx.set_visuals(self.main_theme.visuals());
                state.ui(egui_ctx);
                ran = true;
            });

            if ran {
                self.textures.append(output.textures_delta);
                self.egui_state.handle_platform_output(window, output.platform_output);

                //let ppp = output.pixels_per_point;
                let ppp = egui_winit::pixels_per_point(&ctx, window);
                //log::debug!("Tessellate with ppp: {}", ppp);
                self.paint_jobs = self.egui_ctx.tessellate(output.shapes, ppp);
                //state.perf_stats.gui_time = gui_start.elapsed();
            }
        }
    }

    /// Render egui.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext,
    ) {
        let gui_render_start = Instant::now();

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

        self.render_time = gui_render_start.elapsed();
    }
}
