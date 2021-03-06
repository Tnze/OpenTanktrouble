use std::{error::Error, time};
use std::cell::RefCell;

use cgmath::SquareMatrix;
use crossbeam_channel::{bounded, Receiver, Select, Sender, tick};
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

use maze_layer::{MazeData, MazeLayer};
use tank_layer::{TankInstance, TankLayer};

use crate::input::{Controller, input_center::InputCenter};

use super::{maze::Maze, render_layer::Layer, SceneRender, SceneUpdater};

mod maze_layer;
mod tank_layer;

const PHYSICAL_DT: f32 = 1.0 / 90.0;

pub struct GameSceneRender {
    clean_color: wgpu::Color,

    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    tank_layer: TankLayer,
    maze_layer: MazeLayer,

    maze_size: [usize; 2],

    tank_update_chan: Receiver<Vec<TankInstance>>,
    maze_update_chan: Receiver<MazeData>,
    stop_signal_sender: Sender<()>,

    last_update: time::Instant,
}

pub struct GameSceneUpdater {
    physical: RefCell<PhysicalStatus>,

    tank_update_sender: Sender<Vec<TankInstance>>,
    maze_update_sender: Sender<MazeData>,
    stop_signal_chan: Receiver<()>,
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
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    forecast: f32,
}

pub(crate) fn new(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (GameSceneRender, GameSceneUpdater) {
    info!("Creating GameScene");
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

    let tank_layer = TankLayer::new(device, format.into(), &uniform_bind_group_layout);
    let maze_layer = MazeLayer::new(device, format.into(), &uniform_bind_group_layout);

    // Start physic emulation
    let (tank_update_sender, tank_update_chan) = bounded(0);
    let (maze_update_sender, maze_update_chan) = bounded(0);
    let (stop_signal_sender, stop_signal_chan) = bounded(0);

    let physical = RefCell::new(PhysicalStatus {
        tanks: Vec::new(),
        seq_number: 0,
        pipeline: PhysicsPipeline::new(),
        integration_parameters: IntegrationParameters::default(),
        broad_phase: BroadPhase::new(),
        narrow_phase: NarrowPhase::new(),
        rigid_body_set: RigidBodySet::new(),
        collider_set: ColliderSet::new(),
        joint_set: JointSet::new(),
    });

    (
        GameSceneRender {
            clean_color,
            uniforms,
            uniform_buffer,
            uniform_bind_group,
            tank_layer,
            maze_layer,
            maze_size: [1, 1],

            tank_update_chan,
            maze_update_chan,
            stop_signal_sender,

            last_update: time::Instant::now(),
        },
        GameSceneUpdater {
            physical,
            tank_update_sender,
            maze_update_sender,
            stop_signal_chan,
        },
    )
}

impl GameSceneUpdater {
    fn manage(&self, input_center: &InputCenter) -> Result<(), Box<dyn Error>> {
        let mut physical = self.physical.borrow_mut();
        physical.integration_parameters.dt = PHYSICAL_DT;
        let ticker = tick(time::Duration::from_secs_f32(PHYSICAL_DT));

        let maze = Maze::new(&mut rand::thread_rng());

        // Generate mesh for render
        let (maze_mesh_vertices, maze_mesh_indexes) = maze.triangle_mesh();
        self.maze_update_sender.send(MazeData {
            vertex: maze_mesh_vertices,
            index: maze_mesh_indexes,
            size: [maze.width, maze.height],
        })?;

        // Generate mesh for physic
        let (maze_mesh_vertices, maze_mesh_indexes) = maze.triangle_mesh();
        physical.add_maze(maze_mesh_vertices, maze_mesh_indexes);

        'next_update: loop {
            input_center.update(|_| (), |_, _| ())?;
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
            let i_update_sender = selector.send(&self.tank_update_sender);
            let i_stop_receiver = selector.recv(&self.stop_signal_chan);

            loop {
                let oper = selector.select();
                match oper.index() {
                    i if i == i_stop_receiver => {
                        oper.recv(&self.stop_signal_chan)?;
                        return Ok(());
                    }
                    i if i == i_ticker => {
                        oper.recv(&ticker)?;
                        continue 'next_update;
                    }
                    i if i == i_update_sender => {
                        // This unwrap() never panic because this channel
                        // is delete from selector next line.
                        oper.send(&self.tank_update_sender, update_data.take().unwrap())?;
                        selector.remove(i_update_sender);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn add_player(&self, controller: Box<dyn Controller>) {
        let physical = &mut *self.physical.borrow_mut();
        let right_body = RigidBodyBuilder::new_dynamic()
            .can_sleep(true)
            .mass(0.9)
            .linear_damping(10.0)
            .principal_angular_inertia(0.8)
            .angular_damping(10.0)
            .build();
        let collider = ColliderBuilder::cuboid(0.2, 0.25).build();
        let rigid_body_handle = physical.rigid_body_set.insert(right_body);
        let collider_handle =
            physical
                .collider_set
                .insert(collider, rigid_body_handle, &mut physical.rigid_body_set);

        physical.tanks.push(PhysicTank {
            controller,
            rigid_body_handle,
            collider_handle,
        });
    }
}

impl SceneRender for GameSceneRender {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::SwapChainTexture,
        frame_size: [u32; 2],
    ) -> Result<(), wgpu::SwapChainError> {
        // Update data from physical thread
        if let Ok(instances) = self.tank_update_chan.try_recv() {
            self.last_update = time::Instant::now();
            self.tank_layer.update_instances(device, queue, instances);
        }
        if let Ok(maze_data) = self.maze_update_chan.try_recv() {
            self.maze_size = maze_data.size;
            self.maze_layer.update_maze(device, queue, maze_data);
        }
        // Update uniform
        let frame_size = [frame_size[0] as f32, frame_size[1] as f32];
        self.uniforms = Uniforms {
            view_proj: projection(&frame_size, &self.maze_size).into(),
            forecast: PHYSICAL_DT.min(self.last_update.elapsed().as_secs_f32() * 0.99), // do not forecast greater then physic engine
        };
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
}

impl SceneUpdater for GameSceneUpdater {
    fn update(
        &self,
        _device: &wgpu::Device,
        _format: wgpu::TextureFormat,
        input_center: &InputCenter,
    ) -> Option<(Box<dyn SceneRender + Sync + Send>, Box<dyn SceneUpdater>)> {
        debug!("Start update");
        self.manage(input_center)
            .unwrap_or_else(|err| error!("{}", err));
        debug!("Stop update");
        None
    }
}

impl Drop for GameSceneRender {
    fn drop(&mut self) {
        // This will block until update thread quit
        self.stop_signal_sender.send(()).unwrap();
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
