use rapier2d::{
    na::Vector2,
    dynamics::{JointSet, RigidBodySet, IntegrationParameters,
               RigidBodyBuilder, BodyStatus, RigidBodyHandle},
    geometry::{BroadPhase, NarrowPhase, ColliderSet},
    pipeline::PhysicsPipeline,
};
use std::time::Duration;
use std::thread;
use std::sync::{Mutex, Arc};

pub struct GameScene {
    /// 模拟顺序号
    ///
    simulate_seq_num: i32,
    tanks: Vec<Tank>,
    physical: Arc<Mutex<(
        IntegrationParameters,
        BroadPhase,
        NarrowPhase,
        RigidBodySet,
        ColliderSet,
        JointSet,
    )>>,
}


impl GameScene {
    pub fn create() -> GameScene {
        GameScene {
            simulate_seq_num: 0,
            tanks: Vec::new(),
            physical: Arc::new(Mutex::new((
                IntegrationParameters::default(),
                BroadPhase::new(),
                NarrowPhase::new(),
                RigidBodySet::new(),
                ColliderSet::new(),
                JointSet::new(),
            ))),
        }
    }

    /// 添加一个控制器
    pub fn add_tank(&mut self, controller: Box<dyn Controller + Send>) {
        let right_body = RigidBodyBuilder::new(BodyStatus::Dynamic)
            .can_sleep(true)
            .build();
        let physical_handle = self.physical.lock().unwrap().3.insert(right_body);
        self.tanks.push(Tank { controller, physical_handle });
    }

    pub fn run(mut self) {
        let mut pipeline = PhysicsPipeline::new();
        let gravity = Vector2::new(0.0, 0.0);
        loop {
            self.simulate_seq_num += 1;
            self.update_tank();

            let physical = &mut *self.physical.lock().unwrap();
            pipeline.step(
                &gravity,
                &physical.0,
                &mut physical.1,
                &mut physical.2,
                &mut physical.3,
                &mut physical.4,
                &mut physical.5,
                None,
                None,
                &(),
            );

            thread::sleep(Duration::from_micros(1000 / 60));
        }
    }

    fn update_tank(&mut self) {
        for tank in self.tanks.iter() {
            let (rot, acl) = tank.controller.status();
            let mut right_body = &self.physical.lock().unwrap().3[tank.physical_handle];
            //println!("{}", right_body.linvel())
        }
    }
}


/// 控制器代表用于操控一辆坦克的对象，可以是一个手柄或者一个键盘，甚至一个A.I.。
pub trait Controller {
    /// 用于查询当前该控制器的输入状态
    /// 包括一个指定旋转操作的浮点数，以及一个指定前进、后退操作的浮点数
    /// 两者的取值范围都在[-1.0 .. 1.0]之间
    fn status(&self) -> (f64, f64);
}

struct Tank {
    controller: Box<dyn Controller + Send>,
    physical_handle: RigidBodyHandle,
}