/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the â€œSoftwareâ€),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED â€œAS ISâ€, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
*/

use crate::{app::GRAB_MODE, emulator::Emulator};
use display_manager_eframe::EFrameDisplayManager;
use egui::{CursorGrab, ViewportCommand};
use marty_common::types::keys::MartyKey;
use marty_core::machine::ExecutionOperation;
use marty_frontend_common::{
    constants::{LONG_NOTIFICATION_TIME, NORMAL_NOTIFICATION_TIME},
    HotkeyEvent,
};

#[cfg(not(target_arch = "wasm32"))]
use display_manager_eframe::DisplayManager;
#[cfg(not(target_arch = "wasm32"))]
use marty_display_common::display_manager::DtHandle;

/// Update the hotkey state for one physical key event and execute any completed bindings.
#[allow(unreachable_patterns)]
pub fn process_hotkeys(
    emu: &mut Emulator,
    dm: &mut EFrameDisplayManager,
    ctx: egui::Context,
    key: MartyKey,
    pressed: bool,
    gui_focus: bool,
) {
    let events = if pressed {
        emu.hkm
            .keydown(key, gui_focus, emu.mouse_data.is_captured)
            .unwrap_or_default()
    }
    else {
        emu.hkm.keyup(key);
        Vec::new()
    };

    for hotkey in events {
        match hotkey {
            HotkeyEvent::ToggleGui => {
                log::debug!("ToggleGui hotkey triggered. Toggling GUI visibility.");
                emu.flags.render_gui = !emu.flags.render_gui;
            }
            HotkeyEvent::CaptureMouse => {
                log::debug!("CaptureMouse hotkey triggered. Toggling mouse capture.");
                let Some((viewport_id, display)) = dm.grabbed_display().or_else(|| {
                    dm.display_for_viewport(egui::ViewportId::ROOT)
                        .map(|display| (egui::ViewportId::ROOT, display))
                })
                else {
                    log::warn!("No display is assigned to the main viewport; mouse capture ignored.");
                    continue;
                };
                let Some(dtc) = dm.display_target(display)
                else {
                    continue;
                };
                match dtc.try_write() {
                    Ok(mut dtc_lock) => {
                        if !dtc_lock.grabbed() {
                            dtc_lock.set_grabbed(true, emu.mouse_data.capture_mode);
                            emu.mouse_data.is_captured = true;
                            ctx.send_viewport_cmd_to(viewport_id, ViewportCommand::CursorGrab(GRAB_MODE));
                            ctx.send_viewport_cmd_to(viewport_id, ViewportCommand::CursorVisible(false));

                            let capture_hint = emu
                                .hkm
                                .hotkey_string(HotkeyEvent::CaptureMouse)
                                .map(|hotkey| format!("Press {hotkey} to release mouse"))
                                .unwrap_or_default();
                            let message = if capture_hint.is_empty() {
                                "Mouse captured!".to_string()
                            }
                            else {
                                format!("Mouse captured!\n{capture_hint}")
                            };
                            emu.gui.toasts().info(message).duration(Some(NORMAL_NOTIFICATION_TIME));
                        }
                        else {
                            dtc_lock.set_grabbed(false, emu.mouse_data.capture_mode);
                            emu.mouse_data.is_captured = false;
                            ctx.send_viewport_cmd_to(viewport_id, ViewportCommand::CursorGrab(CursorGrab::None));
                            ctx.send_viewport_cmd_to(viewport_id, ViewportCommand::CursorVisible(true));
                            emu.gui
                                .toasts()
                                .info("Mouse released!")
                                .duration(Some(NORMAL_NOTIFICATION_TIME));
                        }
                    }
                    Err(_e) => {
                        log::error!("Couldn't get lock on display target.");
                    }
                };
            }
            HotkeyEvent::CtrlAltDel => {
                log::debug!("CtrlAltDel hotkey triggered. Sending Ctrl-Alt-Del to machine.");
                emu.machine.emit_ctrl_alt_del();
            }
            HotkeyEvent::Reboot => {
                log::debug!("Reboot hotkey triggered. Restarting machine.");
                emu.machine.reboot();
            }
            HotkeyEvent::ToggleFullscreen => {
                log::debug!("ToggleFullscreen hotkey triggered.");
                let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
            }
            HotkeyEvent::Screenshot => {
                log::debug!("Screenshot hotkey triggered. Capturing screenshot.");
                take_screenshot(emu, dm);
            }
            HotkeyEvent::DebugStep => {
                emu.exec_control.borrow_mut().set_op(ExecutionOperation::Step);
            }
            HotkeyEvent::DebugStepOver => {
                emu.exec_control.borrow_mut().set_op(ExecutionOperation::StepOver);
            }
            HotkeyEvent::JoyToggle => {
                log::debug!("JoyToggle hotkey triggered. Toggling joystick keyboard emulation.");
                let state = emu.gi.toggle_joykeys(0);
                emu.gui
                    .toasts()
                    .error(format!(
                        "Keyboard to Joystick emulation {}",
                        if state { "enabled" } else { "disabled" }
                    ))
                    .duration(Some(NORMAL_NOTIFICATION_TIME));
            }
            HotkeyEvent::Quit => {
                log::debug!("Quit hotkey pressed. Exiting immediately...");
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            _ => {
                log::debug!("Unhandled Hotkey triggered: {:?}", hotkey);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn take_screenshot(emu: &mut Emulator, dm: &mut EFrameDisplayManager) {
    let Some(screenshot_path) = emu.rm.resource_path("screenshot")
    else {
        log::error!("Screenshot directory is not configured.");
        return;
    };

    if let Err(err) = dm.save_screenshot(DtHandle::default(), screenshot_path) {
        log::error!("Failed to save screenshot: {}", err);
        emu.gui
            .toasts()
            .error(err.to_string())
            .duration(Some(LONG_NOTIFICATION_TIME));
    }
}

#[cfg(target_arch = "wasm32")]
fn take_screenshot(emu: &mut Emulator, dm: &mut EFrameDisplayManager) {
    if let Err(err) = dm.request_screenshot_capture(Default::default(), "screenshot.png") {
        log::error!("Failed to capture screenshot: {}", err);
        emu.gui
            .toasts()
            .error(err.to_string())
            .duration(Some(LONG_NOTIFICATION_TIME));
    }
}
