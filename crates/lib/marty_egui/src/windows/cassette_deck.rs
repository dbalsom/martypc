/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------

    marty_egui::windows::cassette_deck.rs

    Cassette player controls.
*/

const CASSETTE_IMAGE_LOGICAL_WIDTH: f32 = 276.0;
const CASSETTE_IMAGE_LOGICAL_HEIGHT: f32 = 171.0;
const X_OFFSET: f32 = 8.0;
const SHADOW_OFFSET: f32 = 6.0;
const CASSETTE_TAPE_LENGTH_METERS: f64 = 87.5;
const CASSETTE_DURATION_SECONDS: f64 = 60.0 * 60.0;
const CASSETTE_TAPE_THICKNESS_METERS: f64 = 16.7e-6;
const CASSETTE_HUB_DIAMETER_METERS: f64 = 22.0e-3;
const CASSETTE_HUB_DIAMETER_LOGICAL_PIXELS: f64 = 53.0;
// const CASSETTE_REEL_DIAMETER_LOGICAL_PIXELS: f64 = 27.0;

const SPOKE_IMAGE_SIZE: f32 = 27.0;
const LEFT_SPOKE_POSITION: (f32, f32) = (64.0, 59.0);
const RIGHT_SPOKE_POSITION: (f32, f32) = (168.0, 59.0);
const SPOKE_FRAME_COUNT: usize = 15;

const SPOKE_ANIMATION_ARC_RADIANS: f64 = std::f64::consts::TAU / 6.0;
const SPOKE_FRAME_ANGLE_RADIANS: f64 = SPOKE_ANIMATION_ARC_RADIANS / SPOKE_FRAME_COUNT as f64;
const SPOKE_FRAME_INDEX_EPSILON: f64 = 1.0e-9;

const TRANSPORT_BUTTON_WIDTH: f32 = 42.0;
const TRANSPORT_BUTTON_HEIGHT: f32 = 56.0;
const TRANSPORT_SYMBOL_SIZE: f32 = 22.0;
const TRANSPORT_BUTTONS: [(&str, &str); 6] = [
    ("⏺", "REC"),
    ("▶", "PLAY"),
    ("⏪", "REW"),
    ("⏩", "FF"),
    ("⏹", "STOP"),
    ("⏸", "PAUSE"),
];

const CASSETTE_BACKGROUND_COLOR: egui::Color32 = egui::Color32::BLACK;
const CASSETTE_TAPE_COLOR: egui::Color32 = egui::Color32::from_rgb(92, 51, 23);
const CASSETTE_SPOOL_SHADOW_COLOR: egui::Color32 = egui::Color32::from_black_alpha(64);
const CASSETTE_SPINDLE_COLOR: egui::Color32 = egui::Color32::from_rgb(0xbd, 0xbd, 0xbd);
// const CASSETTE_SPINDLE_INNER_COLOR: egui::Color32 = egui::Color32::BLACK;

#[derive(Debug, Default)]
pub struct CassetteAnimationController {
    linear_position: f64,
    left_angle: f64,
    right_angle: f64,
}

impl CassetteAnimationController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_linear_position(&mut self, linear_position: f64) {
        if !linear_position.is_finite() {
            return;
        }

