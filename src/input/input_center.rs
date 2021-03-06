use crossbeam_channel::{bounded, Receiver, Select, Sender, tick, unbounded};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

use super::{
    Controller,
    gamepad_controller::Gamepad,
    keyboard_controller::{Key, Keyboard},
};

pub struct InputCenter {
    gilrs: gilrs::Gilrs,
    gamepad_ctrl: Gamepad,
    keyboard_ctrl: Keyboard,
    keyboard_receiver: Receiver<KeyboardInput>,
    gamepad_receiver: Receiver<gilrs::Event>,
}

#[derive(Clone)]
pub struct InputEventSender {
    keyboard_sender: Sender<KeyboardInput>,
    gamepad_sender: Sender<gilrs::Event>,
}

impl InputCenter {
    pub fn new() -> (Self, InputEventSender) {
        let gilrs = gilrs::Gilrs::new().unwrap();
        let (keyboard_sender, keyboard_receiver) = unbounded();
        let (gamepad_sender, gamepad_receiver) = unbounded();
        (
            InputCenter {
                gilrs,
                gamepad_ctrl: Gamepad::new(),
                keyboard_ctrl: Keyboard::new(),
                keyboard_receiver,
                gamepad_receiver,
            },
            InputEventSender {
                keyboard_sender,
                gamepad_sender,
            },
        )
    }

    pub fn update<KH, GH, R>(
        &self,
        keyboard_event_handler: KH,
        gamepad_event_handler: GH,
    ) -> Result<Option<R>, crossbeam_channel::RecvError>
        where
            KH: FnOnce(&KeyboardInput) -> R,
            GH: FnOnce(&gilrs::Gilrs, &gilrs::Event) -> R,
    {
        crossbeam_channel::select! {
            recv(self.keyboard_receiver) -> input => {
                let input = input?;
                self.keyboard_ctrl.input_event(&input);
                Ok(Some(keyboard_event_handler(&input)))
            },
            recv(self.gamepad_receiver) -> event => {
                let event = event?;
                self.gamepad_ctrl.input_event(&self.gilrs, &event);
                Ok(Some(gamepad_event_handler(&self.gilrs, &event)))
            },
            default => Ok(None),
        }
    }

    pub fn create_controller_red(&self) -> impl Controller {
        self.keyboard_ctrl.create_sub_controller([
            Key::LogicKey(VirtualKeyCode::E),
            Key::LogicKey(VirtualKeyCode::D),
            Key::LogicKey(VirtualKeyCode::S),
            Key::LogicKey(VirtualKeyCode::F),
        ])
    }
    pub fn create_controller_green(&self) -> impl Controller {
        self.keyboard_ctrl.create_sub_controller([
            Key::LogicKey(VirtualKeyCode::Up),
            Key::LogicKey(VirtualKeyCode::Down),
            Key::LogicKey(VirtualKeyCode::Left),
            Key::LogicKey(VirtualKeyCode::Right),
        ])
    }
}

impl InputEventSender {
    pub fn gamepad_event(&mut self, gilrs: &mut gilrs::Gilrs, event: &gilrs::Event) {
        self.gamepad_sender.send(*event).unwrap_or(());
    }

    pub fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                self.keyboard_sender.send(*input).unwrap_or(());
            }
            _ => {}
        }
    }
}
