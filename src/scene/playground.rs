use std::{
    sync::{Arc, Mutex},
    thread, time,
};

use crossbeam_channel::{bounded, Receiver, Sender};
use rapier2d::{
    dynamics::{IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle, RigidBodySet},
    geometry::{BroadPhase, ColliderBuilder, ColliderHandle, ColliderSet, NarrowPhase},
    na::{Matrix3, Matrix4, Rotation2, Vector2},
    pipeline::PhysicsPipeline,
};
use wgpu::CommandBuffer;
use winit::dpi::PhysicalSize;

use crate::input::Controller::{self, Gamepad, Keyboard};

trait Scene {
    fn render(&mut self) -> wgpu::CommandBuffer;
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>);
}

pub struct GameScene {
    clean_color: wgpu::Color,
    render_pipeline: wgpu::RenderPipeline,
    update_chan: Receiver<Vector2<f32>>,
}

struct PhysicalStatus {
    seq_number: u32,

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
            attributes: &[wgpu::VertexAttributeDescriptor {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float3,
            }],
        }
    }
}

impl GameScene {
    pub fn new(device: wgpu::Device) -> GameScene {
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

        let render_pipeline = {
            let vs_module = device.create_shader_module(wgpu::include_spirv!("shader.vert.spv"));
            let fs_module = device.create_shader_module(wgpu::include_spirv!("shader.frag.spv"));

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
        let (s, r) = bounded(0);
        thread::spawn(move || Self::run_physic(s));

        GameScene {
            clean_color,
            render_pipeline,
            update_chan: r,
        }
    }

    /// Add a tank to this game scene. Controlled by the controller.
    pub fn add_tank(&self, controller: Controller) {
        let right_body = RigidBodyBuilder::new_dynamic()
            .can_sleep(true)
            .mass(1.0, true)
            .linear_damping(10.0)
            .principal_angular_inertia(1.0, true)
            .angular_damping(5.0)
            .build();
        let collider = ColliderBuilder::cuboid(0.2, 0.25).build();
        let physical = &mut *self.physical.lock().unwrap();
        let rigid_body_handle = physical.rigid_body_set.insert(right_body);
        let collider_handle =
            physical
                .collider_set
                .insert(collider, rigid_body_handle, &mut physical.rigid_body_set);

        self.tanks.lock().unwrap().push(Tank {
            controller,
            rigid_body_handle,
            collider_handle,
        });
    }

    fn run_physic(update_chan: Sender<Vector2<f32>>) -> ! {
        let mut physical = PhysicalStatus {
            seq_number: 0,
            integration_parameters: IntegrationParameters::default(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            joint_set: JointSet::new(),
        };
        let start_time = time::Instant::now();
        let mut pipeline = PhysicsPipeline::new();
        let dt = time::Duration::from_secs_f32(physical.integration_parameters.dt());
        let gravity = Vector2::new(0.0, 0.0);
        loop {
            {
                let tanks = &mut *self.tanks.lock().unwrap();
                // Apply the control to the tank.
                for tank in tanks.iter() {
                    let (rot, acl) = match &tank.controller {
                        Gamepad(c) => c.movement_status(),
                        Keyboard(c) => c.movement_status(),
                    };
                    let right_body = &mut physical.rigid_body_set[tank.rigid_body_handle];

                    let rotation = &Rotation2::from(right_body.position().rotation);

                    right_body.apply_force(rotation * Vector2::new(0.0, acl * -15.0), true);
                    right_body.apply_torque(rot * 20.0, true);
                }
            }
            pipeline.step(
                &gravity,
                &physical.integration_parameters,
                &mut physical.broad_phase,
                &mut physical.narrow_phase,
                &mut physical.rigid_body_set,
                &mut physical.collider_set,
                &mut physical.joint_set,
                None,
                None,
                &(),
            );
            // Increase simulate sequence number.
            physical.seq_number += 1;
            // Send update and sleep
            update_chan.send_deadline(
                Vector2::new(0.0, 0.0),
                start_time + (dt * physical.seq_number),
            );
            if let Some(d) = (dt * physical.seq_number).checked_sub(start_time.elapsed()) {
                thread::sleep(d);
            }
        }
    }
}

impl Scene for GameScene {
    fn render(&mut self) -> CommandBuffer {
        self.update_chan.try_recv(); // Update data from physical thread
        unimplemented!()
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.size = new_size;
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
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
