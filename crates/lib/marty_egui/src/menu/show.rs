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
*/

//! Implement the main emulator menu.

use crate::{state::GuiState, GuiEvent, GuiWindow};
use marty_frontend_common::MartyGuiTheme;

#[cfg(target_arch = "wasm32")]
use crate::{GuiBoolean, GuiVariable, GuiVariableContext};

#[cfg(feature = "use_display")]
use marty_core::device_traits::videocard::VideoCardId;

use egui::RichText;

#[cfg(feature = "use_display")]
fn display_target_count_for_card(
    display_info: &[marty_display_common::display_manager::DisplayTargetInfo],
    card: VideoCardId,
) -> usize {
    display_info.iter().filter(|display| display.vid == Some(card)).count()
}

impl GuiState {
    pub fn show_menu(&mut self, ui: &mut egui::Ui) {
        let modal_mode = self.active_modal_mode(ui.ctx());
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Emulator", |ui| {
                ui.set_min_width(120.0);

                if !modal_mode.is_active() {
                    if ui.button("⏱ Performance...").clicked() {
                        *self.window_flag(GuiWindow::PerfViewer) = true;
                        ui.close();
                    }

                    ui.menu_button("🎨 Theme", |ui| {
                        ui.set_min_width(140.0);
                        for theme in MartyGuiTheme::ALL {
                            if ui.radio(theme == self.current_theme(), theme.label()).clicked() {
                                self.set_current_theme(theme);
                                ui.close();
                            }
                        }
                    });

                    // On wasm we give a quick-shortcut to the OSD keyboard under Emulator,
                    // but this may be unnecessary since we can swipe up for it...
                    #[cfg(target_arch = "wasm32")]
                    {
                        ui.separator();

                        let mut osd_keyboard_enabled = self.get_option(GuiBoolean::OsdKeyboard).unwrap_or(false);
                        if ui
                            .add_enabled(
                                self.osd_keyboard_available(),
                                egui::Checkbox::new(&mut osd_keyboard_enabled, "On-screen keyboard"),
                            )
                            .changed()
                        {
                            self.set_osd_keyboard_enabled(osd_keyboard_enabled);
                            self.event_queue.send(GuiEvent::VariableChanged(
                                GuiVariableContext::Global,
                                GuiVariable::Bool(GuiBoolean::OsdKeyboard, osd_keyboard_enabled),
                            ));
                            ui.close();
                        }
                    }

                    ui.separator();
                }

                #[cfg(target_arch = "wasm32")]
                if ui.button("↩ Return to Launcher").clicked() {
                    self.event_queue.send(GuiEvent::Exit);
                    ui.close();
                }

                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("⎆ Quit").clicked() {
                    self.event_queue.send(GuiEvent::Exit);
                    ui.close();
                }
            });

            // Only show the Emulator menu if a modal dialog is open.
            if modal_mode.is_active() {
                return;
            }

            self.show_machine_menu(ui);

            self.show_media_menu(ui);

            ui.menu_button("Sound", |ui| {
                ui.set_min_width(240.0);
                if !self.sound_sources.is_empty() {
                    self.draw_sound_menu(ui);
                }
                else {
                    ui.label(RichText::new("No sound sources available.").italics());
                }
            });

            #[cfg(feature = "use_display")]
            {
                ui.menu_button("Display", |ui| {
                    ui.set_min_size(egui::vec2(240.0, 0.0));

                    let display_info = self.display_info.clone();
                    let separate_card_menus = self
                        .video_cards
                        .iter()
                        .copied()
                        .filter(|card| display_target_count_for_card(&display_info, *card) != 1)
                        .collect::<Vec<_>>();

                    if display_info.len() >= 3 {
                        #[cfg(feature = "scaler_ui")]
                        {
                            ui.menu_button("All Displays", |ui| {
                                self.draw_all_displays_menu(ui);
                            });
                            ui.separator();
                        }
                    }

                    // Cards with multiple display targets get a separate menu for card-level
                    // actions. A zero-target card also needs a menu because it has no display
                    // menu that could host those actions.
                    for card in &separate_card_menus {
                        ui.menu_button(format!("Card {}: {:?}", card.idx, card.vtype), |ui| {
                            self.draw_video_card_menu(ui, *card);
                        });
                    }

                    if !separate_card_menus.is_empty() && !display_info.is_empty() {
                        ui.separator();
                    }

                    // If there is only one display, emit the display menu directly. Otherwise,
                    // emit named menus for each display. Card-level actions remain inline when
                    // the card has exactly one display target.
                    if display_info.len() == 1 {
                        self.draw_display_menu(ui, display_info[0].handle);

                        if let Some(card) = display_info[0]
                            .vid
                            .filter(|card| display_target_count_for_card(&display_info, *card) == 1)
                        {
                            ui.separator();
                            self.draw_video_card_menu(ui, card);
                        }
                    }
                    else if display_info.len() > 1 {
                        for (display_idx, display) in display_info.iter().enumerate() {
                            let card = display
                                .vid
                                .filter(|card| display_target_count_for_card(&display_info, *card) == 1);

                            ui.menu_button(format!("Display {}: {}", display_idx, display.name), |ui| {
                                self.draw_display_menu(ui, display.handle);

                                if let Some(card) = card {
                                    ui.separator();
                                    self.draw_video_card_menu(ui, card);
                                }
                            });
                        }
                    }
                    else if separate_card_menus.is_empty() {
                        ui.label(RichText::new("No video cards or display targets available.").italics());
                    }
                });
            }

            self.draw_debug_menu(ui);

            ui.menu_button("Help", |ui| {
                ui.set_min_width(120.0);
                self.workspace_window_open_button(ui, GuiWindow::Hotkeys, true, true);
                ui.separator();
                self.workspace_window_open_button(ui, GuiWindow::About, true, true);
            });

            // Draw drive indicators, etc.
            self.draw_status_widgets(ui);
        });
    }

    pub fn draw_status_widgets(&mut self, _ui: &mut egui::Ui) {
        // Can we put stuff on the right hand side of the menu bar?
        // ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        //     ui.label("💾");
        // });
        //
        // ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        //     ui.label("🐢");
        // });
    }
}
