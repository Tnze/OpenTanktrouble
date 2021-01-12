use std::sync::{Arc, Mutex};

use gilrs::{GamepadId, Gilrs};

pub struct GamepadController {
    gamepad: gilrs::GamepadId,
}

impl GamepadController {
    pub fn create_gamepad_controller(gamepad: GamepadId) -> GamepadController {
        GamepadController {
            gamepad,
        }
    }
}

impl GamepadController {
    pub(crate) fn movement_status(&self, gilrs: &Gilrs) -> (f32, f32) {
        let gamepad = gilrs.gamepad(self.gamepad);
        (
            gamepad.axis_data(gilrs::Axis::LeftStickY).map_or(0.0, |x| x.value()),
            gamepad.axis_data(gilrs::Axis::LeftStickX).map_or(0.0, |x| x.value()),
        )
    }
}