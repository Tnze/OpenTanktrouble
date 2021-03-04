use crate::input::{Controller, input_center::InputHandler};

// pub mod main_menu;
pub mod game_scene;
mod maze;
pub(crate) mod prepare_scene;
mod render_layer;

pub trait SceneRender {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::SwapChainTexture,
        frame_size: [u32; 2],
    ) -> Result<(), wgpu::SwapChainError>;
}

pub trait SceneUpdater {
    fn update(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        input_handler: &InputHandler,
    ) -> (Box<dyn SceneRender + Sync + Send>, Box<dyn SceneUpdater>);
}
