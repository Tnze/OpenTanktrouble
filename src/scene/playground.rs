use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crossbeam_channel::{bounded, Receiver, Sender, tick};
use crossbeam_channel::internal::SelectHandle;
use rapier2d::{
    dynamics::{IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle, RigidBodySet},
    geometry::{BroadPhase, ColliderBuilder, ColliderHandle, ColliderSet, NarrowPhase},
    na::{Matrix3, Matrix4, Rotation2, Vector2},
    pipeline::PhysicsPipeline,
};
use wgpu::{CommandBuffer, PipelineLayout, util::DeviceExt};
use winit::dpi::PhysicalSize;

use crate::input::Controller::{self, Gamepad, Keyboard};

pub(crate) trait Scene {
    fn render(
        &mut self,
        device: &wgpu::Device,
        frame: &wgpu::SwapChainTexture,
    ) -> wgpu::CommandBuffer;
}

pub struct GameScene {
    clean_color: wgpu::Color,
    render_pipeline: wgpu::RenderPipeline,

    tank_module_buffer: wgpu::Buffer,
    tank_module_num: u32,

    instances: Box<Vec<TankInstance>>,
    update_chan: Receiver<Box<Vec<TankInstance>>>,
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
    position: [f32; 3],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        wgpu::VertexBufferDescriptor {
            stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float3],
        }
    }
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
        // Create render objects
        const A: f32 = 0.2;
        const B: f32 = 0.25;
        const VERTICES: &[Vertex] = &[
            Vertex {
                position: [-A, -B, 0.0],
            },
            Vertex {
                position: [A, -B, 0.0],
            },
            Vertex {
                position: [A, B, 0.0],
            },
            Vertex {
                position: [-A, -B, 0.0],
            },
            Vertex {
                position: [A, B, 0.0],
            },
            Vertex {
                position: [-A, B, 0.0],
            },
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
                    vertex_buffers: &[Vertex::desc()],
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false,
            })
        };

        // Start physic emulation
        let (update_sender, r) = bounded(0);
        thread::spawn(move || {
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
                let update_data = physical
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
                    .collect::<Vec<TankInstance>>();
                loop {
                    crossbeam_channel::select! {
                        recv(ticker) -> _ => continue 'next_update,
                        send(update_sender, Box::new(update_data)) -> _ => break,
                    }
                }
                loop {
                    crossbeam_channel::select! {
                        recv(ticker) -> _ => continue 'next_update,
                    }
                }
            }
        });

        GameScene {
            clean_color,
            render_pipeline,
            tank_module_buffer,
            tank_module_num: VERTICES.len() as u32,
            instances: Box::new(Vec::new()),
            update_chan: r,
        }
    }

    /* /// Add a tank to this game scene. Controlled by the controller.
    // pub fn add_tank(&self, controller: Controller) {
    //     let right_body = RigidBodyBuilder::new_dynamic()
    //         .can_sleep(true)
    //         .mass(1.0, true)
    //         .linear_damping(10.0)
    //         .principal_angular_inertia(1.0, true)
    //         .angular_damping(5.0)
    //         .build();
    //     let collider = ColliderBuilder::cuboid(0.2, 0.25).build();
    //     let physical = &mut *self.physical.lock().unwrap();
    //     let rigid_body_handle = physical.rigid_body_set.insert(right_body);
    //     let collider_handle =
    //         physical
    //             .collider_set
    //             .insert(collider, rigid_body_handle, &mut physical.rigid_body_set);
    //
    //     self.tanks.lock().unwrap().push(Tank {
    //         controller,
    //         rigid_body_handle,
    //         collider_handle,
    //     });
    // }*/
}

impl Scene for GameScene {
    fn render(&mut self, device: &wgpu::Device, frame: &wgpu::SwapChainTexture) -> CommandBuffer {
        self.update_chan.try_recv(); // Update data from physical thread
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
            render_pass.draw(0..self.tank_module_num, 0..(self.instances.len() as u32));
        }
        encoder.finish()
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
