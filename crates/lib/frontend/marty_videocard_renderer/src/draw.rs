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

    ---------------------------------------------------------------------------

    marty_videocard_renderer::draw.rs

    Drawing routines for VideoRenderer
*/

use crate::{consts::*, resize::*};
use marty_core::devices::cga;
use web_time::Instant;

use super::*;

impl VideoRenderer {
    pub fn clear(&mut self) {
        self.buf.fill(0);
    }

    /// Draw the direct (indexed) framebuffer created by a Videocard to the specified output buffer, given
    /// the specified display extents. This base method will call the appropriate drawing routine based on
    /// video card type. Optionally, the raster beam position can be visualized if 'beam_pos' is specified.
    pub fn draw(
        &mut self,
        input_buf: &[u8],
        output_buf: &mut [u8],
        extents: &DisplayExtents,
        beam_pos: Option<(u32, u32)>,
        palette: Option<Vec<[u8; 4]>>,
    ) {
        let render_start = Instant::now();

        let do_software_aspect = matches!(self.params.aspect_correction, AspectCorrectionMode::Software);
        let mut screenshot_taken = false;

        let (first_pass_buf, mut second_pass_buf) = if self.screenshot_requested {
            // Either we are rendering a screenshot this pass, or we are doing software aspect correction.
            // Render to internal buffer first instead of backend.
            (&mut self.screenshot_buf[..], Some(output_buf))
        }
        else if do_software_aspect {
            (&mut self.buf[..], Some(output_buf))
        }
        else {
            // Render directly to output buffer.
            (output_buf, None)
        };

        match self.video_type {
            VideoType::MDA => {
                VideoRenderer::draw_mda_direct_u32(
                    first_pass_buf,
                    self.params.render.w,
                    self.params.render.h,
                    input_buf,
                    self.params.aperture,
                    extents,
                );
            }
            VideoType::CGA | VideoType::TGA => {
                if self.composite_enabled {
                    VideoRenderer::draw_cga_direct_composite_reenigne(
                        first_pass_buf,
                        self.params.render.w,
                        self.params.render.h,
                        input_buf,
                        &mut self.composite_bufs,
                        &mut self.composite_ctx,
                        &self.composite_params,
                        self.params.aperture,
                        extents,
                    );
                }
                else {
                    VideoRenderer::draw_cga_direct_u32(
                        first_pass_buf,
                        self.params.render.w,
                        self.params.render.h,
                        input_buf,
                        self.params.aperture,
                        extents,
                    )
                }
            }
            #[cfg(feature = "ega")]
            VideoType::EGA => VideoRenderer::draw_ega_direct_u32(
                first_pass_buf,
                self.params.render.w,
                self.params.render.h,
                input_buf,
                self.params.aperture,
                extents,
                RenderBpp::Six,
            ),
            #[cfg(feature = "vga")]
            VideoType::VGA => VideoRenderer::draw_vga_direct_u32(
                first_pass_buf,
                palette.expect("VGA did not provide a palette!"),
                self.params.render.w,
                self.params.render.h,
                input_buf,
                self.params.aperture,
                extents,
            ),
        }

        // Draw raster beam position if provided
        if let Some(beam) = beam_pos {
            let beam_x = beam.0 - extents.apertures[self.params.aperture as usize].x;
            let mut beam_y = beam.1 - &extents.apertures[self.params.aperture as usize].y;
            if self.params.line_double {
                beam_y *= 2
            };

            VideoRenderer::draw_horizontal_xor_line(
                first_pass_buf,
                self.params.render.w,
                self.params.render.w,
                self.params.render.h,
                beam_y,
            );
            VideoRenderer::draw_vertical_xor_line(
                first_pass_buf,
                self.params.render.w,
                self.params.render.w,
                self.params.render.h,
                beam_x,
            );
        }

        // We have now drawn to 'first_pass_buf' which might have been internal or the buffer
        // specified by the draw() call (most likely the backend's display buffer).

        if self.screenshot_requested {
            // We can now write the screenshot out to the specified file, then copy the image to the
            // second pass buffer.
            if let Some(path) = self.screenshot_path.as_ref() {
                // We may not have been given a path, in which case the caller can retrieve the
                // internal screenshot buffer, ie, for use in a GUI.
                match image::save_buffer(
                    path.clone(),
                    first_pass_buf,
                    self.params.render.w,
                    self.params.render.h,
                    image::ColorType::Rgba8,
                ) {
                    Ok(_) => {
                        screenshot_taken = true;
                        log::info!("Saved screenshot: {}", path.display())
                    }
                    Err(e) => {
                        log::error!("Error writing screenshot: {}: {}", path.display(), e)
                    }
                }
            }

            if !do_software_aspect {
                // If software aspect correction is on, we don't need to blit the image as the resampling
                // operation will take care of it. But if not, we need to do it ourselves.
                if let Some(ref mut second_pass) = second_pass_buf {
                    second_pass.copy_from_slice(first_pass_buf);
                }
            }

            self.screenshot_path = None;
            self.screenshot_requested = false;
        }

        // If we are doing software aspect correction, we now need to draw into the output_buf.
        if do_software_aspect {
            if let Some(second_pass) = second_pass_buf {
                //log::debug!("Performing aspect correction...");
                resize_linear_fast(
                    first_pass_buf,
                    self.params.render.w,
                    self.params.render.h,
                    second_pass,
                    self.params.aspect_corrected.w,
                    self.params.aspect_corrected.h,
                    &mut self.resample_context,
                );
            }
        }

        if screenshot_taken {
            self.send_event(RendererEvent::ScreenshotSaved);
        }

        self.last_render_time = render_start.elapsed();
        //log::debug!("render time: {}", self.last_render_time.as_secs_f64());
    }