        self.linear_position = linear_position.clamp(0.0, CASSETTE_TAPE_LENGTH_METERS);
        self.left_angle = (wound_tape_angle(CASSETTE_TAPE_LENGTH_METERS) - wound_tape_angle(self.left_tape_length()))
            .rem_euclid(std::f64::consts::TAU);
        self.right_angle = wound_tape_angle(self.right_tape_length()).rem_euclid(std::f64::consts::TAU);
    }

    pub fn linear_position(&self) -> f64 {
        self.linear_position
    }

    pub fn normalized_position(&self) -> f64 {
        self.linear_position / CASSETTE_TAPE_LENGTH_METERS
    }

    pub fn set_normalized_position(&mut self, normalized_position: f64) {
        if !normalized_position.is_finite() {
            return;
        }

        self.set_linear_position(normalized_position.clamp(0.0, 1.0) * CASSETTE_TAPE_LENGTH_METERS);
    }

    pub fn left_tape_length(&self) -> f64 {
        CASSETTE_TAPE_LENGTH_METERS - self.linear_position
    }

    pub fn right_tape_length(&self) -> f64 {
        CASSETTE_TAPE_LENGTH_METERS - self.left_tape_length()
    }

    pub fn left_tape_diameter_logical_pixels(&self) -> f32 {
        tape_diameter_logical_pixels(self.left_tape_length())
    }

    pub fn right_tape_diameter_logical_pixels(&self) -> f32 {
        tape_diameter_logical_pixels(self.right_tape_length())
    }

    pub fn left_angle(&self) -> f64 {
        self.left_angle
    }

    pub fn right_angle(&self) -> f64 {
        self.right_angle
    }

    pub fn left_frame(&self) -> usize {
        resolve_spoke_frame(self.left_angle)
    }

    pub fn right_frame(&self) -> usize {
        resolve_spoke_frame(self.right_angle)
    }
}

fn tape_radius(tape_length: f64) -> f64 {
    let hub_radius = CASSETTE_HUB_DIAMETER_METERS * 0.5;
    (hub_radius * hub_radius + tape_length * CASSETTE_TAPE_THICKNESS_METERS / std::f64::consts::PI).sqrt()
}

fn tape_diameter_logical_pixels(tape_length: f64) -> f32 {
    (2.0 * tape_radius(tape_length) * CASSETTE_HUB_DIAMETER_LOGICAL_PIXELS / CASSETTE_HUB_DIAMETER_METERS) as f32
}

fn wound_tape_angle(tape_length: f64) -> f64 {
    let hub_radius = CASSETTE_HUB_DIAMETER_METERS * 0.5;
    2.0 * std::f64::consts::PI * (tape_radius(tape_length) - hub_radius) / CASSETTE_TAPE_THICKNESS_METERS
}

fn format_elapsed_time(normalized_position: f64) -> String {
    let elapsed_milliseconds =
        (normalized_position.clamp(0.0, 1.0) * CASSETTE_DURATION_SECONDS * 1000.0).round() as u64;
    let minutes = elapsed_milliseconds / 60_000;
    let seconds = (elapsed_milliseconds / 1_000) % 60;
    let milliseconds = elapsed_milliseconds % 1_000;

    format!("{minutes:02}:{seconds:02}.{milliseconds:03}")
}

/// Resolve a counter-clockwise spindle animation frame from an angle in radians.
pub fn resolve_spoke_frame(angle_radians: f64) -> usize {
    if !angle_radians.is_finite() {
        return 0;
    }

    ((angle_radians.rem_euclid(SPOKE_ANIMATION_ARC_RADIANS) / SPOKE_FRAME_ANGLE_RADIANS + SPOKE_FRAME_INDEX_EPSILON)
        .floor() as usize)
        % SPOKE_FRAME_COUNT
}

pub struct CassetteDeck {
    animation: CassetteAnimationController,
    draw_cassette_png: bool,
    scale: f32,
}

impl CassetteDeck {
    pub fn new() -> Self {
        Self {
            animation: CassetteAnimationController::new(),
            draw_cassette_png: true,
            scale: 2.0,
        }
    }

    pub fn animation_controller(&self) -> &CassetteAnimationController {
        &self.animation
    }

