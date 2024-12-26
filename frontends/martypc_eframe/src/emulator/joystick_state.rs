use frontend_common::types::joykeys::{JoyKeyEntry, JoyKeyInput};
use marty_core::keys::MartyKey;
use std::collections::HashMap;

/// This structure is only used to maintain the state for keyboard joystick emulation.
/// Actual joysticks will be read directly via a controller input library.
#[allow(dead_code)]
#[derive(Default)]
pub struct JoystickData {
    pub enabled:   bool,
    pub key_state: HashMap<MartyKey, (JoyKeyInput, bool)>,
    pub joy_state: HashMap<JoyKeyInput, bool>,
}
impl JoystickData {
    pub fn new(keys: Vec<JoyKeyEntry>, enabled: bool) -> Self {
        let mut jd = JoystickData::default();

        for key in keys {
            jd.key_state.insert(key.key, (key.input, false));
            jd.joy_state.insert(key.input, false);
        }
        jd.enabled = enabled;
        jd
    }

    fn get_xy(&self) -> (f64, f64) {
        let x = if *self.joy_state.get(&JoyKeyInput::JoyLeft).unwrap() {
            -1.0
        }
        else if *self.joy_state.get(&JoyKeyInput::JoyRight).unwrap() {
            1.0
        }
        else {
            0.0
        };
        let y = if *self.joy_state.get(&JoyKeyInput::JoyUp).unwrap() {
            -1.0
        }
        else if *self.joy_state.get(&JoyKeyInput::JoyDown).unwrap() {
            1.0
        }
        else {
            0.0
        };

        //log::debug!("Joystick XY: ({}, {})", x, y);
        (x, y)
    }
}
