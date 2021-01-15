use cgmath::Vector2;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, DrawError, pool::standard::StandardCommandPoolBuilder,
};

use crate::scene::user_interface::ClickHandler;

use super::user_interface::{Button, Element as UIElement};

pub struct MainMenuScene {
    buttons: (Button<SettingButton>, Button<SettingButton>),
}

impl MainMenuScene {
    fn create() -> MainMenuScene {
        MainMenuScene {
            buttons: (
                Button {
                    pos: Vector2::new(0.0, 0.0),
                    size: (0.0, 0.0),
                    click_handler: SettingButton {},
                },
                Button {
                    pos: Vector2::new(0.0, 0.0),
                    size: (0.0, 0.0),
                    click_handler: SettingButton {},
                },
            ),
        }
    }
}

impl UIElement for MainMenuScene {
    fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        Ok(builder)
    }
    fn click(&self, pos: Vector2<f32>) -> bool {
        self.buttons.0.click(pos) || self.buttons.1.click(pos)
    }
}


struct SettingButton {}

impl ClickHandler for SettingButton {
    fn click(&self) {
        unimplemented!()
    }
}
