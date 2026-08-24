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

    devices::keyboard.rs

    Implementation of various keyboards.

*/

use std::{
    collections::{HashMap, VecDeque},
    str::FromStr,
    vec::Vec,
};

use crate::{
    device_types::keyboard::KeyboardType,
    devices::keyboards::{model_f::ModelF, pcjr::PcJrKeyboard, tandy1000::Tandy1000Keyboard},
    keys::MartyKey,
    machine::KeybufferEntry,
};
use anyhow::{bail, Result};
use serde_derive::Deserialize;
use strum::IntoEnumIterator;
use toml;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct KeyboardModifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl KeyboardModifiers {
    pub fn have_any(&self) -> bool {
        self.control || self.alt || self.shift || self.meta
    }
}

/// Incoming keycode-presses can be translated two possible ways.
/// In macro mode, translation produces additional keycodes that are fed back
/// into the emulator's keyboard buffer for later delivery and processing.
/// In scancode mode, translation produces a series of scancodes to be
/// inserted into the emulated keyboard's keyboard buffer directly.
#[derive(Clone, Debug)]
pub enum TranslationType {
    Keycode(Vec<KeybufferEntry>),
    Scancode(Vec<u8>),
}

pub enum TranslationMode {
    Keycode,
    Scancode,
}

#[derive(Clone, Debug)]
pub struct KeyState {
    pressed: bool,
    pressed_time: f64,            // Time the key has been pressed in microseconds.
    repeat_time: f64,             // Time accumulator until next repeat (at typematic_rate ms)
    translation: Option<Vec<u8>>, // The scancode translation applied to this key when it was pressed.
}

/// KeyState defaults to unpressed.
impl Default for KeyState {
    fn default() -> KeyState {
        KeyState {
            pressed: false,
            pressed_time: 0.0,
            repeat_time: 0.0,
            translation: None,
        }
    }
}

// Keyboard mapping file definitions
#[derive(Debug, Deserialize)]
pub struct KeyboardMappingFile {
    keyboard: KeyboardDefinition,
}

#[derive(Debug, Deserialize)]
pub struct KeyboardDefinition {
    modelf: ModelF,
    tandy1000: Tandy1000Keyboard,
    pcjr: PcJrKeyboard,
}

#[derive(Debug, Deserialize)]
pub struct KeycodeMapping {
    keycode: String,
    modifiers: Vec<String>,
    key_macro: Vec<String>,
    macro_translate: bool,
    scancodes: Vec<u8>,
}

/// Keyboard definition struct.
/// We maintain a hashmap of MartyKey to KeyState. This allows us to track
/// which keys are currently pressed or not, and how long they have been
/// pressed.
///
/// For speed of updating the keyboard device, currently pressed keys are
/// stored in the keys_pressed vector. This allows us to avoid iterating
/// through all keys in the kb_map every keyboard update. We must add
/// keys to keys_pressed on keydown and remove them on keyup.
pub struct Keyboard {
    debug: bool,
    kb_type: KeyboardType,
    kb_hash: HashMap<MartyKey, KeyState>,
    keys_pressed: Vec<MartyKey>,
    typematic: bool,
    typematic_delay: f64, // Typematic repeat delay from initial keypress (ms)
    typematic_rate: f64,  // Typematic repeat rate (ms)
    kb_buffer_size: usize,
    kb_buffer: Vec<u8>, // Keyboard buffer. Variable length depending on keyboard model.
    kb_buffer_overflow: bool,
    reset_buffer: Vec<u8>, // Keyboard buffer to hold queued reset scancodes on keyboard reset.
    keycode_mappings: Vec<KeycodeMapping>,
}

impl Default for Keyboard {
    fn default() -> Keyboard {
        Keyboard {
            debug: true,
            kb_type: KeyboardType::ModelF,
            kb_hash: HashMap::new(),
            keys_pressed: Vec::new(),
            typematic: true,
            typematic_delay: 500.0,
            typematic_rate: 100.0,
            kb_buffer_size: 1,
            kb_buffer: Vec::new(),
            kb_buffer_overflow: false,
            reset_buffer: Vec::new(),
            keycode_mappings: Vec::new(),
        }
    }
}

impl Keyboard {
    pub fn new(kb_type: KeyboardType, debug: bool) -> Self {
        let mut kb = Keyboard {
            debug,
            kb_type,
            ..Keyboard::default()
        };

        // Create a hash entry for each possible key
        for martykey in MartyKey::iter() {
            kb.kb_hash.insert(martykey, KeyState::default());
        }

        kb
    }

