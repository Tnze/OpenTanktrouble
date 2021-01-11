use std::sync::{Arc, Mutex};

use crate::maze::Controller;

pub struct GamepadController<'a> {
    gamepad: Mutex<gilrs::Gamepad<'a>>
}

impl GamepadController<'_> {
    pub fn create_gamepad_controller(gamepad: gilrs::Gamepad) -> GamepadController {
        GamepadController {
            gamepad: Mutex::new(gamepad)
        }
    }
}

impl Controller for GamepadController<'_> {
    fn movement_status(&self) -> (f64, f64) {
        let gamepad = &*self.gamepad.lock().unwrap();
        (
            gamepad.axis_data(gilrs::Axis::LeftStickX).unwrap().value() as f64,
            gamepad.axis_data(gilrs::Axis::LeftStickY).unwrap().value() as f64,
        )
    }
}