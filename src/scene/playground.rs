use std::{
    error::Error,
    thread,
    time::{Duration, Instant},
};

use cgmath::SquareMatrix;
use crossbeam_channel::{bounded, Receiver, Select, Sender, tick, unbounded};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use rapier2d::{
    dynamics::{IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle, RigidBodySet},
    geometry::{BroadPhase, ColliderBuilder, ColliderHandle, ColliderSet, NarrowPhase},
    math::{Point, Rotation},
    na::{Matrix4, Rotation2, Vector2},
    pipeline::PhysicsPipeline,
};
use wgpu::util::DeviceExt;

use crate::input::Controller::{self, Gamepad, Keyboard};
use crate::scene::maze::{Maze, TriangleIndexList, VertexList};

const PHYSICAL_DT: f32 = 1.0 / 90.0;

pub(crate) trait Scene {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::SwapChainTexture,
        frame_size: (u32, u32),
    ) -> Result<(), wgpu::SwapChainError>;
    fn add_controller(&self, ctrl: Controller);
}

pub struct GameScene {
    clean_color: wgpu::Color,

    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    tank_module_buffer: wgpu::Buffer,
    tank_module_num: u32,
    tank_render_pipeline: wgpu::RenderPipeline,

    maze_mesh_data: (Vec<Vertex>, Vec<u32>),
    maze_mesh_buffer: wgpu::Buffer,
    maze_mesh_index_buffer: wgpu::Buffer,
    maze_mesh_index_num: u32,
    maze_render_pipeline: wgpu::RenderPipeline,

    instances_data: Vec<TankInstance>,
    instances_buffer: wgpu::Buffer,

    tank_update_chan: Receiver<Vec<TankInstance>>,
    maze_update_chan: Receiver<(Vec<Vertex>, Vec<u32>)>,
    add_controller_chan: Sender<Controller>,

    last_update: Instant,
}

struct PhysicalStatus {
    tanks: Vec<PhysicTank>,
    seq_number: u32,

    pipeline: PhysicsPipeline,
    integration_parameters: IntegrationParameters,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    joint_set: JointSet,
}

struct PhysicTank {
    controller: Controller,
    rigid_body_handle: RigidBodyHandle,
    collider_handle: ColliderHandle,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 2],
}

impl Vertex {
    pub const fn new(x: f32, y: f32) -> Vertex {
        Vertex { position: [x, y] }
    }
}

impl TriangleIndexList<u32> for Vec<u32> {
    fn new() -> Self {
        Vec::new()
    }

    fn push(&mut self, p0: u32, p1: u32, p2: u32) {
        self.push(p0);
        self.push(p1);
        self.push(p2);
    }
}

impl VertexList<f32> for Vec<Vertex> {
    fn new() -> Self {
        Vec::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        Vec::with_capacity(capacity)
    }

    fn push(&mut self, p0: f32, p1: f32) {
        self.push(Vertex::new(p0, p1));
    }
}

impl TriangleIndexList<u32> for Vec<[u32; 3]> {
    fn new() -> Self {
        Vec::new()
    }

    fn push(&mut self, p0: u32, p1: u32, p2: u32) {
        self.push([p0, p1, p2]);
    }
}

impl VertexList<f32> for Vec<Point<f32>> {
    fn new() -> Self {
        Vec::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        Vec::with_capacity(capacity)
    }

    fn push(&mut self, p0: f32, p1: f32) {
        self.push(Point::new(p0, p1));
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TankInstance {
    position: [f32; 2],
    velocity: [f32; 2],
    rotation: f32,
    rotation_v: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    forecast: f32,
}

impl GameScene {
    pub(crate) fn new(device: &wgpu::Device, sc_desc: &wgpu::SwapChainDescriptor) -> GameScene {
        info!("Creating GameScene");
        // Create render objects
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

        let clean_color = wgpu::Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };

        let tank_module_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tank Vertex Buffer"),
            contents: bytemuck::cast_slice(TANK_VERTICES),
            usage: wgpu::BufferUsage::VERTEX,
        });

