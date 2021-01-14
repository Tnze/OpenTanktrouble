use cgmath::Vector2;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, DrawError, pool::standard::StandardCommandPoolBuilder,
};

use super::user_interface::Element as UIElement;

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

trait ClickHandler {
    fn click(&self);
}

struct Button<C: ClickHandler> {
    pos: Vector2<f32>,
    size: (f32, f32),
    // (width, height)
    click_handler: C,
}

impl<C: ClickHandler> Button<C> {
    fn click(&self, pos: Vector2<f32>) -> bool {
        if self.is_hit(pos) {
            self.click_handler.click();
            return true;
        }
        false
    }
    fn is_hit(&self, pos: Vector2<f32>) -> bool {
        pos.x < self.pos.x + self.size.0 / 2.0
            && pos.x > self.pos.x + self.size.0 / 2.0
            && pos.y < self.pos.y + self.size.1 / 2.0
            && pos.y > self.pos.y + self.size.1 / 2.0
    }
}

struct SettingButton {}

impl ClickHandler for SettingButton {
    fn click(&self) {
        unimplemented!()
    }
}
