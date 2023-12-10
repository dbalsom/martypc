/*
    MartyPC Emulator
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/martypc

    ---------------------------------------------------------------------------

    marty_pixels_renderer:lib.rs
    Implement a custom scaling renderer for Pixels that with selectable modes
    and fill color.

    This module adapted from the rust Pixels crate.
    https://github.com/parasyte/pixels

    ---------------------------------------------------------------------------

    Copyright 2019 Jay Oster

    Permission is hereby granted, free of charge, to any person obtaining a copy of
    this software and associated documentation files (the "Software"), to deal in
    the Software without restriction, including without limitation the rights to
    use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
    the Software, and to permit persons to whom the Software is furnished to do so,
    subject to the following conditions:

    The above copyright notice and this permission notice shall be included in all
    copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
    FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
    COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
    IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
    CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

*/

use bytemuck::{Pod, Zeroable};

// Reexport trait items
pub use frontend_common::{
    color::MartyColor,
    display_scaler::{DisplayScaler, ScalerEffect, ScalerFilter, ScalerMode, ScalerOption},
};

use ultraviolet::Mat4;
use wgpu::{util::DeviceExt, TextureDescriptor};

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

#[derive(Copy, Clone, Debug)]
struct ScalingMatrix {
    transform: Mat4,
}
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ScalerOptionsUniform {
    mode: u32,
    hres: u32,
    vres: u32,
    pad2: u32,
    crt_params: CrtParamUniform,
    fill_color: [f32; 4],
}

#[allow(dead_code)]
fn create_texture_view(pixels: &pixels::Pixels, width: u32, height: u32) -> wgpu::TextureView {
    let device = pixels.device();
    let texture_descriptor = TextureDescriptor {
        label: None,
        size: pixels::wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: pixels.render_texture_format(),
        view_formats: &[pixels.render_texture_format()],
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
    };

    device
        .create_texture(&texture_descriptor)
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_bind_group(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    matrix_buffer: &wgpu::Buffer,
    param_buffer: &wgpu::Buffer,
) -> pixels::wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:   None,
        layout:  bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding:  0,
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
            wgpu::BindGroupEntry {
                binding:  1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding:  2,
                resource: matrix_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding:  3,
                resource: param_buffer.as_entire_binding(),
            },
        ],
    })
}

/// The default renderer that scales your frame to the screen size.
pub struct MartyScaler {
    mode: ScalerMode,
    texture_view: wgpu::TextureView,
    nearest_sampler: wgpu::Sampler,
    bilinear_sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    nearest_bind_group: wgpu::BindGroup,
    bilinear_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    transform_uniform_buffer: wgpu::Buffer,
    params_uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    texture_width: u32,
    texture_height: u32,
    target_width: u32,
    target_height: u32,
    screen_width: u32,
    screen_height: u32,
    screen_margin_y: u32,
    bilinear: bool,
    fill_color: wgpu::Color,
    margin_l: u32,
    margin_r: u32,
    margin_t: u32,
    margin_b: u32,

    brightness: f32,
    contrast: f32,
    gamma: f32,

    scanlines: u32,
    do_scanlines: bool,
    h_curvature: f32,
    v_curvature: f32,
    corner_radius: f32,
    mono: bool,
    mono_color: wgpu::Color,
    #[allow(dead_code)]
    effect: ScalerEffect,
    #[allow(dead_code)]
    crt_params: CrtParamUniform,
}

