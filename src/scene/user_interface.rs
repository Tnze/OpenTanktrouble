use cgmath::Vector2;
// use future
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, DrawError, pool::standard::StandardCommandPoolBuilder,
};

pub(crate) trait Element {
    fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        Ok(builder)
    }
    fn click(&self, pos: Vector2<f32>) -> bool;
    fn touch(&self, pos: Vector2<f32>) -> bool {
        self.click(pos)
    }
}
