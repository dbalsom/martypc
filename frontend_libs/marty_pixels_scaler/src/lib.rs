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

struct MartyColor(wgpu::Color);

impl From<MartyColor> for [f32; 4] {
    fn from(color: MartyColor) -> Self {
        [color.0.r as f32, color.0.g as f32, color.0.b as f32, color.0.a as f32]
    }
}

/// A logical texture size for a window surface.
#[derive(Debug)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

use ultraviolet::Mat4;
use wgpu::{
    TextureDescriptor,
    util::DeviceExt
};

use display_scaler::{DisplayScaler, ScalerEffect, ScalerFilter, ScalerMode, ScalerOption};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CrtParamUniform {
    h_curvature: f32,
    v_curvature: f32,
    corner_radius: f32,
    scanlines: u32
}

impl Default for CrtParamUniform {
    fn default() -> Self {
        Self {
            h_curvature: 0.0,
            v_curvature: 0.0,
            corner_radius: 0.0,
            scanlines: 0
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
    pad0: u32,
    pad1: u32,
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
        label: None,
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: matrix_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
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
    screen_width: u32,
    screen_height: u32,
    screen_margin_y: u32,
    bilinear: bool,
    fill_color: wgpu::Color,
    margin_l: u32,
    margin_r: u32,
    margin_t: u32,
    margin_b: u32,
    effect: ScalerEffect,
    #[allow(dead_code)]
    crt_params: CrtParamUniform
}

impl MartyScaler {
    pub fn new(
        mode: ScalerMode,
        pixels: &pixels::Pixels,
        texture_width: u32,
        texture_height: u32,
        screen_width: u32,
        screen_height: u32,
        screen_margin_y: u32,
        bilinear: bool,
        fill_color: wgpu::Color
    ) -> Self {

        let device = pixels.device();
        let scale_shader = wgpu::include_wgsl!("./shaders/scaler.wgsl");
        let scale_module = device.create_shader_module(scale_shader);

        //let texture_view = create_texture_view(pixels, screen_width, screen_height);
        let texture_view = pixels.texture().create_view(&wgpu::TextureViewDescriptor::default());

        // Create a texture sampler with nearest neighbor
        let nearest_sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
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
            }
        );

        // Create a texture sampler with bilinear filtering
        let bilinear_sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
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
            }
        );

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
            [ 1.0, -1.0], // Bottom right
            [-1.0,  1.0], // Top left

            // Second triangle
            [ 1.0, -1.0], // Bottom right
            [-1.0,  1.0], // Top left
            [ 1.0,  1.0], // Top right
        ];

        let vertex_data_slice = bytemuck::cast_slice(&vertex_data);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("marty_renderer_vertex_buffer"),
            contents: vertex_data_slice,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: (vertex_data_slice.len() / vertex_data.len()) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        // Create uniform buffer for vertex shader
        let matrix = ScalingMatrix::new(
            mode,
            (texture_width as f32, texture_height as f32),
            (screen_width as f32, screen_height as f32),
            screen_margin_y as f32
        );
        let transform_bytes = matrix.as_bytes();
        let transform_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("marty_renderer_matrix_uniform_buffer"),
            contents: transform_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });



        // Create uniform buffer for fragment shader params
        let scaler_param_bytes = MartyScaler::get_default_param_uniform();
        let params_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("marty_renderer_params_uniform_buffer"),
            contents: &scaler_param_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        println!(">>>>>>>> len of scaler_params: {:?}", scaler_param_bytes.len());
        println!(">>>>>>>> size of scaler_params buffer uniform: {:?}", std::mem::size_of::<ScalerOptionsUniform>() as usize);
        // Create bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("marty_renderer_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        multisampled: false,
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
            &params_uniform_buffer
        );

        let bilinear_bind_group = create_bind_group(
            device,
            &bind_group_layout,
            &texture_view,
            &bilinear_sampler,
            &transform_uniform_buffer,
            &params_uniform_buffer
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
            screen_width,
            screen_height,
            screen_margin_y,
            bilinear,
            fill_color,
            margin_l: 0,
            margin_r: 0,
            margin_t: 0,
            margin_b: 0,
            effect: ScalerEffect::None,
            crt_params: Default::default()
        }
    }

    fn update_matrix(&mut self, pixels: &pixels::Pixels) {
        let matrix = ScalingMatrix::new(
            self.mode,
            (self.texture_width as f32, self.texture_height as f32),
            (self.screen_width as f32, self.screen_height as f32),
            self.screen_margin_y as f32
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
            pad0: 0,
            pad1: 0,
            pad2: 0,
            crt_params,
            fill_color: MartyColor(wgpu::Color{r: 0.0, g: 0.0, b: 0.0, a: 0.0}).into(),

        };
        bytemuck::bytes_of(&uniform_struct).to_vec()
    }

    fn get_param_uniform_bytes(&mut self) -> Vec<u8> {
        // Build CRT shader params or default
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
                    scanlines: self.screen_height
                }
            }
        };

        let uniform_struct = ScalerOptionsUniform {

            mode: self.mode as u32,
            pad0: 0,
            pad1: 0,
            pad2: 0,
            crt_params,
            fill_color: MartyColor(self.fill_color).into(),
        };

        bytemuck::bytes_of(&uniform_struct).to_vec()
    }
    fn update_uniforms(&mut self, pixels: &pixels::Pixels) {

        println!("Updating uniform data...");
        // Calculate current scaling matrix.
        let matrix = ScalingMatrix::new(
            self.mode,
            (self.texture_width as f32, self.texture_height as f32),
            (self.screen_width as f32, self.screen_height as f32),
            self.screen_margin_y as f32
        );

        let transform_bytes = matrix.as_bytes();

        let queue = pixels.queue();
        queue.write_buffer(&self.transform_uniform_buffer, 0, transform_bytes);

        // Calculate shader parameters
        let uniform_vec = self.get_param_uniform_bytes();

        queue.write_buffer(
            &self.params_uniform_buffer,
            0,
            &uniform_vec,
        );
    }
}

