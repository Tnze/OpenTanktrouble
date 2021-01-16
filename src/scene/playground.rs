use std::{
    sync::{Arc, Mutex},
    thread, time,
};

use rapier2d::{
    dynamics::{IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle, RigidBodySet},
    geometry::{BroadPhase, ColliderSet, NarrowPhase},
    na::{Matrix4, Rotation2, Vector2},
    pipeline::PhysicsPipeline,
};
use rapier2d::na::Matrix3;
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool},
    command_buffer::{
        AutoCommandBufferBuilder, DrawError, DynamicState,
        pool::standard::StandardCommandPoolBuilder,
    },
    descriptor::{descriptor_set::PersistentDescriptorSet, PipelineLayoutAbstract},
    device::Device,
    format::Format,
    framebuffer::{RenderPassAbstract, Subpass},
    pipeline::{GraphicsPipeline, GraphicsPipelineAbstract, viewport::Viewport},
};

use crate::input::Controller::{self, Gamepad, Keyboard};

use super::user_interface::{Element as UIElement, Scene as UIScene};

pub struct GameScene {
    tanks: Mutex<Vec<Tank>>,
    physical: Mutex<PhysicalStatus>,
    render: Mutex<RenderObjects>,
}

struct Tank {
    controller: Controller,
    physical_handle: RigidBodyHandle,
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

pub struct RenderObjects {
    pub dynamic_state: DynamicState,
    pub pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    uniform_buffer: CpuBufferPool<vs::ty::Data>,
    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
}

mod vs {
    vulkano_shaders::shader! { ty: "vertex", path: "src/scene/shaders/tank.vert" }
}

mod fs {
    vulkano_shaders::shader! { ty: "fragment", path: "src/scene/shaders/tank.frag" }
}

#[derive(Default, Copy, Clone)]
pub struct Vertex {
    position: (f32, f32),
}
vulkano::impl_vertex!(Vertex, position);

impl GameScene {
    pub fn new(device: Arc<Device>, format: Format) -> GameScene {
        let vs = vs::Shader::load(Arc::clone(&device)).unwrap();
        let fs = fs::Shader::load(Arc::clone(&device)).unwrap();

        let render_pass = Arc::new(Box::new(
            vulkano::single_pass_renderpass!(
                device.clone(),
                attachments: {
                    color: {
                        load: Clear,
                        store: Store,
                        format: format,
                        samples: 1,
                    }
                },
                pass: {color: [color],  depth_stencil: {}}
            )
                .unwrap(),
        ));
        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_input_single_buffer::<Vertex>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                .build(device.clone())
                .unwrap(),
        );

        let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(device.clone(), BufferUsage::all());
        let dynamic_state = DynamicState {
            line_width: None,
            viewports: None,
            scissors: None,
            compare_mask: None,
            write_mask: None,
            reference: None,
        };
        let (a, b) = (0.2, 0.25);

        let vertex_buffer = {
            CpuAccessibleBuffer::from_iter(
                device.clone(),
                BufferUsage::all(),
                false,
                [
                    Vertex { position: (-a, -b) },
                    Vertex { position: (a, -b) },
                    Vertex { position: (a, b) },
                    Vertex { position: (-a, -b) },
                    Vertex { position: (a, b) },
                    Vertex { position: (-a, b) },
                ]
                    .iter()
                    .cloned(),
            )
                .unwrap()
        };

        GameScene {
            tanks: Mutex::new(Vec::new()),
            physical: Mutex::new(PhysicalStatus {
                seq_number: 0,
                integration_parameters: IntegrationParameters::default(),
                broad_phase: BroadPhase::new(),
                narrow_phase: NarrowPhase::new(),
                rigid_body_set: RigidBodySet::new(),
                collider_set: ColliderSet::new(),
                joint_set: JointSet::new(),
            }),
            render: Mutex::new(RenderObjects {
                dynamic_state,
                pipeline,
                render_pass,
                uniform_buffer,
                vertex_buffer,
            }),
        }
    }

    /// Add a tank to this game scene. Controlled by the controller.
    pub fn add_tank(&self, controller: Controller) {
        let right_body = RigidBodyBuilder::new_dynamic()
            .can_sleep(true)
            .mass(1.0, true)
            .linear_damping(10.0)
            .principal_angular_inertia(1.0, true)
            .angular_damping(10.0)
            .build();
        let physical_handle = self
            .physical
            .lock()
            .unwrap()
            .rigid_body_set
            .insert(right_body);
        self.tanks.lock().unwrap().push(Tank {
            controller,
            physical_handle,
        });
    }

    pub fn run_physic(&self) -> ! {
        let start_time = time::Instant::now();
        let mut pipeline = PhysicsPipeline::new();
        let dt = time::Duration::from_secs_f32(
            self.physical.lock().unwrap().integration_parameters.dt(),
        );
        let gravity = Vector2::new(0.0, 0.0);
        loop {
            let sleep_time = {
                let physical = &mut *self.physical.lock().unwrap();
                {
                    let tanks = &mut *self.tanks.lock().unwrap();
                    // Apply the control to the tank.
                    for tank in tanks.iter() {
                        let (rot, acl) = match &tank.controller {
                            Gamepad(c) => c.movement_status(),
                            Keyboard(c) => c.movement_status(),
                        };
                        let right_body = &mut physical.rigid_body_set[tank.physical_handle];

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
                // Calculate sleep time.
                (dt * physical.seq_number).checked_sub(start_time.elapsed())
            };
            if let Some(d) = sleep_time {
                thread::sleep(d);
            }
        }
    }
}

impl UIScene for Arc<GameScene> {
    fn render_pass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.render.lock().unwrap().render_pass.clone()
    }

    fn reset_viewport(&self, dimension: [f32; 2]) {
        self.render.lock().unwrap().dynamic_state.viewports = Some(vec![Viewport {
            origin: [0.0, 0.0],
            dimensions: dimension,
            depth_range: 0.0..1.0,
        }]);
    }
}

impl UIElement for Arc<GameScene> {
    fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
        dimensions: [f32; 2],
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        let render = &mut *self.render.lock().unwrap();
        let physical = &mut *self.physical.lock().unwrap();
        for tank in self.tanks.lock().unwrap().iter() {
            let uniform_buffer_subbuffer = {
                let tank_body = physical
                    .rigid_body_set
                    .get(tank.physical_handle)
                    .expect("Used an invalid rigid body handler");
                let loc = tank_body.position().to_homogeneous();
                let proj = projection(&dimensions, 1.0 / 3.0);
                let trans: Matrix4<_> = (proj * loc).fixed_resize(0.0);
                let uniform_data = vs::ty::Data {
                    trans: trans.into(),
                };
                render
                    .uniform_buffer
                    .next(uniform_data)
                    .expect("GPU memory is not enough")
            };
            let layout = render.pipeline.descriptor_set_layout(0).unwrap();
            let set = Arc::new(
                PersistentDescriptorSet::start(layout.clone())
                    .add_buffer(uniform_buffer_subbuffer)
                    .unwrap()
                    .build()
                    .unwrap(),
            );
            builder.draw(
                render.pipeline.clone(),
                &render.dynamic_state,
                vec![render.vertex_buffer.clone()],
                set,
                (),
            )?;
        }
        Ok(builder)
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
