use gilrs::{Axis, Button, GamepadId, Gilrs};

pub struct Controller {
    gamepad: gilrs::GamepadId,
}

impl Controller {
    pub fn create_gamepad_controller(gamepad: GamepadId) -> Controller {
        Controller { gamepad }
    }
}

impl Controller {
    pub(crate) fn movement_status(&self, gilrs: &Gilrs) -> (f32, f32) {
        let gamepad = gilrs.gamepad(self.gamepad);
        let get_axis = |axis| gamepad.axis_data(axis).map_or(0.0, |x| x.value());
        let get_button = |(neg, a), (pos, b)| {
            0.0 - gamepad.is_pressed(neg) as i32 as f32 * a
                + gamepad.is_pressed(pos) as i32 as f32 * b
        };

        (
            get_button((Button::DPadLeft, 1.0), (Button::DPadRight, 1.0))
                .max(get_axis(Axis::RightStickX))
                .max(get_axis(Axis::LeftStickX)),
            get_button((Button::DPadDown, 0.6), (Button::DPadUp, 1.0))
                .max(get_axis(Axis::RightStickX))
                .max(get_axis(Axis::RightStickY)),
        )
    }
}