    pub fn draw_horizontal_xor_line_2x(&mut self, frame: &mut [u8], w: u32, span: u32, h: u32, y: u32) {
        if y > (h - 1) {
            return;
        }

        let frame_row0_offset = ((y * 2) * (span * 4)) as usize;
        let frame_row1_offset = (((y * 2) * (span * 4)) + (span * 4)) as usize;

        for x in 0..w {
            let fo0 = frame_row0_offset + (x * 4) as usize;
            let fo1 = frame_row1_offset + (x * 4) as usize;

            let r = frame[fo0];
            let g = frame[fo0 + 1];
            let b = frame[fo0 + 2];

            frame[fo1] = r ^ XOR_COLOR;
            frame[fo1 + 1] = g ^ XOR_COLOR;
            frame[fo1 + 2] = b ^ XOR_COLOR;
        }
    }

    pub fn draw_horizontal_xor_line(frame: &mut [u8], w: u32, span: u32, h: u32, y: u32) {
        if y > (h - 1) {
            return;
        }

        let frame_row0_offset = (y * (span * 4)) as usize;

        for x in 0..w {
            let fo0 = frame_row0_offset + (x * 4) as usize;

            let r = frame[fo0];
            let g = frame[fo0 + 1];
            let b = frame[fo0 + 2];

            frame[fo0] = r ^ XOR_COLOR;
            frame[fo0 + 1] = g ^ XOR_COLOR;
            frame[fo0 + 2] = b ^ XOR_COLOR;
        }
    }

    pub fn draw_vertical_xor_line_2x(frame: &mut [u8], w: u32, span: u32, h: u32, x: u32) {
        if x > (w - 1) {
            return;
        }

        let frame_x0_offset = (x * 4) as usize;

        for y in 0..h {
            let fo0 = frame_x0_offset + ((y * 2) * (span * 4)) as usize;
            let fo1 = frame_x0_offset + (((y * 2) * (span * 4)) + (span * 4)) as usize;

            let r = frame[fo0];
            let g = frame[fo0 + 1];
            let b = frame[fo0 + 2];

            frame[fo0] = r ^ XOR_COLOR;
            frame[fo0 + 1] = g ^ XOR_COLOR;
            frame[fo0 + 2] = b ^ XOR_COLOR;

            frame[fo1] = r ^ XOR_COLOR;
            frame[fo1 + 1] = g ^ XOR_COLOR;
            frame[fo1 + 2] = b ^ XOR_COLOR;
        }
    }

    pub fn draw_vertical_xor_line(frame: &mut [u8], w: u32, span: u32, h: u32, x: u32) {
        if x > (w - 1) {
            return;
        }

        let frame_x0_offset = (x * 4) as usize;

        for y in 0..h {
            let fo0 = frame_x0_offset + (y * (span * 4)) as usize;

            let r = frame[fo0];
            let g = frame[fo0 + 1];
            let b = frame[fo0 + 2];

            frame[fo0] = r ^ XOR_COLOR;
            frame[fo0 + 1] = g ^ XOR_COLOR;
            frame[fo0 + 2] = b ^ XOR_COLOR;
        }
    }