    pub fn set_debug(&mut self, state: bool) {
        self.debug = state;
    }

    /// Set typematic repeat parameters. Optional arguments allow only updating some parmeters.
    pub fn set_typematic_params(&mut self, enabled: Option<bool>, delay: Option<f64>, rate: Option<f64>) {
        if let Some(enabled) = enabled {
            self.typematic = enabled;
        }

        if let Some(delay) = delay {
            self.typematic_delay = delay;
        }

        if let Some(rate) = rate {
            self.typematic_rate = rate;
        }

        log::debug!(
            "Typematic paramters set: enabled: {}, delay: {:.2}, rate: {:.2}",
            self.typematic,
            self.typematic_delay,
            self.typematic_rate
        );
    }

    pub fn load_mapping(&mut self, map: &str) -> Result<()> {
        let toml_mapping: KeyboardMappingFile = toml::from_str(map)?;

        match self.kb_type {
            KeyboardType::ModelF => {
                self.keycode_mappings = toml_mapping.keyboard.modelf.keycode_mappings;
            }
            KeyboardType::Tandy1000 => {
                self.keycode_mappings = toml_mapping.keyboard.tandy1000.keycode_mappings;
            }
            KeyboardType::Pcjr => {
                self.keycode_mappings = toml_mapping.keyboard.pcjr.keycode_mappings;
            }
            _ => unimplemented!(),
        }

        Ok(())
    }

    pub fn get_type(&self) -> KeyboardType {
        self.kb_type
    }

    pub fn set_type(&mut self, kb_type: KeyboardType) {
        self.kb_type = kb_type;
        // Do any reinitialization here
    }

    /// Get the KeyState for the corresponding key.
    pub fn get_keycode_state(&self, key_code: MartyKey) -> Option<KeyState> {
        self.kb_hash.get(&key_code).cloned()
    }

    pub fn modifiers_from_strings(modifier_strings: &Vec<String>) -> KeyboardModifiers {
        let mut modifiers = KeyboardModifiers::default();

        for mstring in modifier_strings {
            if mstring.eq_ignore_ascii_case("control") {
                modifiers.control = true;
            }

            if mstring.eq_ignore_ascii_case("shift") {
                modifiers.shift = true;
            }

            if mstring.eq_ignore_ascii_case("alt") {
                modifiers.alt = true;
            }

            if mstring.eq_ignore_ascii_case("meta") {
                modifiers.meta = true;
            }
        }

        modifiers
    }

    /*
    #[derive(Copy, Clone, Debug)]
    pub struct KeybufferEntry {
        pub keycode: MartyKey,
        pub pressed: bool,
        pub modifiers: KeyboardModifiers
    } */

    pub fn keycodes_from_strings(keycode_strings: &Vec<String>, macro_translate: bool) -> Result<Vec<KeybufferEntry>> {
        let mut keycodes = Vec::new();

        for kstring in keycode_strings {
            // Process the first character, which should be a + for keydown, or - for keyup.

            if let Some((i, first_char)) = kstring.char_indices().nth(0) {
                let is_keydown = match first_char {
                    '+' => true,
                    '-' => false,
                    _ => bail!("Invalid keycode in macro defintion - missing keydown/keyup code."),
                };
                let rest = &kstring[i + first_char.len_utf8()..];

                let keycode_opt = MartyKey::from_str(rest);

                if let Ok(keycode) = keycode_opt {
                    keycodes.push(KeybufferEntry {
                        keycode,
                        pressed: is_keydown,
                        modifiers: KeyboardModifiers::default(),
                        translate: macro_translate,
                    });
                }
            }
            else {
                bail!("String too short in macro definition!");
            }
        }

        Ok(keycodes)
    }

