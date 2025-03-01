use marty_core::devices::keyboard::KeyboardModifiers;

pub struct KeyboardData {
    pub modifiers:    KeyboardModifiers,
    pub ctrl_pressed: bool,
}
impl KeyboardData {
    pub fn new() -> Self {
        Self {
            modifiers:    KeyboardModifiers::default(),
            ctrl_pressed: false,
        }
    }
}
