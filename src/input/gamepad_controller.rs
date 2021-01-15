use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gilrs::{Axis, Button, Event, EventType, GamepadId, Gilrs};

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
    fn input_event(&mut self, Event { id, event, .. }: Event) {
        if let Some(ctrl) = self.controllers.get(&id) {
            let val = {
                let gamepad = self.gilrs.gamepad(id);
                let get_axis = |axis| gamepad.axis_data(axis).map_or(0.0, |x| x.value());
                let get_button = |(neg, a), (pos, b)| {
                    0.0 - gamepad.is_pressed(neg) as i32 as f32 * a
                        + gamepad.is_pressed(pos) as i32 as f32 * b
                };
                (
                    get_button((Button::DPadLeft, 1.0), (Button::DPadRight, 1.0)),
                    // .max(get_axis(Axis::RightStickX))
                    // .max(get_axis(Axis::LeftStickX)),
                    get_button((Button::DPadDown, 0.6), (Button::DPadUp, 1.0))
                    // .max(get_axis(Axis::RightStickX))
                    // .max(get_axis(Axis::RightStickY)),
                )
            };
            *ctrl.lock().unwrap() = val;
        }
    }
}

pub struct Controller {
    status: Arc<Mutex<(f32, f32)>>,
    gamepad: gilrs::GamepadId,
}

impl Controller {
    pub fn create_gamepad_controller(parent: &mut Gamepad, gamepad: GamepadId) -> Controller {
        let status = Arc::new(Mutex::new((0.0, 0.0)));
        parent.controllers.insert(gamepad, status.clone());
        Controller { status, gamepad }
    }
}

impl Controller {
    pub(crate) fn movement_status(&self) -> (f32, f32) {
        *self.status.lock().unwrap()
    }
}
