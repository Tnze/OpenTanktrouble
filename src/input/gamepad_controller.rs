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
                let get_axis = |axis: &Axis| gamepad.axis_data(*axis).map_or(0.0, |x| x.value());
                let get_button = |button, a| gamepad.is_pressed(button) as i32 as f32 * a;
                let mix = |neg, pos| pos - neg;
                let x = [Axis::RightStickX, Axis::LeftStickX]
                    .iter()
                    .map(get_axis)
                    .map(|x| (x.min(0.0), x.max(0.0)))
                    .collect::<Vec<(f32, f32)>>();
                let y = [Axis::RightStickY, Axis::LeftStickY]
                    .iter()
                    .map(get_axis)
                    .map(|x| (x.min(0.0), x.max(0.0)))
                    .collect::<Vec<(f32, f32)>>();
                (
                    mix(
                        get_button(Button::DPadLeft, 1.0).min(x[0].0).min(x[1].0),
                        get_button(Button::DPadRight, 1.0).max(x[0].1).max(x[1].1),
                    ),
                    mix(
                        get_button(Button::DPadDown, 1.0).min(y[0].0).min(y[1].0),
                        get_button(Button::DPadUp, 1.0).max(y[0].1).max(y[1].1),
                    ),
                )
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

impl Controller {
    pub(crate) fn movement_status(&self) -> (f32, f32) {
        *self.status.lock().unwrap()
    }
}
