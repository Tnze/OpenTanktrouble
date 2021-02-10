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
    na::{Matrix4, Rotation2, Vector2, Vector3},
    pipeline::PhysicsPipeline,
};
use wgpu::util::DeviceExt;

use crate::input::Controller;
use crate::scene::{
    maze::{Maze, util},
    render_layer::{BasicLayer, Layer, VertexAndIndexes, VertexAndInstances},
};

const PHYSICAL_DT: f32 = 1.0 / 90.0;

pub(crate) trait Scene {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::SwapChainTexture,
        frame_size: [u32; 2],
    ) -> Result<(), wgpu::SwapChainError>;
    fn add_controller(&self, ctrl: Box<dyn Controller>);
}

pub struct GameScene {
    clean_color: wgpu::Color,

    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    tank_layer: BasicLayer<VertexAndInstances>,
    maze_layer: BasicLayer<VertexAndIndexes>,

    maze_size: [usize; 2],

    tank_update_chan: Receiver<Vec<TankInstance>>,
    maze_update_chan: Receiver<MazeData>,
    add_controller_chan: Sender<Box<dyn Controller>>,

    last_update: Instant,
}

struct MazeData {
    vertex: Vec<Vertex>,
    index: Vec<u32>,
    size: [usize; 2],
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
    controller: Box<dyn Controller>,
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

        let uniforms = Uniforms {
            view_proj: cgmath::Matrix4::identity().into(),
            forecast: 0.0,
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tank Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let maze_mesh_vertexes = Vec::<Vertex>::new();
        let maze_mesh_indexes = Vec::<u32>::new();
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

        let tank_layer = BasicLayer {
            pipeline: {
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
            },
            buffer: VertexAndInstances {
                vertex: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Tank Vertex Buffer"),
                    contents: bytemuck::cast_slice(TANK_VERTICES),
                    usage: wgpu::BufferUsage::VERTEX,
                }),
                vertex_num: TANK_VERTICES.len() as _,
                instance: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("TankInstance Buffer"),
                    contents: bytemuck::cast_slice(&Vec::<TankInstance>::new()),
                    usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                }),
                instance_num: 0,
            },
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

        let maze_layer = BasicLayer {
            pipeline: maze_render_pipeline,
            buffer: VertexAndIndexes {
                vertex: maze_mesh_buffer,
                index: maze_mesh_index_buffer,
                index_num: maze_mesh_indexes.len(),
            },
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
            tank_layer,
            maze_layer,
            maze_size: [1, 1],
            tank_update_chan,
            maze_update_chan,
            add_controller_chan,
            last_update: Instant::now(),
        }
    }

    fn manage(
        tank_update_sender: Sender<Vec<TankInstance>>,
        maze_update_sender: Sender<MazeData>,
        ctrl_receiver: Receiver<Box<dyn Controller>>,
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

        // Generate mesh for render
        let (maze_mesh_vertices, maze_mesh_indexes) = maze.triangle_mesh();
        maze_update_sender.send(MazeData {
            vertex: maze_mesh_vertices,
            index: maze_mesh_indexes,
            size: [maze.width, maze.height],
        })?;

        // Generate mesh for physic
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
        frame_size: [u32; 2],
    ) -> Result<(), wgpu::SwapChainError> {
        // Update data from physical thread
        if let Ok(instances) = self.tank_update_chan.try_recv() {
            self.last_update = Instant::now();
            if self.tank_layer.buffer.instance_num < instances.len() {
                // Recreate buffer
                self.tank_layer.buffer.instance =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Instance Buffer"),
                        contents: bytemuck::cast_slice(&instances),
                        usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                    });
                self.tank_layer.buffer.instance_num = instances.len();
            } else {
                // Just send to the existing buffer
                queue.write_buffer(
                    &self.tank_layer.buffer.instance,
                    0,
                    bytemuck::cast_slice(&instances),
                );
            }
        }
        if let Ok(MazeData {
                      vertex: maze_mesh_vertexes,
                      index: maze_mesh_indexes,
                      size: maze_size,
                  }) = self.maze_update_chan.try_recv()
        {
            self.maze_layer.buffer.vertex =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Maze Vertex Buffer"),
                    contents: bytemuck::cast_slice(&maze_mesh_vertexes),
                    usage: wgpu::BufferUsage::VERTEX,
                });
            self.maze_layer.buffer.index =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Maze Index Buffer"),
                    contents: bytemuck::cast_slice(&maze_mesh_indexes),
                    usage: wgpu::BufferUsage::INDEX,
                });
            self.maze_layer.buffer.index_num = maze_mesh_indexes.len();
            self.maze_size = maze_size;
        }
        // Update uniform
        self.uniforms.view_proj = projection(
            &[frame_size[0] as f32, frame_size[1] as f32],
            &self.maze_size,
        )
            .into();
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

            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            render_pass.push_debug_group("Draw maze");
            self.maze_layer.sub_render_pass(&mut render_pass);
            render_pass.pop_debug_group();

            render_pass.push_debug_group("Draw tanks");
            self.tank_layer.sub_render_pass(&mut render_pass);
            render_pass.pop_debug_group();
        }
        encoder.pop_debug_group();

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    fn add_controller(&self, ctrl: Box<dyn Controller>) {
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
            let (rot, acl) = tank.controller.movement_status();
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

    pub fn add_player(&mut self, controller: Box<dyn Controller>) {
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
fn projection(frame_size: &[f32; 2], maze_size: &[usize; 2]) -> Matrix4<f32> {
    const MOVIE_WIDTH: f32 = 692.0;
    const MOVIE_HEIGHT: f32 = 480.0;
    const HEIGHT_TO_BOTTOM: f32 = 80.0;
    const MOVIE_PADDING: f32 = 10.0;
    const VIEW_WIDTH: f32 = MOVIE_WIDTH - MOVIE_PADDING;
    const VIEW_HEIGHT: f32 = MOVIE_HEIGHT - MOVIE_PADDING - HEIGHT_TO_BOTTOM;

    let maze_size = [maze_size[0] as f32 + 0.125, maze_size[1] as f32 + 0.125];
    let basic_scale = (VIEW_WIDTH / maze_size[0]).min(VIEW_HEIGHT / maze_size[1]);
    let window_scale = (frame_size[0] / MOVIE_WIDTH).min(frame_size[1] / MOVIE_HEIGHT) * 2.0;
    Matrix4::identity()
        .append_scaling(basic_scale)
        .append_translation(&Vector3::new(0.0, HEIGHT_TO_BOTTOM / 2.0, 0.0))
        .append_nonuniform_scaling(&Vector3::new(
            window_scale / frame_size[0],
            window_scale / frame_size[1],
            1.0,
        ))
}
