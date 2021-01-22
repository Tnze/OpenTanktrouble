use cgmath::{num_traits::FloatConst, Vector2};
use std::sync::Arc;

use super::user_interface::{
    ClickHandler, Element as UIElement, RectButton, RoundButton, Scene as UIScene,
};

pub struct MainMenuScene {
    buttons: (RectButton<StartButton>, RoundButton<StartButton>),
}

impl MainMenuScene {
    pub fn new(device: &Arc<Device>) -> MainMenuScene {
        MainMenuScene {
            buttons: (
                RectButton {
                    pos: Vector2::new(0.0, 0.0),
                    size: (1.0, 1.0),
                    click_handler: StartButton {},
                },
                RoundButton {
                    pos: Vector2::new(0.0, 0.0),
                    size: 0.1,
                    click_handler: StartButton::new(device.clone()),
                },
            ),
        }
    }
}

impl UIScene for MainMenuScene {
    fn render_pass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        unimplemented!()
    }

    fn reset_viewport(&self, _dimension: [f32; 2]) {
        unimplemented!()
    }
}

impl UIElement for MainMenuScene {
    fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
        _dimensions: [f32; 2],
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        Ok(builder)
    }
    fn click(&self, pos: Vector2<f32>) -> bool {
        self.buttons.0.click(pos) || self.buttons.1.click(pos)
    }
}

struct StartButton {}

impl StartButton {
    fn new(device: Arc<Device>) -> StartButton {
        let _vertex_buffer = {
            CpuAccessibleBuffer::from_iter(
                device,
                BufferUsage::all(),
                false,
                gen_gear_vertexes(12, 0.1, 0.12, 0.2).iter().cloned(),
            )
                .unwrap()
        };
        unimplemented!()
    }
}

impl ClickHandler for StartButton {
    fn click(&self) {
        unimplemented!()
    }
}

impl StartButton {}

#[derive(Default, Copy, Clone)]
pub struct Vertex {
    position: (f32, f32),
}
vulkano::impl_vertex!(Vertex, position);

/// Generate vertexes of a gear pattern.
pub fn gen_gear_vertexes(teeth: i32, ir: f32, or: f32, overlap: f32) -> Vec<Vertex> {
    let central_angle = 2.0 * f32::PI() / teeth as f32;
    let overlap_angle = central_angle * overlap;
    let calc_pos = |angle: f32, length| {
        let (sin, cos) = angle.sin_cos();
        (sin * length, cos * length)
    };

    let mut vertexes = Vec::with_capacity(teeth as usize * 3);
    for i in 1..teeth {
        vertexes.push(Vertex {
            position: calc_pos((i - 1) as f32 - overlap_angle, ir),
        });
        vertexes.push(Vertex {
            position: calc_pos((i as f32 - 0.5) * central_angle, or),
        });
        vertexes.push(Vertex {
            position: calc_pos(i as f32 + overlap_angle, ir),
        });
    }
    vertexes
}
