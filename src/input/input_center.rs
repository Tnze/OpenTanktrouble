use crossbeam_channel::{bounded, Receiver, Select, Sender, tick, unbounded};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

use super::gamepad_controller::Gamepad;
use super::keyboard_controller::Keyboard;

pub struct InputCenter {
    pub keyboard_controller: Keyboard,
    pub gamepad_controller: Gamepad,

    fire_sender: Sender<()>,
    input_handler: InputHandler,
}

#[derive(Clone)]
pub struct InputHandler {
    pub fire_receiver: Receiver<()>,
}

impl InputCenter {
    pub fn new() -> Self {
        let (fire_sender, fire_receiver) = unbounded();
        InputCenter {
            keyboard_controller: Keyboard::new(),
            gamepad_controller: Gamepad::new(),

            fire_sender,
            input_handler: InputHandler { fire_receiver },
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                input:
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Q),
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => self.fire_sender.send(()).unwrap(),
            _ => {}
        }
        if let WindowEvent::KeyboardInput { input, .. } = event {
            self.keyboard_controller.input_event(input)
        }
    }

    pub fn gamepad_event(&mut self, gilrs: &mut gilrs::Gilrs, event: &gilrs::Event) {
        self.gamepad_controller.input_event(gilrs, event);
    }

    pub fn input_handler(&self) -> InputHandler {
        self.input_handler.clone()
    }
}
