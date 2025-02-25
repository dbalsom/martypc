/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2019 Jay Oster
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

    ---------------------------------------------------------------------------
*/

//! Utility functions for wgpu backend.

use std::sync::Arc;

use anyhow::Error;
use display_backend_trait::TextureDimensions;
use egui_wgpu::wgpu;

/// Create a new wgpu texture of the specified width, height and texture format.
/// Returns a tuple containing the new texture, and the size of any required backing buffer
/// for the texture, in bytes.
/// Note that although this function returns a `Result`, it does not currently cannot fail.
/// If texture creation fails, wgpu will likely panic.
pub(crate) fn create_texture(
    device: Arc<wgpu::Device>,
    dim: TextureDimensions,
    texture_format: wgpu::TextureFormat,
) -> Result<(wgpu::Texture, usize), Error> {
    // Creating a texture in wgpu is fairly straightforward.
    // First we define the dimensions of the texture. wgpu uses `Extent3d` as textures can be
    // three-dimensional!
    let texture_extent = wgpu::Extent3d {
        width: dim.w,
        height: dim.h,
        depth_or_array_layers: 1,
    };

    // Next we create the texture with a `TextureDescriptor`.
    // Since we are just using orthographic textures, we don't need mipmaps or anything fancy.
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("martypc_eframe_wgpu_texture"),
        size: texture_extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: texture_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let texture_format_size = texture_format_size(texture_format);
    let backing_buffer_size = ((dim.w * dim.h) as f32 * texture_format_size) as usize;

    Ok((texture, backing_buffer_size))
}

/// Return whether the texture format requires a color swap from RGBA to BGRA. We can
/// accomplish this by swizzling in the shader.
#[inline]
pub const fn texture_format_color_swap(texture_format: wgpu::TextureFormat) -> bool {
    use wgpu::TextureFormat::*;
    match texture_format {
        Rgba8Unorm | Rgba8UnormSrgb => false,
        Bgra8Unorm | Bgra8UnormSrgb => true,
        _ => false,
    }
}