    pub fn animation_controller_mut(&mut self) -> &mut CassetteAnimationController {
        &mut self.animation
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn set_scale(&mut self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.scale = scale;
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            let (image_rect, _) = ui.allocate_exact_size(
                egui::vec2(
                    CASSETTE_IMAGE_LOGICAL_WIDTH * self.scale,
                    CASSETTE_IMAGE_LOGICAL_HEIGHT * self.scale,
                ),
                egui::Sense::hover(),
            );
            //ui.painter().rect_filled(image_rect, 0.0, CASSETTE_BACKGROUND_COLOR);
            egui::Image::new(egui::include_image!("../../../../../assets/cassette_tape_shadow.png"))
                .texture_options(egui::TextureOptions::NEAREST)
                .paint_at(ui, image_rect);
            paint_tape_reel_shadow(
                ui,
                image_rect,
                LEFT_SPOKE_POSITION,
                self.animation.left_tape_diameter_logical_pixels(),
                self.scale,
            );
            paint_tape_reel(
                ui,
                image_rect,
                LEFT_SPOKE_POSITION,
                self.animation.left_tape_diameter_logical_pixels(),
                self.scale,
            );
            paint_tape_reel(
                ui,
                image_rect,
                RIGHT_SPOKE_POSITION,
                self.animation.right_tape_diameter_logical_pixels(),
                self.scale,
            );
            paint_spindle_hub(ui, image_rect, LEFT_SPOKE_POSITION, self.scale);
            paint_spindle_hub(ui, image_rect, RIGHT_SPOKE_POSITION, self.scale);
            paint_spindle(
                ui,
                image_rect,
                LEFT_SPOKE_POSITION,
                self.animation.left_frame(),
                self.scale,
            );
            paint_spindle(
                ui,
                image_rect,
                RIGHT_SPOKE_POSITION,
                self.animation.right_frame(),
                self.scale,
            );
            if self.draw_cassette_png {
                egui::Image::new(egui::include_image!("../../../../../assets/cassette_tape_02.png"))
                    .texture_options(egui::TextureOptions::NEAREST)
                    .paint_at(ui, image_rect);
            }

            ui.add_space(8.0);
            let mut normalized_position = self.animation.normalized_position();
            ui.label(format_elapsed_time(normalized_position));
            let slider_response = ui
                .scope(|ui| {
                    ui.spacing_mut().slider_width = ui.available_width();
                    ui.add(egui::Slider::new(&mut normalized_position, 0.0..=1.0).show_value(false))
                })
                .inner;
            if slider_response.changed() {
                self.animation.set_normalized_position(normalized_position);
            }

            ui.add_space(8.0);
            let button_row_width = TRANSPORT_BUTTON_WIDTH * TRANSPORT_BUTTONS.len() as f32
                + ui.spacing().item_spacing.x * (TRANSPORT_BUTTONS.len() - 1) as f32;
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - button_row_width) * 0.5).max(0.0));
                for (symbol, label) in TRANSPORT_BUTTONS {
                    let _ = ui.add(CassetteDeckButton::new(symbol, label));
                }
            });
            ui.checkbox(&mut self.draw_cassette_png, "Draw cassette PNG");
        });
    }
}

fn paint_tape_reel_shadow(
    ui: &egui::Ui,
    cassette_rect: egui::Rect,
    position: (f32, f32),
    logical_diameter: f32,
    scale: f32,
) {
    let excluded_rect = spindle_rect(cassette_rect, position, scale);
    let shadow_center = excluded_rect.center() + egui::Vec2::splat(SHADOW_OFFSET * scale);
    paint_circle_excluding_rect(
        ui,
        shadow_center,
        logical_diameter * scale * 0.5,
        CASSETTE_SPOOL_SHADOW_COLOR,
        excluded_rect,
    );
}

fn paint_tape_reel(ui: &egui::Ui, cassette_rect: egui::Rect, position: (f32, f32), logical_diameter: f32, scale: f32) {
    let excluded_rect = spindle_rect(cassette_rect, position, scale);
    paint_circle_excluding_rect(
        ui,
        excluded_rect.center(),
        logical_diameter * scale * 0.5,
        CASSETTE_TAPE_COLOR,
        excluded_rect,
    );
}

