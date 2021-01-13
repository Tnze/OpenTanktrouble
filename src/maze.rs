use std::{
    sync::{Arc, Mutex},
    thread, time,
};

use gilrs::Gilrs;
use rapier2d::{
    dynamics::{IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle, RigidBodySet},
    geometry::{BroadPhase, ColliderSet, NarrowPhase},
    na::{Isometry2, Matrix4, Vector2},
    pipeline::PhysicsPipeline,
};
use rapier2d::na::Rotation2;
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
    pipeline::{GraphicsPipeline, GraphicsPipelineAbstract},
};

use crate::gamepad_controller::GamepadController;
use crate::keyboard_controller::SubKeyboardController;
use crate::maze::Controller::{Gamepad, Keyboard};

pub struct GameScene {
    tanks: Mutex<Vec<Tank>>,
    physical: Mutex<PhysicalStatus>,
    pub(crate) render: Mutex<RenderObjects>,
}

struct PhysicalStatus {
    /// 物理模拟顺序号
    seq_number: u32,

    integration_parameters: IntegrationParameters,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    joint_set: JointSet,
}

pub struct RenderObjects {
    pub(crate) dynamic_state: DynamicState,
    pub(crate) pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub(crate) render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    uniform_buffer: CpuBufferPool<vs::ty::Data>,
    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
				#version 450 core
				layout(set = 0, binding = 0) uniform Data {
                    mat4 trans;
                    vec2 pos;
                } uniforms;
				layout(location = 0) in vec2 position;

				void main() {
				    mat4 t = uniforms.trans;
				    vec3 v = mat3(
				        t[0][0], t[0][1], t[0][2],
				        t[1][0], t[1][1], t[1][2],
				        t[2][0], t[2][1], t[2][2]
				    ) * vec3(position, 1.0);
				    vec2 p = vec2(v[0]/v[2], v[1]/v[2]) + uniforms.pos;
					gl_Position = vec4(p, 0.0, 1.0);
				}
			"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
				#version 450 core
				layout(location = 0) out vec4 f_color;
				void main() {
					f_color = vec4(0.0, 0.0, 0.0, 1.0);
				}
			"
    }
}

#[derive(Default, Copy, Clone)]
pub struct Vertex {
    position: (f32, f32),
}
vulkano::impl_vertex!(Vertex, position);

impl GameScene {
    pub fn create(device: Arc<Device>, format: Format) -> GameScene {
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
                        // TODO:
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

        let (top, left, width, height) = (-0.3, -0.3, 0.6, 0.6);
        let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(device.clone(), BufferUsage::all());
        let dynamic_state = DynamicState {
            line_width: None,
            viewports: None,
            scissors: None,
            compare_mask: None,
            write_mask: None,
            reference: None,
        };

        let vertex_buffer = {
            CpuAccessibleBuffer::from_iter(
                device.clone(),
                BufferUsage::all(),
                false,
                [
                    Vertex {
                        position: (left, top),
                    },
                    Vertex {
                        position: (left + width, top),
                    },
                    Vertex {
                        position: (left + width, top + height),
                    },
                    Vertex {
                        position: (left, top),
                    },
                    Vertex {
                        position: (left + width, top + height),
                    },
                    Vertex {
                        position: (left, top + height),
                    },
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
                uniform_buffer,
                render_pass,
                vertex_buffer,
            }),
        }
    }

    /// 添加一个控制器
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

    pub fn run_physic(&self) {
        let mut gilrs = Gilrs::new().unwrap();

        let start_time = time::Instant::now();
        let mut pipeline = PhysicsPipeline::new();
        let dt = time::Duration::from_secs_f32(
            self.physical.lock().unwrap().integration_parameters.dt(),
        );
        let gravity = Vector2::new(0.0, 0.0);
        loop {
            let mut physical_mg = self.physical.lock().unwrap();
            let physical = &mut *physical_mg;
            // update controller when gamepad eventevent
            while let Some(gilrs::Event { id, .. }) = gilrs.next_event() {
                self.tanks.lock().unwrap()[0].controller =
                    Gamepad(GamepadController::create_gamepad_controller(id));
            }
            for tank in self.tanks.lock().unwrap().iter() {
                let (rot, acl) = match &tank.controller {
                    Gamepad(c) => c.movement_status(&gilrs),
                    Keyboard(c) => c.movement_status(),
                };
                let right_body = &mut physical.rigid_body_set[tank.physical_handle];
                let agl = right_body.position().rotation;
                right_body.apply_force(Rotation2::from(agl) * Vector2::new(0.0, acl * -10.0), true);
                right_body.apply_torque(rot * -15.0, true);
                // right_body.set_position(Isometry2::new(, rot), true);
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
            physical.seq_number += 1; // 模拟顺序号+1

            if let Some(d) = (dt * physical.seq_number).checked_sub(start_time.elapsed()) {
                drop(physical_mg);
                thread::sleep(d);
            }
        }
    }

    pub fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        let render = &mut *self.render.lock().unwrap();
        let physical = &mut *self.physical.lock().unwrap();
        for tank in self.tanks.lock().unwrap().iter() {
            let uniform_buffer_subbuffer = {
                let tank_body = physical
                    .rigid_body_set
                    .get(tank.physical_handle)
                    .expect("Used an invalid rigid body handler");
                let pos = tank_body.position();
                // dbg!(trans);
                let trans = pos.rotation.to_homogeneous();
                let uniform_data = vs::ty::Data {
                    trans: Matrix4::new(
                        trans.m11, trans.m12, trans.m13, 0f32, trans.m21, trans.m22, trans.m23,
                        0f32, trans.m31, trans.m32, trans.m33, 0f32, 0f32, 0f32, 0f32, 1f32,
                    )
                        .into(),
                    pos: Vector2::new(pos.translation.vector[0], pos.translation.vector[1]).into(),
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

/// 控制器代表用于操控一辆坦克的对象，可以是一个手柄或者一个键盘，甚至一个A.I.。
/// 一般拥有一个movement_status方法用于查询当前该控制器的输入状态
/// 包括一个指定旋转操作的浮点数，以及一个指定前进、后退操作的浮点数
/// 两者的取值范围都在[-1.0 .. 1.0]之间
pub enum Controller {
    Gamepad(GamepadController),
    Keyboard(SubKeyboardController),
}

struct Tank {
    controller: Controller,
    physical_handle: RigidBodyHandle,
}
