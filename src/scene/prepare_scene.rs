use std::{error::Error, sync::Arc};

#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use wgpu::{Device, Queue, SwapChainError, SwapChainTexture};
use winit::event::VirtualKeyCode;

use crate::input::{Controller, input_center::InputHandler};

use super::{game_scene, SceneRender, SceneUpdater};

enum ControllerStatus {
    Prepared,
    Unknown,
    Unprepared,
}

struct Player {
    controller: Box<dyn Controller>,
    status: ControllerStatus,
}

pub struct PrepareSceneRender {}

pub struct PrepareSceneUpdater {}

pub fn new(
    device: Arc<wgpu::Device>,
    format: wgpu::TextureFormat,
) -> (PrepareSceneRender, PrepareSceneUpdater) {
    (PrepareSceneRender {}, PrepareSceneUpdater {})
}

impl PrepareSceneUpdater {
    fn manage(&self, input_handler: &InputHandler) -> Result<Vec<Player>, Box<dyn Error>> {
        let mut finish = false;
        let mut players = vec![];

        let on_keyboard_input = |event| -> Result<bool, Box<dyn Error>> {
            let winit::event::KeyboardInput {
                state,
                scancode,
                virtual_keycode,
                ..
            } = event;
            if let Some(VirtualKeyCode::Q) = virtual_keycode {
                // players.push(Player{
                //     controller: Box::new(input_handler.),
                //     status: ControllerStatus::Prepared
                // });
                return Ok(true);
            }

            debug!("{:?}", state);
            Ok(false)
        };
        let on_gamepad_input = |event: gilrs::Event| -> Result<bool, Box<dyn Error>> { Ok(false) };
        while !finish {
            finish = crossbeam_channel::select! {
                recv(input_handler.keyboard_event) -> res => on_keyboard_input(res?)?,
                recv(input_handler.gamepad_event) -> res => on_gamepad_input(res?)?,
            }
        }
        Ok(players)
    }
}

impl SceneRender for PrepareSceneRender {
    fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        frame: &SwapChainTexture,
        frame_size: [u32; 2],
    ) -> Result<(), SwapChainError> {
        Ok(())
    }
}

impl SceneUpdater for PrepareSceneUpdater {
    fn update(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        input_handler: &InputHandler,
    ) -> Option<(Box<dyn SceneRender + Sync + Send>, Box<dyn SceneUpdater>)> {
        let players = self.manage(input_handler).unwrap();
        let (render, updater) = game_scene::new(device, format);
        for p in players {
            updater.add_player(p.controller);
        }
        Some((Box::new(render), Box::new(updater)))
    }
}
