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
*/

use std::collections::BTreeMap;

use marty_common::types::keys::MartyKey;
use marty_frontend_common::keyboard_manager::{OsdKey, OsdKeyboard};

use anyhow::{Context, Error};
use egui::{
    scroll_area::{ScrollBarVisibility, ScrollSource},
    Color32,
    ColorImage,
    Frame,
    PointerButton,
    Pos2,
    Rect,
    ScrollArea,
    Sense,
    TextureHandle,
    TextureOptions,
    Vec2,
};

const POINTER_HOLD_THRESHOLD_SECONDS: f64 = 0.15;
const TOUCH_POINTER_SUPPRESSION_SECONDS: f64 = 0.4;
const PAN_SPEED_POINTS_PER_SECOND: f32 = 600.0;
const HIDE_SWIPE_THRESHOLD: f32 = 40.0;
const UNHIDE_SWIPE_THRESHOLD: f32 = 80.0;

const BUTTON_BAR_BUTTON_H: f32 = 24.0;
const PAN_CONTROL_HITBOX_W: f32 = 36.0;

type TouchContactId = (egui::TouchDeviceId, egui::TouchId);

#[derive(Copy, Clone)]
struct TouchInputEvent {
    id:    TouchContactId,
    phase: egui::TouchPhase,
    pos:   Pos2,
}

