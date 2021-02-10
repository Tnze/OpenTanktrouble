use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use gilrs::{Axis, Button, Event, GamepadId, Gilrs};

pub struct Gamepad {
    gilrs: Gilrs,
    controllers: HashMap<GamepadId, Arc<Mutex<(f32, f32)>>>,
}

impl Gamepad {
    pub fn new() -> Gamepad {
        Gamepad {
            gilrs: Gilrs::new().unwrap(),
            controllers: HashMap::new(),
        }
    }
    pub fn next(&mut self) -> Option<Event> {
        let event = self.gilrs.next_event();
        if let Some(e) = event {
            self.input_event(e);
            self.gilrs.inc();
        }
        event
    }
    fn input_event(&mut self, Event { id, .. }: Event) {
        if let Some(ctrl) = self.controllers.get(&id) {
            *ctrl.lock().unwrap() = {
                let gamepad = self.gilrs.gamepad(id);
                let get_axis = |axis: Axis| gamepad.axis_data(axis).map_or(0.0, |x| x.value());
                let get_button = |pos, neg| {
                    (gamepad.is_pressed(pos) as i32 - gamepad.is_pressed(neg) as i32) as f32
                };
                let gamepad_status = [
                    [
                        get_axis(Axis::RightStickX),
                        get_axis(Axis::LeftStickX),
                        get_button(Button::DPadRight, Button::DPadLeft),
                    ], // (rot) left and right
                    [
                        get_axis(Axis::RightStickY),
                        get_axis(Axis::LeftStickY),
                        get_button(Button::DPadUp, Button::DPadDown),
                    ], // (acl) up and down
                ];
                let mut control = gamepad_status.iter().map(|x| {
                    let (max_x, min_x) = x
                        .iter()
                        .map(|v| (v.max(0.0), v.min(0.0))) // split values into two part
                        .fold((0f32, 0f32), |acc, x| (acc.0.max(x.0), acc.1.min(x.1))); // get the max and the min
                    max_x + min_x
                });
                let rot = control.next().unwrap();
                let acl = control.next().unwrap();
                (rot, acl.max(-0.6))
            };
        }
    }
}

pub struct Controller {
    status: Arc<Mutex<(f32, f32)>>,
}

impl Controller {
    pub fn create_gamepad_controller(parent: &mut Gamepad, gamepad: GamepadId) -> Controller {
        let status = Arc::new(Mutex::new((0.0, 0.0)));
        parent.controllers.insert(gamepad, status.clone());
        Controller { status }
    }
}

impl super::Controller for Controller {
    fn movement_status(&self) -> (f32, f32) {
        *self.status.lock().unwrap()
    }
}
