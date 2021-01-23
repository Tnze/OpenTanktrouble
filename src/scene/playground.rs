use std::{
    thread,
    time::{Duration, Instant},
};
use std::error::Error;

use crossbeam_channel::{bounded, Receiver, Select, Sender, tick, unbounded};
use log::{debug, error, info, log_enabled};
use rapier2d::{
    dynamics::{IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle, RigidBodySet},
    geometry::{BroadPhase, ColliderBuilder, ColliderHandle, ColliderSet, NarrowPhase},
    na::{Matrix3, Matrix4, Rotation2, Vector2},
    pipeline::PhysicsPipeline,
};
use wgpu::{CommandBuffer, util::DeviceExt};

use crate::input::Controller::{self, Gamepad, Keyboard};

pub(crate) trait Scene {
    fn render(
        &mut self,
        device: &wgpu::Device,
        frame: &wgpu::SwapChainTexture,
    ) -> wgpu::CommandBuffer;
    fn add_controller(&self, ctrl: Controller);
}

pub struct GameScene {
    clean_color: wgpu::Color,
    render_pipeline: wgpu::RenderPipeline,

    tank_module_buffer: wgpu::Buffer,
    tank_module_num: u32,

    instances_data: Box<Vec<TankInstance>>,
    instances_buffer: wgpu::Buffer,
    update_chan: Receiver<Box<Vec<TankInstance>>>,
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
struct Vertex {
    position: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TankInstance {
    position: [f32; 2],
    velocity: [f32; 2],
    rotation: f32,
}

impl GameScene {
    pub(crate) fn new(device: &wgpu::Device, sc_desc: &wgpu::SwapChainDescriptor) -> GameScene {
        info!("Creating GameScene");
        // Create render objects
        const A: f32 = 0.2;
        const B: f32 = 0.25;
        const VERTICES: &[Vertex] = &[
            Vertex { position: [-A, -B] },
            Vertex { position: [A, -B] },
            Vertex { position: [A, B] },
            Vertex { position: [-A, -B] },
            Vertex { position: [A, B] },
            Vertex { position: [-A, B] },
        ];

        let clean_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };

        let tank_module_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsage::VERTEX,
        });

        let instances_data = Box::new(Vec::new());
        let instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instances_data),
            usage: wgpu::BufferUsage::VERTEX,
        });

        let render_pipeline = {
            let vs_module =
                device.create_shader_module(wgpu::include_spirv!("shaders/tank.vert.spv"));
            let fs_module =
                device.create_shader_module(wgpu::include_spirv!("shaders/tank.frag.spv"));

            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Tank Render Pipeline Layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Tank Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex_stage: wgpu::ProgrammableStageDescriptor {
                    module: &vs_module,
                    entry_point: "main",
                },
                fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                    module: &fs_module,
                    entry_point: "main",
                }),
                rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: wgpu::CullMode::Back,
                    depth_bias: 0,
                    depth_bias_slope_scale: 0.0,
                    depth_bias_clamp: 0.0,
                    clamp_depth: false,
                }),
                color_states: &[wgpu::ColorStateDescriptor {
                    format: sc_desc.format,
                    color_blend: wgpu::BlendDescriptor::REPLACE,
                    alpha_blend: wgpu::BlendDescriptor::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                }],
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                depth_stencil_state: None,
                vertex_state: wgpu::VertexStateDescriptor {
                    index_format: wgpu::IndexFormat::Uint16,
                    vertex_buffers: &[
                        wgpu::VertexBufferDescriptor {
                            stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                            step_mode: wgpu::InputStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float2],
                        },
                        wgpu::VertexBufferDescriptor {
                            stride: std::mem::size_of::<TankInstance>() as wgpu::BufferAddress,
                            step_mode: wgpu::InputStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![1 => Float2, 2 => Float2, 3 => Float],
                        }
                    ],
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false,
            })
        };

        // Init controller channel
        let (add_controller_chan, recv_controller_chan) = unbounded();

        // Start physic emulation
        let (update_sender, update_chan) = bounded(0);
        thread::spawn(move || {
            Self::manage(update_sender, recv_controller_chan)
                .unwrap_or_else(|err| error!("{}", err));
        });

        GameScene {
            clean_color,
            render_pipeline,
            tank_module_buffer,
            tank_module_num: VERTICES.len() as u32,
            instances_data,
            instances_buffer,
            update_chan,
            add_controller_chan,
            last_update: Instant::now(),
        }
    }

    fn manage(
        update_sender: Sender<Box<Vec<TankInstance>>>,
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
        let ticker = tick(Duration::from_secs_f32(
            physical.integration_parameters.dt(),
        ));
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
                        }
                    })
                    .collect::<Vec<TankInstance>>(),
            );

            // Wait for next tick, and do other things.
            let mut selector = Select::new();
            let ticker_index = selector.recv(&ticker);
            let update_sender_index = selector.send(&update_sender);
            let ctrl_receive_index = selector.recv(&ctrl_receiver);

            loop {
                let oper = selector.select();
                match oper.index() {
                    i if i == ticker_index => {
                        oper.recv(&ticker)?;
                        continue 'next_update;
                    }
                    i if i == update_sender_index => {
                        oper.send(&update_sender, Box::new(update_data.take().unwrap()))?;
                        // Remove this selector because we only need
                        // send the data once after each update tick
                        selector.remove(update_sender_index);
                    }
                    i if i == ctrl_receive_index => {
                        physical.add_player(oper.recv(&ctrl_receiver)?);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl Scene for GameScene {
    fn render(&mut self, device: &wgpu::Device, frame: &wgpu::SwapChainTexture) -> CommandBuffer {
        // Update data from physical thread
        if let Ok(instances) = self.update_chan.try_recv() {
            self.last_update = Instant::now();
            self.instances_data = instances;
            self.instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&self.instances_data),
                usage: wgpu::BufferUsage::VERTEX,
            });
        }
        // Building command buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("GameScene Render Encoder"),
        });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.tank_module_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instances_buffer.slice(..));
            render_pass.draw(0..self.tank_module_num, 0..(self.instances_data.len() as _));
        }
        encoder.finish()
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
            right_body.apply_force(rotation * Vector2::new(0.0, acl * -15.0), true);
            right_body.apply_torque(rot * 20.0, true);
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
            .mass(1.0, true)
            .translation(0.3, 0.3)
            .linear_damping(10.0)
            .principal_angular_inertia(1.0, true)
            .angular_damping(5.0)
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
}

#[inline]
fn projection(frame_size: &[f32; 2], scale: f32) -> Matrix3<f32> {
    Matrix3::new(
        scale,
        0.0,
        0.0,
        0.0,
        scale * frame_size[0] / frame_size[1],
        0.0,
        0.0,
        0.0,
        1.0,
    )
}
