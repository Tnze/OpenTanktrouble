use std::ops::{Deref, DerefMut};

use wgpu::util::DeviceExt;

use crate::scene::render_layer::{BasicLayer, VertexAndIndexes};

use super::Vertex;

pub struct MazeData {
    pub vertex: Vec<Vertex>,
    pub index: Vec<u32>,
    pub size: [usize; 2],
}

pub struct MazeLayer(BasicLayer<VertexAndIndexes>);

impl Deref for MazeLayer {
    type Target = BasicLayer<VertexAndIndexes>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MazeLayer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl MazeLayer {
    pub fn new(
        device: &wgpu::Device,
        fragment_format: wgpu::ColorTargetState,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let pipeline = Self::pipeline(device, fragment_format, uniform_bind_group_layout);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Maze Vertex Buffer"),
            contents: &[],
            usage: wgpu::BufferUsage::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Maze Index Buffer"),
            contents: &[],
            usage: wgpu::BufferUsage::INDEX,
        });

        MazeLayer(BasicLayer {
            pipeline,
            buffer: VertexAndIndexes {
                vertex: vertex_buffer,
                index: index_buffer,
                index_num: 0,
            },
        })
    }

    fn pipeline(
        device: &wgpu::Device,
        fragment_format: wgpu::ColorTargetState,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let vs_module = device.create_shader_module(&wgpu::include_spirv!("shaders/maze.vert.spv"));
        let fs_module = device.create_shader_module(&wgpu::include_spirv!("shaders/maze.frag.spv"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Maze Layer Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Maze Layer Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float2],
                }],
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

    pub fn update_maze(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, maze: MazeData) {
        self.buffer.vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Maze Vertex Buffer"),
            contents: bytemuck::cast_slice(&maze.vertex),
            usage: wgpu::BufferUsage::VERTEX,
        });
        self.buffer.index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Maze Index Buffer"),
            contents: bytemuck::cast_slice(&maze.index),
            usage: wgpu::BufferUsage::INDEX,
        });
        self.buffer.index_num = maze.index.len();
    }
}
