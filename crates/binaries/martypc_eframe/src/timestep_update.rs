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

    event_loop/update.rs

    Process an event loop update
*/

#[cfg(not(target_arch = "wasm32"))]
use crate::native::file_save::save_non_interactive_file;
#[cfg(target_arch = "wasm32")]
use crate::wasm::file_save::save_non_interactive_file;
use crate::{
    event_loop::egui_update::update_egui,
    file_transfer::{load_non_interactive_file, NonInteractiveFileLoadError},
};
use web_time::{Duration, Instant};

use crate::{emulator::Emulator, event_loop::render_frame::render_frame, input::GamepadEvent};
use display_manager_eframe::{DisplayManager, EFrameDisplayManager};
use marty_core::{bus::DeviceEvent, cpu_common::ServiceEvent, devices::game_port::GamePort, machine::MachineEvent};
use marty_frontend_common::{
    constants::{LONG_NOTIFICATION_TIME, NORMAL_NOTIFICATION_TIME, SHORT_NOTIFICATION_TIME},
    marty_common::types::ui::MouseCaptureMode,
    sound_file_manager::PresentableSoundAction,
    thread_events::{FileOpenContext, FileSaveContext, FileSelectionContext, FrontendThreadEvent},
    timestep_manager::{MachinePerfStats, TimestepManager},
};
use marty_videocard_renderer::RendererEvent;
/*use crate::{
    event_loop::{egui_update::update_egui, render_frame::render_frame},
    Emulator,
};*/

pub fn update_mouse_buttons(emu: &mut Emulator) {
    let l_button_state = if emu.mouse_data.l_button_was_released {
        false
    }
    else {
        emu.mouse_data.l_button_was_pressed
    };

    let r_button_state = if emu.mouse_data.r_button_was_released {
        false
    }
    else {
        emu.mouse_data.r_button_was_pressed
    };

    emu.mouse_data.l_button_is_pressed = l_button_state;
    emu.mouse_data.r_button_is_pressed = r_button_state;
}

