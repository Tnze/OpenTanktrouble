use std::cell::RefCell;

use crossbeam_channel::{Receiver, Sender, unbounded};
use gilrs::GamepadId;
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use winit::event::{KeyboardInput, VirtualKeyCode, WindowEvent};

use super::{
    Controller,
    gamepad_controller::Gamepad,
    keyboard_controller::{Key, Keyboard},
};

pub struct InputCenter {
    gilrs: RefCell<gilrs::Gilrs>,
    gamepad_ctrl: Gamepad,
    keyboard_ctrl: Keyboard,
    keyboard_receiver: Receiver<KeyboardInput>,
}

#[derive(Clone)]
pub struct InputEventSender {
    keyboard_sender: Sender<KeyboardInput>,
}

impl InputCenter {
    pub fn new() -> (Self, InputEventSender) {
        let gilrs = gilrs::Gilrs::new().unwrap();
        let (keyboard_sender, keyboard_receiver) = unbounded();
        (
            InputCenter {
                gilrs: RefCell::new(gilrs),
                gamepad_ctrl: Gamepad::new(),
                keyboard_ctrl: Keyboard::new(),
                keyboard_receiver,
            },
            InputEventSender { keyboard_sender },
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
        let gilrs = &mut *self.gilrs.borrow_mut();
        if let Ok(input) = self.keyboard_receiver.try_recv() {
            self.keyboard_ctrl.input_event(&input);
            Ok(Some(keyboard_event_handler(&input)))
        } else if let Some(event) = gilrs.next_event() {
            self.gamepad_ctrl.input_event(gilrs, &event);
            Ok(Some(gamepad_event_handler(gilrs, &event)))
        } else {
            Ok(None)
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
    pub fn create_gamepad_controller(&self, id: GamepadId) -> impl Controller {
        self.gamepad_ctrl.create_gamepad_controller(id)
    }
}

impl InputEventSender {
    pub fn window_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput { input, .. } = event {
            self.keyboard_sender.send(*input).unwrap_or(());
        }
    }
}
