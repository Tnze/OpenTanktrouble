use std::{error::Error, sync::Arc};

use gilrs::{Event, EventType};
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
    _device: Arc<wgpu::Device>,
    _format: wgpu::TextureFormat,
) -> (PrepareSceneRender, PrepareSceneUpdater) {
    (PrepareSceneRender {}, PrepareSceneUpdater {})
}

impl PrepareSceneUpdater {
    fn manage(&self, input_center: &InputCenter) -> Result<Vec<Player>, Box<dyn Error>> {
        use std::cell::RefCell;
        let players = RefCell::new(vec![]);
        while players.borrow().len() < 2 {
            input_center
                .update(
                    |event| {
                        let players = &mut *players.borrow_mut();
                        if let winit::event::KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode,
                            ..
                        } = *event
                        {
                            match virtual_keycode {
                                Some(VirtualKeyCode::Q) => {
                                    debug!("New player: {}", "Q");
                                    players.push(Player {
                                        controller: Box::new(input_center.create_controller_red()),
                                        status: ControllerStatus::Prepared,
                                    })
                                }
                                Some(VirtualKeyCode::M) => {
                                    debug!("New player: {}", "M");
                                    players.push(Player {
                                        controller: Box::new(input_center.create_controller_green()),
                                        status: ControllerStatus::Prepared,
                                    })
                                }
                                _ => {}
                            }
                        }
                    },
                    |gilrs, event| {
                        let players = &mut *players.borrow_mut();
                        if let Event {
                            id,
                            event: EventType::ButtonPressed(gilrs::Button::South, ..),
                            ..
                        } = *event
                        {
                            debug!("New player: {}", gilrs.gamepad(id).name());
                            players.push(Player {
                                controller: Box::new(input_center.create_gamepad_controller(id)),
                                status: ControllerStatus::Prepared,
                            })
                        }
                    },
                )?
                .unwrap_or(());
        }
        Ok(players.take())
    }
}

impl SceneRender for PrepareSceneRender {
    fn render(
        &mut self,
        _device: &Device,
        _queue: &Queue,
        _frame: &SwapChainTexture,
        _frame_size: [u32; 2],
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