struct TouchContact {
    key: Option<MartyKey>,
    key_sent: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PanDirection {
    Left,
    Right,
}

struct UnhideTouchContact {
    id: TouchContactId,
    start_pos: Pos2,
    cancelled: bool,
}

struct HideTouchContact {
    id: TouchContactId,
    start_pos: Pos2,
    cancelled: bool,
}

#[derive(Default)]
struct TouchPointerGuard {
    suppressed_until: f64,
}

impl TouchPointerGuard {
    fn update(&mut self, saw_touch_event: bool, has_active_contacts: bool, current_time: f64) -> bool {
        if saw_touch_event {
            self.suppressed_until = current_time + TOUCH_POINTER_SUPPRESSION_SECONDS;
        }
        has_active_contacts || current_time < self.suppressed_until
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OsdKeyboardEvent {
    KeyPress(MartyKey),
    KeyRelease(MartyKey),
    ResetKeyboard,
    HideKeyboard,
}

#[derive(Default)]
struct KeyLatchState {
    sticky_modifier: Option<MartyKey>,
    hold_enabled: bool,
    held_key: Option<MartyKey>,
}

impl KeyLatchState {
    fn commit_key_press(&mut self, key: MartyKey, events: &mut Vec<OsdKeyboardEvent>) -> bool {
        if self.key_is_latched(key) {
            return true;
        }

        if self.hold_enabled && self.held_key.is_none() {
            self.held_key = Some(key);
            events.push(OsdKeyboardEvent::KeyPress(key));
            if let Some(modifier) = self.sticky_modifier.take() {
                events.push(OsdKeyboardEvent::KeyRelease(modifier));
            }
            return true;
        }

        events.push(OsdKeyboardEvent::KeyPress(key));
        if let Some(modifier) = self.sticky_modifier.take() {
            events.push(OsdKeyboardEvent::KeyRelease(modifier));
            events.push(OsdKeyboardEvent::KeyRelease(key));
            true
        }
        else {
            false
        }
    }

    fn finish_key_press(&mut self, key: MartyKey, events: &mut Vec<OsdKeyboardEvent>) {
        events.push(OsdKeyboardEvent::KeyRelease(key));
    }

    fn toggle_hold(&mut self, events: &mut Vec<OsdKeyboardEvent>) {
        self.hold_enabled = !self.hold_enabled;
        if !self.hold_enabled {
            if let Some(key) = self.held_key.take() {
                events.push(OsdKeyboardEvent::KeyRelease(key));
            }
        }
    }

    fn toggle_sticky_modifier(&mut self, key: MartyKey, events: &mut Vec<OsdKeyboardEvent>) {
        if self.sticky_modifier == Some(key) {
            self.sticky_modifier = None;
            events.push(OsdKeyboardEvent::KeyRelease(key));
            return;
        }
        if self.held_key == Some(key) {
            return;
        }

        if let Some(modifier) = self.sticky_modifier.replace(key) {
            events.push(OsdKeyboardEvent::KeyRelease(modifier));
        }
        events.push(OsdKeyboardEvent::KeyPress(key));
    }

    fn release_all(&mut self, events: &mut Vec<OsdKeyboardEvent>) {
        self.hold_enabled = false;
        let modifier = self.sticky_modifier.take();
        if let Some(key) = modifier {
            events.push(OsdKeyboardEvent::KeyRelease(key));
        }
        if let Some(key) = self.held_key.take() {
            if Some(key) != modifier {
                events.push(OsdKeyboardEvent::KeyRelease(key));
            }
        }
    }

    fn key_is_latched(&self, key: MartyKey) -> bool {
        self.sticky_modifier == Some(key) || self.held_key == Some(key)
    }

    fn modifier_is_latched(&self, key: MartyKey) -> bool {
        self.sticky_modifier == Some(key)
    }
}

struct KeyboardRenderData {
    keyboard: OsdKeyboard,
    source_image: Option<ColorImage>,
    bezel_image: Option<ColorImage>,
    source_texture: Option<TextureHandle>,
    bezel_texture: Option<TextureHandle>,
}

#[derive(Default)]
pub struct OsdKeyboardPanel {
    keyboard: Option<KeyboardRenderData>,
    pressed_key: Option<MartyKey>,
    pressed_key_sent: bool,
    press_started_at: f64,
    latch: KeyLatchState,
    touch_contacts: BTreeMap<TouchContactId, TouchContact>,
    touch_pan_contacts: BTreeMap<TouchContactId, PanDirection>,
    pointer_pan: Option<PanDirection>,
    hide_contact: Option<HideTouchContact>,
    unhide_contact: Option<UnhideTouchContact>,
    pointer_guard: TouchPointerGuard,
}

impl OsdKeyboardPanel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_keyboard(&mut self, keyboard: Option<OsdKeyboard>) -> Result<(), Error> {
        self.pressed_key = None;
        self.pressed_key_sent = false;
        self.latch = KeyLatchState::default();
        self.touch_contacts.clear();
        self.touch_pan_contacts.clear();
        self.pointer_pan = None;
        self.hide_contact = None;
        self.unhide_contact = None;
        self.pointer_guard = TouchPointerGuard::default();
        self.keyboard = keyboard.map(Self::prepare_textures).transpose()?;
        Ok(())
    }

    pub fn is_available(&self) -> bool {
        self.keyboard.is_some()
    }

    pub fn detect_unhide_swipe(&mut self, ctx: &egui::Context, viewport_rect: Rect) -> bool {
        let touch_events = ctx.input(|input| {
            input
                .raw
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Touch {
                        device_id,
                        id,
                        phase,
                        pos,
                        ..
                    } => Some(TouchInputEvent {
                        id:    (*device_id, *id),
                        phase: *phase,
                        pos:   *pos,
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>()
        });

        for touch_event in touch_events {
            match touch_event.phase {
                egui::TouchPhase::Start => {
                    if self.unhide_contact.is_some() {
                        if let Some(contact) = self.unhide_contact.as_mut() {
                            contact.cancelled = true;
                        }
                    }
                    else if viewport_rect.contains(touch_event.pos) {
                        self.unhide_contact = Some(UnhideTouchContact {
                            id: touch_event.id,
                            start_pos: touch_event.pos,
                            cancelled: false,
                        });
                    }
                }
                egui::TouchPhase::Move => {
                    let Some(contact) = self.unhide_contact.as_ref()
                    else {
                        continue;
                    };
                    if contact.id == touch_event.id
                        && !contact.cancelled
                        && is_unhide_swipe(touch_event.pos - contact.start_pos)
                    {
                        self.unhide_contact = None;
                        return true;
                    }
                }
                egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                    if self.unhide_contact.as_ref().map(|contact| contact.id) == Some(touch_event.id) {
                        self.unhide_contact = None;
                    }
                }
            }
        }

        false
    }

    pub fn cancel_unhide_gesture(&mut self) {
        self.unhide_contact = None;
    }

    // Cancel all pressed key events.
    // Called when we hide the OSD keyboard
    pub fn cancel_pressed_keys(&mut self) -> Vec<OsdKeyboardEvent> {
        let pressed_key = self.pressed_key.take();
        let mut releases = Vec::new();
        if self.pressed_key_sent {
            if let Some(key) = pressed_key {
                releases.push(OsdKeyboardEvent::KeyRelease(key));
            }
        }
        self.pressed_key_sent = false;
        for contact in self.touch_contacts.values() {
            if contact.key_sent {
                if let Some(key) = contact.key {
                    releases.push(OsdKeyboardEvent::KeyRelease(key));
                }
            }
        }
        self.touch_contacts.clear();
        self.touch_pan_contacts.clear();
        self.pointer_pan = None;
        self.hide_contact = None;
        self.latch.release_all(&mut releases);
        releases
    }

    /// Draw the OSD keyboard
    pub fn show(&mut self, root_ui: &mut egui::Ui) -> Vec<OsdKeyboardEvent> {
        let Some(render_data) = self.keyboard.as_mut()
        else {
            return Vec::new();
        };

        Self::load_textures_indempotent(root_ui.ctx(), render_data);
        let Some(source_texture) = render_data.source_texture.as_ref()
        else {
            return Vec::new();
        };
        let Some(bezel_texture) = render_data.bezel_texture.as_ref()
        else {
            return Vec::new();
        };

        let input_state = root_ui.input(|input| {
            let touch_events = input
                .raw
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Touch {
                        device_id,
                        id,
                        phase,
                        pos,
                        ..
                    } => Some(TouchInputEvent {
                        id:    (*device_id, *id),
                        phase: *phase,
                        pos:   *pos,
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.button_released(PointerButton::Primary),
                input.pointer.interact_pos(),
                input.time,
                input.stable_dt,
                touch_events,
            )
        });
        let (pointer_pressed, pointer_down, pointer_released, pointer_position, current_time, frame_dt, touch_events) =
            input_state;
        let touch_contact_active = !touch_events.is_empty()
            || !self.touch_contacts.is_empty()
            || !self.touch_pan_contacts.is_empty()
            || self.hide_contact.is_some();
        let pointer_input_suppressed =
            self.pointer_guard
                .update(!touch_events.is_empty(), touch_contact_active, current_time);

        let mut events = Vec::new();
        let keyboard = &render_data.keyboard;
        let base_color = Color32::from_rgba_unmultiplied(
            keyboard.bitmap.base_color[0],
            keyboard.bitmap.base_color[1],
            keyboard.bitmap.base_color[2],
            keyboard.bitmap.base_color[3],
        );
        let bitmap_size = Vec2::new(keyboard.bitmap.width as f32, keyboard.bitmap.height as f32);
        let frame = Frame::NONE.fill(base_color);

        let mut hide_requested = false;

        // Make a new Panel for the OSD keyboard
        egui::Panel::bottom("martypc_osd_keyboard_panel")
            .exact_size(bitmap_size.y)
            .frame(frame)
            .show_inside(root_ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                // Use the entire panel dimensions
                let panel_rect = ui.max_rect();
                let viewport_width = ui.available_width();

                // Determine if the keyboard fits width-wise into the current viewport
                let keyboard_needs_pan = bitmap_size.x > viewport_width;

                // Align buttons from center.
                let button_size = Vec2::new(72.0, BUTTON_BAR_BUTTON_H);
                let hold_button_rect = Rect::from_center_size(
                    Pos2::new(panel_rect.center().x, panel_rect.top() + button_size.y * 0.5 + 4.0),
                    button_size,
                );
                let button_offset = Vec2::new(button_size.x + 4.0, 0.0);

                let ctrl_button_rect = hold_button_rect.translate(-button_offset * 2.0);
                let shift_button_rect = hold_button_rect.translate(-button_offset);

                let reset_button_rect = hold_button_rect.translate(button_offset);
                let hide_button_rect = hold_button_rect.translate(button_offset * 2.0);

                let pan_hitbox_width = PAN_CONTROL_HITBOX_W;
                let pan_hitbox_top = hold_button_rect.bottom() + 4.0;
                let pan_left_hitbox = Rect::from_min_max(
                    Pos2::new(panel_rect.left(), pan_hitbox_top),
                    Pos2::new(panel_rect.left() + pan_hitbox_width, panel_rect.bottom()),
                );
                let pan_right_hitbox = Rect::from_min_max(
                    Pos2::new(panel_rect.right() - pan_hitbox_width, pan_hitbox_top),
                    panel_rect.right_bottom(),
                );

                let button_rects = [
                    ctrl_button_rect,
                    shift_button_rect,
                    hold_button_rect,
                    reset_button_rect,
                    hide_button_rect,
                ];

                let position_over_toolbar = |position| button_rects.iter().any(|rect| rect.contains(position));
                let pan_direction_at_position = |position| {
                    if !keyboard_needs_pan || position_over_toolbar(position) {
                        None
                    }
                    else if pan_left_hitbox.contains(position) {
                        Some(PanDirection::Left)
                    }
                    else if pan_right_hitbox.contains(position) {
                        Some(PanDirection::Right)
                    }
                    else {
                        None
                    }
                };

                if !keyboard_needs_pan {
                    self.touch_pan_contacts.clear();
                    self.pointer_pan = None;
                }

                // Keyboard pan area is a ScrollArea with scrollbar hidden.
                // Only pan controls on each side scroll.
                let scroll_output = ScrollArea::horizontal()
                    .id_salt("martypc_osd_keyboard_scroll")
                    .scroll_source(ScrollSource::MOUSE_WHEEL)
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let pan_padding = if keyboard_needs_pan { pan_hitbox_width } else { 0.0 };
                        let scrollable_width = bitmap_size.x + pan_padding * 2.0;

                        // Allocate 'canvas' for our scrolling keyboard - width + padding
                        let container_size = Vec2::new(scrollable_width.max(viewport_width), bitmap_size.y);
                        let (container_rect, _) = ui.allocate_exact_size(container_size, Sense::hover());
                        let canvas_rect = Rect::from_min_size(
                            Pos2::new(
                                container_rect.min.x + pan_padding + (container_rect.width() - scrollable_width) * 0.5,
                                container_rect.min.y,
                            ),
                            bitmap_size,
                        );

                        // The user can swipe down from the top of the keyboard to hide it, if
                        // there is room to do so (usually horizontal layout)
                        let hide_swipe_region = Rect::from_min_max(
                            Pos2::new(canvas_rect.left().max(panel_rect.left()), canvas_rect.top()),
                            Pos2::new(
                                canvas_rect.right().min(panel_rect.right()),
                                pan_hitbox_top.min(canvas_rect.bottom()),
                            ),
                        );
                        let position_in_hide_swipe_region = |position| hide_swipe_region.contains(position);

                        // Handle touch events for mobile/tablets
                        for touch_event in &touch_events {
                            match touch_event.phase {
                                egui::TouchPhase::Start => {
                                    if !panel_rect.contains(touch_event.pos) {
                                        continue;
                                    }
                                    if self.hide_contact.is_some() {
                                        if let Some(contact) = self.hide_contact.as_mut() {
                                            contact.cancelled = true;
                                        }
                                    }
                                    else if position_in_hide_swipe_region(touch_event.pos) {
                                        self.hide_contact = Some(HideTouchContact {
                                            id: touch_event.id,
                                            start_pos: touch_event.pos,
                                            cancelled: false,
                                        });
                                        continue;
                                    }
                                    if let Some(direction) = pan_direction_at_position(touch_event.pos) {
                                        let contacts = std::mem::take(&mut self.touch_contacts);
                                        for (_, contact) in contacts {
                                            cancel_touch_contact(contact, &mut events);
                                        }
                                        self.touch_pan_contacts.insert(touch_event.id, direction);
                                        continue;
                                    }
                                    if position_over_toolbar(touch_event.pos) {
                                        continue;
                                    }
                                    if !self.touch_pan_contacts.is_empty() {
                                        continue;
                                    }
                                    let had_active_contacts = !self.touch_contacts.is_empty();
                                    if had_active_contacts {
                                        for contact in self.touch_contacts.values_mut() {
                                            commit_touch_contact(contact, &mut self.latch, &mut events);
                                        }
                                    }

                                    let key = keyboard
                                        .keys
                                        .iter()
                                        .find(|key| source_rect(canvas_rect.min, key).contains(touch_event.pos))
                                        .map(|key| key.code);
                                    let mut contact = TouchContact { key, key_sent: false };
                                    if had_active_contacts {
                                        commit_touch_contact(&mut contact, &mut self.latch, &mut events);
                                    }
                                    self.touch_contacts.insert(touch_event.id, contact);
                                }
                                egui::TouchPhase::Move => {
                                    // Handle hide swipe gesture
                                    let Some(contact) = self.hide_contact.as_ref()
                                    else {
                                        continue;
                                    };

                                    if contact.id != touch_event.id {
                                        continue;
                                    }
                                    if !contact.cancelled && is_hide_swipe(touch_event.pos - contact.start_pos) {
                                        hide_requested = true;
                                        self.hide_contact = None;
                                    }
                                }
                                egui::TouchPhase::End => {
                                    if self.hide_contact.as_ref().map(|contact| contact.id) == Some(touch_event.id) {
                                        self.hide_contact = None;
                                        continue;
                                    }
                                    self.touch_pan_contacts.remove(&touch_event.id);
                                    if let Some(mut contact) = self.touch_contacts.remove(&touch_event.id) {
                                        finish_touch_contact(&mut contact, &mut self.latch, &mut events);
                                    }
                                }
                                egui::TouchPhase::Cancel => {
                                    if self.hide_contact.as_ref().map(|contact| contact.id) == Some(touch_event.id) {
                                        self.hide_contact = None;
                                        continue;
                                    }
                                    self.touch_pan_contacts.remove(&touch_event.id);
                                    if let Some(contact) = self.touch_contacts.remove(&touch_event.id) {
                                        cancel_touch_contact(contact, &mut events);
                                    }
                                }
                            }
                        }

                        if pointer_input_suppressed {
                            if self.pressed_key_sent {
                                if let Some(key) = self.pressed_key {
                                    events.push(OsdKeyboardEvent::KeyRelease(key));
                                }
                            }
                            self.pressed_key = None;
                            self.pressed_key_sent = false;
                            self.pointer_pan = None;
                        }
                        else {
                            if pointer_pressed {
                                if self.pressed_key_sent {
                                    if let Some(key) = self.pressed_key {
                                        events.push(OsdKeyboardEvent::KeyRelease(key));
                                    }
                                }
                                self.pressed_key = None;
                                self.pressed_key_sent = false;
                                self.pointer_pan = None;

                                if let Some(position) = pointer_position {
                                    if let Some(direction) = pan_direction_at_position(position) {
                                        self.pointer_pan = Some(direction);
                                    }
                                    else if !position_over_toolbar(position) {
                                        if let Some(key) = keyboard
                                            .keys
                                            .iter()
                                            .find(|key| source_rect(canvas_rect.min, key).contains(position))
                                        {
                                            self.pressed_key = Some(key.code);
                                            self.press_started_at = current_time;
                                        }
                                    }
                                }
                            }

                            if pointer_down
                                && self.pressed_key.is_some()
                                && !self.pressed_key_sent
                                && current_time - self.press_started_at >= POINTER_HOLD_THRESHOLD_SECONDS
                            {
                                self.pressed_key_sent = true;
                                let key = self.pressed_key.unwrap();
                                if self.latch.commit_key_press(key, &mut events) {
                                    self.pressed_key = None;
                                    self.pressed_key_sent = false;
                                }
                            }

                            if pointer_released || (!pointer_down && self.pressed_key.is_some()) {
                                if let Some(key) = self.pressed_key {
                                    if !self.pressed_key_sent {
                                        if self.latch.commit_key_press(key, &mut events) {
                                            self.pressed_key = None;
                                        }
                                    }
                                    if self.pressed_key == Some(key) {
                                        self.pressed_key = None;
                                        self.latch.finish_key_press(key, &mut events);
                                    }
                                }
                                self.pressed_key_sent = false;
                            }
                            if pointer_released || !pointer_down {
                                self.pointer_pan = None;
                            }
                        }

                        // Draw the keys
                        let painter = ui.painter();
                        painter.rect_filled(canvas_rect, 0.0, base_color);

                        // Draw all pressed keys first, then non-pressed keys (painter's algorithm)
                        for pressed in [true, false] {
                            for key in &keyboard.keys {
                                let key_is_pressed = self.pressed_key == Some(key.code)
                                    || self.latch.key_is_latched(key.code)
                                    || self
                                        .touch_contacts
                                        .values()
                                        .any(|contact| contact.key == Some(key.code));
                                if key_is_pressed == pressed {
                                    let tint = if self.latch.key_is_latched(key.code) {
                                        Color32::from_gray(180)
                                    }
                                    else {
                                        Color32::WHITE
                                    };
                                    draw_key(
                                        painter,
                                        source_texture,
                                        canvas_rect.min,
                                        keyboard.bitmap.width,
                                        keyboard.bitmap.height,
                                        key,
                                        pressed,
                                        tint,
                                    );
                                }
                            }
                        }

                        // Draw the transparent bezel on top.
                        painter.image(
                            bezel_texture.id(),
                            canvas_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    });

                // Draw the UI buttons on top of the keyboard bezel
                if ui
                    .put(
                        hold_button_rect,
                        egui::Button::new("Hold").selected(self.latch.hold_enabled),
                    )
                    .on_hover_text("Hold the next key until Hold is turned off")
                    .clicked()
                {
                    self.latch.toggle_hold(&mut events);
                }

                if ui
                    .put(
                        shift_button_rect,
                        egui::Button::new("Shift").selected(self.latch.modifier_is_latched(MartyKey::ShiftLeft)),
                    )
                    .on_hover_text("Hold left Shift for the next key")
                    .clicked()
                {
                    self.latch.toggle_sticky_modifier(MartyKey::ShiftLeft, &mut events);
                }

                if ui
                    .put(
                        ctrl_button_rect,
                        egui::Button::new("Ctrl").selected(self.latch.modifier_is_latched(MartyKey::ControlLeft)),
                    )
                    .on_hover_text("Hold left Ctrl for the next key")
                    .clicked()
                {
                    self.latch.toggle_sticky_modifier(MartyKey::ControlLeft, &mut events);
                }

                if ui
                    .put(reset_button_rect, egui::Button::new("Reset"))
                    .on_hover_text("Reset the emulated keyboard")
                    .clicked()
                {
                    if self.pressed_key_sent {
                        if let Some(key) = self.pressed_key {
                            events.push(OsdKeyboardEvent::KeyRelease(key));
                        }
                    }
                    self.pressed_key = None;
                    self.pressed_key_sent = false;
                    for contact in self.touch_contacts.values() {
                        if contact.key_sent {
                            if let Some(key) = contact.key {
                                events.push(OsdKeyboardEvent::KeyRelease(key));
                            }
                        }
                    }
                    self.touch_contacts.clear();
                    self.touch_pan_contacts.clear();
                    self.pointer_pan = None;
                    self.latch.release_all(&mut events);
                    events.push(OsdKeyboardEvent::ResetKeyboard);
                }

                if ui
                    .put(hide_button_rect, egui::Button::new("Hide"))
                    .on_hover_text("Hide the on-screen keyboard")
                    .clicked()
                {
                    hide_requested = true;
                }

                // Draw the pan controls (only shown if the keyboard doesn't fit horizontally into
                // available viewport rect)
                if keyboard_needs_pan {
                    let painter = ui.painter();
                    painter.rect_stroke(
                        pan_left_hitbox.shrink(2.0),
                        0.0,
                        (2.0, Color32::WHITE),
                        egui::StrokeKind::Inside,
                    );
                    painter.rect_stroke(
                        pan_right_hitbox.shrink(2.0),
                        0.0,
                        (2.0, Color32::WHITE),
                        egui::StrokeKind::Inside,
                    );
                    let pan_left_active = self.pointer_pan == Some(PanDirection::Left)
                        || self
                            .touch_pan_contacts
                            .values()
                            .any(|direction| *direction == PanDirection::Left);
                    let pan_right_active = self.pointer_pan == Some(PanDirection::Right)
                        || self
                            .touch_pan_contacts
                            .values()
                            .any(|direction| *direction == PanDirection::Right);
                    let pan_direction = match (pan_left_active, pan_right_active) {
                        (true, false) => -1.0,
                        (false, true) => 1.0,
                        _ => 0.0,
                    };

                    if pan_direction != 0.0 {
                        let max_pan_offset = (scroll_output.content_size.x - scroll_output.inner_rect.width()).max(0.0);
                        let scroll_id = scroll_output.id;
                        let mut scroll_state = scroll_output.state;
                        let old_offset = scroll_state.offset.x;

                        scroll_state.offset.x = (old_offset
                            + pan_direction * PAN_SPEED_POINTS_PER_SECOND * frame_dt.min(0.05))
                        .clamp(0.0, max_pan_offset);

                        if scroll_state.offset.x != old_offset {
                            scroll_state.store(ui.ctx(), scroll_id);
                            // not strictly needed but in case we ever switch back to requested
                            // repaints the keyboard will still work
                            ui.ctx().request_repaint();
                        }
                    }
                }
            });

        if hide_requested {
            // Cancel all pressed keys on hide.
            events.extend(self.cancel_pressed_keys());
            events.push(OsdKeyboardEvent::HideKeyboard);
        }

        events
    }

    fn prepare_textures(keyboard: OsdKeyboard) -> Result<KeyboardRenderData, Error> {
        let source_image = decode_image(&keyboard.source_image, "source")?;
        let bezel_image = decode_image(&keyboard.bezel_image, "bezel")?;

        let expected_size = [keyboard.bitmap.width as usize, keyboard.bitmap.height as usize];
        if source_image.size != expected_size {
            return Err(anyhow::anyhow!(
                "OSD keyboard source image is {}x{}, but the layout declares {}x{}",
                source_image.size[0],
                source_image.size[1],
                expected_size[0],
                expected_size[1]
            ));
        }
        if bezel_image.size != expected_size {
            return Err(anyhow::anyhow!(
                "OSD keyboard bezel image is {}x{}, but the layout declares {}x{}",
                bezel_image.size[0],
                bezel_image.size[1],
                expected_size[0],
                expected_size[1]
            ));
        }

        Ok(KeyboardRenderData {
            keyboard,
            source_image: Some(source_image),
            bezel_image: Some(bezel_image),
            source_texture: None,
            bezel_texture: None,
        })
    }

    fn load_textures_indempotent(ctx: &egui::Context, render_data: &mut KeyboardRenderData) {
        if render_data.source_texture.is_none() {
            if let Some(image) = render_data.source_image.take() {
                render_data.source_texture = Some(ctx.load_texture(
                    format!("osd_keyboard_{}_source", render_data.keyboard.keyboard_name),
                    image,
                    TextureOptions::LINEAR,
                ));
            }
        }
        if render_data.bezel_texture.is_none() {
            if let Some(image) = render_data.bezel_image.take() {
                render_data.bezel_texture = Some(ctx.load_texture(
                    format!("osd_keyboard_{}_bezel", render_data.keyboard.keyboard_name),
                    image,
                    TextureOptions::LINEAR,
                ));
            }
        }
    }
}

fn commit_touch_contact(contact: &mut TouchContact, latch: &mut KeyLatchState, events: &mut Vec<OsdKeyboardEvent>) {
    if contact.key_sent {
        return;
    }
    let Some(key) = contact.key
    else {
        return;
    };

    contact.key_sent = true;
    if latch.commit_key_press(key, events) {
        contact.key = None;
        contact.key_sent = false;
    }
}

fn finish_touch_contact(contact: &mut TouchContact, latch: &mut KeyLatchState, events: &mut Vec<OsdKeyboardEvent>) {
    let Some(key) = contact.key.take()
    else {
        return;
    };

    if contact.key_sent {
        latch.finish_key_press(key, events);
    }
    else if !latch.commit_key_press(key, events) {
        latch.finish_key_press(key, events);
    }
    contact.key_sent = false;
}

fn cancel_touch_contact(contact: TouchContact, events: &mut Vec<OsdKeyboardEvent>) {
    if contact.key_sent {
        if let Some(key) = contact.key {
            events.push(OsdKeyboardEvent::KeyRelease(key));
        }
    }
}

fn is_unhide_swipe(delta: Vec2) -> bool {
    let upward_distance = -delta.y;
    upward_distance >= UNHIDE_SWIPE_THRESHOLD && upward_distance >= delta.x.abs() * 1.25
}

fn is_hide_swipe(delta: Vec2) -> bool {
    delta.y >= HIDE_SWIPE_THRESHOLD && delta.y >= delta.x.abs() * 1.25
}

fn decode_image(data: &[u8], description: &str) -> Result<ColorImage, Error> {
    let image = image::load_from_memory(data)
        .with_context(|| format!("Failed to decode OSD keyboard {description} image"))?
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}

fn source_rect(origin: Pos2, key: &OsdKey) -> Rect {
    Rect::from_min_size(
        origin + Vec2::new(key.source[0] as f32, key.source[1] as f32),
        Vec2::new(key.source[2] as f32, key.source[3] as f32),
    )
}

fn draw_key(
    painter: &egui::Painter,
    texture: &TextureHandle,
    origin: Pos2,
    bitmap_width: u32,
    bitmap_height: u32,
    key: &OsdKey,
    pressed: bool,
    tint: Color32,
) {
    // Keys are offset to a 2d point specified by the layout file.
    // If a key isn't pressed its destination rectangle is its source rectangle.
    let destination = if pressed {
        key.destination
    }
    else {
        [key.source[0], key.source[1]]
    };

    let destination_rect = Rect::from_min_size(
        origin + Vec2::new(destination[0] as f32, destination[1] as f32),
        Vec2::new(key.source[2] as f32, key.source[3] as f32),
    );

    let uv_rect = Rect::from_min_max(
        Pos2::new(
            key.source[0] as f32 / bitmap_width as f32,
            key.source[1] as f32 / bitmap_height as f32,
        ),
        Pos2::new(
            (key.source[0] + key.source[2]) as f32 / bitmap_width as f32,
            (key.source[1] + key.source[3]) as f32 / bitmap_height as f32,
        ),
    );

    // Draw the key
    painter.image(texture.id(), destination_rect, uv_rect, tint);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticky_modifier_is_released_by_the_next_different_key() {
        let mut latch = KeyLatchState::default();
        let mut events = Vec::new();
        latch.toggle_sticky_modifier(MartyKey::ShiftLeft, &mut events);
        assert_eq!(events, vec![OsdKeyboardEvent::KeyPress(MartyKey::ShiftLeft)]);
        assert!(latch.modifier_is_latched(MartyKey::ShiftLeft));

        events.clear();
        assert!(latch.commit_key_press(MartyKey::ShiftLeft, &mut events));
        assert!(events.is_empty());
        assert!(latch.modifier_is_latched(MartyKey::ShiftLeft));

        assert!(latch.commit_key_press(MartyKey::KeyA, &mut events));
        assert_eq!(
            events,
            vec![
                OsdKeyboardEvent::KeyPress(MartyKey::KeyA),
                OsdKeyboardEvent::KeyRelease(MartyKey::ShiftLeft),
                OsdKeyboardEvent::KeyRelease(MartyKey::KeyA),
            ]
        );
        assert!(latch.sticky_modifier.is_none());
    }

    #[test]
    fn hold_keeps_one_key_down_while_other_keys_are_tapped() {
        let mut latch = KeyLatchState::default();
        let mut events = Vec::new();
        latch.toggle_hold(&mut events);
        assert!(latch.hold_enabled);
        assert!(events.is_empty());

        assert!(latch.commit_key_press(MartyKey::AltLeft, &mut events));
        assert_eq!(events, vec![OsdKeyboardEvent::KeyPress(MartyKey::AltLeft)]);
        assert_eq!(latch.held_key, Some(MartyKey::AltLeft));

        events.clear();
        assert!(!latch.commit_key_press(MartyKey::Numpad1, &mut events));
        latch.finish_key_press(MartyKey::Numpad1, &mut events);
        assert_eq!(
            events,
            vec![
                OsdKeyboardEvent::KeyPress(MartyKey::Numpad1),
                OsdKeyboardEvent::KeyRelease(MartyKey::Numpad1),
            ]
        );
        assert!(latch.hold_enabled);
        assert_eq!(latch.held_key, Some(MartyKey::AltLeft));

        events.clear();
        latch.toggle_hold(&mut events);
        assert_eq!(events, vec![OsdKeyboardEvent::KeyRelease(MartyKey::AltLeft)]);
        assert!(!latch.hold_enabled);
        assert!(latch.held_key.is_none());
    }

    #[test]
    fn dedicated_modifier_buttons_toggle_and_switch_the_latched_key() {
        let mut latch = KeyLatchState::default();
        let mut events = Vec::new();

        latch.toggle_sticky_modifier(MartyKey::ControlLeft, &mut events);
        assert_eq!(events, vec![OsdKeyboardEvent::KeyPress(MartyKey::ControlLeft)]);
        assert!(latch.modifier_is_latched(MartyKey::ControlLeft));

        events.clear();
        latch.toggle_sticky_modifier(MartyKey::ShiftLeft, &mut events);
        assert_eq!(
            events,
            vec![
                OsdKeyboardEvent::KeyRelease(MartyKey::ControlLeft),
                OsdKeyboardEvent::KeyPress(MartyKey::ShiftLeft),
            ]
        );
        assert!(latch.modifier_is_latched(MartyKey::ShiftLeft));

        events.clear();
        latch.toggle_sticky_modifier(MartyKey::ShiftLeft, &mut events);
        assert_eq!(events, vec![OsdKeyboardEvent::KeyRelease(MartyKey::ShiftLeft)]);
        assert!(latch.sticky_modifier.is_none());
    }

    #[test]
    fn reset_releases_all_latched_keys_and_disables_hold() {
        let mut latch = KeyLatchState::default();
        let mut events = Vec::new();
        latch.toggle_hold(&mut events);
        assert!(latch.commit_key_press(MartyKey::AltLeft, &mut events));
        latch.toggle_sticky_modifier(MartyKey::ControlLeft, &mut events);

        events.clear();
        latch.release_all(&mut events);
        assert_eq!(
            events,
            vec![
                OsdKeyboardEvent::KeyRelease(MartyKey::ControlLeft),
                OsdKeyboardEvent::KeyRelease(MartyKey::AltLeft),
            ]
        );
        assert!(!latch.hold_enabled);
        assert!(latch.sticky_modifier.is_none());
        assert!(latch.held_key.is_none());
    }

    #[test]
    fn unhide_swipe_must_move_upward_and_be_vertically_dominant() {
        assert!(is_unhide_swipe(Vec2::new(0.0, -UNHIDE_SWIPE_THRESHOLD)));
        assert!(!is_unhide_swipe(Vec2::new(0.0, UNHIDE_SWIPE_THRESHOLD)));
        assert!(!is_unhide_swipe(Vec2::new(40.0, -40.0)));
    }

    #[test]
    fn hide_swipe_must_move_downward_and_be_vertically_dominant() {
        assert!(is_hide_swipe(Vec2::new(0.0, HIDE_SWIPE_THRESHOLD)));
        assert!(!is_hide_swipe(Vec2::new(0.0, -HIDE_SWIPE_THRESHOLD)));
        assert!(!is_hide_swipe(Vec2::new(40.0, 40.0)));
    }

    #[test]
    fn raw_touch_suppresses_followup_pointer_events() {
        let mut guard = TouchPointerGuard::default();
        assert!(guard.update(true, false, 1.0));
        assert!(guard.update(false, false, 1.2));
        assert!(!guard.update(false, false, 1.5));
    }

    #[test]
    fn touch_tap_emits_one_key_press_pair() {
        let mut latch = KeyLatchState::default();
        let mut events = Vec::new();
        let mut contact = TouchContact {
            key: Some(MartyKey::KeyA),
            key_sent: false,
        };

        finish_touch_contact(&mut contact, &mut latch, &mut events);
        assert_eq!(
            events,
            vec![
                OsdKeyboardEvent::KeyPress(MartyKey::KeyA),
                OsdKeyboardEvent::KeyRelease(MartyKey::KeyA),
            ]
        );
    }

    #[test]
    fn simultaneous_touch_contacts_keep_both_keys_pressed() {
        let mut latch = KeyLatchState::default();
        let mut events = Vec::new();
        let mut shift = TouchContact {
            key: Some(MartyKey::ShiftLeft),
            key_sent: false,
        };
        let mut one = TouchContact {
            key: Some(MartyKey::Digit1),
            key_sent: false,
        };

        commit_touch_contact(&mut shift, &mut latch, &mut events);
        commit_touch_contact(&mut one, &mut latch, &mut events);
        assert_eq!(
            events,
            vec![
                OsdKeyboardEvent::KeyPress(MartyKey::ShiftLeft),
                OsdKeyboardEvent::KeyPress(MartyKey::Digit1),
            ]
        );

        finish_touch_contact(&mut one, &mut latch, &mut events);
        assert_eq!(events.last(), Some(&OsdKeyboardEvent::KeyRelease(MartyKey::Digit1)));
        assert!(shift.key_sent);

        finish_touch_contact(&mut shift, &mut latch, &mut events);
        assert_eq!(events.last(), Some(&OsdKeyboardEvent::KeyRelease(MartyKey::ShiftLeft)));
    }
}
