use std::sync::Arc;

use cgmath::Vector2;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, DrawError, pool::standard::StandardCommandPoolBuilder,
};
use vulkano::framebuffer::RenderPassAbstract;

use crate::scene::user_interface::ClickHandler;

use super::user_interface::{Button, Element as UIElement, Scene as UIScene};

pub struct MainMenuScene {
    buttons: (Button<StartButton>, Button<StartButton>),
}

impl MainMenuScene {
    pub fn new() -> MainMenuScene {
        MainMenuScene {
            buttons: (
                Button {
                    pos: Vector2::new(0.0, 0.0),
                    size: (1.0, 1.0),
                    click_handler: StartButton {},
                },
                Button {
                    pos: Vector2::new(0.0, 0.0),
                    size: (0.0, 0.0),
                    click_handler: StartButton {},
                },
            ),
        }
    }
}

impl UIScene for MainMenuScene {
    fn render_pass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        unimplemented!()
    }

    fn reset_viewport(&self, dimension: [f32; 2]) {
        unimplemented!()
    }
}

impl UIElement for MainMenuScene {
    fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
        dimensions: [f32; 2],
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        Ok(builder)
    }
    fn click(&self, pos: Vector2<f32>) -> bool {
        self.buttons.0.click(pos) || self.buttons.1.click(pos)
    }
}

struct StartButton {}

impl ClickHandler for StartButton {
    fn click(&self) {
        unimplemented!()
    }
}
