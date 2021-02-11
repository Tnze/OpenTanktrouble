use std::ops::{Deref, DerefMut};

use wgpu::util::DeviceExt;

use super::super::render_layer::{BasicLayer, VertexAndInstances};
use super::Vertex;

const A: f32 = 0.2;
const B: f32 = 0.25;
const TANK_VERTICES: &[Vertex] = &[
    Vertex::new(-A, -B),
    Vertex::new(A, -B),
    Vertex::new(A, B),
    Vertex::new(-A, -B),
    Vertex::new(A, B),
    Vertex::new(-A, B),
];

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TankInstance {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub rotation: f32,
    pub rotation_v: f32,
}

pub struct TankLayer(BasicLayer<VertexAndInstances>);

impl Deref for TankLayer {
    type Target = BasicLayer<VertexAndInstances>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TankLayer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TankLayer {
    pub fn new(
        device: &wgpu::Device,
        fragment_format: wgpu::ColorTargetState,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let pipeline = Self::pipeline(device, fragment_format, uniform_bind_group_layout);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tank Vertex Buffer"),
            contents: bytemuck::cast_slice(TANK_VERTICES),
            usage: wgpu::BufferUsage::VERTEX,
        });

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TankInstance Buffer"),
            contents: &[],
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        TankLayer(BasicLayer {
            pipeline,
            buffer: VertexAndInstances {
                vertex: vertex_buffer,
                vertex_num: TANK_VERTICES.len() as _,
                instance: instance_buffer,
                instance_num: 0,
            },
        })
    }

    fn pipeline(
        device: &wgpu::Device,
        fragment_format: wgpu::ColorTargetState,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let vs_module = device.create_shader_module(&wgpu::include_spirv!("shaders/tank.vert.spv"));
        let fs_module = device.create_shader_module(&wgpu::include_spirv!("shaders/tank.frag.spv"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Tank Render Pipeline Layout"),
                bind_group_layouts: &[uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tank Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::InputStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float2],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<TankInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::InputStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![1 => Float2, 2 => Float2, 3 => Float, 4 => Float],
                    }
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: "main",
                targets: &[fragment_format],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        })
    }

    pub fn update_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: Vec<TankInstance>,
    ) {
        if self.buffer.instance_num < instances.len() {
            // Recreate buffer
            self.buffer.instance = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
            });
            self.buffer.instance_num = instances.len();
        } else {
            // Just send to the existing buffer
            queue.write_buffer(&self.buffer.instance, 0, bytemuck::cast_slice(&instances));
        }
    }
}