    /// Set the corresponding key to pressed.
    pub fn key_down(
        &mut self,
        key_code: MartyKey,
        modifiers: &KeyboardModifiers,
        kb_buf: Option<&mut VecDeque<KeybufferEntry>>,
    ) {
        // Translation will produce either a Scancode or Keycode result
        let translation = self.translate_keydown(key_code, modifiers);

        match translation {
            TranslationType::Keycode(kvec) => {
                // Add keycodes to emulator's keyboard buffer for future delivery
                // and processing.
                if let Some(kb_buf) = kb_buf {
                    kb_buf.extend(kvec);
                }
            }
            TranslationType::Scancode(svec) => {
                if !svec.is_empty() {
                    if self.debug {
                        log::debug!(
                            "key_down(): Got scancode translation for key: {:?}: {:X?}",
                            key_code,
                            svec
                        );
                    }

                    if let Some(key) = self.kb_hash.get_mut(&key_code) {
                        let mut key_pressed = false;
                        for vkey in &self.keys_pressed {
                            if *vkey == key_code {
                                // Key is already pressed, ignore
                                key_pressed = true;
                            }
                        }

                        // Key not marked as pressed, add to pressed key vec.
                        if !key_pressed {
                            key.pressed = true;

                            key.translation = Some(svec.clone());
                            key.repeat_time = 0.0;
                            key.pressed_time = 0.0;

                            self.keys_pressed.push(key_code);
                            self.send_scancodes(&svec);
                        }
                    }
                }
                else {
                    log::warn!("key_down(): Got no scancode translation for key: {:?}", key_code);
                }
            }
        }
    }

    /// Set the corresponding key to unpressed.
    pub fn key_up(&mut self, key_code: MartyKey) {
        let mut convert_translation = None;
        //log::debug!("in key_up(): key_code: {:?}", key_code);

        if let Some(key) = self.kb_hash.get_mut(&key_code) {
            key.pressed = false;
            // If key was translated, get the corresponding key up codes
            if let Some(translation) = &mut key.translation {
                convert_translation = Some(translation.clone());

                if self.debug {
                    log::debug!("key_up(): got translation: {:X?}", convert_translation);
                }
            }
        }

        if let Some(mut to_convert) = convert_translation {
            self.translate_keyup(self.kb_type, &mut to_convert);
            self.send_scancodes(&to_convert);
        }

        // Remove this key from keys_pressed.
        self.keys_pressed.retain(|&k| k != key_code);
    }

    /// Reset key states for all keys to unpressed.
    /// # Arguments
    /// - `send_break` - Send key up scancodes for all keys currently pressed.
    pub fn clear(&mut self, send_break: bool) {
        let mut up_keys: Vec<MartyKey> = Vec::new();
        let mut translations: Vec<Vec<u8>> = Vec::new();

        for (key, state) in self.kb_hash.iter_mut() {
            if state.pressed {
                if send_break {
                    state.pressed = false;
                    // If key was translated, get the corresponding key up codes
                    if let Some(translation) = &mut state.translation {
                        translations.push(translation.clone());
                    }

                    // Remove this key from keys_pressed.
                    up_keys.push(*key);
                }
                *state = KeyState::default();
            }
        }

        for translation in translations.iter_mut() {
            self.translate_keyup(self.kb_type, translation);
            self.reset_buffer.extend(translation.clone());
        }

        for key in up_keys.iter() {
            self.keys_pressed.retain(|&k| k != *key);
        }

        self.kb_buffer.clear();
    }

    /// Send the corresponding scancodes to the keyboard buffer.
    pub fn send_scancodes(&mut self, keys: &[u8]) {
        if !keys.is_empty() {
            if self.kb_buffer_size > 1 {
                // We have a keyboard buffer
                if self.kb_buffer.len() + keys.len() >= self.kb_buffer_size {
                    // KB overflow!
                    self.kb_buffer_overflow = true;
                }
            }
            else if self.kb_buffer_size == 1 {
                // No keyboard buffer (kb_buffer_size == 1). Just set one scancode.
                self.kb_buffer.clear();
                self.kb_buffer.push(keys[0]);
            }
            else {
                panic!("invalid kb_buffer_size");
            }
        }
    }

    /// Read out a scancode from the keyboard or None if no key in buffer.
    pub fn recv_scancode(&mut self) -> Option<u8> {
        // Prioritize reset buffer over keyboard buffer.
        if !self.reset_buffer.is_empty() {
            return self.reset_buffer.pop();
        }

        if self.kb_buffer_overflow {
            // Send the keyboard overflow scancode
            self.kb_buffer_overflow = false;
            Some(0xFF)
        }
        else {
            self.kb_buffer.pop()
        }
    }

