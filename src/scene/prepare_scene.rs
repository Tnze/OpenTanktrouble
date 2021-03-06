use std::{error::Error, sync::Arc};

#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use wgpu::{Device, Queue, SwapChainError, SwapChainTexture};
use winit::event::{ElementState, VirtualKeyCode};

use crate::input::{Controller, input_center::InputCenter};

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
    fn manage(&self, input_center: &InputCenter) -> Result<Vec<Player>, Box<dyn Error>> {
        let mut finish = false;
        let mut players = vec![];

        while !finish {
            finish = input_center
                .update(
                    |event| -> Result<bool, Box<dyn Error>> {
                        let &winit::event::KeyboardInput {
                            state,
                            scancode,
                            virtual_keycode,
                            ..
                        } = event;
                        if let ElementState::Pressed = state {
                            match virtual_keycode {
                                Some(VirtualKeyCode::Q) => players.push(Player {
                                    controller: Box::new(input_center.create_controller_red()),
                                    status: ControllerStatus::Prepared,
                                }),
                                Some(VirtualKeyCode::M) => players.push(Player {
                                    controller: Box::new(input_center.create_controller_green()),
                                    status: ControllerStatus::Prepared,
                                }),
                                _ => {}
                            }
                            Ok(players.len() >= 2)
                        } else {
                            Ok(false)
                        }
                    },
                    |gilrs, event| -> Result<bool, Box<dyn Error>> { Ok(false) },
                )?
                .unwrap_or(Ok(false))?;
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
        input_center: &InputCenter,
    ) -> Option<(Box<dyn SceneRender + Sync + Send>, Box<dyn SceneUpdater>)> {
        let players = self.manage(input_center).unwrap();
        let (render, updater) = game_scene::new(device, format);
        for p in players {
            updater.add_player(p.controller);
        }
        Some((Box::new(render), Box::new(updater)))
    }
}
