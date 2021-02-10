pub mod gamepad_controller;
mod input_status;
pub mod keyboard_controller;

/// 控制器代表用于操控一辆坦克的对象，可以是一个手柄或者一个键盘，甚至一个A.I.。
/// 一般拥有一个movement_status方法用于查询当前该控制器的输入状态
/// 包括一个指定旋转操作的浮点数，以及一个指定前进、后退操作的浮点数
/// 两者的取值范围都在[-1.0 .. 1.0]之间
pub trait Controller: Sync + Send {
    fn movement_status(&self) -> (f32, f32);
}
