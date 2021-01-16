// use future
use std::sync::Arc;

use cgmath::Vector2;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, DrawError, pool::standard::StandardCommandPoolBuilder,
    },
    framebuffer::RenderPassAbstract,
};

pub(crate) trait Element {
    fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
        _dimensions: [f32; 2],
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        Ok(builder)
    }
    fn click(&self, _pos: Vector2<f32>) -> bool {
        false
    }
    fn touch(&self, pos: Vector2<f32>) -> bool {
        self.click(pos)
    }
}

pub(crate) trait Scene: Element {
    fn render_pass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync>;
    fn reset_viewport(&self, dimension: [f32; 2]);
}

pub(crate) trait ClickHandler {
    fn click(&self);
}

pub(crate) struct Button<C: ClickHandler> {
    pub(crate) pos: Vector2<f32>,
    pub(crate) size: (f32, f32),
    pub(crate) click_handler: C,
}

impl<C: ClickHandler> Button<C> {
    pub(crate) fn click(&self, pos: Vector2<f32>) -> bool {
        if self.is_hit(pos) {
            self.click_handler.click();
            return true;
        }
        false
    }
    pub(crate) fn is_hit(&self, pos: Vector2<f32>) -> bool {
        pos.x < self.pos.x + self.size.0 / 2.0
            && pos.x > self.pos.x + self.size.0 / 2.0
            && pos.y < self.pos.y + self.size.1 / 2.0
            && pos.y > self.pos.y + self.size.1 / 2.0
    }
}
