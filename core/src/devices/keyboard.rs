/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

use anyhow::{bail, Result};
use std::{
    collections::{HashMap, VecDeque},
    fs::read_to_string,
    path::Path,
    str::FromStr,
    vec::Vec,
};
use strum::IntoEnumIterator;

use serde_derive::Deserialize;
use toml;

use crate::{keys::MartyKey, machine::KeybufferEntry};

// Define the various types of keyboard we can emulate.
#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
pub enum KeyboardType {
    ModelF,
    ModelM,
}

impl FromStr for KeyboardType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s {
            "ModelF" => Ok(KeyboardType::ModelF),
            "ModelM" => Ok(KeyboardType::ModelM),
            _ => Err("Bad value for keyboard_type".to_string()),
        }
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct KeyboardModifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl Default for KeyboardModifiers {
    fn default() -> KeyboardModifiers {
        KeyboardModifiers {
            control: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }
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
    modelf: Modelf,
}

#[derive(Debug, Deserialize)]
pub struct Modelf {
    keycode_mappings: Vec<KeycodeMapping>,
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
    }

    pub fn load_mapping(&mut self, map_file: &Path) -> Result<()> {
        let toml_mapping_str = read_to_string(map_file)?;
        let toml_mapping: KeyboardMappingFile = toml::from_str(&toml_mapping_str)?;

        match self.kb_type {
            KeyboardType::ModelF => {
                self.keycode_mappings = toml_mapping.keyboard.modelf.keycode_mappings;
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

    /// Convert a MartyKey key code into a physical scancode based on the configured
    /// keyboard model.
    pub fn keycode_to_scancodes(&self, key_code: MartyKey) -> Vec<u8> {
        let mut scancodes = Vec::new();

        match self.kb_type {
            KeyboardType::ModelF => {
                // The model F was the original keyboard shipped with the IBM PC.
                // It had two variants, an 83-key version without lock status lights
                // and an 84-key version with an added 'sysreq' key.

                let scancode = match key_code {
                    // From Left to Right on IBM XT keyboard
                    MartyKey::F1 => Some(0x3b),
                    MartyKey::F2 => Some(0x3c),
                    MartyKey::F3 => Some(0x3d),
                    MartyKey::F4 => Some(0x3e),
                    MartyKey::F5 => Some(0x3f),
                    MartyKey::F6 => Some(0x40),
                    MartyKey::F7 => Some(0x41),
                    MartyKey::F8 => Some(0x42),
                    MartyKey::F9 => Some(0x43),
                    MartyKey::F10 => Some(0x44),
                    MartyKey::Escape => Some(0x01),
                    MartyKey::Tab => Some(0x0F),
                    MartyKey::ControlLeft => Some(0x1D),
                    MartyKey::ShiftLeft => Some(0x2A),
                    MartyKey::AltLeft => Some(0x38),
                    MartyKey::ControlRight => Some(0x1D),
                    MartyKey::AltRight => Some(0x38),
                    MartyKey::Digit1 => Some(0x02),
                    MartyKey::Digit2 => Some(0x03),
                    MartyKey::Digit3 => Some(0x04),
                    MartyKey::Digit4 => Some(0x05),
                    MartyKey::Digit5 => Some(0x06),
                    MartyKey::Digit6 => Some(0x07),
                    MartyKey::Digit7 => Some(0x08),
                    MartyKey::Digit8 => Some(0x09),
                    MartyKey::Digit9 => Some(0x0A),
                    MartyKey::Digit0 => Some(0x0B),
                    MartyKey::Minus => Some(0x0C),
                    MartyKey::Equal => Some(0x0D),
                    MartyKey::KeyA => Some(0x1E),
                    MartyKey::KeyB => Some(0x30),
                    MartyKey::KeyC => Some(0x2E),
                    MartyKey::KeyD => Some(0x20),
                    MartyKey::KeyE => Some(0x12),
                    MartyKey::KeyF => Some(0x21),
                    MartyKey::KeyG => Some(0x22),
                    MartyKey::KeyH => Some(0x23),
                    MartyKey::KeyI => Some(0x17),
                    MartyKey::KeyJ => Some(0x24),
                    MartyKey::KeyK => Some(0x25),
                    MartyKey::KeyL => Some(0x26),
                    MartyKey::KeyM => Some(0x32),
                    MartyKey::KeyN => Some(0x31),
                    MartyKey::KeyO => Some(0x18),
                    MartyKey::KeyP => Some(0x19),
                    MartyKey::KeyQ => Some(0x10),
                    MartyKey::KeyR => Some(0x13),
                    MartyKey::KeyS => Some(0x1F),
                    MartyKey::KeyT => Some(0x14),
                    MartyKey::KeyU => Some(0x16),
                    MartyKey::KeyV => Some(0x2F),
                    MartyKey::KeyW => Some(0x11),
                    MartyKey::KeyX => Some(0x2D),
                    MartyKey::KeyY => Some(0x15),
                    MartyKey::KeyZ => Some(0x2C),
                    MartyKey::Backslash => Some(0x2B),
                    MartyKey::Space => Some(0x39),
                    MartyKey::Backspace => Some(0x0E),
                    MartyKey::BracketLeft => Some(0x1A),
                    MartyKey::BracketRight => Some(0x1B),
                    MartyKey::Semicolon => Some(0x27),
                    MartyKey::Backquote => Some(0x29), // Grave
                    MartyKey::Quote => Some(0x28),     // Apostrophe
                    MartyKey::Comma => Some(0x33),
                    MartyKey::Period => Some(0x34),
                    MartyKey::Slash => Some(0x35),
                    MartyKey::Enter => Some(0x1C), // Return
                    MartyKey::ShiftRight => Some(0x36),
                    MartyKey::CapsLock => Some(0x3A),    // 'Capital'?
                    MartyKey::PrintScreen => Some(0x37), // 'Snapshot'ù
                    MartyKey::Delete => Some(0x53),
                    MartyKey::NumLock => Some(0x45),
                    MartyKey::ScrollLock => Some(0x46),
                    MartyKey::Numpad0 | MartyKey::Insert => Some(0x52),
                    MartyKey::Numpad1 | MartyKey::End => Some(0x4F),
                    MartyKey::Numpad2 | MartyKey::ArrowDown => Some(0x50),
                    MartyKey::Numpad3 | MartyKey::PageDown => Some(0x51),
                    MartyKey::Numpad4 | MartyKey::ArrowLeft => Some(0x4B),
                    MartyKey::Numpad5 => Some(0x4C),
                    MartyKey::Numpad6 | MartyKey::ArrowRight => Some(0x4D),
                    MartyKey::Numpad7 | MartyKey::Home => Some(0x47),
                    MartyKey::Numpad8 | MartyKey::ArrowUp => Some(0x48),
                    MartyKey::Numpad9 | MartyKey::PageUp => Some(0x49),
                    MartyKey::NumpadSubtract => Some(0x4A),
                    MartyKey::NumpadAdd => Some(0x4E),
                    MartyKey::NumpadDecimal => Some(0x53),
                    MartyKey::NumpadEnter => Some(0x1C),
                    MartyKey::NumpadDivide => None,      // Can't directly map to shift-7
                    MartyKey::NumpadMultiply => None,    // Can't directly map to shift-8
                    MartyKey::NumpadEqual => Some(0x0D), // Present on Mac
                    _ => None,
                };

                if let Some(s) = scancode {
                    //log::debug!("Converted key: {:?} to scancode: {:02X}", key_code, s);
                    scancodes.push(s);
                }
            }
            _ => {
                unimplemented!();
            }
        }

        scancodes
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
                if svec.len() > 0 {
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
    pub fn clear(&mut self) {
        for key in self.kb_hash.keys().cloned().collect::<Vec<MartyKey>>() {
            self.kb_hash.insert(key, KeyState::default());
        }
    }

    /// Send the corresponding scancodes to the keyboard buffer.
    pub fn send_scancodes(&mut self, keys: &[u8]) {
        if keys.len() > 0 {
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

                let mut matched = false;
                if trans.modifiers[0].eq_ignore_ascii_case("any") {
                    // Use this translation regardless of modifiers.
                    matched = true;
                }
                else if trans.modifiers[0].eq_ignore_ascii_case("none") {
                    // Use this translation if there are no modifiers
                    if trans_modifiers.have_any() {
                        matched = true;
                    }
                }
                else if trans_modifiers == *modifiers {
                    // We have a list of modifiers. Use this translation if modifiers match.
                    matched = true;
                }

                // Load proper translation if we matched. If a macro definition is present,
                // it overrides scancode translation.
                if matched {
                    if trans.key_macro.len() > 0 {
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
            translation = TranslationType::Scancode(self.keycode_to_scancodes(key_code));
        }
        else {
            if self.debug {
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
        }

        translation
    }

    /// Convert a translated scancode sequence to its corresponding keyup sequence.
    fn translate_keyup(&self, kb_type: KeyboardType, translation: &mut [u8]) {
        match kb_type {
            KeyboardType::ModelF => {
                // ModelF has no keyboard buffer, therefore, translations should only have one keycode.
                assert_eq!(translation.len(), 1);

                if self.debug {
                    log::debug!(
                        "translate_keyup(): sending key_up: {:02X} for keydown translation: {:02X}",
                        translation[0] | 0x80,
                        translation[0]
                    );
                }

                translation[0] = translation[0] | 0x80;
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
            if self.is_typematic_key(*vkey) {
                if let Some(key_state) = self.kb_hash.get_mut(&vkey) {
                    key_state.pressed_time += ms;

                    // TODO: implement key repeat here
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
