use crossbeam_channel::{bounded, Receiver, Select, Sender, tick, unbounded};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

use super::gamepad_controller::Gamepad;
use super::keyboard_controller::Keyboard;

pub struct InputCenter {
    keyboard_sender: Sender<KeyboardInput>,
    gamepad_sender: Sender<gilrs::Event>,
    input_handler: InputHandler,
}

#[derive(Clone)]
pub struct InputHandler {
    pub keyboard_event: Receiver<KeyboardInput>,
    pub gamepad_event: Receiver<gilrs::Event>,
}

impl InputCenter {
    pub fn new() -> Self {
        let (keyboard_sender, keyboard_receiver) = unbounded();
        let (gamepad_sender, gamepad_receiver) = unbounded();
        InputCenter {
            keyboard_sender,
            gamepad_sender,
            input_handler: InputHandler {
                keyboard_event: keyboard_receiver,
                gamepad_event: gamepad_receiver,
            },
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                self.keyboard_sender.send(*input).unwrap_or(());
            }
            _ => {}
        }
    }

    pub fn gamepad_event(&mut self, gilrs: &mut gilrs::Gilrs, event: &gilrs::Event) {
        self.gamepad_sender.send(*event).unwrap_or(());
    }

    pub fn input_handler(&self) -> InputHandler {
        self.input_handler.clone()
    }
}
