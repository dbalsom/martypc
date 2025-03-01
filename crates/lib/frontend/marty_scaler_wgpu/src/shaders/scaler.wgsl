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
    gamma: f32,
    brightness: f32,
    contrast: f32,
    mono: u32,
    mono_color: vec4<f32>,
};

struct ScalerOptionsUniform {
    mode: u32,
    hres: u32,
    vres: u32,
    pad2: u32,
    crt_params: CrtParamUniform,
    fill_color: vec4<f32>,
};

@group(0) @binding(2) var<uniform> r_locals: VertexUniform;
@group(0) @binding(3) var<uniform> scaler_opts: ScalerOptionsUniform;

/*@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) a_tex_coord: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = fma(position, vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5));
    out.position = r_locals.transform * vec4<f32>(position, 0.0, 1.0);
    return out;
}*/

@vertex
fn vs_main(@builtin(vertex_index) vidx: u32) -> VertexOutput {

    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );

    var output : VertexOutput;
    output.position = r_locals.transform * vec4<f32>(positions[vidx].x, positions[vidx].y, 0.0, 1.0);
    output.tex_coord = fma(positions[vidx], vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5));

    return output;
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

fn do_monochrome(color: vec4<f32>, gamma: f32) -> vec4<f32> {
    var brightness = brightness(color);
    if (brightness < 0.0) {
        brightness = 0.0;
    }
    let baseColor = scaler_opts.crt_params.mono_color;
    let modulatedColor = baseColor * pow(abs(brightness), gamma);
    return modulatedColor;
}

fn do_scanlines(color: vec4<f32>, y_coord: f32, lines: u32, intensity: f32) -> vec4<f32> {

    var newColor: vec4<f32>;
    let factor = 1.0 - intensity;

    // Determine what scanline we're on.
    let s_line = floor(y_coord * (f32(lines) * 2.0));

    // Determine if we are on an 'even' or 'odd' line for the scanline effect
    let isEvenLine = (s_line % 2.0) == 0.0;

    let scanlineEffect = select(1.0, factor, isEvenLine);

    // Apply the scanline effect
    newColor.r = color.r * scanlineEffect;
    newColor.g = color.g * scanlineEffect;
    newColor.b = color.b * scanlineEffect;

    return newColor;
}

// Fragment shader bindings
@group(0) @binding(0) var r_tex_color: texture_2d<f32>;
@group(0) @binding(1) var r_tex_sampler: sampler;

@fragment
fn fs_main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    let curved_tex_coord = apply_crt_curvature(tex_coord);

    let is_outside = any(curved_tex_coord < vec2<f32>(0.0, 0.0)) || any(curved_tex_coord > vec2<f32>(1.0, 1.0));
    let is_inside_corner = is_inside_corner_radius(curved_tex_coord, scaler_opts.crt_params.corner_radius * 0.1);

    //var bg = textureSample(r_tex_color, r_tex_sampler, tex_coord);
    var color = textureSample(r_tex_color, r_tex_sampler, curved_tex_coord);

    if (is_outside || !is_inside_corner) {
        if (true) { discard; } // trick naga DX12 backend into thinking we return a color from each control path
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    } else {

        let gamma = scaler_opts.crt_params.gamma;
        let scanlines = scaler_opts.crt_params.scanlines;
        let mono = scaler_opts.crt_params.mono;

        if (scanlines > 0u) {
            color = do_scanlines(color, curved_tex_coord.y, scanlines, 0.3);
        }

        if (mono != 0u) {
            color = do_monochrome(color, gamma);
        }

        // We can emit a solid color for debugging...
        // return vec4<f32>(0.0, 0.0, 1.0, 1.0);
        return color.bgra;
    }
}