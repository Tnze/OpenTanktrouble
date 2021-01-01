use vulkano::device::Device;
use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage};
use std::sync::Arc;
use cgmath::Vector2;
use vulkano::memory::pool::{StdMemoryPoolAlloc, PotentialDedicatedAllocation};

pub struct MainMenu {
    elements: Arc<[Button; 1]>
}

impl MainMenu {
    pub fn create(device: Arc<Device>) -> MainMenu {
        MainMenu {
            elements: Arc::from([
                Button::new(Arc::clone(&device), 0.3, -0.7, 0.4, 0.2),
            ])
        }
    }
    pub fn draw(&self) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
        self.elements[0].vb()
    }
}

#[derive(Default, Copy, Clone)]
pub struct Vertex {
    position: (f32, f32),
}
vulkano::impl_vertex!(Vertex, position);

struct Button {
    vb: Arc<CpuAccessibleBuffer<[Vertex]>>
}

impl Button {
    pub fn new(device: Arc<Device>, top: f32, left: f32, width: f32, height: f32) -> Button {
        Button {
            vb: {
                CpuAccessibleBuffer::from_iter(
                    Arc::clone(&device),
                    BufferUsage::all(),
                    false,
                    [
                        Vertex { position: (left, top) },
                        Vertex { position: (left + width, top) },
                        Vertex { position: (left + width, top + height) },
                        Vertex { position: (left, top + height) },
                    ].iter().cloned(),
                ).unwrap()
            }
        }
    }
    pub fn vb(&self) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
        self.vb.clone()
    }
}