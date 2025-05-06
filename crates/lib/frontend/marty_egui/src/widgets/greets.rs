use std::collections::VecDeque;

use egui::*;

const AMPLITUDE: f32 = 2.0;
const FREQUENCY: f32 = 3.0;
const PHASE_SCALE: f32 = 0.01;
const COLOR: Color32 = Color32::WHITE;
const NAME_SPACING: f32 = 24.0;

pub struct GreetsWidget {
    master: &'static [&'static str],
    visible: VecDeque<&'static str>,
    source: VecDeque<&'static str>,
    scroll_offset: f32,
    speed: f32,
    font_id: FontId,
}

impl GreetsWidget {
    pub fn new(names: &'static [&'static str], font_id: FontId, speed: f32) -> Self {
        Self {
            master: names,
            visible: VecDeque::new(),
            source: VecDeque::from(names.to_vec()),
            scroll_offset: 0.0,
            speed,
            font_id,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let desired_size = Vec2::new(ui.available_width(), 24.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
        let painter = ui.painter_at(rect);
        let time = ui.input(|i| i.time as f32);
        let mut x = -self.scroll_offset;

        for &name in &self.visible {
            for ch in name.chars() {
                let text = ch.to_string();
                let galley = painter.layout_no_wrap(text.clone(), self.font_id.clone(), COLOR);
                let char_width = galley.size().x;

                let phase = x * PHASE_SCALE;
                let y = AMPLITUDE * ((time * FREQUENCY + phase) * std::f32::consts::TAU).sin();

                let pos = rect.left_top() + vec2(x, y);
                painter.text(pos, Align2::LEFT_TOP, text, self.font_id.clone(), COLOR);

                x += char_width;
            }
            x += NAME_SPACING; // space between names
        }

        // Advance scroll
        self.scroll_offset += self.speed;

        // Remove fully-scrolled names
        if let Some(&front) = self.visible.front() {
            let total_width: f32 = front
                .chars()
                .map(|ch| {
                    painter
                        .layout_no_wrap(ch.to_string(), self.font_id.clone(), COLOR)
                        .size()
                        .x
                })
                .sum::<f32>()
                + NAME_SPACING;

            if self.scroll_offset > total_width {
                self.scroll_offset -= total_width;
                self.visible.pop_front();
            }
        }

        // Add more names if room
        if x < rect.width() {
            if let Some(next) = self.source.pop_front() {
                self.visible.push_back(next);
            }
        }

        // Refill from master list
        if self.source.is_empty() {
            self.source = VecDeque::from(self.master.to_vec());
        }

        response
    }
}