impl DisplayScaler for MartyScaler {
    fn get_texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    fn set_mode(&mut self, pixels: &pixels::Pixels, new_mode: ScalerMode) {
        self.mode = new_mode;
        self.update_matrix(pixels);
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

    fn set_fill_color(&mut self, fill: wgpu::Color) {
        self.fill_color = fill;
    }

    fn set_option(&mut self, pixels: &pixels::Pixels, opt: ScalerOption) {

        println!("Setting scaler option...");
        match opt {
            ScalerOption::Mode(new_mode) => {
                self.set_mode(pixels, new_mode);
            }
            ScalerOption::Filtering(filter) => {
                let bilinear;
                match filter {
                    ScalerFilter::Nearest => bilinear = false,
                    ScalerFilter::Linear => bilinear = true,
                }
                self.set_bilinear(bilinear);
            }
            ScalerOption::FillColor{r, g, b, a} => {
                self.set_fill_color(wgpu::Color{r: r as f64, g: g as f64, b: b as f64, a: a as f64});
            }
            ScalerOption::Effect(effect) => {
                self.effect = effect;
                self.update_uniforms(pixels);
            }
            ScalerOption::Margins{l, r, t, b} => {
                self.set_margins(l, r, t, b);
            }
        }

        self.update_uniforms(pixels);
    }
    fn set_options(&mut self, pixels: &pixels::Pixels, opts: Vec<ScalerOption>) {
        println!("Got opts of len : {}", opts.len());

        for opt in opts {
            self.set_option(pixels, opt);
        }
    }

    /// Draw the pixel buffer to the render target.
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        bilinear: bool
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("StretchRenderer render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.fill_color),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        rpass.set_pipeline(&self.render_pipeline);

        if bilinear {
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
            &self.params_uniform_buffer
        );

        let matrix = ScalingMatrix::new(
            self.mode,
            (texture_width as f32, texture_height as f32),
            (screen_width as f32, screen_height as f32),
            self.screen_margin_y as f32
        );
        let transform_bytes = matrix.as_bytes();

        self.texture_width = texture_width;
        self.texture_height = texture_height;
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


    fn resize_screen(
        &mut self,
        pixels: &pixels::Pixels,
        screen_width: u32,
        screen_height: u32,
    ) {

        self.texture_view = create_texture_view(pixels, self.screen_width, self.screen_height);
        self.bind_group = create_bind_group(
            pixels.device(),
            &self.bind_group_layout,
            &self.texture_view,
            &self.sampler,
            &self.uniform_buffer,
        );

        let matrix = ScalingMatrix::new(
            (self.texture_width as f32, self.texture_height as f32),
            (self.screen_width as f32, self.screen_height as f32),
        );
        let transform_bytes = matrix.as_bytes();

        self.screen_width = screen_width;
        self.screen_height = screen_height;

        pixels
            .queue()
            .write_buffer(&self.uniform_buffer, 0, transform_bytes);
    }
    */
}



impl ScalingMatrix {
    // texture_size is the dimensions of the drawing texture
    // screen_size is the dimensions of the surface being drawn to
    fn new(mode: ScalerMode, texture_size: (f32, f32), screen_size: (f32, f32), margin_y: f32) -> Self {

        match mode {
            ScalerMode::None => ScalingMatrix::none_matrix(texture_size, screen_size, margin_y),
            ScalerMode::Integer => ScalingMatrix::integer_matrix(texture_size, screen_size, margin_y),
            ScalerMode::Fit => ScalingMatrix::fit_matrix(texture_size, screen_size, margin_y),
            ScalerMode::Stretch => ScalingMatrix::stretch_matrix(texture_size, screen_size, margin_y),
        }
    }

    fn none_matrix(texture_size: (f32, f32), screen_size: (f32, f32), margin_y: f32) -> Self {

        let _margin_y = margin_y / 2.0;

        let (texture_width, texture_height) = texture_size;
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
        let sh = texture_height / screen_height;
        let tx = (screen_width / 2.0).fract() / screen_width;
        let ty = (screen_height / 2.0).fract() / screen_height;
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

    fn integer_matrix(texture_size: (f32, f32), screen_size: (f32, f32), margin_y: f32) -> Self {

        let _margin_y = margin_y / 2.0;

        let (texture_width, texture_height) = texture_size;
        let (screen_width, screen_height) = screen_size;

        let width_ratio = (screen_width / texture_width).max(1.0);
        let height_ratio = (screen_height / texture_height).max(1.0);

        // Get smallest scale size
        let scale = width_ratio.clamp(1.0, height_ratio).floor();

        let scaled_width = texture_width * scale;
        let scaled_height = texture_height * scale;

        // Create a transformation matrix
        let sw = scaled_width / screen_width;
        let sh = scaled_height / screen_height;
        let tx = (screen_width / 2.0).fract() / screen_width;
        let ty = (screen_height / 2.0).fract() / screen_height;
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

    fn stretch_matrix(texture_size: (f32, f32), screen_size: (f32, f32), margin_y: f32) -> Self {

        let (_texture_width, _texture_height) = texture_size;
        let (_screen_width, screen_height) = screen_size;

        //let w_scale = screen_width / texture_width;
        let h_scale = screen_height / (screen_height - margin_y);

        // Calculate the scaled dimensions
        //let scaled_width = texture_width * w_scale;
        //let scaled_height = texture_height * h_scale;

        //let sw = scaled_width / texture_width;
        //let sh = scaled_height / texture_height;

        let sw = 1.0;
        let sh = h_scale;
        let ty = (margin_y / screen_height) * 2.0; // Convert margin to NDC and account for the origin at the top left

        #[rustfmt::skip]
        let transform: [f32; 16] = [
            sw,   0.0,  0.0,  0.0,
            0.0,   sh,  0.0,  0.0,
            0.0,  0.0,  1.0,  0.0,
            0.0,  -ty,  0.0,  1.0,
        ];

        Self {
            transform: Mat4::from(transform),
        }
    }

    fn fit_matrix(texture_size: (f32, f32), screen_size: (f32, f32), margin_y: f32) -> Self {

        let margin_y = margin_y / 2.0;

        let (texture_width, texture_height) = texture_size;
        let (screen_width, screen_height) = screen_size;

        let width_ratio = (screen_width / texture_width).max(1.0);
        let height_ratio = ((screen_height - margin_y) / texture_height).max(1.0);

        // Get smallest scale size. (Removed floor() call from integer scaler)
        let scale = width_ratio.clamp(1.0, height_ratio);

        let scaled_width = texture_width * scale;
        let scaled_height = texture_height * scale;

        // Create a transformation matrix
        let sw = scaled_width / screen_width;
        let sh = (scaled_height - margin_y) / (screen_height - margin_y);

        let tx = 0.0; // Centered on the x-axis, no need for fract() because we're not translating the x-axis
        let ty = (margin_y / screen_height) * 2.0; // Convert margin to NDC and account for the origin at the top left

        #[rustfmt::skip]
        let transform: [f32; 16] = [
            sw,  0.0,  0.0,  0.0,
            0.0,  sh,  0.0,  0.0,
            0.0, 0.0,  1.0,  0.0,
            tx,  -ty,  0.0,  1.0,
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