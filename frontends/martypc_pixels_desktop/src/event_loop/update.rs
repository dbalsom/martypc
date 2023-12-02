/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    event_loop/update.rs

    Process an event loop update
*/

use std::time::Instant;

use winit::event_loop::EventLoopWindowTarget;

use display_manager_wgpu::DisplayManager;
use marty_core::{
    machine::ExecutionState,
    videocard::RenderMode,
};
use marty_egui::GuiBoolean;

use crate::{
    Emulator,
    event_loop::{egui_update::update_egui, render_frame::render_frame},
    FPS_TARGET,
    MICROS_PER_FRAME,
    MIN_RENDER_HEIGHT,
    MIN_RENDER_WIDTH,
};

pub fn process_update(emu: &mut Emulator, elwt: &EventLoopWindowTarget<()>) {
    emu.stat_counter.current_ups += 1;

    // Calculate FPS
    let elapsed_ms = emu.stat_counter.last_second.elapsed().as_millis();
    if elapsed_ms > 1000 {
        // One second elapsed, calculate FPS/CPS
        let pit_ticks = emu.machine.pit_cycles();
        let cpu_cycles = emu.machine.cpu_cycles();
        let system_ticks = emu.machine.system_ticks();

        emu.stat_counter.current_cpu_cps = cpu_cycles - emu.stat_counter.last_cpu_cycles;
        emu.stat_counter.last_cpu_cycles = cpu_cycles;

        emu.stat_counter.current_pit_tps = pit_ticks - emu.stat_counter.last_pit_ticks;
        emu.stat_counter.last_pit_ticks = pit_ticks;

        emu.stat_counter.current_sys_tps = system_ticks - emu.stat_counter.last_system_ticks;
        emu.stat_counter.last_system_ticks = system_ticks;

        //println!("fps: {} | cps: {} | pit tps: {}",
        //    stat_counter.current_fps,
        //    stat_counter.current_cpu_cps,
        //    stat_counter.current_pit_tps);

        emu.stat_counter.ups = emu.stat_counter.current_ups;
        emu.stat_counter.current_ups = 0;
        emu.stat_counter.fps = emu.stat_counter.current_fps;
        emu.stat_counter.current_fps = 0;

        // Update IPS and reset instruction count for next second

        emu.stat_counter.current_cps = emu.stat_counter.cycle_count;
        emu.stat_counter.cycle_count = 0;

        emu.stat_counter.emulated_fps = emu.stat_counter.current_emulated_frames as u32;
        emu.stat_counter.current_emulated_frames = 0;

        emu.stat_counter.current_ips = emu.stat_counter.instr_count;
        emu.stat_counter.instr_count = 0;
        emu.stat_counter.last_second = Instant::now();
    }

    // Decide whether to draw a frame
    let elapsed_us = emu.stat_counter.last_frame.elapsed().as_micros();
    emu.stat_counter.last_frame = Instant::now();

    emu.stat_counter.accumulated_us += elapsed_us;

    while emu.stat_counter.accumulated_us > MICROS_PER_FRAME as u128 {
        emu.stat_counter.accumulated_us -= MICROS_PER_FRAME as u128;
        emu.stat_counter.last_frame = Instant::now();
        emu.stat_counter.frame_count += 1;
        emu.stat_counter.current_fps += 1;
        //println!("frame: {} elapsed: {}", world.current_fps, elapsed_us);

        // Get single step flag from GUI and either step or run CPU
        // TODO: This logic is messy, figure out a better way to control CPU state
        //       via gui

        //if framework.gui.get_cpu_single_step() {
        //    if framework.gui.get_cpu_step_flag() {
        //        machine.run(CYCLES_PER_FRAME, &exec_control.borrow(), 0);
        //    }
        //}
        //else {
        //    machine.run(CYCLES_PER_FRAME, &exec_control.borrow(), bp_addr);
        //    // Check for breakpoint
        //    if machine.cpu().get_flat_address() == bp_addr && bp_addr != 0 {
        //        log::debug!("Breakpoint hit at {:06X}", bp_addr);
        //        framework.gui.set_cpu_single_step();
        //    }
        //}

        if let Some(mouse) = emu.machine.mouse_mut() {
            // Send any pending mouse update to machine if mouse is captured
            if emu.mouse_data.is_captured && emu.mouse_data.have_update {
                mouse.update(
                    emu.mouse_data.l_button_was_pressed,
                    emu.mouse_data.r_button_was_pressed,
                    emu.mouse_data.frame_delta_x,
                    emu.mouse_data.frame_delta_y,
                );

                // Handle release event
                let l_release_state = if emu.mouse_data.l_button_was_released {
                    false
                }
                else {
                    emu.mouse_data.l_button_was_pressed
                };

                let r_release_state = if emu.mouse_data.r_button_was_released {
                    false
                }
                else {
                    emu.mouse_data.r_button_was_pressed
                };

                if emu.mouse_data.l_button_was_released || emu.mouse_data.r_button_was_released {
                    // Send release event
                    mouse.update(l_release_state, r_release_state, 0.0, 0.0);
                }

                // Reset mouse for next frame
                emu.mouse_data.reset();
            }
        }

        // Emulate a frame worth of instructions
        // ---------------------------------------------------------------------------

        // Recalculate cycle target based on current CPU speed if it has changed (or uninitialized)
        let mhz = emu.machine.get_cpu_mhz();
        if mhz != emu.stat_counter.cpu_mhz {
            emu.stat_counter.cycles_per_frame = (emu.machine.get_cpu_mhz() * 1000000.0 / FPS_TARGET) as u32;
            emu.stat_counter.cycle_target = emu.stat_counter.cycles_per_frame;
            log::info!(
                "CPU clock has changed to {}Mhz; new cycle target: {}",
                mhz,
                emu.stat_counter.cycle_target
            );
            emu.stat_counter.cpu_mhz = mhz;
        }

        let emulation_start = Instant::now();
        emu.stat_counter.instr_count += emu
            .machine
            .run(emu.stat_counter.cycle_target, &mut emu.exec_control.borrow_mut());
        emu.stat_counter.emulation_time = Instant::now() - emulation_start;

        // Add instructions to IPS counter
        emu.stat_counter.cycle_count += emu.stat_counter.cycle_target as u64;

        // Add emulated frames from video card device to emulated frame counter
        let mut frame_count = 0;
        if let Some(video_card) = emu.machine.primary_videocard() {
            // We have a video card to query
            frame_count = video_card.get_frame_count()
        }
        let elapsed_frames = frame_count - emu.stat_counter.emulated_frames;
        emu.stat_counter.emulated_frames += elapsed_frames;
        emu.stat_counter.current_emulated_frames += elapsed_frames;

        // Emulation time budget is 16ms - marty_render time in ms - fudge factor
        let render_time = emu.stat_counter.render_time.as_micros();
        let emulation_time = emu.stat_counter.emulation_time.as_micros();

        let mut emulation_time_allowed_us = 15667;
        if render_time < 15667 {
            // Rendering time has left us some emulation headroom
            emulation_time_allowed_us = 15667_u128.saturating_sub(render_time);
        }
        else {
            // Rendering is too long to run at 60fps. Just ignore marty_render time for now.
        }

        // If emulation time took too long, reduce CYCLE_TARGET
        if emulation_time > emulation_time_allowed_us {
            // Emulation running slower than 60fps
            let factor: f64 = (emu.stat_counter.emulation_time.as_micros() as f64) / emulation_time_allowed_us as f64;
            // Decrease speed by half of scaling factor

            let old_target = emu.stat_counter.cycle_target;
            let new_target = (emu.stat_counter.cycle_target as f64 / factor) as u32;
            emu.stat_counter.cycle_target -= (old_target - new_target) / 2;

            /*
            log::trace!("Emulation speed slow: ({}ms > {}ms). Reducing cycle target: {}->{}",
                emulation_time,
                emulation_time_allowed_ms,
                old_target,
                stat_counter.cycle_target
            );
            */
        }
        else if (emulation_time > 0) && (emulation_time < emulation_time_allowed_us) {
            // Emulation could run faster

            // Increase speed by half of scaling factor
            let factor: f64 = (emu.stat_counter.emulation_time.as_micros() as f64) / emulation_time_allowed_us as f64;

            let old_target = emu.stat_counter.cycle_target;
            let new_target = (emu.stat_counter.cycle_target as f64 / factor) as u32;
            emu.stat_counter.cycle_target += (new_target - old_target) / 2;

            if emu.stat_counter.cycle_target > emu.stat_counter.cycles_per_frame {
                // Warpspeed runs entire emulator as fast as possible
                // TODO: Limit cycle target based on marty_render/gui time to maintain 60fps GUI updates
                if !emu.config.emulator.warpspeed {
                    emu.stat_counter.cycle_target = emu.stat_counter.cycles_per_frame;
                }
            }
            else {
                /*
                log::trace!("Emulation speed recovering. ({}ms < {}ms). Increasing cycle target: {}->{}" ,
                    emulation_time,
                    emulation_time_allowed_ms,
                    old_target,
                    stat_counter.cycle_target
                );
                */
            }
        }

        /*
        log::debug!(
            "Cycle target: {} emulation time: {} allowed_ms: {}",
            stat_counter.cycle_target,
            emulation_time,
            emulation_time_allowed_ms
        );
        */

        // Do per-frame updates (Serial port emulation)
        emu.machine.frame_update();

        let render_start = Instant::now();

        // Render all videocards.
        emu.machine.for_each_videocard(|vci| {
            let mut new_w = 0;
            let mut new_h = 0;

            match vci.card.get_render_mode() {
                RenderMode::Direct => {
                    (new_w, new_h) = vci.card.get_display_aperture();
                }
                RenderMode::Indirect => {
                    (new_w, new_h) = vci.card.get_display_size();
                }
            }

            // If CGA, we will double scanlines later in the renderer, so make our buffer twice
            // as high.

            let doublescan = if vci.card.get_scanline_double() {
                new_h = new_h * 2;
                true
            }
            else {
                false
            };

            // Resize the card.
            if let Err(_) = emu.dm.on_card_resized(vci.id, new_w, new_h, doublescan) {
                log::error!("Error resizing videocard");
            }

            // Detect resolution changes and resize card.
            if new_w >= MIN_RENDER_WIDTH && new_h >= MIN_RENDER_HEIGHT {
                // Resolve video renderer by id.
                if let Some(renderer) = emu.dm.get_renderer_by_card_id(vci.id) {
                    if renderer.would_resize((new_w, new_h).into()) {
                        // Resize renderer & pixels
                        vci.card
                            .write_trace_log(format!("Setting internal resolution to ({},{})", new_w, new_h));
                        log::debug!(
                            "Aperture changed. Setting front buffer resolution to ({},{})",
                            new_w,
                            new_h
                        );
                        renderer.resize((new_w, new_h).into());

                        /*
                        if let Err(e) = pixels.resize_buffer(new_buf_size.w, new_buf_size.h) {
                            log::error!("Failed to resize pixel pixel buffer: {}", e);
                        }
                        pixels.frame_mut().fill(0);
                        */

                        //VideoRenderer::set_alpha(pixels.frame_mut(), new_buf_size.w, new_buf_size.h, 255);
                    }
                }
            }

            // Render videocard
            let _composite_enabled = emu.gui.get_option(GuiBoolean::CompositeDisplay).unwrap_or(false);
            let beam_pos;
            let videocard_buffer;

            // Get the appropriate buffer depending on run mode. If execution is paused
            // (debugging) show the back buffer instead of front buffer.
            // TODO: Discriminate between paused in debug mode vs user paused state
            match emu.exec_control.borrow_mut().get_state() {
                ExecutionState::Paused | ExecutionState::BreakpointHit | ExecutionState::Halted => {
                    if emu.gui.get_option(GuiBoolean::ShowBackBuffer).unwrap_or(false) {
                        videocard_buffer = vci.card.get_back_buf();
                    }
                    else {
                        videocard_buffer = vci.card.get_display_buf();
                    }
                    beam_pos = vci.card.get_beam_pos();
                }
                _ => {
                    videocard_buffer = vci.card.get_display_buf();
                    beam_pos = None;
                }
            }

            let extents = vci.card.get_display_extents();

            //log::debug!("extents: {}x{}", extents.field_w, extents.field_h);

            if let Some(renderer) = emu.dm.get_renderer_by_card_id(vci.id) {
                if renderer.get_mode_byte() != extents.mode_byte {
                    // Mode byte has changed, recalculate composite parameters
                    renderer.cga_direct_mode_update(extents.mode_byte);
                    renderer.set_mode_byte(extents.mode_byte);
                }

                emu.dm.render_card(vci.id);
                //video.draw_with_backend(videocard_buffer, &extents, composite_enabled, beam_pos);
            }
        });
        emu.stat_counter.render_time = Instant::now() - render_start;

        // Update egui data
        update_egui(emu, elwt);

        // Render the current frame for all window display targets.
        render_frame(emu);
    }
}
