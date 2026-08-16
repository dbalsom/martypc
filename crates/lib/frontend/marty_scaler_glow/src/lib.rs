/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

use std::sync::Arc;
// Reexport trait items
pub use marty_frontend_common::color::MartyColor;

use marty_display_common::display_scaler::{DisplayScaler, ScalerFilter, ScalerGeometry, ScalerMode, ScalerOption};

use eframe::{
    glow,
    glow::{Context, HasContext, Program, Texture, UniformLocation, VertexArray},
};
use ultraviolet::Mat4;

/// A logical texture size for a window surface.
#[derive(Debug)]
pub struct SurfaceSize {
    pub width:  u32,
    pub height: u32,
}

#[derive(Copy, Clone, Debug)]
struct ScalingMatrix {
    transform: Mat4,
}

fn fit_scale(texture_width: f32, target_height: f32, screen_width: f32, screen_height: f32, margin_y: f32) -> f32 {
    if texture_width <= 0.0 || target_height <= 0.0 || screen_width <= 0.0 || screen_height <= 0.0 {
        return 0.0;
    }

    let adjusted_screen_h = (screen_height - margin_y).max(0.0);
    let width_ratio = screen_width / texture_width;
    let height_ratio = adjusted_screen_h / target_height;
    width_ratio.min(height_ratio).max(0.0)
}

struct ShaderUniforms {
    texture: Option<UniformLocation>,
    transform: Option<UniformLocation>,
    h_curvature: Option<UniformLocation>,
    v_curvature: Option<UniformLocation>,
    corner_radius: Option<UniformLocation>,
    scanlines: Option<UniformLocation>,
    gamma: Option<UniformLocation>,
    mono: Option<UniformLocation>,
    mono_color: Option<UniformLocation>,
    vres: Option<UniformLocation>,
    texture_order: Option<UniformLocation>,
    crtc_frame_parity: Option<UniformLocation>,
    crtc_interlaced: Option<UniformLocation>,
    crtc_interlace_support: Option<UniformLocation>,
    power_off: Option<UniformLocation>,
}

impl ShaderUniforms {
    fn new(gl: &Context, program: Program) -> Self {
        unsafe {
            Self {
                texture: gl.get_uniform_location(program, "u_texture"),
                transform: gl.get_uniform_location(program, "u_transform"),
                h_curvature: gl.get_uniform_location(program, "u_h_curvature"),
                v_curvature: gl.get_uniform_location(program, "u_v_curvature"),
                corner_radius: gl.get_uniform_location(program, "u_corner_radius"),
                scanlines: gl.get_uniform_location(program, "u_scanlines"),
                gamma: gl.get_uniform_location(program, "u_gamma"),
                mono: gl.get_uniform_location(program, "u_mono"),
                mono_color: gl.get_uniform_location(program, "u_mono_color"),
                vres: gl.get_uniform_location(program, "u_vres"),
                texture_order: gl.get_uniform_location(program, "u_texture_order"),
                crtc_frame_parity: gl.get_uniform_location(program, "u_crtc_frame_parity"),
                crtc_interlaced: gl.get_uniform_location(program, "u_crtc_interlaced"),
                crtc_interlace_support: gl.get_uniform_location(program, "u_crtc_interlace_support"),
                power_off: gl.get_uniform_location(program, "u_power_off"),
            }
        }
    }
}

pub struct MartyScaler {
    mode: ScalerMode,

    program: Program,
    uniforms: ShaderUniforms,
    vertex_array: VertexArray,
    _vbo: glow::Buffer,

    screen_size: (u32, u32),
    target_size: (u32, u32),
    texture_size: (u32, u32),
    margin_y: u32,

    scaling_matrix: ScalingMatrix,
    bilinear: bool,
    gamma: f32,
    scanlines: u32,
    do_scanlines: bool,
    h_curvature: f32,
    v_curvature: f32,
    corner_radius: f32,
    mono: bool,
    mono_color: [f32; 4],
    crtc_frame_parity: u32,
    crtc_interlaced: bool,
    crtc_interlace_support: bool,
    power_off: f32,
}

