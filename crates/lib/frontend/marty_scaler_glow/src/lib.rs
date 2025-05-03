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
*/

use bytemuck::{Pod, Zeroable};
use std::default::Default;

// Reexport trait items
pub use marty_frontend_common::color::MartyColor;

use marty_display_common::display_scaler::{
    DisplayScaler,
    ScalerEffect,
    ScalerFilter,
    ScalerGeometry,
    ScalerMode,
    ScalerOption,
};

use eframe::{
    glow,
    glow::{Context, HasContext, NativeTexture, Program, UniformLocation, VertexArray},
};
use ultraviolet::Mat4;

/// A logical texture size for a window surface.
#[derive(Debug)]
pub struct SurfaceSize {
    pub width:  u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CrtParamUniform {
    h_curvature: f32,
    v_curvature: f32,
    corner_radius: f32,
    scanlines: u32,
    gamma: f32,
    brightness: f32,
    contrast: f32,
    mono: u32,
    mono_color: [f32; 4],
}

impl Default for CrtParamUniform {
    fn default() -> Self {
        Self {
            h_curvature: 0.0,
            v_curvature: 0.0,
            corner_radius: 0.0,
            scanlines: 0,
            gamma: 1.0,
            brightness: 1.0,
            contrast: 1.0,
            mono: 0,
            mono_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Logical pixel coordinates
    /// (0,0) is the top left corner of the screen
    pub pos: [f32; 2], // 64 bit

    /// sRGBA with premultiplied alpha
    //pub color: u32, // 32 bit

    /// Normalized texture coordinates.
    /// (0, 0) is the top left corner of the texture
    /// (1, 1) is the bottom right corner of the texture
    pub uv: [f32; 2],
}

#[derive(Copy, Clone, Debug)]
struct ScalingMatrix {
    transform: Mat4,
}
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
struct ScalerOptionsUniform {
    mode: u32,
    hres: u32,
    vres: u32,
    pad2: u32,
    crt_params: CrtParamUniform,
    fill_color: [f32; 4],
    texture_order: u32,
    _padding: [u32; 3], // 12 bytes to pad struct to 96 bytes
}

pub struct MartyScaler {
    mode: ScalerMode,

    program: Program,
    vertex_array: VertexArray,
    // texture: NativeTexture,
    // u_transform: UniformLocation,
    // u_texture: UniformLocation,
    // transform: [f32; 16],
    //
    screen_size: (f32, f32),
    target_size: (f32, f32),
    texture_size: (f32, f32),
    margin_y: f32,
    //
    // bilinear: bool,
    // //fill_color: wgpu::Color,
    // margin_l: u32,
    // margin_r: u32,
    // margin_t: u32,
    // margin_b: u32,
    //
    // brightness: f32,
    // contrast: f32,
    // gamma: f32,
    //
    // scanlines: u32,
    // do_scanlines: bool,
    // h_curvature: f32,
    // v_curvature: f32,
    // corner_radius: f32,
    // mono: bool,
    // //mono_color: wgpu::Color,
    // #[allow(dead_code)]
    // effect: ScalerEffect,
    // #[allow(dead_code)]
    // crt_params: CrtParamUniform,
    // texture_order: u32,
}

impl MartyScaler {
    pub fn new(
        gl: &Context,
        texture_size: (f32, f32),
        target_size: (f32, f32),
        screen_size: (f32, f32),
        margin_y: f32,
        mode: ScalerMode,
    ) -> Self {
        let shader_version = if cfg!(target_arch = "wasm32") {
            "#version 300 es"
        }
        else {
            "#version 330"
        };

        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let (vertex_shader_source, fragment_shader_source) = (
                r#"
                    const vec2 verts[3] = vec2[3](
                        vec2(0.0, 1.0),
                        vec2(-1.0, -1.0),
                        vec2(1.0, -1.0)
                    );
                    const vec4 colors[3] = vec4[3](
                        vec4(1.0, 0.0, 0.0, 1.0),
                        vec4(0.0, 1.0, 0.0, 1.0),
                        vec4(0.0, 0.0, 1.0, 1.0)
                    );
                    out vec4 v_color;
                    void main() {
                        v_color = colors[gl_VertexID];
                        gl_Position = vec4(verts[gl_VertexID], 0.0, 1.0);
                        gl_Position.x *= cos(0.0);
                    }
                "#,
                r#"
                    precision mediump float;
                    in vec4 v_color;
                    out vec4 out_color;
                    void main() {
                        out_color = v_color;
                    }
                "#,
            );

            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl.create_shader(*shader_type).expect("Cannot create shader");
                    gl.shader_source(shader, &format!("{shader_version}\n{shader_source}"));
                    gl.compile_shader(shader);
                    assert!(
                        gl.get_shader_compile_status(shader),
                        "Failed to compile {shader_type}: {}",
                        gl.get_shader_info_log(shader)
                    );
                    gl.attach_shader(program, shader);
                    shader
                })
                .collect();

            gl.link_program(program);
            assert!(
                gl.get_program_link_status(program),
                "{}",
                gl.get_program_info_log(program)
            );

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let vertex_array = gl.create_vertex_array().expect("Cannot create vertex array");

            Self {
                mode,
                program,
                vertex_array,
                screen_size,
                target_size,
                texture_size,
                margin_y,
            }
        }
    }

