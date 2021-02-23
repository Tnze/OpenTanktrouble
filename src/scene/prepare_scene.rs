use std::error::Error;

use crossbeam_channel::{bounded, Receiver, Select, Sender, tick, unbounded};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use wgpu::{Device, Queue, SwapChainError, SwapChainTexture};

use crate::input::{Controller, input_center::InputHandler};

use super::Scene;

enum ControllerStatus {
    Prepared,
    Unknown,
    Unprepared,
}

struct Player {
    controller: Box<dyn Controller>,
    status: ControllerStatus,
}

pub struct PrepareScene {
    player_list: Vec<Player>,
}

impl PrepareScene {
    fn manage(
        input_handler: InputHandler,
        stop_signal: Receiver<()>,
    ) -> Result<(), Box<dyn Error>> {
        let on_keyboard_input = |event| -> Result<(), Box<dyn Error>> {
            let winit::event::KeyboardInput { state, .. } = event;
            debug!("{:?}", state);
            Ok(())
        };
        let on_gamepad_input = |event: gilrs::Event| -> Result<(), Box<dyn Error>> { Ok(()) };
        loop {
            crossbeam_channel::select! {
                recv(input_handler.keyboard_event) -> res => on_keyboard_input(res?)?,
                recv(input_handler.gamepad_event) -> res => on_gamepad_input(res?)?,
                recv(stop_signal) -> _ => return Ok(()),
            }
        }
    }
}

impl Scene for PrepareScene {
    fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        frame: &SwapChainTexture,
        frame_size: [u32; 2],
    ) -> Result<(), SwapChainError> {
        unimplemented!()
    }

    fn add_controller(&self, ctrl: Box<dyn Controller>) {
        unimplemented!()
    }
}