impl MartyScaler {
    pub fn new(
        mode: ScalerMode,
        pixels: &pixels::Pixels,
        texture_width: u32,
        texture_height: u32,
        target_width: u32,
        target_height: u32,
        screen_width: u32,
        screen_height: u32,
        screen_margin_y: u32,
        bilinear: bool,
        fill_color: MartyColor,
    ) -> Self {
        let device = pixels.device();
        let scale_shader = wgpu::include_wgsl!("./shaders/scaler.wgsl");
        let scale_module = device.create_shader_module(scale_shader);

        //let texture_view = create_texture_view(pixels, screen_width, screen_height);
        let texture_view = pixels.texture().create_view(&wgpu::TextureViewDescriptor::default());

        // Create a texture sampler with nearest neighbor
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("marty_scaler_nearest_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        // Create a texture sampler with bilinear filtering
        let bilinear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("marty_scaler_linear_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 16,
            border_color: None,
        });

        // Create vertex buffer; array-of-array of position and texture coordinates
        // One full-screen triangle
        // See: https://github.com/parasyte/pixels/issues/180
        /*
        let vertex_data: [[f32; 2]; 3] = [
            [-1.0, -1.0],
            [3.0, -1.0],
            [-1.0, 3.0],
        ];
        */

        let vertex_data: [[f32; 2]; 6] = [
            // First triangle
            [-1.0, -1.0], // Bottom left
            [1.0, -1.0],  // Bottom right
            [-1.0, 1.0],  // Top left
            // Second triangle
            [1.0, -1.0], // Bottom right
            [-1.0, 1.0], // Top left
            [1.0, 1.0],  // Top right
        ];

        let vertex_data_slice = bytemuck::cast_slice(&vertex_data);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("marty_renderer_vertex_buffer"),
            contents: vertex_data_slice,
            usage:    wgpu::BufferUsages::VERTEX,
        });
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: (vertex_data_slice.len() / vertex_data.len()) as wgpu::BufferAddress,
            step_mode:    wgpu::VertexStepMode::Vertex,
            attributes:   &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        // Create uniform buffer for vertex shader
        let matrix = ScalingMatrix::new(
            mode,
            (texture_width as f32, texture_height as f32),
            (target_width as f32, target_height as f32),
            (screen_width as f32, screen_height as f32),
            screen_margin_y as f32,
        );
        let transform_bytes = matrix.as_bytes();
        let transform_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("marty_renderer_matrix_uniform_buffer"),
            contents: transform_bytes,
            usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create uniform buffer for fragment shader params
        let scaler_param_bytes = MartyScaler::get_default_param_uniform();
        let params_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("marty_renderer_params_uniform_buffer"),
            contents: &scaler_param_bytes,
            usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        //println!(">>>>>>>> len of scaler_params: {:?}", scaler_param_bytes.len());
        //println!(
        //    ">>>>>>>> size of scaler_params buffer uniform: {:?}",
        //    std::mem::size_of::<ScalerOptionsUniform>() as usize
        //);
        // Create bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("marty_renderer_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        multisampled:   false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(transform_bytes.len() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new((std::mem::size_of::<ScalerOptionsUniform>()) as u64),
                    },
                    count: None,
                },
            ],
        });

        let nearest_bind_group = create_bind_group(
            device,
            &bind_group_layout,
            &texture_view,
            &nearest_sampler,
            &transform_uniform_buffer,
            &params_uniform_buffer,
        );

        let bilinear_bind_group = create_bind_group(
            device,
            &bind_group_layout,
            &texture_view,
            &bilinear_sampler,
            &transform_uniform_buffer,
            &params_uniform_buffer,
        );

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("marty_renderer_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("marty_renderer_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &scale_module,
                entry_point: "vs_main",
                buffers: &[vertex_buffer_layout],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &scale_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: pixels.render_texture_format(),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        let fill_color = fill_color.to_wgpu_color();

        //println!(">>>>>> have fill color: {:?}", fill_color);

        Self {
            mode,
            texture_view,
            nearest_sampler,
            bilinear_sampler,
            bind_group_layout,
            nearest_bind_group,
            bilinear_bind_group,
            render_pipeline,
            transform_uniform_buffer,
            params_uniform_buffer,
            vertex_buffer,
            texture_width,
            texture_height,
            target_width,
            target_height,
            screen_width,
            screen_height,
            screen_margin_y,
            bilinear,
            fill_color,
            margin_l: 0,
            margin_r: 0,
            margin_t: 0,
            margin_b: 0,

            brightness: 1.0,
            contrast: 1.0,
            gamma: 1.0,

            effect: ScalerEffect::None,

            scanlines: 0,
            do_scanlines: false,
            h_curvature: 0.0,
            v_curvature: 0.0,
            corner_radius: 0.0,
            mono: false,
            mono_color: wgpu::Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            crt_params: Default::default(),
        }
    }

    fn update_matrix(&mut self, pixels: &pixels::Pixels) {
        let matrix = ScalingMatrix::new(
            self.mode,
            (self.texture_width as f32, self.texture_height as f32),
            (self.target_width as f32, self.target_height as f32),
            (self.screen_width as f32, self.screen_height as f32),
            self.screen_margin_y as f32,
        );
        let transform_bytes = matrix.as_bytes();

        pixels
            .queue()
            .write_buffer(&self.transform_uniform_buffer, 0, transform_bytes);
    }

    fn get_default_param_uniform() -> Vec<u8> {
        let crt_params = Default::default();

        let uniform_struct = ScalerOptionsUniform {
            mode: 0,
            hres: 0,
            vres: 0,
            pad2: 0,
            crt_params,
            fill_color: MartyColor {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
            .into(),
        };
        bytemuck::bytes_of(&uniform_struct).to_vec()
    }

    fn get_param_uniform_bytes(&mut self) -> Vec<u8> {
        // Build CRT shader params or default
        /*
        let crt_params = match &self.effect {
            ScalerEffect::None => {
                println!("getting default crt parameter uniform (effect disabled)");
                Default::default()
            },
            ScalerEffect::Crt{h_curvature, v_curvature, corner_radius, .. } => {
                println!("getting crt parameter uniform. corner_radius: {}", *corner_radius);
                CrtParamUniform {
                    h_curvature: *h_curvature,
                    v_curvature: *v_curvature,
                    corner_radius: *corner_radius,
                    scanlines: self.screen_height,

                    gamma: 1.0,
                    brightness: 1.0,
                    contrast: 1.0,
                    mono: self.mono as u32,
                    mono_color: MartyColor(self.mono_color).into(),
                }
            }
        };*/

        let crt_params = CrtParamUniform {
            h_curvature: self.h_curvature,
            v_curvature: self.v_curvature,
            corner_radius: self.corner_radius,
            scanlines: if self.do_scanlines { self.scanlines as u32 } else { 0 },

            gamma: self.gamma,
            brightness: self.brightness,
            contrast: self.contrast,
            mono: self.mono as u32,
            mono_color: MartyColor::from(self.mono_color).into(),
        };

        let uniform_struct = ScalerOptionsUniform {
            mode: self.mode as u32,
            hres: self.screen_width,
            vres: self.texture_height,
            pad2: 0,
            crt_params,
            fill_color: MartyColor::from(self.fill_color).into(),
        };

        bytemuck::bytes_of(&uniform_struct).to_vec()
    }
    fn update_uniforms(&mut self, pixels: &pixels::Pixels) {
        //println!("Updating uniform data...");

        // Calculate current scaling matrix.
        let matrix = ScalingMatrix::new(
            self.mode,
            (self.texture_width as f32, self.texture_height as f32),
            (self.target_width as f32, self.target_height as f32),
            (self.screen_width as f32, self.screen_height as f32),
            self.screen_margin_y as f32,
        );

        let transform_bytes = matrix.as_bytes();

        let queue = pixels.queue();
        queue.write_buffer(&self.transform_uniform_buffer, 0, transform_bytes);

        // Calculate shader parameters
        let uniform_vec = self.get_param_uniform_bytes();

        queue.write_buffer(&self.params_uniform_buffer, 0, &uniform_vec);
    }
}