    /// Set the alpha component of each pixel in a the specified buffer.
    pub fn set_alpha(frame: &mut [u8], w: u32, h: u32, a: u8) {
        //log::warn!("set_alpha: h: {}", h);

        for o in (0..((w * h * 4) as usize)).step_by(4) {
            frame[o + 3] = a;
        }
    }

    /// Draw the CGA card in Direct Mode.
    /// The CGA in Direct mode generates its own indexed-color framebuffer, which is
    /// converted to 32-bit RGBA for display based on the selected display aperture profile.
    ///
    /// This version uses bytemuck to convert the framebuffer 32 bits at a time, which
    /// is much faster (benchmarked)
    pub fn draw_cga_direct_u32(
        frame: &mut [u8],
        w: u32,
        h: u32,
        dbuf: &[u8],
        aperture: DisplayApertureType,
        extents: &DisplayExtents,
    ) {
        let aperture = &extents.apertures[aperture as usize];

        let mut horiz_adjust = aperture.x;
        let mut vert_adjust = aperture.y;
        // Ignore aperture adjustments if it pushes us outside the field boundaries
        if aperture.x + aperture.w >= extents.field_w {
            horiz_adjust = 0;
        }
        if aperture.y + aperture.h >= extents.field_h {
            vert_adjust = 0;
        }

        let max_y = std::cmp::min(h / 2, aperture.h);
        let max_x = std::cmp::min(w, aperture.w);

        //log::debug!("w: {w} h: {h} max_x: {max_x}, max_y: {max_y}");

        let frame_u32: &mut [u32] = bytemuck::cast_slice_mut(frame);

        for y in 0..max_y {
            let dbuf_row_offset = (y + vert_adjust) as usize * extents.row_stride;

            let frame_row0_offset = ((y * 2) * w) as usize;
            let frame_row1_offset = (((y * 2) * w) + w) as usize;

            for x in 0..max_x {
                let fo0 = frame_row0_offset + x as usize;
                let fo1 = frame_row1_offset + x as usize;

                let dbo = dbuf_row_offset + (x + horiz_adjust) as usize;

                // TODO: Would it be better for cache concurrency to do one line at a time?
                frame_u32[fo0] = CGA_RGBA_COLORS_U32[0][(dbuf[dbo] & 0x0F) as usize];
                frame_u32[fo1] = CGA_RGBA_COLORS_U32[0][(dbuf[dbo] & 0x0F) as usize];
            }
        }
    }

    /// Render the CGA Direct framebuffer as a composite artifact color simulation.
    pub fn draw_cga_direct_composite(
        &mut self,
        frame: &mut [u8],
        w: u32,
        h: u32,
        dbuf: &[u8],
        aperture: DisplayApertureType,
        extents: &DisplayExtents,
        composite_params: &CompositeParams,
    ) {
        let aperture = &extents.apertures[aperture as usize];

        if let Some(composite_buf) = &mut self.composite_buf {
            let max_w = std::cmp::min(w, aperture.w);
            let max_h = std::cmp::min(h / 2, aperture.h);

            //log::debug!("composite: w: {w} h: {h} max_w: {max_w}, max_h: {max_h}");
            //log::debug!("composite: aperture.x: {}", extents.aperture.x);

            process_cga_composite_int(
                dbuf,
                aperture.w,
                aperture.h,
                aperture.x,
                aperture.y,
                extents.row_stride as u32,
                composite_buf,
            );

            // Regen sync table if width changed
            if self.sync_table_w != (max_w * 2) {
                self.sync_table
                    .resize(((max_w * 2) + CCYCLE as u32) as usize, (0.0, 0.0, 0.0));
                regen_sync_table(&mut self.sync_table, (max_w * 2) as usize);
                // Update to new width
                self.sync_table_w = max_w * 2;
            }

            artifact_colors_fast(
                composite_buf,
                max_w * 2,
                max_h,
                &self.sync_table,
                frame,
                max_w,
                max_h,
                composite_params.hue as f32,
                composite_params.sat as f32,
                composite_params.luma as f32,
            );
        }
    }

