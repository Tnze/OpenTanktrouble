use std::sync::Arc;

use cgmath::{num_traits::FloatConst, Vector2};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool},
    command_buffer::{
        AutoCommandBufferBuilder, DrawError, pool::standard::StandardCommandPoolBuilder,
    },
    framebuffer::RenderPassAbstract,
};

use super::user_interface::{
    ClickHandler, Element as UIElement, RectButton, RoundButton, Scene as UIScene,
};

pub struct MainMenuScene {
    buttons: (RectButton<StartButton>, RoundButton<StartButton>),
}

impl MainMenuScene {
    pub fn new() -> MainMenuScene {
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
    // fn new() -> StartButton {
    //     let vertex_buffer = {
    //         CpuAccessibleBuffer::from_iter(
    //             device,
    //             BufferUsage::all(),
    //             false,
    //             gen_gear_vertexes(12, 0.1, 0.12).iter().cloned(),
    //         )
    //         .unwrap()
    //     };
    // }
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

struct PairIter<Item, Iter> {
    iter: Iter,
    pre: Option<Item>,
}

impl<T: ?Sized> WindowIterator for T where T: Iterator {}

/// # Examples
///
/// ```rust
/// let data: Vec<u64> = vec![1, 2, 3, 4, 5];
/// let input = data.iter();
/// let output = windows(input);
///
/// assert_eq!(Some((1, 2)), output.next());
/// assert_eq!(Some((2, 3)), output.next());
/// assert_eq!(Some((3, 4)), output.next());
/// assert_eq!(Some((4, 5)), output.next());
/// ```
trait WindowIterator: Iterator {
    fn sliding_pair(self) -> PairIter<Self::Item, Self>
        where
            Self: Sized,
    {
        PairIter {
            iter: self,
            pre: None,
        }
    }
}

impl<Iter, Item> Iterator for PairIter<Item, Iter>
    where
        Iter: Iterator<Item=Item>,
        Item: Clone,
{
    type Item = (Item, Item);

    fn next(&mut self) -> Option<Self::Item> {
        let pre = self.pre.take().or_else(|| self.iter.next());
        let current = self.iter.next();
        self.pre = current.clone();
        pre.zip(current)
    }
}

/// Generate vertexes of a gear pattern.
pub fn gen_gear_vertexes(teeth: i32, ir: f32, or: f32) -> Vec<Vertex> {
    let mut vertexes = Vec::with_capacity(teeth as usize * 3);
    let central_angle = 2.0 * f32::PI() / teeth as f32;
    let calc_pos = |angle: f32, length| {
        let (sin, cos) = angle.sin_cos();
        (sin * length, cos * length)
    };
    for (pre, mid, cur) in {
        (1..teeth)
            .map(|i| i as f32 * central_angle)
            .sliding_pair()
            .map(|(pre, cur)| (pre, (pre + cur) / 2.0, cur))
    } {
        vertexes.push(Vertex {
            position: calc_pos(pre, ir),
        });
        vertexes.push(Vertex {
            position: calc_pos(mid, or),
        });
        vertexes.push(Vertex {
            position: calc_pos(cur, ir),
        });
    }
    vertexes
}