impl DisplayScaler<pixels::Pixels> for MartyScaler {
    type NativeTextureView = wgpu::TextureView;
    type NativeEncoder = wgpu::CommandEncoder;
    fn get_texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    fn set_mode(&mut self, pixels: &pixels::Pixels, new_mode: ScalerMode) {
        self.mode = new_mode;
        self.update_matrix(pixels);
    }

    fn get_mode(&self) -> ScalerMode {
        self.mode
    }

    fn set_margins(&mut self, l: u32, r: u32, t: u32, b: u32) {
        self.margin_l = l;
        self.margin_r = r;
        self.margin_t = t;
        self.margin_b = b;
    }

    fn set_bilinear(&mut self, bilinear: bool) {
        self.bilinear = bilinear
    }

    fn set_fill_color(&mut self, fill: MartyColor) {
        self.fill_color = fill.to_wgpu_color();
    }

    /// Apply a ScalerOption. Update of uniform buffers is controlled by the 'update' boolean. If
    /// it is true we will perform an immediate uniform update; if false it will be delayed and
    /// set_option() will return true to indicate that the caller should perform an update.
    fn set_option(&mut self, pixels: &pixels::Pixels, opt: ScalerOption, update: bool) -> bool {
        let mut update_uniform = false;

        match opt {
            ScalerOption::Mode(new_mode) => {
                self.set_mode(pixels, new_mode);
            }
            ScalerOption::Adjustment { h: _h, s: _s, b, c, g } => {
                self.brightness = b;
                self.gamma = g;
                self.contrast = c;
                update_uniform = true;
            }
            ScalerOption::Filtering(filter) => {
                let bilinear;
                match filter {
                    ScalerFilter::Nearest => bilinear = false,
                    ScalerFilter::Linear => bilinear = true,
                }
                self.set_bilinear(bilinear);
            }
            ScalerOption::FillColor { r, g, b, a } => {
                self.set_fill_color(MartyColor {
                    r: r as f32,
                    g: g as f32,
                    b: b as f32,
                    a: a as f32,
                });
                update_uniform = true;
            }
            ScalerOption::Geometry {
                h_curvature,
                v_curvature,
                corner_radius,
            } => {
                self.h_curvature = h_curvature;
                self.v_curvature = v_curvature;
                self.corner_radius = corner_radius;
                update_uniform = true;
            }
            ScalerOption::Mono { enabled, r, g, b, a } => {
                self.mono = enabled;
                self.mono_color = wgpu::Color {
                    r: r as f64,
                    g: g as f64,
                    b: b as f64,
                    a: a as f64,
                };
                update_uniform = true;
            }
            ScalerOption::Margins { l, r, t, b } => {
                self.set_margins(l, r, t, b);
            }
            ScalerOption::Scanlines {
                enabled,
                lines,
                intensity: _i,
            } => {
                self.scanlines = lines.unwrap_or(self.scanlines);
                self.do_scanlines = enabled.unwrap_or(self.do_scanlines);
                update_uniform = true;
            }
            ScalerOption::Effect(_) => {}
        }

        if update && update_uniform {
            self.update_uniforms(pixels);
        }
        else if update_uniform {
            return true;
        }
        return false;
    }