    /// Render the CGA Direct framebuffer as a composite artifact color simulation.
    /// This version uses bytemuck to convert the framebuffer 32 bits at a time, which is
    /// much faster (benchmarked)
    pub fn draw_cga_direct_composite_u32(
        &mut self,
        frame: &mut [u8],
        w: u32,
        h: u32,
        dbuf: &[u8],
        aperture: DisplayApertureType,
        extents: &DisplayExtents,
        composite_params: &CompositeParams,
    ) {
        let aperture = &extents.apertures[aperture as usize];

        if let Some(composite_buf) = &mut self.composite_buf {
            let max_w = std::cmp::min(w, aperture.w);
            let max_h = std::cmp::min(h / 2, aperture.h);

            //log::debug!("composite: w: {w} h: {h} max_w: {max_w}, max_h: {max_h}");

            process_cga_composite_int(
                dbuf,
                aperture.w,
                aperture.h,
                aperture.x,
                aperture.y,
                extents.row_stride as u32,
                composite_buf,
            );

            // Regen sync table if width changed
            if self.sync_table_w != (max_w * 2) {
                self.sync_table
                    .resize(((max_w * 2) + CCYCLE as u32) as usize, (0.0, 0.0, 0.0));
                regen_sync_table(&mut self.sync_table, (max_w * 2) as usize);
                // Update to new width
                self.sync_table_w = max_w * 2;
            }

            artifact_colors_fast_u32(
                composite_buf,
                max_w * 2,
                max_h,
                &self.sync_table,
                frame,
                max_w,
                max_h,
                composite_params.hue as f32,
                composite_params.sat as f32,
                composite_params.luma as f32,
            );
        }
    }

    /// Render the CGA Direct framebuffer as a composite artifact color simulation.
    ///
    /// This version uses reenigne's composite color multiplexer algorithm.
    /// It is 3x faster than my sampling algorithm and produces more accurate colors;
    /// I know when I'm beat.
    pub fn draw_cga_direct_composite_reenigne(
        frame: &mut [u8],
        w: u32,
        h: u32,
        dbuf: &[u8],
        bufs: &mut ReCompositeBuffers,
        ctx: &mut ReCompositeContext,
        params: &CompositeParams,
        aperture: DisplayApertureType,
        extents: &DisplayExtents,
    ) {
        let aperture = &extents.apertures[aperture as usize];

        let phase_adjust = if aperture.w < (extents.field_w - 4) {
            // We have room to shift phase
            params.phase
        }
        else {
            // No room to adjust phase, disable phase adjustment.
            0
        };

        // Convert to composite line by line
        for y in 0..(h / 2) {
            //let s_o (= ((y * w) ) as usize;
            let s_o = ((y + aperture.y) as usize * extents.row_stride) + (aperture.x as usize) + phase_adjust;
            let d_o = ((y * 2) as usize) * ((w as usize) * size_of::<u32>());

            let in_slice = &dbuf[s_o..(s_o + (w as usize))];

            let d_span = (w as usize) * size_of::<u32>();
            let d_end = d_o + d_span + d_span;
            // Create an output slice that is 2x one scanline. We copy the first part of the scanline
            // to the last part after scanline processing to double scanlines.
            let out_slice = &mut frame[d_o..d_end];
            let out_slice32: &mut [u32] = bytemuck::cast_slice_mut(out_slice);

            ctx.composite_process(0, w as usize, bufs, in_slice, out_slice32);

            out_slice32.copy_within(0..(w as usize), w as usize);
        }
    }

    /// Inform the CGA Direct renderer of mode changes. This is only really required by
    /// reenigne's composite conversion algorithm as it will recalculate composite parameters
    /// based on the hires or color mode bits changing.
    pub fn cga_direct_mode_update(&mut self, mode: u8) {
        // Ignore enable bit when comparing mode.
        if (mode & cga::CGA_MODE_ENABLE_MASK) != (self.last_cga_mode & cga::CGA_MODE_ENABLE_MASK) {
            // Mode has changed; recalculate composite parameters.
            //log::debug!("mode changed: new:{:02X} old:{:02X} recalculating composite parameters...", mode, self.last_cga_mode);
            self.composite_ctx.recalculate(mode);
            self.last_cga_mode = mode;
        }
    }

