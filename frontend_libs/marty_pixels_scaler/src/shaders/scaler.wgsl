/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/martypc

    ---------------------------------------------------------------------------

    pixels_stretch_renderer::shaders::stretch.wgsl
    
    Implement a stretching renderer for Pixels when we want to fill the entire 
    window without maintaining square pixels.

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

// Vertex shader bindings

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

struct VertexUniform {
    transform: mat4x4<f32>,
}

struct CrtParamUniform {
    h_curvature: f32,
    v_curvature: f32,
    corner_radius: f32,
    scanlines: u32,
};

struct ScalerOptionsUniform {
    mode: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
    crt_params: CrtParamUniform,
    fill_color: vec4<f32>,
};

@group(0) @binding(2) var<uniform> r_locals: VertexUniform;
@group(0) @binding(3) var<uniform> scaler_opts: ScalerOptionsUniform;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = fma(position, vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5));
    out.position = r_locals.transform * vec4<f32>(position, 0.0, 1.0);
    return out;
}

fn brightness(color: vec4<f32>) -> f32 {
    return 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
}

fn apply_crt_curvature(uv: vec2<f32>) -> vec2<f32> {

    var curvature_x = scaler_opts.crt_params.h_curvature * 0.1;
    var curvature_y = scaler_opts.crt_params.v_curvature * 0.1;

   // Remap UV from [0,1] to [-1,1]
    var uv_mapped = uv * 2.0 - 1.0;
    // Calculate squared radius
    let radius_squared = uv_mapped.x * uv_mapped.x + uv_mapped.y * uv_mapped.y;
    // Apply barrel distortion
    let distortion = 1.0 - radius_squared * (curvature_x + curvature_y);
    // Apply the distortion to UV coordinates
    uv_mapped /= distortion;
    // Remap distorted UV back to [0,1] range
    var uv_distorted = uv_mapped * 0.5 + 0.5;

    return uv_distorted;
}

fn min4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    let ab = min(a, b);
    let cd = min(c, d);
    return min(ab, cd);
}

fn is_inside_corner_radius(uv: vec2<f32>, corner_radius: f32) -> bool {
    // Calculate the radius in UV space
    let uv_radius = vec2<f32>(corner_radius, corner_radius);

    // The centers of the corner circles in uv space
    let topLeftCenter: vec2<f32> = vec2<f32>(uv_radius.x, uv_radius.y);
    let topRightCenter: vec2<f32> = vec2<f32>(1.0 - uv_radius.x, uv_radius.y);
    let bottomLeftCenter: vec2<f32> = vec2<f32>(uv_radius.x, 1.0 - uv_radius.y);
    let bottomRightCenter: vec2<f32> = vec2<f32>(1.0 - uv_radius.x, 1.0 - uv_radius.y);

    let leftSide: bool = uv.x < uv_radius.x;
    let rightSide: bool = uv.x > (1.0 - uv_radius.x);
    let topSide: bool = uv.y < uv_radius.y;
    let bottomSide: bool = uv.y > (1.0 -uv_radius.y);

    let inTopLeftCorner: bool = (leftSide && topSide) && (distance(uv, topLeftCenter) > corner_radius);
    let inTopRightCorner: bool = (rightSide && topSide) && (distance(uv, topRightCenter) > corner_radius);
    let inBottomLeftCorner: bool = (leftSide && bottomSide) && (distance(uv, bottomLeftCenter) > corner_radius);
    let inBottomRightCorner: bool = (rightSide && bottomSide) && (distance(uv, bottomRightCenter) > corner_radius);

    // Determine if this fragment is in one of the rounded corners
    if (inTopLeftCorner || inTopRightCorner || inBottomLeftCorner || inBottomRightCorner) {
        return false;
    }
    // If none of the corners rejected the point, it's inside the safe area
    return true;
}

// Fragment shader bindings
@group(0) @binding(0) var r_tex_color: texture_2d<f32>;
@group(0) @binding(1) var r_tex_sampler: sampler;

@fragment
fn fs_main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    let curved_tex_coord = apply_crt_curvature(tex_coord);

    let is_outside = any(curved_tex_coord < vec2<f32>(0.0, 0.0)) || any(curved_tex_coord > vec2<f32>(1.0, 1.0));

    //var bg = textureSample(r_tex_color, r_tex_sampler, tex_coord);
    var color = textureSample(r_tex_color, r_tex_sampler, curved_tex_coord);
    if (is_outside) {
        //return vec4<f32>(scaler_opts.crt_params.corner_radius, scaler_opts.crt_params.h_curvature, scaler_opts.crt_params.v_curvature, 1.0); // Red color with full alpha
        discard;
    } else {

        if (!is_inside_corner_radius(curved_tex_coord, scaler_opts.crt_params.corner_radius * 0.05)) {
            discard;
        }

        // Otherwise, sample the texture color as usual
        let crtResolution: f32 = 224.0;
        let s_line = floor(curved_tex_coord.y * (crtResolution * 2.0));

        // Determine if we are on an 'even' or 'odd' line for the scanline effect
        let isEvenLine = (s_line % 2.0) == 0.0;
        
        // Calculate scanline effect as a factor, 0.7 for darkened lines, 1.0 for normal lines
        let scanlineEffect = select(1.0, 0.7, isEvenLine);
        
        // Apply the scanline effect
        color.r = color.r * scanlineEffect;
        color.g = color.g * scanlineEffect;
        color.b = color.b * scanlineEffect;

        let baseColor = vec4<f32>(0.0, 1.0, 0.0, 1.0);
        let gamma = 0.8;
        let brightness = brightness(color);
        let modulatedColor = baseColor * pow(brightness, gamma);

        return modulatedColor;
    }
}