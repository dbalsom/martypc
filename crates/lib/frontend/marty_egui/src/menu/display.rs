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
*/

use crate::{state::GuiState, GuiEnum, GuiEvent, GuiVariable, GuiVariableContext, GuiWindow};
use marty_core::device_traits::videocard::VideoType;
use marty_display_common::display_manager::{DisplayTargetType, DtHandle};

use strum::IntoEnumIterator;

impl GuiState {
    pub fn draw_display_menu(&mut self, ui: &mut egui::Ui, display: DtHandle) {
        // TODO: Refactor all uses of display.into(), to use a hash map of DtHandle instead.
        //       Currently DtHandle is a wrapper around a usize index, but we should make it value
        //       agnostic.
        let vctx = GuiVariableContext::Display(display);

        #[cfg(feature = "scaler_ui")]
        {
            let mut dtype_opt = self
                .get_option_enum_mut(GuiEnum::DisplayType(Default::default()), Some(vctx))
                .and_then(|oe| {
                    if let GuiEnum::DisplayType(dt) = *oe {
                        Some(dt)
                    }
                    else {
                        None
                    }
                });

            ui.menu_button("Display Type", |ui| {
                for dtype in DisplayTargetType::iter() {
                    if let Some(enum_mut) =
                        self.get_option_enum_mut(GuiEnum::DisplayType(Default::default()), Some(vctx))
                    {
                        let checked = *enum_mut == GuiEnum::DisplayType(dtype);

                        if ui.add(egui::RadioButton::new(checked, format!("{}", dtype))).clicked() {
                            *enum_mut = GuiEnum::DisplayType(dtype);
                            self.event_queue.send(GuiEvent::VariableChanged(
                                GuiVariableContext::Display(display),
                                GuiVariable::Enum(GuiEnum::DisplayType(dtype)),
                            ));
                        }
                    }
                }
            });

            if dtype_opt == Some(DisplayTargetType::WindowBackground) {
                ui.menu_button("Scaler Mode", |ui| {
                    for (_scaler_idx, mode) in self.scaler_modes.clone().iter().enumerate() {
                        if let Some(enum_mut) =
                            self.get_option_enum_mut(GuiEnum::DisplayScalerMode(Default::default()), Some(vctx))
                        {
                            let checked = *enum_mut == GuiEnum::DisplayScalerMode(*mode);

                            if ui.add(egui::RadioButton::new(checked, format!("{:?}", mode))).clicked() {
                                *enum_mut = GuiEnum::DisplayScalerMode(*mode);
                                self.event_queue.send(GuiEvent::VariableChanged(
                                    GuiVariableContext::Display(display),
                                    GuiVariable::Enum(GuiEnum::DisplayScalerMode(*mode)),
                                ));
                            }
                        }
                    }
                });
            }
            else {
                ui.menu_button("Window Options", |ui| {
                    // Use a horizontal ui to avoid squished menu
                    ui.horizontal(|ui| {
                        if let Some(enum_mut) =
                            self.get_option_enum_mut(GuiEnum::WindowBezel(Default::default()), Some(vctx))
                        {
                            let mut checked = *enum_mut == GuiEnum::WindowBezel(true);

                            if ui.checkbox(&mut checked, "Bezel Overlay").changed() {
                                *enum_mut = GuiEnum::WindowBezel(checked);
                                self.event_queue.send(GuiEvent::VariableChanged(
                                    GuiVariableContext::Display(display),
                                    GuiVariable::Enum(GuiEnum::WindowBezel(checked)),
                                ));
                            }
                        }
                    });
                });
            }

            #[cfg(feature = "scaler_params")]
            {
                ui.menu_button("Scaler Presets", |ui| {
                    for (_preset_idx, preset) in self.scaler_presets.clone().iter().enumerate() {
                        if ui.button(preset).clicked() {
                            self.set_option_enum(GuiEnum::DisplayScalerPreset(preset.clone()), Some(vctx));
                            self.event_queue.send(GuiEvent::VariableChanged(
                                GuiVariableContext::Display(display),
                                GuiVariable::Enum(GuiEnum::DisplayScalerPreset(preset.clone())),
                            ));
                            ui.close_menu();
                        }
                    }
                });

                if ui.button("Scaler Adjustments...").clicked() {
                    *self.window_flag(GuiWindow::ScalerAdjust) = true;
                    self.scaler_adjust.select_card(display.into());
                    ui.close_menu();
                }
            }
        }

        ui.menu_button("Display Aperture", |ui| {
            let mut aperture_vec = Vec::new();
            if let Some(aperture_vec_ref) = self.display_apertures.get(&display.into()) {
                aperture_vec = aperture_vec_ref.clone()
            };

            for aperture in aperture_vec.iter() {
                if let Some(enum_mut) =
                    self.get_option_enum_mut(GuiEnum::DisplayAperture(Default::default()), Some(vctx))
                {
                    let checked = *enum_mut == GuiEnum::DisplayAperture(aperture.aper_enum);

                    if ui.add(egui::RadioButton::new(checked, aperture.name)).clicked() {
                        *enum_mut = GuiEnum::DisplayAperture(aperture.aper_enum);
                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Display(display),
                            GuiVariable::Enum(GuiEnum::DisplayAperture(aperture.aper_enum)),
                        ));
                    }
                }
            }
        });

        let mut state_changed = false;
        let mut new_state = false;
        if let Some(GuiEnum::DisplayAspectCorrect(state)) =
            &mut self.get_option_enum_mut(GuiEnum::DisplayAspectCorrect(false), Some(vctx))
        {
            if ui.checkbox(state, "Correct Aspect Ratio").clicked() {
                //let new_opt = self.get_option_enum_mut()
                state_changed = true;
                new_state = *state;
                ui.close_menu();
            }
        }
        if state_changed {
            self.event_queue.send(GuiEvent::VariableChanged(
                GuiVariableContext::Display(display),
                GuiVariable::Enum(GuiEnum::DisplayAspectCorrect(new_state)),
            ));
        }

        // CGA-specific options.
        if matches!(
            self.display_info[usize::from(display)].vtype,
            Some(VideoType::CGA) | Some(VideoType::TGA)
        ) {
            let mut state_changed = false;
            let mut new_state = false;

            if let Some(GuiEnum::DisplayComposite(state)) =
                self.get_option_enum_mut(GuiEnum::DisplayComposite(Default::default()), Some(vctx))
            {
                if ui.checkbox(state, "Composite Monitor").clicked() {
                    state_changed = true;
                    new_state = *state;
                    ui.close_menu();
                }
            }
            if state_changed {
                self.event_queue.send(GuiEvent::VariableChanged(
                    GuiVariableContext::Display(display),
                    GuiVariable::Enum(GuiEnum::DisplayComposite(new_state)),
                ));
            }

            /* TODO: Snow should be set per-adapter, not per-display
            if ui
                .checkbox(&mut self.get_option_mut(GuiBoolean::EnableSnow), "Enable Snow")
                .clicked()
            {
                let new_opt = self.get_option(GuiBoolean::EnableSnow).unwrap();

                self.event_queue.send(GuiEvent::OptionChanged(GuiOption::Bool(
                    GuiBoolean::EnableSnow,
                    new_opt,
                )));

                ui.close_menu();
            }
             */

            if ui.button("Composite Adjustments...").clicked() {
                *self.window_flag(GuiWindow::CompositeAdjust) = true;
                self.composite_adjust.select_card(display.into());
                ui.close_menu();
            }
        }

        self.workspace_window_open_button_with(ui, GuiWindow::TextModeViewer, true, |state| {
            state.text_mode_viewer.select_card(display.into());
        });

        // On the web, fullscreen is basically free when the user hits f11 to go fullscreen.
        // We can't programmatically request fullscreen. So, we don't show the option.
        #[cfg(not(target_arch = "wasm32"))]
        if ui.button("🖵 Toggle Fullscreen").clicked() {
            self.event_queue.send(GuiEvent::ToggleFullscreen(display.into()));
            ui.close_menu();
        };

        ui.separator();

        if ui.button("🖼 Take Screenshot").clicked() {
            self.event_queue.send(GuiEvent::TakeScreenshot(display.into()));
            ui.close_menu();
        };
    }
}
