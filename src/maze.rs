use rapier2d::{
    dynamics::{
        BodyStatus, IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle,
        RigidBodySet,
    },
    geometry::{BroadPhase, ColliderSet, NarrowPhase},
    na::{Vector, Vector2},
    pipeline::PhysicsPipeline,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time};
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool};
use vulkano::command_buffer::pool::standard::StandardCommandPoolBuilder;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DrawError, DynamicState};
use vulkano::descriptor::descriptor_set::{DescriptorSetsCollection, PersistentDescriptorSet};
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::framebuffer::{Subpass, RenderPass, RenderPassDesc, RenderPassAbstract};
use vulkano::pipeline::vertex::{VertexSource, SingleBufferDefinition};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::SafeDeref;

pub struct GameScene<RenderP>
    where RenderP: RenderPassAbstract + Send + Sync {
    tanks: Vec<Tank>,
    physical: Arc<Mutex<PhysicalStatus>>,
    pub(crate) render: RenderObjects<Box<dyn PipelineLayoutAbstract + Send + Sync>, Arc<RenderP>>,
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

struct RenderObjects<Layout, RenderP>
    where RenderP: RenderPassAbstract + Send + Sync,
          Layout: PipelineLayoutAbstract + Send + Sync {
    pub(crate) dynamic_state: DynamicState,
    pub(crate) pipeline: Arc<GraphicsPipeline<SingleBufferDefinition<Vertex>, Layout, RenderP>>,
    pub(crate) render_pass: RenderP,
    uniform_buffer: CpuBufferPool::<vs::ty::Data>,
    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
				#version 450 core
				layout(set = 0, binding = 0) uniform Data {
                    vec2 trans;
                } uniforms;
				layout(location = 0) in vec2 position;

				void main() {
					gl_Position = vec4(position + uniforms.trans, 0.0, 1.0);
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
					f_color = vec4(1.0, 0.0, 0.0, 1.0);
				}
			"
    }
}

#[derive(Default, Copy, Clone)]
struct Vertex {
    position: (f32, f32),
}
vulkano::impl_vertex!(Vertex, position);

impl<RenderP> GameScene<RenderP>
    where RenderP: PipelineLayoutAbstract + Send + Sync {
    pub fn create(device: Arc<Device>, format: Format) -> GameScene<RenderP> {
        let vs = vs::Shader::load(Arc::clone(&device)).unwrap();
        let fs = fs::Shader::load(Arc::clone(&device)).unwrap();

        let render_pass = Arc::new(
            vulkano::single_pass_renderpass!(
                device,
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
            ).unwrap(),
        );
        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_input_single_buffer()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                .build(device.clone())
                .unwrap(),
        );
        let layout = pipeline.layout();

        let (top, left, width, height) = (0.3, -0.7, 0.4, 0.2);
        let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(device.clone(), BufferUsage::all());
        let mut dynamic_state = DynamicState {
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
                    Vertex { position: (left, top) },
                    Vertex { position: (left + width, top) },
                    Vertex { position: (left + width, top + height) },
                    Vertex { position: (left, top) },
                    Vertex { position: (left + width, top + height) },
                    Vertex { position: (left, top + height) },
                ]
                    .iter()
                    .cloned(),
            )
                .unwrap()
        };

        GameScene {
            tanks: Vec::new(),
            physical: Arc::new(Mutex::new(PhysicalStatus {
                seq_number: 0,
                integration_parameters: IntegrationParameters::default(),
                broad_phase: BroadPhase::new(),
                narrow_phase: NarrowPhase::new(),
                rigid_body_set: RigidBodySet::new(),
                collider_set: ColliderSet::new(),
                joint_set: JointSet::new(),
            })),
            render: RenderObjects {
                dynamic_state,
                pipeline,
                uniform_buffer,
                render_pass,
                vertex_buffer,
            },
        }
    }

    /// 添加一个控制器
    pub fn add_tank(&mut self, controller: Box<dyn Controller + Send + Sync>) {
        let right_body = RigidBodyBuilder::new(BodyStatus::Dynamic)
            .can_sleep(true)
            .build();
        let physical_handle = self
            .physical
            .lock()
            .unwrap()
            .rigid_body_set
            .insert(right_body);
        self.tanks.push(Tank {
            controller,
            physical_handle,
        });
    }

    pub fn run_physic(&self) {
        let start_time = time::Instant::now();
        let mut pipeline = PhysicsPipeline::new();
        let dt = time::Duration::from_secs_f32(
            self.physical.lock().unwrap().integration_parameters.dt(),
        );
        let gravity = Vector2::new(0.0, 0.0);
        loop {
            let physical = &mut *self.physical.lock().unwrap();
            for tank in self.tanks.iter() {
                let (rot, acl) = tank.controller.movement_status();
                let right_body = &mut physical.rigid_body_set[tank.physical_handle];

                right_body.set_linvel(Vector2::new(0.0, 0.0), false);
                right_body.set_angvel(rot as f32, false);
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
                thread::sleep(d);
            }
        }
    }

    pub fn draw<'a>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError> {
        let uniform_buffer_subbuffer = {
            // let elapsed = time_start.elapsed();
            // let elapsed =
            //     elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 / 1_000_000_000.0;
            // let trans = Vector2::new(0.5 + elapsed.sin() * 0.5, -0.5 + elapsed.cos() * 0.5);
            let trans = Vector2::new(0.0, 0.0);
            let uniform_data = vs::ty::Data {
                trans: trans.into(),
            };
            self.render.uniform_buffer.next(uniform_data).unwrap()
        };
        let layout = self.render.pipeline.layout().descriptor_set_layout(0).unwrap();
        let set = Arc::new(
            PersistentDescriptorSet::start(layout.clone())
                .add_buffer(uniform_buffer_subbuffer)
                .unwrap()
                .build()
                .unwrap(),
        );
        builder.draw(&self.render.pipeline, &self.render.dynamic_state, &self.render.vertex_buffer, set, ())
    }
}

/// 控制器代表用于操控一辆坦克的对象，可以是一个手柄或者一个键盘，甚至一个A.I.。
pub trait Controller {
    /// 用于查询当前该控制器的输入状态
    /// 包括一个指定旋转操作的浮点数，以及一个指定前进、后退操作的浮点数
    /// 两者的取值范围都在[-1.0 .. 1.0]之间
    fn movement_status(&self) -> (f64, f64);
}

struct Tank {
    controller: Box<dyn Controller + Send + Sync>,
    physical_handle: RigidBodyHandle,
}