impl MartyScaler {
    #[rustfmt::skip]
    pub fn new(
        gl: &Context,
        texture_size: (u32, u32),
        target_size: (u32, u32),
        screen_size: (u32, u32),
        margin_y: u32,
        mode: ScalerMode,
    ) -> Self {
        let shader_version = if cfg!(target_arch = "wasm32") {
            "#version 300 es"
        } else {
            "#version 330"
        };

        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            // Vertex + UV quad (triangle strip)
            let vertices: [f32; 16] = [
                -1.0,  1.0, 0.0, 0.0, // top-left
                -1.0, -1.0, 0.0, 1.0, // bottom-left
                 1.0,  1.0, 1.0, 0.0, // top-right
                 1.0, -1.0, 1.0, 1.0, // bottom-right
            ];

            let shader_sources = [
                (glow::VERTEX_SHADER, include_str!("shaders/scaler.vert")),
                (glow::FRAGMENT_SHADER, include_str!("shaders/scaler.frag")),
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

            let uniforms = ShaderUniforms::new(gl, program);

            // --- Vertex Array / Buffer setup ---
            let vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao));

            let vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&vertices), glow::STATIC_DRAW);

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

            let matrix = ScalingMatrix::new(
                mode,
                (texture_size.0 as f32, texture_size.1 as f32),
                (target_size.0 as f32, target_size.1 as f32),
                (screen_size.0 as f32, screen_size.1 as f32),
                margin_y as f32,
            );

            Self {
                mode,
                program,
                uniforms,
                vertex_array: vao,
                _vbo: vbo,
                screen_size,
                target_size,
                texture_size,
                margin_y,
                scaling_matrix: matrix,
                bilinear: true,
                gamma: 1.0,
                scanlines: 0,
                do_scanlines: false,
                h_curvature: 0.0,
                v_curvature: 0.0,
                corner_radius: 0.0,
                mono: false,
                mono_color: [1.0, 1.0, 1.0, 1.0],
                crtc_frame_parity: 0,
                crtc_interlaced: false,
                crtc_interlace_support: true,
                power_off: 0.0,
            }
        }
    }

    fn output_height(&self) -> f32 {
        let texture_width = self.texture_size.0 as f32;
        let texture_height = self.texture_size.1 as f32;
        let target_width = self.target_size.0 as f32;
        let target_height = self.target_size.1 as f32;
        let screen_width = self.screen_size.0 as f32;
        let screen_height = self.screen_size.1 as f32;
        let margin_y = self.margin_y as f32;

        if texture_width <= 0.0 || texture_height <= 0.0 || target_height <= 0.0 || screen_height <= 0.0 {
            return 0.0;
        }

        match self.mode {
            ScalerMode::Null | ScalerMode::Fixed => target_height,
            ScalerMode::Stretch => (screen_height - margin_y).max(0.0),
            ScalerMode::Windowed => {
                Self::fit_output_height(texture_width, target_height, target_width, target_height, margin_y)
            }
            ScalerMode::Integer => {
                let adjusted_screen_h = screen_height - margin_y;
                let max_height_factor = (adjusted_screen_h / screen_height).max(1.0);
                let width_ratio = (screen_width / texture_width).max(1.0);
                let height_ratio = (adjusted_screen_h / target_height).max(max_height_factor);
                target_height * width_ratio.clamp(1.0, height_ratio).floor()
            }
            ScalerMode::Fit => {
                Self::fit_output_height(texture_width, target_height, screen_width, screen_height, margin_y)
            }
        }
    }

    fn fit_output_height(
        texture_width: f32,
        target_height: f32,
        screen_width: f32,
        screen_height: f32,
        margin_y: f32,
    ) -> f32 {
        target_height * fit_scale(texture_width, target_height, screen_width, screen_height, margin_y)
    }

    fn scanlines_allowed(&self) -> bool {
        self.texture_size.1 > 0 && self.output_height() >= self.texture_size.1 as f32 * 2.0
    }
}

