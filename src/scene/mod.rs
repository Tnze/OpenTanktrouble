use crate::input::Controller;

// pub mod main_menu;
pub mod game_scene;
mod maze;
mod prepare_scene;
mod render_layer;

pub trait Scene {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::SwapChainTexture,
        frame_size: [u32; 2],
    ) -> Result<(), wgpu::SwapChainError>;
    fn add_controller(&self, ctrl: Box<dyn Controller>);
}