    fn compute_transform(
        mode: ScalerMode,
        texture: (f32, f32),
        target: (f32, f32),
        screen: (f32, f32),
        margin_y: f32,
    ) -> [f32; 16] {
        let (tw, th) = texture;
        let (tw_out, th_out) = target;
        let (sw, sh) = screen;
        let margin_ndc = margin_y / (sh / 2.0);
        let mut sw_f = 1.0;
        let mut sh_f = 1.0;

        match mode {
            ScalerMode::Null | ScalerMode::Fixed => {
                sw_f = tw / sw;
                sh_f = th_out / sh;
            }
            ScalerMode::Integer => {
                let scale = (sw / tw).min((sh - margin_y) / th_out).floor();
                sw_f = (tw * scale) / sw;
                sh_f = (th_out * scale) / sh;
            }
            ScalerMode::Fit | ScalerMode::Windowed => {
                let scale = (sw / tw).min((sh - margin_y) / th_out);
                sw_f = (tw * scale) / sw;
                sh_f = (th_out * scale) / sh;
            }
            ScalerMode::Stretch => {
                sw_f = 1.0;
                sh_f = (sh - margin_y) / sh;
            }
        }

        let tx = (sw_f / 2.0).fract();
        let ty = (sh_f / 2.0).fract() - margin_ndc / 2.0;

        [
            sw_f, 0.0, 0.0, 0.0, 0.0, sh_f, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0,
        ]
    }
}

impl DisplayScaler<Context, (), ()> for MartyScaler {
    type NativeContext = Context;
    type NativeRenderPass = ();
    type NativeTextureView = ();
    type NativeEncoder = ();

    fn texture_view(&self) -> &() {
        &()
    }

    fn render(&self, _encoder: &mut (), _render_target: &()) {
        // Glow does not use an encoder
    }