fn paint_spindle_hub(ui: &egui::Ui, cassette_rect: egui::Rect, position: (f32, f32), scale: f32) {
    let excluded_rect = spindle_rect(cassette_rect, position, scale);
    paint_circle_excluding_rect(
        ui,
        excluded_rect.center(),
        CASSETTE_HUB_DIAMETER_LOGICAL_PIXELS as f32 * scale * 0.5,
        CASSETTE_SPINDLE_COLOR,
        excluded_rect,
    );
    // Leave the spindle backing transparent so the PNG frame's alpha can expose
    // the untouched content beneath it.
    // ui.painter().circle_filled(
    //     excluded_rect.center(),
    //     CASSETTE_REEL_DIAMETER_LOGICAL_PIXELS as f32 * scale * 0.5,
    //     CASSETTE_SPINDLE_INNER_COLOR,
    // );
}

fn paint_circle_excluding_rect(
    ui: &egui::Ui,
    center: egui::Pos2,
    radius: f32,
    color: egui::Color32,
    excluded_rect: egui::Rect,
) {
    let bounds = ui.clip_rect();
    let excluded_rect = excluded_rect.intersect(bounds);
    let clip_rects = [
        egui::Rect::from_min_max(bounds.min, egui::pos2(bounds.right(), excluded_rect.top())),
        egui::Rect::from_min_max(egui::pos2(bounds.left(), excluded_rect.bottom()), bounds.max),
        egui::Rect::from_min_max(
            egui::pos2(bounds.left(), excluded_rect.top()),
            egui::pos2(excluded_rect.left(), excluded_rect.bottom()),
        ),
        egui::Rect::from_min_max(
            egui::pos2(excluded_rect.right(), excluded_rect.top()),
            egui::pos2(bounds.right(), excluded_rect.bottom()),
        ),
    ];

    for clip_rect in clip_rects {
        if clip_rect.width() > 0.0 && clip_rect.height() > 0.0 {
            ui.painter()
                .with_clip_rect(clip_rect)
                .circle_filled(center, radius, color);
        }
    }
}

fn paint_spindle(ui: &egui::Ui, cassette_rect: egui::Rect, position: (f32, f32), frame: usize, scale: f32) {
    let spindle_rect = spindle_rect(cassette_rect, position, scale);
    egui::Image::new(spoke_image(frame))
        .texture_options(egui::TextureOptions::NEAREST)
        .paint_at(ui, spindle_rect);
}

fn spindle_rect(cassette_rect: egui::Rect, position: (f32, f32), scale: f32) -> egui::Rect {
    egui::Rect::from_min_size(
        cassette_rect.min + egui::vec2(position.0 + X_OFFSET, position.1) * scale,
        egui::Vec2::splat(SPOKE_IMAGE_SIZE * scale),
    )
}

fn spoke_image(frame: usize) -> egui::ImageSource<'static> {
    match frame % SPOKE_FRAME_COUNT {
        0 => egui::include_image!("../../../../../assets/cassette_spoke01.png"),
        1 => egui::include_image!("../../../../../assets/cassette_spoke02.png"),
        2 => egui::include_image!("../../../../../assets/cassette_spoke03.png"),
        3 => egui::include_image!("../../../../../assets/cassette_spoke04.png"),
        4 => egui::include_image!("../../../../../assets/cassette_spoke05.png"),
        5 => egui::include_image!("../../../../../assets/cassette_spoke06.png"),
        6 => egui::include_image!("../../../../../assets/cassette_spoke07.png"),
        7 => egui::include_image!("../../../../../assets/cassette_spoke08.png"),
        8 => egui::include_image!("../../../../../assets/cassette_spoke09.png"),
        9 => egui::include_image!("../../../../../assets/cassette_spoke10.png"),
        10 => egui::include_image!("../../../../../assets/cassette_spoke11.png"),
        11 => egui::include_image!("../../../../../assets/cassette_spoke12.png"),
        12 => egui::include_image!("../../../../../assets/cassette_spoke13.png"),
        13 => egui::include_image!("../../../../../assets/cassette_spoke14.png"),
        14 => egui::include_image!("../../../../../assets/cassette_spoke15.png"),
        _ => unreachable!(),
    }
}

struct CassetteDeckButton<'a> {
    symbol: &'a str,
    label:  &'a str,
}