/// This function calculates the size of a single texel in bytes for a given texture format.
/// It will periodically need updating as wgpu adds more texture formats.
#[rustfmt::skip]
#[inline]
pub const fn texture_format_size(texture_format: wgpu::TextureFormat) -> f32 {
    use wgpu::{AstcBlock::*, TextureFormat::*};

    // TODO: Use constant arithmetic when supported.
    // See: https://github.com/rust-lang/rust/issues/57241
    match texture_format {
        // Note that these sizes are typically estimates. For instance, GPU vendors decide whether
        // their implementation uses 5 or 8 bytes per texel for formats like `Depth32PlusStencil8`.
        // In cases where it is unclear, we choose to overestimate.
        //
        // See:
        // - https://gpuweb.github.io/gpuweb/#plain-color-formats
        // - https://gpuweb.github.io/gpuweb/#depth-formats
        // - https://gpuweb.github.io/gpuweb/#packed-formats

        // 8-bit formats, 8 bits per component
        R8Unorm
        | R8Snorm
        | R8Uint
        | R8Sint
        | Stencil8 => 1.0, // 8.0 / 8.0

        // 16-bit formats, 8 bits per component
        R16Uint
        | R16Sint
        | R16Float
        | R16Unorm
        | R16Snorm
        | Rg8Unorm
        | Rg8Snorm
        | Rg8Uint
        | Rg8Sint
        | Rgb9e5Ufloat
        | Depth16Unorm => 2.0, // 16.0 / 8.0

        // 32-bit formats, 8 bits per component
        R32Uint
        | R32Sint
        | R32Float
        | Rg16Uint
        | Rg16Sint
        | Rg16Float
        | Rg16Unorm
        | Rg16Snorm
        | Rgba8Unorm
        | Rgba8UnormSrgb
        | Rgba8Snorm
        | Rgba8Uint
        | Rgba8Sint
        | Bgra8Unorm
        | Bgra8UnormSrgb
        | Rgb10a2Uint
        | Rgb10a2Unorm
        | Rg11b10Ufloat
        | Depth32Float
        | Depth24Plus
        | Depth24PlusStencil8 => 4.0, // 32.0 / 8.0

        // 64-bit formats, 8 bits per component
        Rg32Uint
        | Rg32Sint
        | Rg32Float
        | Rgba16Uint
        | Rgba16Sint
        | Rgba16Float
        | Rgba16Unorm
        | Rgba16Snorm
        | Depth32FloatStencil8 => 8.0, // 64.0 / 8.0

        // 128-bit formats, 8 bits per component
        Rgba32Uint
        | Rgba32Sint
        | Rgba32Float => 16.0, // 128.0 / 8.0

        // Compressed formats

        // 4x4 blocks, 8 bytes per block
        Bc1RgbaUnorm
        | Bc1RgbaUnormSrgb
        | Bc4RUnorm
        | Bc4RSnorm
        | Etc2Rgb8Unorm
        | Etc2Rgb8UnormSrgb
        | Etc2Rgb8A1Unorm
        | Etc2Rgb8A1UnormSrgb
        | EacR11Unorm
        | EacR11Snorm => 0.5, // 4.0 * 4.0 / 8.0

        // 4x4 blocks, 16 bytes per block
        Bc2RgbaUnorm
        | Bc2RgbaUnormSrgb
        | Bc3RgbaUnorm
        | Bc3RgbaUnormSrgb
        | Bc5RgUnorm
        | Bc5RgSnorm
        | Bc6hRgbUfloat
        | Bc6hRgbFloat
        | Bc7RgbaUnorm
        | Bc7RgbaUnormSrgb
        | EacRg11Unorm
        | EacRg11Snorm
        | Etc2Rgba8Unorm
        | Etc2Rgba8UnormSrgb
        | Astc { block: B4x4, channel: _ } => 1.0, // 4.0 * 4.0 / 16.0

        // 5x4 blocks, 16 bytes per block
        Astc { block: B5x4, channel: _ } => 1.25, // 5.0 * 4.0 / 16.0

        // 5x5 blocks, 16 bytes per block
        Astc { block: B5x5, channel: _ } => 1.5625, // 5.0 * 5.0 / 16.0

        // 6x5 blocks, 16 bytes per block
        Astc { block: B6x5, channel: _ } => 1.875, // 6.0 * 5.0 / 16.0

        // 6x6 blocks, 16 bytes per block
        Astc { block: B6x6, channel: _ } => 2.25, // 6.0 * 6.0 / 16.0

        // 8x5 blocks, 16 bytes per block
        Astc { block: B8x5, channel: _ } => 2.5, // 8.0 * 5.0 / 16.0

        // 8x6 blocks, 16 bytes per block
        Astc { block: B8x6, channel: _ } => 3.0, // 8.0 * 6.0 / 16.0

        // 8x8 blocks, 16 bytes per block
        Astc { block: B8x8, channel: _ } => 4.0, // 8.0 * 8.0 / 16.0

        // 10x5 blocks, 16 bytes per block
        Astc { block: B10x5, channel: _ } => 3.125, // 10.0 * 5.0 / 16.0

        // 10x6 blocks, 16 bytes per block
        Astc { block: B10x6, channel: _ } => 3.75, // 10.0 * 6.0 / 16.0

        // 10x8 blocks, 16 bytes per block
        Astc { block: B10x8, channel: _ } => 5.0, // 10.0 * 8.0 / 16.0

        // 10x10 blocks, 16 bytes per block
        Astc { block: B10x10, channel: _ } => 6.25, // 10.0 * 10.0 / 16.0

        // 12x10 blocks, 16 bytes per block
        Astc { block: B12x10, channel: _ } => 7.5, // 12.0 * 10.0 / 16.0

        // 12x12 blocks, 16 bytes per block
        Astc { block: B12x12, channel: _ } => 9.0, // 12.0 * 12.0 / 16.0

        // 8-bit two-plane 4:2:0 YUV
        NV12 => 1.5,
    }
}