    fn render_with_context(&self, gl: &Context) {
        use glow::HasContext as _;
        unsafe {
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
    }

    fn render_with_renderpass(&self, _render_pass: &mut Self::NativeRenderPass) {
        // Glow does not use renderpass
    }

    fn resize(
        &mut self,
        device: &Context,
        queue: &(),
        texture: &(),
        texture_width: u32,
        texture_height: u32,
        target_width: u32,
        target_height: u32,
        screen_width: u32,
        screen_height: u32,
    ) {
        // //self.texture_view = create_texture_view(pixels, self.texture_width, self.texture_height);
        // self.texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // self.nearest_bind_group = create_bind_group(
        //     device,
        //     &self.bind_group_layout,
        //     &self.texture_view,
        //     &self.nearest_sampler,
        //     &self.transform_uniform_buffer,
        //     &self.params_uniform_buffer,
        // );
        //
        // self.bilinear_bind_group = create_bind_group(
        //     device,
        //     &self.bind_group_layout,
        //     &self.texture_view,
        //     &self.bilinear_sampler,
        //     &self.transform_uniform_buffer,
        //     &self.params_uniform_buffer,
        // );
        //
        // //println!("screen_margin_y: {}", self.screen_margin_y);
        // let matrix = ScalingMatrix::new(
        //     self.mode,
        //     (texture_width as f32, texture_height as f32),
        //     (target_width as f32, target_height as f32),
        //     (screen_width as f32, screen_height as f32),
        //     self.screen_margin_y as f32,
        // );
        // let transform_bytes = matrix.as_bytes();
        //
        // self.texture_width = texture_width;
        // self.texture_height = texture_height;
        // self.target_width = target_width;
        // self.target_height = target_height;
        // self.screen_width = screen_width;
        // self.screen_height = screen_height;
        //
        // queue.write_buffer(&self.transform_uniform_buffer, 0, transform_bytes);
    }

    fn resize_surface(&mut self, device: &Context, queue: &(), texture: &(), screen_width: u32, screen_height: u32) {
        // self.texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // self.nearest_bind_group = create_bind_group(
        //     device,
        //     &self.bind_group_layout,
        //     &self.texture_view,
        //     &self.nearest_sampler,
        //     &self.transform_uniform_buffer,
        //     &self.params_uniform_buffer,
        // );
        //
        // self.bilinear_bind_group = create_bind_group(
        //     device,
        //     &self.bind_group_layout,
        //     &self.texture_view,
        //     &self.bilinear_sampler,
        //     &self.transform_uniform_buffer,
        //     &self.params_uniform_buffer,
        // );
        //
        // self.screen_width = screen_width;
        // self.screen_height = screen_height;
        // let matrix = ScalingMatrix::new(
        //     self.mode,
        //     (self.texture_width as f32, self.texture_height as f32),
        //     (self.target_width as f32, self.target_height as f32),
        //     (self.screen_width as f32, self.screen_height as f32),
        //     self.screen_margin_y as f32,
        // );
        // let transform_bytes = matrix.as_bytes();
        //
        // queue.write_buffer(&self.transform_uniform_buffer, 0, transform_bytes);
    }

    fn mode(&self) -> ScalerMode {
        self.mode
    }

    fn set_mode(&mut self, _device: &eframe::glow::Context, queue: &(), new_mode: ScalerMode) {
        //println!(">>> set_mode(): {:?}", new_mode);
        self.mode = new_mode;
        //self.update_matrix(queue);
    }

    fn geometry(&self) -> ScalerGeometry {
        ScalerGeometry {
            texture_w: self.texture_size.0 as u32,
            texture_h: self.texture_size.1 as u32,
            target_w:  self.target_size.0 as u32,
            target_h:  self.target_size.1 as u32,
            surface_w: self.screen_size.0 as u32,
            surface_h: self.screen_size.1 as u32,
        }
    }

    fn set_margins(&mut self, l: u32, r: u32, t: u32, b: u32) {
        // self.margin_l = l;
        // self.margin_r = r;
        // self.margin_t = t;
        // self.margin_b = b;
    }

    fn set_bilinear(&mut self, bilinear: bool) {
        //self.bilinear = bilinear
    }

    fn set_fill_color(&mut self, fill: MartyColor) {
        //self.fill_color = fill.to_wgpu_color();
    }

    /// Apply a ScalerOption. Update of uniform buffers is controlled by the 'update' boolean. If
    /// it is true we will perform an immediate uniform update; if false it will be delayed and
    /// set_option() will return true to indicate that the caller should perform an update.
    fn set_option(&mut self, device: &eframe::glow::Context, queue: &(), opt: ScalerOption, update: bool) -> bool {
        // let mut update_uniform = false;
        //
        // match opt {
        //     ScalerOption::Mode(new_mode) => {
        //         self.set_mode(device, queue, new_mode);
        //     }
        //     ScalerOption::Adjustment { h: _h, s: _s, b, c, g } => {
        //         self.brightness = b;
        //         self.gamma = g;
        //         self.contrast = c;
        //         update_uniform = true;
        //     }
        //     ScalerOption::Filtering(filter) => {
        //         let bilinear;
        //         match filter {
        //             ScalerFilter::Nearest => bilinear = false,
        //             ScalerFilter::Linear => bilinear = true,
        //         }
        //         self.set_bilinear(bilinear);
        //     }
        //     ScalerOption::FillColor { r, g, b, a } => {
        //         self.set_fill_color(MartyColor {
        //             r: r as f32,
        //             g: g as f32,
        //             b: b as f32,
        //             a: a as f32,
        //         });
        //         update_uniform = true;
        //     }
        //     ScalerOption::Geometry {
        //         h_curvature,
        //         v_curvature,
        //         corner_radius,
        //     } => {
        //         self.h_curvature = h_curvature;
        //         self.v_curvature = v_curvature;
        //         self.corner_radius = corner_radius;
        //         update_uniform = true;
        //     }
        //     ScalerOption::Mono { enabled, r, g, b, a } => {
        //         self.mono = enabled;
        //         self.mono_color = wgpu::Color {
        //             r: r as f64,
        //             g: g as f64,
        //             b: b as f64,
        //             a: a as f64,
        //         };
        //         update_uniform = true;
        //     }
        //     ScalerOption::Margins { l, r, t, b } => {
        //         self.set_margins(l, r, t, b);
        //     }
        //     ScalerOption::Scanlines {
        //         enabled,
        //         lines,
        //         intensity: _i,
        //     } => {
        //         self.scanlines = lines.unwrap_or(self.scanlines);
        //         self.do_scanlines = enabled.unwrap_or(self.do_scanlines);
        //         update_uniform = true;
        //     }
        //     ScalerOption::Effect(_) => {}
        // }
        //
        // if update && update_uniform {
        //     self.update_uniforms(queue);
        // }
        // else if update_uniform {
        //     return true;
        // }
        false
    }

    /*
    fn resize_texture(
        &mut self,
        pixels: &pixels::Pixels,
        texture_width: u32,
        texture_height: u32,
    ) {

        //self.texture_view = create_texture_view(pixels, self.screen_width, self.screen_height);
        self.bind_group = create_bind_group(
            pixels.device(),
            &self.bind_group_layout,
            &self.texture_view,
            &self.sampler,
            &self.uniform_buffer,
        );

        let matrix = ScalingMatrix::new(
            (texture_width as f32, texture_height as f32),
            (self.screen_width as f32, self.screen_height as f32),
        );
        let transform_bytes = matrix.as_bytes();

        self.texture_width = texture_width;
        self.texture_height = texture_height;

        pixels
            .queue()
            .write_buffer(&self.uniform_buffer, 0, transform_bytes);
    }
    */

    /// Iterate though a vector of ScalerOptions and apply them all. We can defer uniform update
    /// until all options have been processed.
    fn set_options(&mut self, device: &eframe::glow::Context, queue: &(), opts: Vec<ScalerOption>) {
        let mut update_uniform = false;
        for opt in opts {
            let update_flag = self.set_option(device, queue, opt, false);
            if update_flag {
                update_uniform = true;
            }
        }

        if update_uniform {
            //self.update_uniforms(queue);
        }
    }
}

impl ScalingMatrix {
    // texture_size is the dimensions of the drawing texture
    // screen_size is the dimensions of the surface being drawn to
    fn new(
        mode: ScalerMode,
        texture_size: (f32, f32),
        target_size: (f32, f32),
        screen_size: (f32, f32),
        margin_y: f32,
    ) -> Self {
        match mode {
            ScalerMode::Null | ScalerMode::Fixed => {
                ScalingMatrix::none_matrix(texture_size, target_size, screen_size, margin_y)
            }
            ScalerMode::Integer => ScalingMatrix::integer_matrix(texture_size, target_size, screen_size, margin_y),
            ScalerMode::Fit => ScalingMatrix::fit_matrix(texture_size, target_size, screen_size, margin_y),
            ScalerMode::Stretch => ScalingMatrix::stretch_matrix(texture_size, target_size, screen_size, margin_y),
            ScalerMode::Windowed => ScalingMatrix::fit_matrix(texture_size, target_size, target_size, margin_y),
        }
    }