    /// Iterate though a vector of ScalerOptions and apply them all. We can defer uniform update
    /// until all options have been processed.
    fn set_options(&mut self, pixels: &pixels::Pixels, opts: Vec<ScalerOption>) {
        let mut update_uniform = false;
        for opt in opts {
            let update_flag = self.set_option(pixels, opt, false);
            if update_flag {
                update_uniform = true;
            }
        }

        if update_uniform {
            self.update_uniforms(pixels);
        }
    }

    /// Draw the pixel buffer to the marty_render target.
    fn render(&self, encoder: &mut wgpu::CommandEncoder, render_target: &wgpu::TextureView) {
        //println!("render_target: {:?}", render_target);
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("marty_renderer marty_render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(MartyColor::from(self.fill_color).to_wgpu_color_linear()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&self.render_pipeline);

        if self.bilinear {
            rpass.set_bind_group(0, &self.bilinear_bind_group, &[]);
        }
        else {
            rpass.set_bind_group(0, &self.nearest_bind_group, &[]);
        }

        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        /*
        rpass.set_scissor_rect(
            0,
            0,
            1,
            1
        );
        */

        //rpass.draw(0..3, 0..1);
        rpass.draw(0..6, 0..1);
    }

    fn resize(
        &mut self,
        pixels: &pixels::Pixels,
        texture_width: u32,
        texture_height: u32,
        target_width: u32,
        target_height: u32,
        screen_width: u32,
        screen_height: u32,
    ) {
        //self.texture_view = create_texture_view(pixels, self.texture_width, self.texture_height);
        self.texture_view = pixels.texture().create_view(&wgpu::TextureViewDescriptor::default());
        self.nearest_bind_group = create_bind_group(
            pixels.device(),
            &self.bind_group_layout,
            &self.texture_view,
            &self.nearest_sampler,
            &self.transform_uniform_buffer,
            &self.params_uniform_buffer,
        );

        self.bilinear_bind_group = create_bind_group(
            pixels.device(),
            &self.bind_group_layout,
            &self.texture_view,
            &self.bilinear_sampler,
            &self.transform_uniform_buffer,
            &self.params_uniform_buffer,
        );

        //println!("screen_margin_y: {}", self.screen_margin_y);
        let matrix = ScalingMatrix::new(
            self.mode,
            (texture_width as f32, texture_height as f32),
            (target_width as f32, target_height as f32),
            (screen_width as f32, screen_height as f32),
            self.screen_margin_y as f32,
        );
        let transform_bytes = matrix.as_bytes();

        self.texture_width = texture_width;
        self.texture_height = texture_height;
        self.target_width = target_width;
        self.target_height = target_height;
        self.screen_width = screen_width;
        self.screen_height = screen_height;

        pixels
            .queue()
            .write_buffer(&self.transform_uniform_buffer, 0, transform_bytes);
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

    fn resize_surface(&mut self, pixels: &pixels::Pixels, screen_width: u32, screen_height: u32) {
        //self.texture_view = create_texture_view(pixels, self.screen_width, self.screen_height);
        self.texture_view = pixels.texture().create_view(&wgpu::TextureViewDescriptor::default());
        self.nearest_bind_group = create_bind_group(
            pixels.device(),
            &self.bind_group_layout,
            &self.texture_view,
            &self.nearest_sampler,
            &self.transform_uniform_buffer,
            &self.params_uniform_buffer,
        );

        self.bilinear_bind_group = create_bind_group(
            pixels.device(),
            &self.bind_group_layout,
            &self.texture_view,
            &self.bilinear_sampler,
            &self.transform_uniform_buffer,
            &self.params_uniform_buffer,
        );

        self.screen_width = screen_width;
        self.screen_height = screen_height;
        let matrix = ScalingMatrix::new(
            self.mode,
            (self.texture_width as f32, self.texture_height as f32),
            (self.target_width as f32, self.target_height as f32),
            (self.screen_width as f32, self.screen_height as f32),
            self.screen_margin_y as f32,
        );
        let transform_bytes = matrix.as_bytes();

        pixels
            .queue()
            .write_buffer(&self.transform_uniform_buffer, 0, transform_bytes);
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

        // Get smallest scale size
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

        // Get smallest scale size
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

        // Get smallest scale size. (Removed floor() call from integer scaler)
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