    /// Inform the CGA Direct renderer of adjustment changes.
    /// reenigne's composite conversion algorithm will recalculate composite parameters
    /// when adjustments are changed.
    pub fn cga_direct_param_update(&mut self, composite_params: &CompositeParams) {
        self.composite_ctx.adjust(composite_params);
        self.composite_ctx.recalculate(self.last_cga_mode);

        self.composite_params = *composite_params;
    }

    /// Draw the MDA card in Direct Mode.
    /// The MDA in Direct mode generates its own indexed-color framebuffer, which is
    /// converted to 32-bit RGBA for display based on the selected display aperture profile.
    ///
    /// This version uses bytemuck to convert the framebuffer 32 bits at a time, which
    /// is much faster (benchmarked)
    pub fn draw_mda_direct_u32(
        frame: &mut [u8],
        w: u32,
        h: u32,
        dbuf: &[u8],
        aperture_type: DisplayApertureType,
        extents: &DisplayExtents,
    ) {
        let index_mask = if let DisplayApertureType::Debug = aperture_type {
            // Allow all 16 colors for debug drawing
            0x0F
        }
        else {
            // Limit to 4 colors.
            0x03
        };

        let aperture = &extents.apertures[aperture_type as usize];

        let mut horiz_adjust = aperture.x;
        let mut vert_adjust = aperture.y;
        // Ignore aperture adjustments if it pushes us outside of the field boundaries
        if aperture.x + aperture.w >= extents.field_w {
            horiz_adjust = 0;
        }
        if aperture.y + aperture.h >= extents.field_h {
            vert_adjust = 0;
        }

        let max_y = std::cmp::min(h, aperture.h);
        let max_x = std::cmp::min(w, aperture.w);
        let frame_u32: &mut [u32] = bytemuck::cast_slice_mut(frame);

        for y in 0..max_y {
            let dbuf_row_offset = (y + vert_adjust) as usize * extents.row_stride;
            let frame_row0_offset = (y * w) as usize;

            for x in 0..max_x {
                let fo0 = frame_row0_offset + x as usize;
                let dbo = dbuf_row_offset + (x + horiz_adjust) as usize;
                frame_u32[fo0] = MDA_RGBA_COLORS_U32[(dbuf[dbo] & index_mask) as usize];
            }
        }
    }