        let instances_data = Vec::new();
        let instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TankInstance Buffer"),
            contents: bytemuck::cast_slice(&instances_data),
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        let uniforms = Uniforms {
            view_proj: cgmath::Matrix4::identity().into(),
            forecast: 0.0,
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tank Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let maze_mesh_vertexes = Vec::new();
        let maze_mesh_indexes = Vec::new();
        let maze_mesh_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Maze Vertex Buffer"),
            contents: bytemuck::cast_slice(&maze_mesh_vertexes),
            usage: wgpu::BufferUsage::VERTEX,
        });
        let maze_mesh_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Maze Index Buffer"),
            contents: bytemuck::cast_slice(&maze_mesh_indexes),
            usage: wgpu::BufferUsage::INDEX,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let tank_render_pipeline = {
            let vs_module =
                device.create_shader_module(&wgpu::include_spirv!("shaders/tank.vert.spv"));
            let fs_module =
                device.create_shader_module(&wgpu::include_spirv!("shaders/tank.frag.spv"));

            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Tank Render Pipeline Layout"),
                    bind_group_layouts: &[&uniform_bind_group_layout],
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
                    targets: &[sc_desc.format.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            })
        };

        let maze_render_pipeline = {
            let vs_module =
                device.create_shader_module(&wgpu::include_spirv!("shaders/maze.vert.spv"));
            let fs_module =
                device.create_shader_module(&wgpu::include_spirv!("shaders/maze.frag.spv"));

            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Tank Render Pipeline Layout"),
                    bind_group_layouts: &[&uniform_bind_group_layout],
                    push_constant_ranges: &[],
                });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Tank Render Pipeline"),
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
                    targets: &[sc_desc.format.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            })
        };

        // Init controller channel
        let (add_controller_chan, recv_controller_chan) = unbounded();
        // Start physic emulation
        let (tank_update_sender, tank_update_chan) = bounded(0);
        let (maze_update_sender, maze_update_chan) = bounded(0);

        thread::spawn(move || {
            Self::manage(tank_update_sender, maze_update_sender, recv_controller_chan)
                .unwrap_or_else(|err| error!("{}", err));
        });

        GameScene {
            clean_color,
            uniforms,
            uniform_buffer,
            uniform_bind_group,
            tank_module_buffer,
            tank_module_num: TANK_VERTICES.len() as _,
            tank_render_pipeline,
            maze_mesh_data: (maze_mesh_vertexes, maze_mesh_indexes),
            maze_mesh_buffer,
            maze_mesh_index_buffer,
            maze_mesh_index_num: 0,
            maze_render_pipeline,
            instances_data,
            instances_buffer,
            tank_update_chan,
            maze_update_chan,
            add_controller_chan,
            last_update: Instant::now(),
        }
    }

    fn manage(
        tank_update_sender: Sender<Vec<TankInstance>>,
        maze_update_sender: Sender<(Vec<Vertex>, Vec<u32>)>,
        ctrl_receiver: Receiver<Controller>,
    ) -> Result<(), Box<dyn Error>> {
        info!("Update thread spawned");

        let mut physical = PhysicalStatus {
            tanks: Vec::new(),

            seq_number: 0,
            pipeline: PhysicsPipeline::new(),
            integration_parameters: IntegrationParameters::default(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            joint_set: JointSet::new(),
        };
        physical.integration_parameters.dt = PHYSICAL_DT;
        let ticker = tick(Duration::from_secs_f32(PHYSICAL_DT));

        let maze = Maze::new(&mut rand::thread_rng());
        let maze_mesh_data = maze.triangle_mesh();
        maze_update_sender.send(maze_mesh_data)?;

        let (maze_mesh_vertices, maze_mesh_indexes) = maze.triangle_mesh();
        physical.add_maze(maze_mesh_vertices, maze_mesh_indexes);

        'next_update: loop {
            physical.update_tick();
            let mut update_data = Some(
                physical
                    .tanks
                    .iter()
                    .map(|tank| {
                        let rigid_body =
                            physical.rigid_body_set.get(tank.rigid_body_handle).unwrap();
                        let position = rigid_body.position();
                        let velocity = rigid_body.linvel();
                        TankInstance {
                            position: position.translation.vector.into(),
                            velocity: [velocity.x, velocity.y],
                            rotation: position.rotation.angle(),
                            rotation_v: rigid_body.angvel(),
                        }
                    })
                    .collect::<Vec<TankInstance>>(),
            );

            // Wait for next tick, and do other things on idle time.
            // I didn't use 'select!' marco here because we need
            // delete update_sender after send once.
            let mut selector = Select::new();
            let i_ticker = selector.recv(&ticker);
            let i_update_sender = selector.send(&tank_update_sender);
            let i_controller_receiver = selector.recv(&ctrl_receiver);

            loop {
                let oper = selector.select();
                match oper.index() {
                    i if i == i_ticker => {
                        oper.recv(&ticker)?;
                        continue 'next_update;
                    }
                    i if i == i_update_sender => {
                        // This unwrap() never panic because this channel
                        // is delete from selector next line.
                        oper.send(&tank_update_sender, update_data.take().unwrap())?;
                        selector.remove(i_update_sender);
                    }
                    i if i == i_controller_receiver => {
                        physical.add_player(oper.recv(&ctrl_receiver)?);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl Scene for GameScene {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::SwapChainTexture,
        frame_size: (u32, u32),
    ) -> Result<(), wgpu::SwapChainError> {
        // Update data from physical thread
        if let Ok(instances) = self.tank_update_chan.try_recv() {
            self.last_update = Instant::now();
            if self.instances_data.len() < instances.len() {
                // Recreate buffer
                self.instances_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Instance Buffer"),
                        contents: bytemuck::cast_slice(&instances),
                        usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                    });
            } else {
                // Just send to the existing buffer
                queue.write_buffer(&self.instances_buffer, 0, bytemuck::cast_slice(&instances));
            }
            self.instances_data = instances;
        }
        if let Ok((maze_mesh_vertexes, maze_mesh_indexes)) = self.maze_update_chan.try_recv() {
            self.maze_mesh_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Maze Vertex Buffer"),
                contents: bytemuck::cast_slice(&maze_mesh_vertexes),
                usage: wgpu::BufferUsage::VERTEX,
            });
            self.maze_mesh_index_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Maze Index Buffer"),
                    contents: bytemuck::cast_slice(&maze_mesh_indexes),
                    usage: wgpu::BufferUsage::INDEX,
                });
            self.maze_mesh_index_num = maze_mesh_indexes.len() as _;
            self.maze_mesh_data = (maze_mesh_vertexes, maze_mesh_indexes);
        }
        // Update uniform
        self.uniforms.view_proj =
            projection(&[frame_size.0 as f32, frame_size.1 as f32], 0.1).into();
        self.uniforms.forecast = (self.last_update.elapsed().as_secs_f32() * 0.99).min(PHYSICAL_DT); // do not forecast greater then physic engine
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
        // Building command buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("GameScene Render Encoder"),
        });
        encoder.push_debug_group("Draw scene");
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Draw maze and tanks"),
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clean_color),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.push_debug_group("Draw maze");
            render_pass.set_pipeline(&self.maze_render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.maze_mesh_buffer.slice(..));
            render_pass
                .set_index_buffer(self.maze_mesh_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.maze_mesh_index_num, 0, 0..1);
            render_pass.pop_debug_group();

            render_pass.push_debug_group("Draw tanks");
            render_pass.set_pipeline(&self.tank_render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.tank_module_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instances_buffer.slice(..));
            render_pass.draw(0..self.tank_module_num, 0..(self.instances_data.len() as _));
            render_pass.pop_debug_group();
        }
        encoder.pop_debug_group();

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    fn add_controller(&self, ctrl: Controller) {
        self.add_controller_chan.send(ctrl).unwrap_or_else(|err| {
            error!("Add controller to scene error: {}", err);
        });
    }
}

