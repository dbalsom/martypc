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

//! Logic for the Sound menu.

use crate::{
    state::GuiState,
    widgets::big_icon::{BigIcon, IconType},
    GuiEnum,
    GuiEvent,
    GuiVariable,
    GuiVariableContext,
};

impl GuiState {
    pub fn draw_sound_menu(&mut self, ui: &mut egui::Ui) {
        let mut sources = self.sound_sources.clone();

        for (snd_idx, source) in &mut sources.iter_mut().enumerate() {
            let icon = match source.muted {
                true => IconType::SpeakerMuted,
                false => IconType::Speaker,
            };

            let mut volume = source.volume;

            let sctx = GuiVariableContext::SoundSource(snd_idx);

            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(format!("{}", source.name));
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(BigIcon::new(icon, Some(icon.default_color(ui))).medium().text())
                                    .frame(true),
                            )
                            .clicked()
                        {
                            log::warn!("Mute button clicked");
                            source.muted = !source.muted;

                            if let Some(GuiEnum::AudioMuted(state)) =
                                self.get_option_enum_mut(GuiEnum::AudioMuted(Default::default()), Some(sctx))
                            {
                                *state = source.muted;
                                self.event_queue.send(GuiEvent::VariableChanged(
                                    GuiVariableContext::SoundSource(snd_idx),
                                    GuiVariable::Enum(GuiEnum::AudioMuted(source.muted)),
                                ));
                            }
                        };

                        if ui
                            .add(egui::Slider::new(&mut source.volume, 0.0..=1.0).text("Volume"))
                            .changed()
                        {
                            if let Some(GuiEnum::AudioVolume(vol)) =
                                self.get_option_enum_mut(GuiEnum::AudioVolume(Default::default()), Some(sctx))
                            {
                                *vol = source.volume;
                                self.event_queue.send(GuiEvent::VariableChanged(
                                    GuiVariableContext::SoundSource(snd_idx),
                                    GuiVariable::Enum(GuiEnum::AudioVolume(source.volume)),
                                ));
                            }
                        }
                    });
                    ui.label(format!("Sample Rate: {}Hz", source.sample_rate));
                    ui.label(format!("Latency: {:.0}ms", source.latency_ms));
                    // ui.label(format!("Samples: {}", source.sample_ct));
                    // ui.label(format!("Buffers: {}", source.len));
                });
            });
        }
    }
}
