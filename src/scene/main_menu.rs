use cgmath::Vector2;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, DrawError, pool::standard::StandardCommandPoolBuilder,
};

use crate::scene::user_interface::ClickHandler;

use super::user_interface::{Button, Element as UIElement};

pub struct MainMenuScene {
    buttons: (Button<StartButton>, Button<StartButton>),
}

impl MainMenuScene {
    fn create() -> MainMenuScene {
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
