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

    event_loop/egui_update

    Update the egui menu and widget state.
*/
use crate::{emulator::Emulator, event_loop::egui_events::handle_egui_event};

use display_manager_eframe::{DisplayManager, EFrameDisplayManager};
use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::Cpu,
    cpu_common,
    cpu_common::{CpuAddress, CpuOption, TraceMode},
    machine,
    syntax_token::SyntaxToken,
    util,
};
use marty_egui::GuiWindow;
use marty_frontend_common::timestep_manager::{TimestepManager, TimestepUpdate};

pub fn update_egui(emu: &mut Emulator, dm: &mut EFrameDisplayManager, tm: &TimestepManager, tmu: &mut TimestepUpdate) {
    // Is the machine in an error state? If so, display an error dialog.
    if let Some(err) = emu.machine.get_error_str() {
        emu.gui.show_error(err);
        emu.gui.show_window(GuiWindow::DisassemblyViewer);
    }
    else {
        // No error? Make sure we close the error dialog.
        emu.gui.clear_error();
    }

    // Handle custom events received from our GUI
    loop {
        if let Some(gui_event) = emu.gui.get_event() {
            //log::warn!("Handling GUI event!");
            handle_egui_event(emu, dm, tm, tmu, &gui_event);
        }
        else {
            break;
        }
    }

    // -- Update machine state
    emu.gui.set_machine_state(emu.machine.get_state());

    // -- Update sound sources
    if let Some(si) = emu.si.as_ref() {
        emu.gui.set_sound_state(si.info());
    }

    // -- Update gamepads, but only if we have a game port
    if let Some(gameport) = emu.machine.bus().game_port() {
        let gamepad_vec = emu.gi.gamepads().collect::<Vec<_>>();
        emu.gui.set_gameport(true, gameport.controller_layout());

        // log::debug!(
        //     "got gamepads: {:?}",
        //     gamepad_vec.iter().map(|i| i.name.clone()).collect::<Vec<_>>()
        // );
        emu.gui.set_gamepads(gamepad_vec);
    }

    // -- Update serial ports
    emu.gui.set_serial_ports(emu.machine.bus().enumerate_serial_ports());

    // -- Update VHD Creator window
    if emu.gui.is_window_open(GuiWindow::VHDCreator) {
        if let Some(hdc) = emu.machine.hdc_mut() {
            emu.gui.vhd_creator.set_formats(hdc.get_supported_formats());
        }
        else if let Some(hdc) = emu.machine.xtide_mut() {
            emu.gui.vhd_creator.set_formats(hdc.get_supported_formats());
        }
        else {
            log::error!("Couldn't query available formats: No Hard Disk Controller present!");
        }
    }

    // --------------------------------------------------------------------------------------------
    // Update the various debug windows
    // --------------------------------------------------------------------------------------------

    // -- Update CPU Control
    if emu.gui.is_window_open(GuiWindow::CpuControl) {
        let step_over_target = emu.machine.cpu().get_step_over_breakpoint();
        emu.gui.cpu_control.set_step_over_target(step_over_target);

        // Update stopwatch data
        let stopwatch_data = emu.machine.cpu().get_sw_data();
        emu.gui.cpu_control.set_stopwatch_data(stopwatch_data);
    }

    // -- Update CPU state viewer
    if emu.gui.is_window_open(GuiWindow::CpuStateViewer) {
        let cpu_state = emu.machine.cpu().get_string_state();
        emu.gui.cpu_viewer.update_state(cpu_state);
    }

    // -- Update performance viewer
    if emu.gui.is_window_open(GuiWindow::PerfViewer) {
        dm.with_primary_renderer_mut(|renderer| {
            emu.gui.perf_viewer.update_video_data(renderer.params());
        });

        let dti = dm.display_info(&emu.machine);

        let mut sound_stats = Vec::new();
        if let Some(si) = emu.si.as_ref() {
            sound_stats = si.info();
        }

        let (_, frame_history) = tm.get_perf_stats();

        //emu.gui.perf_viewer.update_video_data(*video.params());
        emu.gui.perf_viewer.update(dti, sound_stats, &emu.perf, frame_history)
    }

    // -- Update memory viewer window if open
    if emu.gui.is_window_open(GuiWindow::MemoryViewer) {
        let vewport_len = emu.gui.memory_viewer.viewport_len();
        let (mem_dump_addr_str, _source) = emu.gui.memory_viewer.get_address();
        let (addr, mem_dump_addr) = match emu.machine.cpu().eval_address(&mem_dump_addr_str) {
            Some(i) => {
                let addr: u32 = i.into();
                // Dump at 16 byte block boundaries
                (addr, addr & !0x0F)
            }
            None => {
                // Show address 0 if expression eval fails
                (0, 0)
            }
        };

        let mem_dump_vec = emu
            .machine
            .bus()
            .dump_flat_tokens_ex(mem_dump_addr as usize, addr as usize, vewport_len);

        //framework.gui.memory_viewer.set_row(mem_dump_addr as usize);

        emu.gui.memory_viewer.set_address(addr as usize);
        emu.gui.memory_viewer.set_memory(mem_dump_vec);
    }

    // Update data visualizer
    if emu.gui.is_window_open(GuiWindow::DataVisualizer) {
        let path_opt = emu.rm.resource_path("dump");
        if let Some(path) = path_opt {
            emu.gui.data_visualizer.set_dump_path(path);
        }

        let (viz_addr_str, viz_offset) = emu.gui.data_visualizer.get_address();
        let addr = match emu.machine.cpu().eval_address(&viz_addr_str) {
            Some(i) => {
                let addr: usize = i.into();
                addr + viz_offset
            }
            None => {
                // Show address 0 if expression eval fails
                0
            }
        };

        let data_len = emu.gui.data_visualizer.get_required_data_size();
        emu.gui
            .data_visualizer
            .update_data(emu.machine.bus().get_vec_at_ex(addr, data_len));

        emu.machine.primary_videocard().map(|vc| {
            let palette = vc.get_palette();
            if let Some(pal) = palette {
                emu.gui.data_visualizer.update_palette_u8(pal);
            }
        });
    }

    // -- Update IVR viewer window if open
    if emu.gui.is_window_open(GuiWindow::IvtViewer) {
        let vec = emu.machine.bus_mut().dump_ivt_tokens();
        emu.gui.ivt_viewer.set_content(vec);
    }

    // -- Update IO stats viewer window if open
    if emu.gui.is_window_open(GuiWindow::IoStatsViewer) {
        let vec = emu.machine.bus_mut().dump_io_stats();
        emu.gui.io_stats_viewer.set_content(vec);
    }

    // -- Update PIT viewer window
    if emu.gui.is_window_open(GuiWindow::PitViewer) {
        let pit_state = emu.machine.pit_state();
        emu.gui.update_pit_state(&pit_state);

        //let pit_data = emu.machine.get_pit_buf();
        //emu.gui.pit_viewer.update_channel_data(2, &pit_data);
    }

    // -- Update Serial port viewer window
    if emu.gui.is_window_open(GuiWindow::SerialViewer) {
        let serial_state = emu.machine.serial_state();
        emu.gui.serial_viewer.update_state(&serial_state);
    }

    // -- Update PIC viewer window
    if emu.gui.is_window_open(GuiWindow::PicViewer) {
        let pic_state = emu.machine.pic_state();
        emu.gui.pic_viewer.update_state(&pic_state);
    }

    // -- Update PPI viewer window
    if emu.gui.is_window_open(GuiWindow::PpiViewer) {
        /*        let ppi_state_opt = emu.machine.ppi_state();
        if let Some(ppi_state) = ppi_state_opt {
            emu.gui.ppi_viewer.set_state(ppi_state);
            // TODO: If no PPI, disable debug window
        }*/

        let ppi_state_opt = emu.machine.ppi_display_state();
        if let Some(ppi_state) = ppi_state_opt {
            emu.gui.ppi_viewer.update_state(ppi_state);
        }
    }

    // -- Update DMA viewer window
    if emu.gui.is_window_open(GuiWindow::DmaViewer) {
        let dma_state = emu.machine.dma_state();
        emu.gui.dma_viewer.update_state(dma_state);
    }

    // -- Update FDC viewer window
    if emu.gui.is_window_open(GuiWindow::FdcViewer) {
        if let Some(fdc_state) = emu.machine.fdc_state() {
            //log::warn!("updating fdc viewer with {} log items", fdc_state.cmd_log.len());
            emu.gui.fdc_viewer.update_state(fdc_state);
        }
    }

    // -- Update floppy viewer window
    if emu.gui.is_window_open(GuiWindow::FloppyViewer) {
        if let Some(image_state) = emu.machine.floppy_image_state() {
            emu.gui.floppy_viewer.update_state(image_state);
        }

        let di = emu.gui.floppy_viewer.get_drive_idx();

        if let (Some(_image_lock), viz_writes) = emu.machine.floppy_image(di) {
            emu.gui.floppy_viewer.update_visualization(di, viz_writes);
        }
    }

    // -- Update VideoCard Viewer window
    if emu.gui.is_window_open(GuiWindow::VideoCardViewer) {
        // Only have an update if we have a videocard to update.
        if let Some(videocard_state) = emu.machine.videocard_state() {
            emu.gui.update_videocard_state(videocard_state);
        }
    }

    // -- Update Instruction Trace window
    if emu.gui.is_window_open(GuiWindow::InstructionHistoryViewer) {
        let trace = emu.machine.cpu().dump_instruction_history_tokens();
        emu.gui.trace_viewer.set_content(trace);
    }

    // -- Update Call Stack window
    if emu.gui.is_window_open(GuiWindow::CallStack) {
        let stack = emu.machine.cpu().dump_call_stack();
        emu.gui.call_stack_viewer.set_content(stack);
    }

    // -- Update cycle trace viewer window
    if emu.gui.is_window_open(GuiWindow::CycleTraceViewer) {
        if emu.machine.get_cpu_option(CpuOption::TraceLoggingEnabled(true)) {
            match emu.config.machine.cpu.trace_mode {
                Some(TraceMode::CycleText) => {
                    let trace_vec = emu.machine.cpu().get_cycle_trace();
                    emu.gui.cycle_trace_viewer.update(trace_vec);
                }
                Some(TraceMode::CycleCsv) => {
                    let trace_vec = emu.machine.cpu().get_cycle_trace_tokens();
                    emu.gui.cycle_trace_viewer.update_tokens(trace_vec);
                }
                _ => {}
            }
        }
    }

    /*
    // temp: joystick state display. implement joystick debug window?
    if let Some(gameport) = emu.machine.bus_mut().game_port_mut() {
        let state = gameport.get_state();
        log::debug!(
            "pos: {},{} resistance: {},{}",
            state.sticks[0].0,
            state.sticks[0].1,
            state.resistance[0].0,
            state.resistance[0].1,
        );
    }
    */

    // -- Update disassembly viewer window
    if emu.gui.is_window_open(GuiWindow::DisassemblyViewer) {
        let start_addr_str = emu.gui.disassembly_viewer.get_address();

        // The expression evaluation could result in a segment:offset address or a flat address.
        // The behavior of the viewer will differ slightly depending on whether we have segment:offset
        // information. Wrapping of segments can't be detected if the expression evaluates to a flat
        // address.
        let start_addr = emu.machine.cpu().eval_address(&start_addr_str);
        let start_addr_flat: u32 = match start_addr {
            Some(i) => i.into(),
            None => 0,
        };

        let cpu_type = emu.machine.cpu().get_type();
        let bus = emu.machine.bus_mut();

        let mut listview_vec = Vec::new();

        //let mut disassembly_string = String::new();
        let mut disassembly_addr_flat = start_addr_flat as usize;
        let mut disassembly_addr_seg = start_addr;

        for _ in 0..24 {
            if disassembly_addr_flat < machine::MAX_MEMORY_ADDRESS {
                bus.seek(disassembly_addr_flat);

                let mut decode_vec = Vec::new();

                match cpu_type.decode(bus, true) {
                    Ok(i) => {
                        let instr_vec = bus.get_vec_at_ex(disassembly_addr_flat, i.size as usize);
                        let instr_bytes_str = util::fmt_byte_array(&instr_vec);

                        decode_vec.push(SyntaxToken::MemoryAddressFlat(
                            disassembly_addr_flat as u32,
                            format!("{:05X}", disassembly_addr_flat),
                        ));

                        let mut instr_vec = cpu_type.tokenize_instruction(&i);

                        //let decode_str = format!("{:05X} {:012} {}\n", disassembly_addr, instr_bytes_str, i);

                        disassembly_addr_flat += i.size as usize;

                        // If we have cs:ip, advance the offset. Wrapping of segment may provide different results
                        // from advancing flat address, so if a wrap is detected, adjust the flat address.
                        if let Some(CpuAddress::Segmented(segment, offset)) = disassembly_addr_seg {
                            decode_vec.push(SyntaxToken::MemoryAddressSeg16(
                                segment,
                                offset,
                                format!("{:04X}:{:04X}{}", segment, offset, " ",),
                            ));

                            let new_offset = offset.wrapping_add(i.size as u16);
                            if new_offset < offset {
                                // A wrap of the code segment occurred. Update the linear address to match.
                                disassembly_addr_flat = cpu_common::calc_linear_address(segment, new_offset) as usize;
                            }

                            disassembly_addr_seg = Some(CpuAddress::Segmented(segment, new_offset));
                            //*offset = new_offset;
                        }
                        decode_vec.push(SyntaxToken::InstructionBytes(format!("{:012}", instr_bytes_str)));
                        decode_vec.append(&mut instr_vec);
                    }
                    Err(_) => {
                        decode_vec.push(SyntaxToken::ErrorString("INVALID".to_string()));
                    }
                };

                //disassembly_string.push_str(&decode_str);
                listview_vec.push(decode_vec);
            }
        }

        //framework.gui.update_disassembly_view(disassembly_string);
        emu.gui.disassembly_viewer.set_content(listview_vec);
    }

    // Update text mode viewer.
    if emu.gui.is_window_open(GuiWindow::TextModeViewer) {
        dm.for_each_card(|vid| {
            emu.gui.text_mode_viewer.set_content(
                vid.idx,
                emu.machine
                    .bus()
                    .video(vid)
                    .map_or(Vec::new(), |v| v.get_text_mode_strings()),
            );
        });
    }

    // Update SN chip viewer.
    if emu.gui.is_window_open(GuiWindow::SnViewer) {
        if let Some(sn) = emu.machine.bus_mut().sn_chip_mut() {
            emu.gui.sn_viewer.update_state(sn.display_state());
        }
    }
}
