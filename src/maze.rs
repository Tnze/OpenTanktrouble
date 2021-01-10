use rapier2d::{
    dynamics::{
        BodyStatus, IntegrationParameters, JointSet, RigidBodyBuilder, RigidBodyHandle,
        RigidBodySet,
    },
    geometry::{BroadPhase, ColliderSet, NarrowPhase},
    na::{Vector2, Vector},
    pipeline::PhysicsPipeline,
};
use std::sync::{Arc, Mutex};
use std::{thread, time};
use std::time::Duration;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DrawError, DynamicState};
use vulkano::command_buffer::pool::standard::StandardCommandPoolBuilder;
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::pipeline::vertex::VertexSource;
use vulkano::descriptor::descriptor_set::DescriptorSetsCollection;

pub struct GameScene {
    tanks: Vec<Tank>,
    physical: Arc<Mutex<PhysicalStatus>>,
    render: RenderObjects,
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

struct RenderObjects {}

impl GameScene {
    pub fn create() -> GameScene {
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
            render: RenderObjects {},
        }
    }

    /// 添加一个控制器
    pub fn add_tank(&mut self, controller: Box<dyn Controller + Send + Sync>) {
        let right_body = RigidBodyBuilder::new(BodyStatus::Dynamic)
            .can_sleep(true)
            .build();
        let physical_handle = self.physical.lock().unwrap().rigid_body_set.insert(right_body);
        self.tanks.push(Tank {
            controller,
            physical_handle,
        });
    }

    pub fn run_physic(&self) {
        let start_time = time::Instant::now();
        let mut pipeline = PhysicsPipeline::new();
        let dt = time::Duration::from_secs_f32(self.physical.lock().unwrap().integration_parameters.dt());
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

    pub fn draw<'a, V, S, Gp>(
        &self,
        builder: &'a mut AutoCommandBufferBuilder,
        pipeline: Gp,
        dynamic_state: &DynamicState,
        vertex_buffer: V,
        sets: S,
    ) -> Result<&'a mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, DrawError>
        where Gp: GraphicsPipelineAbstract + VertexSource<V> + Send + Sync + 'static + Clone,
              S: DescriptorSetsCollection
    {
        builder.draw(
            pipeline,
            &dynamic_state,
            vertex_buffer,
            sets,
            (),
        )
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