impl<'a> CassetteDeckButton<'a> {
    fn new(symbol: &'a str, label: &'a str) -> Self {
        Self { symbol, label }
    }
}

impl egui::Widget for CassetteDeckButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let response = ui.add_sized([TRANSPORT_BUTTON_WIDTH, TRANSPORT_BUTTON_HEIGHT], egui::Button::new(""));
        let content_rect = response.rect.shrink(2.0);
        let text_color = ui.style().interact(&response).fg_stroke.color;
        let painter = ui.painter().with_clip_rect(content_rect);
        let center_x = content_rect.center().x;

        painter.text(
            egui::pos2(center_x, content_rect.top() + 15.0),
            egui::Align2::CENTER_CENTER,
            self.symbol,
            egui::FontId::proportional(TRANSPORT_SYMBOL_SIZE),
            text_color,
        );
        painter.text(
            egui::pos2(center_x, content_rect.bottom() - 9.0),
            egui::Align2::CENTER_CENTER,
            self.label,
            egui::TextStyle::Button.resolve(ui.style()),
            text_color,
        );

        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), self.label));

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoke_frame_wraps_every_sixty_degrees() {
        assert_eq!(resolve_spoke_frame(0.0), 0);
        assert_eq!(resolve_spoke_frame(SPOKE_FRAME_ANGLE_RADIANS), 1);
        assert_eq!(resolve_spoke_frame(SPOKE_FRAME_ANGLE_RADIANS * 14.0), 14);
        assert_eq!(resolve_spoke_frame(SPOKE_ANIMATION_ARC_RADIANS), 0);
        assert_eq!(resolve_spoke_frame(std::f64::consts::TAU), 0);
        assert_eq!(resolve_spoke_frame(-SPOKE_FRAME_ANGLE_RADIANS), 14);
    }

    #[test]
    fn tape_amount_moves_between_reels() {
        let mut animation = CassetteAnimationController::new();
        assert_eq!(animation.left_tape_length(), CASSETTE_TAPE_LENGTH_METERS);
        assert_eq!(animation.right_tape_length(), 0.0);
        assert!(
            (animation.right_tape_diameter_logical_pixels() - CASSETTE_HUB_DIAMETER_LOGICAL_PIXELS as f32).abs()
                < 0.001
        );

        animation.set_linear_position(CASSETTE_TAPE_LENGTH_METERS * 0.5);
        assert_eq!(animation.left_tape_length(), animation.right_tape_length());
        assert!(
            (animation.left_tape_diameter_logical_pixels() - animation.right_tape_diameter_logical_pixels()).abs()
                < 0.001
        );

        animation.set_linear_position(CASSETTE_TAPE_LENGTH_METERS);
        assert_eq!(animation.left_tape_length(), 0.0);
        assert_eq!(animation.right_tape_length(), CASSETTE_TAPE_LENGTH_METERS);
        assert!(
            (animation.left_tape_diameter_logical_pixels() - CASSETTE_HUB_DIAMETER_LOGICAL_PIXELS as f32).abs() < 0.001
        );
    }

    #[test]
    fn normalized_position_moves_tape_between_reels() {
        let mut animation = CassetteAnimationController::new();

        animation.set_normalized_position(0.25);
        assert!((animation.linear_position() - CASSETTE_TAPE_LENGTH_METERS * 0.25).abs() < f64::EPSILON);
        assert!((animation.normalized_position() - 0.25).abs() < f64::EPSILON);

        animation.set_normalized_position(1.0);
        assert_eq!(animation.left_tape_length(), 0.0);
        assert_eq!(animation.right_tape_length(), CASSETTE_TAPE_LENGTH_METERS);
    }

    #[test]
    fn elapsed_time_is_formatted_for_a_c60_tape() {
        assert_eq!(format_elapsed_time(0.0), "00:00.000");
        assert_eq!(format_elapsed_time(0.5), "30:00.000");
        assert_eq!(format_elapsed_time(1.0), "60:00.000");
        assert_eq!(format_elapsed_time(0.500_000_277_777_777_8), "30:00.001");
    }
}