impl DisplayScaler<Context, (), Texture> for MartyScaler {
    type NativeContext = Context;
    type NativeRenderPass = ();
    type NativeTexture = Texture;
    type NativeTextureView = ();
    type NativeEncoder = ();

    // fn texture_view(&self) -> &() {
    //     &()
    // }

    fn render(&self, _encoder: &mut (), _render_target: &Self::NativeTextureView) {
        // Glow does not use an encoder
    }

    fn render_with_context(&self, gl: &Context, texture: Arc<Self::NativeTexture>) {
        unsafe {
            gl.disable(glow::CULL_FACE);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vertex_array));

            // Bind texture to unit 0
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(*texture));
            let filter = if self.bilinear { glow::LINEAR } else { glow::NEAREST } as i32;
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);

            let scanlines = if self.do_scanlines && self.scanlines_allowed() {
                self.scanlines
            }
            else {
                0
            };

            gl.uniform_1_i32(self.uniforms.texture.as_ref(), 0);
            gl.uniform_matrix_4_f32_slice(
                self.uniforms.transform.as_ref(),
                false,
                self.scaling_matrix.as_slice_f32(),
            );
            gl.uniform_1_f32(self.uniforms.h_curvature.as_ref(), self.h_curvature);
            gl.uniform_1_f32(self.uniforms.v_curvature.as_ref(), self.v_curvature);
            gl.uniform_1_f32(self.uniforms.corner_radius.as_ref(), self.corner_radius);
            gl.uniform_1_i32(self.uniforms.scanlines.as_ref(), scanlines as i32);
            gl.uniform_1_f32(self.uniforms.gamma.as_ref(), self.gamma);
            gl.uniform_1_i32(self.uniforms.mono.as_ref(), self.mono as i32);
            gl.uniform_4_f32(
                self.uniforms.mono_color.as_ref(),
                self.mono_color[0],
                self.mono_color[1],
                self.mono_color[2],
                self.mono_color[3],
            );
            gl.uniform_1_i32(self.uniforms.vres.as_ref(), self.texture_size.1 as i32);
            gl.uniform_1_i32(self.uniforms.texture_order.as_ref(), 0); // Glow textures are RGBA.
            gl.uniform_1_i32(self.uniforms.crtc_frame_parity.as_ref(), self.crtc_frame_parity as i32);
            gl.uniform_1_i32(self.uniforms.crtc_interlaced.as_ref(), self.crtc_interlaced as i32);
            gl.uniform_1_i32(
                self.uniforms.crtc_interlace_support.as_ref(),
                self.crtc_interlace_support as i32,
            );
            gl.uniform_1_f32(self.uniforms.power_off.as_ref(), self.power_off);

            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }

    fn render_with_renderpass(&self, _render_pass: &mut Self::NativeRenderPass) {
        // Glow does not use renderpass
    }

    fn resize(
        &mut self,
        _device: &Context,
        _queue: &(),
        _texture: &Self::NativeTexture,
        texture_width: u32,
        texture_height: u32,
        target_width: u32,
        target_height: u32,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.texture_size = (texture_width, texture_height);
        self.screen_size = (screen_width, screen_height);
        self.target_size = (target_width, target_height);

        self.scaling_matrix = ScalingMatrix::new(
            self.mode,
            (texture_width as f32, texture_height as f32),
            (target_width as f32, target_height as f32),
            (screen_width as f32, screen_height as f32),
            self.margin_y as f32,
        );
    }

    fn resize_surface(
        &mut self,
        _device: &Context,
        _queue: &(),
        _texture: &Self::NativeTexture,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.screen_size = (screen_width, screen_height);

        self.scaling_matrix = ScalingMatrix::new(
            self.mode,
            (self.texture_size.0 as f32, self.texture_size.1 as f32),
            (self.target_size.0 as f32, self.target_size.1 as f32),
            (self.screen_size.0 as f32, self.screen_size.1 as f32),
            self.margin_y as f32,
        );
    }

    fn mode(&self) -> ScalerMode {
        self.mode
    }

    fn set_mode(&mut self, _device: &eframe::glow::Context, _queue: &(), new_mode: ScalerMode) {
        self.mode = new_mode;
        self.scaling_matrix = ScalingMatrix::new(
            self.mode,
            (self.texture_size.0 as f32, self.texture_size.1 as f32),
            (self.target_size.0 as f32, self.target_size.1 as f32),
            (self.screen_size.0 as f32, self.screen_size.1 as f32),
            self.margin_y as f32,
        );
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

    fn set_margins(&mut self, _l: u32, _r: u32, _t: u32, _b: u32) {}

    fn set_bilinear(&mut self, bilinear: bool) {
        self.bilinear = bilinear;
    }

    fn set_fill_color(&mut self, _fill: MartyColor) {}

    /// Apply a scaler option. Glow uniforms are uploaded from this state immediately before each
    /// draw, so there is no separate uniform-buffer update to defer.
    fn set_option(&mut self, device: &Context, queue: &(), opt: ScalerOption, _update: bool) -> bool {
        match opt {
            ScalerOption::Mode(new_mode) => {
                self.set_mode(device, queue, new_mode);
            }
            ScalerOption::Adjustment { g, .. } => {
                self.gamma = g;
            }
            ScalerOption::Filtering(filter) => {
                self.set_bilinear(matches!(filter, ScalerFilter::Linear));
            }
            ScalerOption::FillColor { r, g, b, a } => {
                self.set_fill_color(MartyColor {
                    r: r as f32,
                    g: g as f32,
                    b: b as f32,
                    a: a as f32,
                });
            }
            ScalerOption::Geometry {
                h_curvature,
                v_curvature,
                corner_radius,
            } => {
                self.h_curvature = h_curvature;
                self.v_curvature = v_curvature;
                self.corner_radius = corner_radius;
            }
            ScalerOption::Mono { enabled, r, g, b, a } => {
                self.mono = enabled;
                self.mono_color = [r, g, b, a];
            }
            ScalerOption::Margins { l, r, t, b } => {
                self.set_margins(l, r, t, b);
            }
            ScalerOption::Scanlines {
                enabled,
                lines,
                intensity: _,
            } => {
                self.scanlines = lines.unwrap_or(self.scanlines);
                self.do_scanlines = enabled.unwrap_or(self.do_scanlines);
            }
            ScalerOption::CrtcFrameParity { enabled, parity } => {
                self.crtc_interlaced = enabled;
                self.crtc_frame_parity = parity & 1;
            }
            ScalerOption::InterlaceSupport(enabled) => {
                self.crtc_interlace_support = enabled;
            }
            ScalerOption::PowerOff { progress } => {
                self.power_off = progress.clamp(0.0, 1.0);
            }
            ScalerOption::Effect(_) => {}
        }
        false
    }

    /// Apply a set of scaler options.
    fn set_options(&mut self, device: &eframe::glow::Context, queue: &(), opts: Vec<ScalerOption>) {
        for opt in opts {
            self.set_option(device, queue, opt, false);
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
        let (texture_width, _texture_height) = texture_size;
        let target_height = target_size.1;
        let (screen_width, screen_height) = screen_size;

        if texture_width <= 0.0 || target_height <= 0.0 || screen_width <= 0.0 || screen_height <= 0.0 {
            return Self {
                transform: Mat4::identity(),
            };
        }

        let margin_ndc = margin_y / (screen_height / 2.0);

        // Fit may scale either up or down while preserving the target aspect ratio.
        let scale = fit_scale(texture_width, target_height, screen_width, screen_height, margin_y);

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

    fn as_slice_f32(&self) -> &[f32] {
        self.transform.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::fit_scale;

    #[test]
    fn fit_scale_can_scale_up_and_down() {
        assert_eq!(fit_scale(640.0, 400.0, 320.0, 200.0, 0.0), 0.5);
        assert_eq!(fit_scale(640.0, 400.0, 1280.0, 800.0, 0.0), 2.0);
    }
}