impl PhysicalStatus {
    fn update_tick(&mut self) {
        let gravity = Vector2::new(0.0, 0.0);

        // Apply the control to the tank.
        for tank in self.tanks.iter() {
            let (rot, acl) = match &tank.controller {
                Gamepad(c) => c.movement_status(),
                Keyboard(c) => c.movement_status(),
            };
            let right_body = &mut self.rigid_body_set[tank.rigid_body_handle];
            let rotation = &Rotation2::from(right_body.position().rotation);
            right_body.apply_force(rotation * Vector2::new(0.0, acl * 30.0), true);
            right_body.apply_torque(-rot * 40.0, true);
            right_body.set_linvel(
                Rotation::new(right_body.angvel() * PHYSICAL_DT) * right_body.linvel(),
                true,
            );
        }

        self.pipeline.step(
            &gravity,
            &self.integration_parameters,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.joint_set,
            None,
            None,
            &(),
        );
        // Increase simulate sequence number.
        self.seq_number += 1;
    }

    pub fn add_player(&mut self, controller: Controller) {
        let right_body = RigidBodyBuilder::new_dynamic()
            .can_sleep(true)
            .mass(0.9)
            .linear_damping(10.0)
            .principal_angular_inertia(0.8)
            .angular_damping(10.0)
            .build();
        let collider = ColliderBuilder::cuboid(0.2, 0.25).build();
        let rigid_body_handle = self.rigid_body_set.insert(right_body);
        let collider_handle =
            self.collider_set
                .insert(collider, rigid_body_handle, &mut self.rigid_body_set);

        self.tanks.push(PhysicTank {
            controller,
            rigid_body_handle,
            collider_handle,
        });
    }

    pub fn add_maze(&mut self, vertices: Vec<Point<f32>>, indices: Vec<[u32; 3]>) {
        let right_body = RigidBodyBuilder::new_static().build();
        let collider = ColliderBuilder::trimesh(vertices, indices).build();
        let rigid_body_handle = self.rigid_body_set.insert(right_body);
        let _collider_handle =
            self.collider_set
                .insert(collider, rigid_body_handle, &mut self.rigid_body_set);
    }
}

#[inline]
fn projection(frame_size: &[f32; 2], scale: f32) -> Matrix4<f32> {
    Matrix4::new(
        scale,
        0.0,
        0.0,
        0.0,
        0.0,
        scale * frame_size[0] / frame_size[1],
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    )
}