    pub fn translate_keydown(&self, key_code: MartyKey, modifiers: &KeyboardModifiers) -> TranslationType {
        let mut translation = TranslationType::Scancode(Vec::new());
        let mut got_translation = false;
        for trans in &self.keycode_mappings {
            // Match keycode by string using Debug for MartyKey.
            if trans.keycode == format!("{:?}", key_code) {
                let trans_modifiers = Keyboard::modifiers_from_strings(&trans.modifiers);
                log::debug!("trans_modifiers: {:?}", trans_modifiers);
                let mut matched = false;
                if trans.modifiers[0].eq_ignore_ascii_case("any") {
                    // Use this translation regardless of modifiers.
                    matched = true;
                }
                else if trans.modifiers[0].eq_ignore_ascii_case("none") {
                    // Use this translation if there are no modifiers
                    if !trans_modifiers.have_any() {
                        matched = true;
                    }
                }
                else {
                    log::debug!(
                        "We have multiple modifiers: {:?}, translation modifiers: {:?}",
                        modifiers,
                        trans_modifiers
                    );
                    if trans_modifiers == *modifiers {
                        // We have a list of modifiers. Use this translation if modifiers match.
                        matched = true;
                    }
                }

                // Load proper translation if we matched. If a macro definition is present,
                // it overrides scancode translation.
                if matched {
                    if !trans.key_macro.is_empty() {
                        // We have a macro.

                        if let Ok(keycodes) = Keyboard::keycodes_from_strings(&trans.key_macro, trans.macro_translate) {
                            translation = TranslationType::Keycode(keycodes);
                            got_translation = true;
                        }
                    }
                    else {
                        translation = TranslationType::Scancode(trans.scancodes.to_vec());
                        got_translation = true;
                    }
                }
            }
        }

        if !got_translation {
            // No defined translation for this key, just use default keyboard translation.
            translation = TranslationType::Scancode(self.kb_type.keycode_to_scancodes(key_code));
        }
        else if self.debug {
            match &translation {
                TranslationType::Scancode(sc) => {
                    log::debug!(
                        "translate_keydown(): got translation from key_code: {:?} to scancodes: {:X?}",
                        key_code,
                        sc
                    );
                }
                TranslationType::Keycode(kc) => {
                    log::debug!(
                        "translate_keydown(): got translation from key_code: {:?} to macro: {:X?}",
                        key_code,
                        kc
                    );
                }
            }
        }

        translation
    }

    /// Convert a translated scancode sequence to its corresponding keyup sequence.
    fn translate_keyup(&self, kb_type: KeyboardType, translation: &mut [u8]) {
        match kb_type {
            KeyboardType::ModelF | KeyboardType::Tandy1000 | KeyboardType::Pcjr => {
                // Translations should only have one keycode.
                assert_eq!(translation.len(), 1);

                if self.debug {
                    log::debug!(
                        "translate_keyup(): sending key_up: {:02X} for keydown translation: {:02X}",
                        translation[0] | 0x80,
                        translation[0]
                    );
                }

                translation[0] |= 0x80;
            }
            _ => {
                unimplemented!();
            }
        }
    }

    /// Run the keyboard device for the specified number of microseconds.
    pub fn run(&mut self, us: f64) {
        // Convert to milliseconds, all typematic delays are in ms.
        let ms: f64 = us / 1000.0;

        let mut repeating_keys = Vec::new();

        // Update keys pressed.
        for vkey in &self.keys_pressed {
            if self.typematic && self.is_typematic_key(*vkey) {
                if let Some(key_state) = self.kb_hash.get_mut(vkey) {
                    key_state.pressed_time += ms;
                    if key_state.pressed_time > (self.typematic_delay - self.typematic_rate) {
                        if self.debug {
                            log::debug!("typematic delay elapsed for: {:?}", vkey);
                        }

                        key_state.repeat_time += ms;
                        if key_state.repeat_time > self.typematic_rate {
                            key_state.repeat_time -= self.typematic_rate;
                            repeating_keys.push(key_state.clone());
                        }
                    }
                }
            }
        }

        // Sort all repeating keys by pressed_time
        repeating_keys.sort_by(|a, b| {
            a.pressed_time
                .partial_cmp(&b.pressed_time)
                .unwrap_or(std::cmp::Ordering::Equal)
                .reverse()
        });

        // Only repeat the oldest pressed key
        if let Some(key) = repeating_keys.pop() {
            if let Some(translation) = key.translation {
                self.send_scancodes(&translation);
            }
        }
    }

    /// Return whether key is a typematic key or not. Modifiers and lock keys are not typematic.
    pub fn is_typematic_key(&self, key_code: MartyKey) -> bool {
        match key_code {
            MartyKey::ControlLeft
            | MartyKey::ControlRight
            | MartyKey::ShiftLeft
            | MartyKey::ShiftRight
            | MartyKey::AltLeft
            | MartyKey::AltRight
            | MartyKey::NumLock
            | MartyKey::ScrollLock
            | MartyKey::CapsLock
            | MartyKey::Insert => false,
            _ => {
                // All other keys ok to repeat
                true
            }
        }
    }
}
