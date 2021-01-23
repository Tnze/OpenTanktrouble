use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use winit::event::{ElementState, KeyboardInput, ScanCode, VirtualKeyCode};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Key {
    LogicKey(VirtualKeyCode),
    PhysicKey(ScanCode),
}

pub struct Keyboard {
    key_map: Arc<Mutex<HashMap<Key, ElementState>>>,
}

impl Keyboard {
    pub fn new() -> Keyboard {
        Keyboard {
            key_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn input_event(&self, e: &KeyboardInput) {
        let KeyboardInput {
            scancode,
            virtual_keycode,
            state,
            ..
        } = e;
        let key_map = &mut *self.key_map.lock().unwrap();
        key_map.insert(Key::PhysicKey(*scancode), *state);
        if let Some(code) = virtual_keycode {
            key_map.insert(Key::LogicKey(*code), *state);
        }
    }
}

impl Keyboard {
    pub fn create_sub_controller(&self, movement_keys: [Key; 4]) -> Controller {
        Controller {
            movement_keys,
            key_map: self.key_map.clone(),
        }
    }
}

pub struct Controller {
    movement_keys: [Key; 4],
    key_map: Arc<Mutex<HashMap<Key, ElementState>>>,
}

impl Controller {
    pub(crate) fn movement_status(&self) -> (f32, f32) {
        let key_map = &*self.key_map.lock().unwrap();
        let get_value = |key, pressed| match key_map.get(&self.movement_keys[key]) {
            Some(ElementState::Pressed) => pressed,
            _ => 0.0,
        };
        (
            get_value(3, 1.0) - get_value(2, 1.0),
            get_value(0, 1.0) - get_value(1, 0.6),
        )
    }
}