    fn none_matrix(texture_size: (f32, f32), target_size: (f32, f32), screen_size: (f32, f32), margin_y: f32) -> Self {
        let margin_ndc = margin_y / (screen_size.1 / 2.0);

        let (texture_width, _texture_height) = texture_size;
        let target_height = target_size.1;
        let (screen_width, screen_height) = screen_size;

        // Do not scale
        //let width_ratio = (screen_width / texture_width).max(1.0);
        //let height_ratio = (screen_height / texture_height).max(1.0);

        // Get the smallest scale size
        //let scale = width_ratio.clamp(1.0, height_ratio).floor();

        //let scaled_width = texture_width * scale;
        //let scaled_height = texture_height * scale;

        // Create a transformation matrix
        let sw = texture_width / screen_width;
        let sh = target_height / screen_height;

        let tx_nudge = (screen_width / 2.0).fract() / screen_width;
        let ty_nudge = (screen_height / 2.0).fract() / screen_height;

        let tx = tx_nudge;
        let ty = ty_nudge - margin_ndc / 2.0;

        #[rustfmt::skip]
        let transform: [f32; 16] = [
            sw,  0.0, 0.0, 0.0,
            0.0, sh,  0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            tx,  ty,  0.0, 1.0,
        ];

        // Create a clipping rectangle
        /*
        let clip_rect = {
            let scaled_width = scaled_width.min(screen_width);
            let scaled_height = scaled_height.min(screen_height);
            let x = ((screen_width - scaled_width) / 2.0) as u32;
            let y = ((screen_height - scaled_height) / 2.0) as u32;

            (x, y, scaled_width as u32, scaled_height as u32)
        };
        */

        Self {
            transform: Mat4::from(transform),
            //clip_rect,
        }
    }