pub fn process_update(emu: &mut Emulator, dm: &mut EFrameDisplayManager, tm: &mut TimestepManager) {
    tm.wm_update(
        emu,
        dm,
        |emuc| {
            // log::debug!(
            //     "Second update: Running at {} Mhz, {} cycles, {} instructions, {} ticks, {} frames",
            //     emuc.machine.get_cpu_mhz(),
            //     emuc.machine.cpu_cycles(),
            //     emuc.machine.cpu_instructions(),
            //     emuc.machine.system_ticks(),
            //     emuc.machine
            //         .primary_videocard()
            //         .map(|vc| vc.get_frame_count())
            //         .unwrap_or(0)
            // );

            // Per second freq
            MachinePerfStats {
                cpu_mhz: emuc.machine.get_cpu_mhz(),
                cpu_cycles: emuc.machine.cpu_cycles(),
                cpu_instructions: emuc.machine.cpu_instructions(),
                system_ticks: emuc.machine.system_ticks(),
                emu_frames: emuc.machine.primary_videocard().map(|vc| vc.frame_count()),
            }
        },
        |emuc, cycles| {
            // Per emu update freq
            emuc.machine.run(cycles, &mut emuc.exec_control.borrow_mut());
        },
        |emuc, dmc, tmc, &perf, duration, tmu| {
            // Per frame freq
            emuc.perf = perf;

            match emuc.mouse_data.capture_mode {
                MouseCaptureMode::Mouse => {
                    update_mouse_buttons(emuc);
                    if let Some(mouse) = emuc.machine.mouse_mut() {
                        // If we're in mouse capture mode, only update if we have a mouse update.
                        if emuc.mouse_data.is_captured && emuc.mouse_data.have_update {
                            mouse.update(
                                emuc.mouse_data.l_button_is_pressed,
                                emuc.mouse_data.r_button_is_pressed,
                                emuc.mouse_data.frame_delta_x,
                                emuc.mouse_data.frame_delta_y,
                            );
                        }
                    }
                }
                MouseCaptureMode::LightPen => {
                    // In light pen mode, we want to update the mouse state every frame, even if we don't have an update,
                    // so that the renderer can update the light pen position and latching.
                    update_mouse_buttons(emuc);

                    // Update renderer here
                    dmc.with_primary_renderer_mut(|renderer| {
                        renderer.update_cursor(
                            emuc.mouse_data.frame_delta_x,
                            emuc.mouse_data.frame_delta_y,
                            true, // Light pen should always latch if reset, independent of button
                        )
                    });
                }
            }

            // Reset mouse for next frame
            emuc.mouse_data.reset();

            // Do gamepad events
            #[cfg(feature = "use_gilrs")]
            if let Some(gameport) = emuc.machine.bus_mut().game_port_mut() {
                // Check if gamepad is connected
                let events = emuc.gi.poll();
                for event in events {
                    match event {
                        GamepadEvent::Connected(gamepad_info) => {
                            log::debug!("Gamepad {:?} connected", gamepad_info);
                            emuc.gui.set_gamepad_mapping(emuc.gi.mapping())
                        }
                        GamepadEvent::Disconnected(id) => {
                            log::warn!("Gamepad {} disconnected", id);
                            emuc.gui.set_gamepad_mapping(emuc.gi.mapping())
                        }
                        GamepadEvent::Event(gilrs_event) => {
                            emuc.gui.set_gamepad_mapping(emuc.gi.mapping());
                            let resolved_gamepad = emuc.gi.select_id(gilrs_event.id);

                            if let Some(gamepad_idx) = resolved_gamepad {
                                let deadzone = emuc.gi.deadzone();
                                match gilrs_event.event {
                                    gilrs::EventType::AxisChanged(axis, mut value, _) => {
                                        // Apply deadzone
                                        if value.abs() < deadzone {
                                            log::debug!("input in deadzone: {}", value);
                                            value = 0.0;
                                        }
                                        match axis {
                                            gilrs::Axis::LeftStickX => {
                                                gameport.set_stick_pos(gamepad_idx, 0, Some(value as f64), None)
                                            }
                                            gilrs::Axis::LeftStickY => {
                                                gameport.set_stick_pos(gamepad_idx, 0, None, Some(value as f64))
                                            }
                                            gilrs::Axis::RightStickX => {
                                                gameport.set_stick_pos(gamepad_idx, 1, Some(value as f64), None)
                                            }
                                            gilrs::Axis::RightStickY => {
                                                gameport.set_stick_pos(gamepad_idx, 1, None, Some(value as f64))
                                            }
                                            _ => {}
                                        }
                                    }
                                    gilrs::EventType::ButtonPressed(button, _) => {
                                        match button {
                                            // Treat West and South as button 1
                                            // TODO: Do button mappings
                                            gilrs::Button::West => {
                                                gameport.set_button(gamepad_idx, 0, true);
                                            }
                                            gilrs::Button::South => {
                                                gameport.set_button(gamepad_idx, 0, true);
                                            }
                                            // Treat North and East as button 2
                                            gilrs::Button::North => {
                                                gameport.set_button(gamepad_idx, 1, true);
                                            }
                                            gilrs::Button::East => {
                                                gameport.set_button(gamepad_idx, 1, true);
                                            }
                                            gilrs::Button::DPadLeft => {
                                                gameport.set_stick_pos(gamepad_idx, 0, Some(GamePort::LEFT), None)
                                            }
                                            gilrs::Button::DPadRight => {
                                                gameport.set_stick_pos(gamepad_idx, 0, Some(GamePort::RIGHT), None)
                                            }
                                            gilrs::Button::DPadUp => {
                                                gameport.set_stick_pos(gamepad_idx, 0, None, Some(GamePort::UP))
                                            }
                                            gilrs::Button::DPadDown => {
                                                gameport.set_stick_pos(gamepad_idx, 0, None, Some(GamePort::DOWN))
                                            }
                                            _ => {}
                                        }
                                    }
                                    gilrs::EventType::ButtonReleased(button, _) => {
                                        match button {
                                            // Treat West and South as button 1
                                            gilrs::Button::West => {
                                                gameport.set_button(gamepad_idx, 0, false);
                                            }
                                            gilrs::Button::South => {
                                                gameport.set_button(gamepad_idx, 0, false);
                                            }
                                            // Treat North and East as button 2
                                            gilrs::Button::North => {
                                                gameport.set_button(gamepad_idx, 1, false);
                                            }
                                            gilrs::Button::East => {
                                                gameport.set_button(gamepad_idx, 1, false);
                                            }
                                            gilrs::Button::DPadLeft => {
                                                gameport.set_stick_pos(gamepad_idx, 0, Some(GamePort::CENTER), None)
                                            }
                                            gilrs::Button::DPadRight => {
                                                gameport.set_stick_pos(gamepad_idx, 0, Some(GamePort::CENTER), None)
                                            }
                                            gilrs::Button::DPadUp => {
                                                gameport.set_stick_pos(gamepad_idx, 0, None, Some(GamePort::CENTER))
                                            }
                                            gilrs::Button::DPadDown => {
                                                gameport.set_stick_pos(gamepad_idx, 0, None, Some(GamePort::CENTER))
                                            }
                                            _ => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            let mut machine_was_reset = false;

            // Drain machine events
            while let Some(event) = emuc.machine.get_event() {
                match event {
                    MachineEvent::CheckpointHit(checkpoint, pri) => {
                        log::info!(
                            "CHECKPOINT: {}",
                            emuc.machine
                                .get_checkpoint_string(checkpoint)
                                .unwrap_or("ERROR".to_string())
                        );

                        if let Some(pri_level) = emuc.config.emulator.debugger.checkpoint_notify_level {
                            if pri <= pri_level {
                                // Send notification

                                emuc.gui
                                    .toasts()
                                    .info(format!(
                                        "CHECKPOINT: {}",
                                        emuc.machine
                                            .get_checkpoint_string(checkpoint)
                                            .unwrap_or("ERROR".to_string())
                                    ))
                                    .duration(Some(NORMAL_NOTIFICATION_TIME));
                            }
                        }
                    }
                    MachineEvent::Reset => {
                        machine_was_reset = true;

                        // Send notification
                        emuc.gui
                            .toasts()
                            .info("Machine reset!".to_string())
                            .duration(Some(NORMAL_NOTIFICATION_TIME));

                        if emuc.config.machine.reload_roms {
                            // Reload ROMs from the saved list of ROM sets.
                            match emuc.romm.create_manifest(emuc.romsets.clone(), &mut emuc.rm) {
                                Ok(manifest) => match emuc.machine.reinstall_roms(manifest) {
                                    Ok(_) => {
                                        emuc.gui
                                            .toasts()
                                            .info("ROMs reloaded!".to_string())
                                            .duration(Some(NORMAL_NOTIFICATION_TIME));
                                    }
                                    Err(err) => {
                                        log::error!("Error reloading ROMs: {}", err);
                                        emuc.gui
                                            .toasts()
                                            .error(format!("Failed to reload ROMs: {}", err))
                                            .duration(Some(LONG_NOTIFICATION_TIME));
                                    }
                                },
                                Err(err) => {
                                    log::error!("Error creating ROM manifest: {}", err);
                                    emuc.gui
                                        .toasts()
                                        .error(format!("Failed to reload ROMs: {}", err))
                                        .duration(Some(LONG_NOTIFICATION_TIME));
                                }
                            }
                        }
                    }
                    MachineEvent::Halted => {
                        emuc.gui
                            .toasts()
                            .error("CPU permanently halted!".to_string())
                            .duration(Some(LONG_NOTIFICATION_TIME));
                    }
                    MachineEvent::Service(service_event) => match service_event {
                        ServiceEvent::QuitEmulator(delay) => {
                            let _ = emuc.sender.send(FrontendThreadEvent::QuitRequested);
                            log::warn!("Emulator quit requested after delay {}", delay);
                        }
                        ServiceEvent::GuestFileTransferComplete {
                            filename,
                            data,
                            non_interactive,
                        } => {
                            if non_interactive {
                                match save_non_interactive_file(&emuc.rm, &filename, &data) {
                                    Ok(path) => log::info!("Guest file transferred non-interactively to '{}'", path),
                                    Err(error) => log::error!("Non-interactive guest file transfer failed: {}", error),
                                }
                            }
                            else {
                                emuc.gui.save_file_dialog(
                                    FileSaveContext::guest_file(filename, data),
                                    "Save File from Guest",
                                    Vec::new(),
                                    None,
                                );
                            }
                        }
                        ServiceEvent::HostFileTransferRequested { filename } => {
                            if let Some(filename) = filename {
                                match load_non_interactive_file(&mut emuc.rm, &filename) {
                                    Ok((filename, data)) => emuc.machine.stage_service_host_file(filename, data),
                                    Err(NonInteractiveFileLoadError::NotFound(error)) => {
                                        log::warn!("Non-interactive host file was not found: {}", error);
                                        emuc.machine.service_host_file_not_found();
                                    }
                                    Err(NonInteractiveFileLoadError::Other(error)) => {
                                        log::error!("Non-interactive host file transfer failed: {}", error);
                                        emuc.machine.abort_service_host_file_request();
                                    }
                                }
                            }
                            else {
                                emuc.gui.open_file_dialog(
                                    FileOpenContext::ServiceHostFile {
                                        fsc: FileSelectionContext::Uninitialized,
                                    },
                                    "Receive File from Host",
                                    Vec::new(),
                                    true,
                                );
                            }
                        }
                        _ => {}
                    },
                }
            }

            for event in emuc.machine.presentable_event_receiver().try_iter() {
                log::debug!("Presentable device event: {:?}", event);
                if let Some(action) = emuc.sound_file_manager.resolve_presentable_event(event) {
                    log::debug!("Resolved presentable sound action: {:?}", action);
                    if let Some(sound_interface) = emuc.si.as_mut() {
                        let result = match action {
                            PresentableSoundAction::OneShot(effect) => {
                                sound_interface.play_sound(&effect.samples, effect.sample_rate, effect.stereo)
                            }
                            PresentableSoundAction::StartLoop { key, intro, looping } => {
                                sound_interface.start_loop(key, intro, looping)
                            }
                            PresentableSoundAction::StopLoop { key, outro } => sound_interface.stop_loop(key, outro),
                        };

                        if let Err(err) = result {
                            log::error!("Failed to execute presentable sound action {:?}: {}", action, err);
                        }
                    }
                }
            }

            if machine_was_reset {
                if let Some(sound_interface) = emuc.si.as_mut() {
                    sound_interface.device_reset();
                }
            }

            // Do per-frame updates (Serial port emulation)
            let events = emuc.machine.frame_update();
            for event in events {
                match event {
                    DeviceEvent::TurboToggled(state) => {
                        // Send notification
                        if state {
                            emuc.gui
                                .toasts()
                                .info("Turbo mode enabled!".to_string())
                                .duration(Some(SHORT_NOTIFICATION_TIME));
                        }
                        else {
                            emuc.gui
                                .toasts()
                                .info("Turbo mode disabled!".to_string())
                                .duration(Some(SHORT_NOTIFICATION_TIME));
                        }
                    }
                    _ => {}
                }
            }

            // Resize windows
            if let Err(err) = dmc.resize_viewports() {
                log::error!("Error resizing windows: {}", err);
            }

            let render_start = Instant::now();

            // Preserve the last rendered surface while the power-off effect is active. A reset can
            // change card extents immediately, which would otherwise replace that captured frame.
            if !emuc.display_power.frame_frozen() {
                emuc.machine.for_each_videocard(|vci| {
                    let extents = vci.card.display_extents();
                    // Resize the card.
                    if let Err(_) = dmc.on_card_resized(&vci.id, &extents) {
                        log::error!("Error resizing videocard");
                    }
                });
            }
            emuc.stat_counter.render_time = Instant::now() - render_start;

            // Update egui data
            update_egui(emuc, dmc, tmc, tmu);

            // Run sound
            if let Some(sound) = &mut emuc.si {
                let looping_sounds_paused = emuc.exec_control.borrow().get_state().is_paused();
                sound.set_looping_sounds_paused(looping_sounds_paused);
                sound.run(duration);
            }

            // Render the current frame for all window display targets.
            render_frame(emuc, dmc);

            // Handle renderer events
            dmc.for_each_renderer(|renderer, _vid, _backend_buf| {
                while let Some(event) = renderer.get_event() {
                    match event {
                        RendererEvent::ScreenshotSaved => {
                            emuc.gui
                                .toasts()
                                .info("Screenshot saved!".to_string())
                                .duration(Some(Duration::from_secs(5)));
                        }
                    }
                }
            });
        },
    );
}
