use std::error::Error;
use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver, Select, Sender, tick, unbounded};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use wgpu::{Device, Queue, SwapChainError, SwapChainTexture};

use crate::input::{Controller, input_center::InputHandler};

use super::{game_scene::GameScene, SceneRender, SceneUpdater};

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

pub struct PrepareSceneRender {
    player_list: Vec<Player>,
}

pub struct PrepareSceneUpdater {}

impl PrepareScene {
    pub fn new(
        device: Arc<wgpu::Device>,
        format: wgpu::TextureFormat,
    ) -> (PrepareSceneRender, PrepareSceneUpdater) {
        (
            PrepareSceneRender {
                player_list: vec![],
            },
            PrepareSceneUpdater {},
        )
    }
}

impl PrepareSceneUpdater {
    fn manage(
        input_handler: &InputHandler,
        // stop_signal: Receiver<()>,
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
                // recv(stop_signal) -> _ => return Ok(()),
            }
        }
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
    ) -> (Box<dyn SceneRender + Sync + Send>, Box<dyn SceneUpdater>) {
        // Self::manage(input_handler).unwrap();
        let (render, updater) = GameScene::new(device, format);
        (Box::new(render), Box::new(updater))
    }
}