    fn integer_matrix(
        texture_size: (f32, f32),
        target_size: (f32, f32),
        screen_size: (f32, f32),
        margin_y: f32,
    ) -> Self {
        let margin_ndc = margin_y / (screen_size.1 / 2.0);

        let (texture_width, _texture_height) = texture_size;
        let target_height = target_size.1;
        let (screen_width, screen_height) = screen_size;

        let max_height_factor = ((screen_height - margin_y) / screen_height).max(1.0);
        let adjusted_screen_h = screen_height - margin_y;

        let width_ratio = (screen_width / texture_width).max(1.0);
        let height_ratio = (adjusted_screen_h / target_height).max(max_height_factor);

        // Get the smallest scale size
        let scale = width_ratio.clamp(1.0, height_ratio).floor();

        let scaled_width = texture_width * scale;
        let scaled_height = target_height * scale;

        // Create a transformation matrix
        let sw = scaled_width / screen_width;
        let sh = scaled_height / screen_height;

        let tx_nudge = (screen_width / 2.0).fract() / screen_width;
        let ty_nudge = (screen_height / 2.0).fract() / screen_height;

        let tx = tx_nudge;
        let ty = ty_nudge - margin_ndc / 2.0;

        #[rustfmt::skip]
        let transform: [f32; 16] = [
            sw,  0.0, 0.0, 0.0,
            0.0, sh,  0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            tx,  ty,  0.0, 1.0,
        ];

        // Create a clipping rectangle
        let _clip_rect = {
            let scaled_width = scaled_width.min(screen_width);
            let scaled_height = scaled_height.min(screen_height);
            let x = ((screen_width - scaled_width) / 2.0) as u32;
            let y = ((screen_height - scaled_height) / 2.0) as u32;

            (x, y, scaled_width as u32, scaled_height as u32)
        };

        Self {
            transform: Mat4::from(transform),
            //clip_rect,
        }
    }

    /// Create a transformation matrix that stretches the texture across the entire surface,
    /// ignoring aspect ratio.
    fn stretch_matrix(
        _texture_size: (f32, f32),
        _target_size: (f32, f32),
        screen_size: (f32, f32),
        margin_y: f32,
    ) -> Self {
        let screen_height = screen_size.1;
        let margin_ndc = margin_y / (screen_height / 2.0);

        let sw = 1.0;
        let sh = (screen_height - margin_y) / screen_size.1;

        let ty = -margin_ndc / 2.0;

        #[rustfmt::skip]
        let transform: [f32; 16] = [
            sw,   0.0,  0.0,  0.0,
            0.0,   sh,  0.0,  0.0,
            0.0,  0.0,  1.0,  0.0,
            0.0,   ty,  0.0,  1.0,
        ];

        Self {
            transform: Mat4::from(transform),
        }
    }

    /// Create a transformation matrix that fits the texture by scaling it proportionally to the
    /// largest size that will fit the surface, proportionally
    fn fit_matrix(texture_size: (f32, f32), target_size: (f32, f32), screen_size: (f32, f32), margin_y: f32) -> Self {
        //let margin_y = margin_y / 2.0;
        let offset = 0.0;
        let margin_ndc = (margin_y + offset) / (screen_size.1 / 2.0);

        let (texture_width, _texture_height) = texture_size;
        let target_height = target_size.1;
        let (screen_width, screen_height) = screen_size;
        let adjusted_screen_h = screen_height - margin_y;

        let max_height_factor = ((screen_height - margin_y) / screen_height).max(1.0);
        let width_ratio = (screen_width / texture_width).max(1.0);
        let height_ratio = (adjusted_screen_h / target_height).max(max_height_factor);

        // Get the smallest scale size. (Removed floor() call from integer scaler)
        let scale = width_ratio.clamp(1.0, height_ratio);

        let scaled_width = texture_width * scale;
        let scaled_height = target_height * scale;

        // Create a transformation matrix
        let sw = scaled_width / screen_width;
        let sh = scaled_height / screen_height;

        let tx_nudge = (screen_width / 2.0).fract() / screen_width;
        let ty_nudge = (screen_height / 2.0).fract() / screen_height;

        let tx = tx_nudge;
        let ty = -margin_ndc / 2.0 + ty_nudge;

        #[rustfmt::skip]
        let transform: [f32; 16] = [
            sw,  0.0,  0.0,  0.0,
            0.0,  sh,  0.0,  0.0,
            0.0, 0.0,  1.0,  0.0,
            tx,   ty,  0.0,  1.0,
        ];

        // Create a clipping rectangle
        let _clip_rect = {
            let scaled_width = scaled_width.min(screen_width);
            let scaled_height = scaled_height.min(screen_height);
            let x = ((screen_width - scaled_width) / 2.0) as u32;
            let y = ((screen_height - scaled_height) / 2.0) as u32;

            (x, y, scaled_width as u32, scaled_height as u32)
        };

        Self {
            transform: Mat4::from(transform),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        self.transform.as_byte_slice()
    }
}