    /// Draw the EGA card in Direct Mode.
    /// The EGA in Direct mode generates its own indexed-color framebuffer, which is
    /// converted to 32-bit RGBA for display based on the selected display aperture profile.
    pub fn draw_ega_direct_u32(
        frame: &mut [u8],
        w: u32,
        mut h: u32,
        dbuf: &[u8],
        aperture: DisplayApertureType,
        extents: &DisplayExtents,
        bpp: RenderBpp,
    ) {
        let aperture = &extents.apertures[aperture as usize];

        let mut horiz_adjust = aperture.x;
        let mut vert_adjust = aperture.y;
        // Ignore aperture adjustments if it pushes us outside of the field boundaries
        if aperture.x + aperture.w >= extents.field_w {
            horiz_adjust = 0;
        }
        if aperture.y + aperture.h >= extents.field_h {
            vert_adjust = 0;
        }

        if extents.double_scan {
            h = h / 2;
        }

        if h as usize * extents.row_stride > dbuf.len() {
            log::warn!(
                "draw_ega_direct_u32(): extents {}x{} greater than buffer: {}",
                w,
                h,
                dbuf.len()
            );
            return;
        }

        let max_y = std::cmp::min(h, aperture.h + aperture.x);
        let max_x = std::cmp::min(w, aperture.w + aperture.y);

        //log::debug!("w: {w} h: {h} max_x: {max_x}, max_y: {max_y}");

        let frame_u32: &mut [u32] = bytemuck::cast_slice_mut(frame);

        match bpp {
            RenderBpp::Four => {
                if extents.double_scan {
                    for y in 0..max_y {
                        let dbuf_row_offset = (y + vert_adjust) as usize * extents.row_stride;

                        let frame_row0_offset = ((y * 2) * w) as usize;
                        let frame_row1_offset = (((y * 2) * w) + w) as usize;

                        for x in 0..max_x {
                            let fo0 = frame_row0_offset + x as usize;
                            let fo1 = frame_row1_offset + x as usize;

                            let dbo = dbuf_row_offset + (x + horiz_adjust) as usize;

                            // TODO: Would it be better for cache concurrency to do one line at a time?
                            frame_u32[fo0] = CGA_RGBA_COLORS_U32[0][(dbuf[dbo] & 0x0F) as usize];
                            frame_u32[fo1] = CGA_RGBA_COLORS_U32[0][(dbuf[dbo] & 0x0F) as usize];
                        }
                    }
                }
                else {
                    for y in 0..max_y {
                        let dbuf_row_offset = (y + vert_adjust) as usize * extents.row_stride;
                        let frame_row_offset = (y * w) as usize;

                        for x in 0..max_x {
                            let fo = frame_row_offset + x as usize;
                            let dbo = dbuf_row_offset + (x + horiz_adjust) as usize;

                            frame_u32[fo] = CGA_RGBA_COLORS_U32[0][(dbuf[dbo] & 0x0F) as usize];
                        }
                    }
                }
            }
            RenderBpp::Six => {
                if extents.double_scan {
                    for y in 0..max_y {
                        let dbuf_row_offset = (y + vert_adjust) as usize * extents.row_stride;

                        let frame_row0_offset = ((y * 2) * w) as usize;
                        let frame_row1_offset = (((y * 2) * w) + w) as usize;

                        for x in 0..max_x {
                            let fo0 = frame_row0_offset + x as usize;
                            let fo1 = frame_row1_offset + x as usize;

                            let dbo = dbuf_row_offset + (x + horiz_adjust) as usize;

                            // TODO: Would it be better for cache concurrency to do one line at a time?
                            frame_u32[fo0] = EGA_RGBA_COLORS_U32[(dbuf[dbo] & 0x3F) as usize];
                            frame_u32[fo1] = EGA_RGBA_COLORS_U32[(dbuf[dbo] & 0x3F) as usize];
                        }
                    }
                }
                else {
                    for y in 0..max_y {
                        let dbuf_row_offset = (y + vert_adjust) as usize * extents.row_stride;
                        let frame_row_offset = (y * w) as usize;

                        for x in 0..max_x {
                            let fo = frame_row_offset + x as usize;
                            let dbo = dbuf_row_offset + (x + horiz_adjust) as usize;

                            frame_u32[fo] = EGA_RGBA_COLORS_U32[(dbuf[dbo] & 0x3F) as usize];
                        }
                    }
                }
            }
            _ => {
                unreachable!("EGA: Unimplemented BPP mode!");
            }
        }
    }

    /// Draw the VGA card in Direct Mode.
    /// The VGA in Direct mode generates its own indexed-color framebuffer, which is
    /// converted to 32-bit RGBA for display based on the provided palette and
    /// selected display aperture profile.
    pub fn draw_vga_direct_u32(
        frame: &mut [u8],
        palette: Vec<[u8; 4]>,
        w: u32,
        mut h: u32,
        dbuf: &[u8],
        aperture: DisplayApertureType,
        extents: &DisplayExtents,
    ) {
        let aperture = &extents.apertures[aperture as usize];

        let mut horiz_adjust = aperture.x;
        let mut vert_adjust = aperture.y;
        // Ignore aperture adjustments if it pushes us outside of the field boundaries
        if aperture.x + aperture.w >= extents.field_w {
            horiz_adjust = 0;
        }
        if aperture.y + aperture.h >= extents.field_h {
            vert_adjust = 0;
        }

        if extents.double_scan {
            h = h / 2;
        }

        if h as usize * extents.row_stride > dbuf.len() {
            log::warn!(
                "draw_vga_direct_u32(): extents {}x{} greater than buffer: {}",
                w,
                h,
                dbuf.len()
            );
            return;
        }

        let max_y = std::cmp::min(h, aperture.h + aperture.x);
        let max_x = std::cmp::min(w, aperture.w + aperture.y);

        //log::debug!("w: {w} h: {h} max_x: {max_x}, max_y: {max_y}");

        let frame_u32: &mut [u32] = bytemuck::cast_slice_mut(frame);
        let palette_u32: &[u32] = bytemuck::cast_slice(&palette);

        for y in 0..max_y {
            let dbuf_row_offset = (y + vert_adjust) as usize * extents.row_stride;
            let frame_row_offset = (y * w) as usize;

            for x in 0..max_x {
                let fo = frame_row_offset + x as usize;
                let dbo = dbuf_row_offset + (x + horiz_adjust) as usize;
                frame_u32[fo] = palette_u32[dbuf[dbo] as usize];
            }
        }
    }
}
