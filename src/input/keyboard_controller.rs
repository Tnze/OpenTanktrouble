use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use winit::event::{ElementState, KeyboardInput, ScanCode, VirtualKeyCode};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Key {
    LogicKey(VirtualKeyCode),
    PhysicKey(ScanCode),
}

pub struct Keyboard {
    key_map: HashMap<Key, ElementState>,
}

impl Keyboard {
    pub fn new() -> Keyboard {
        Keyboard {
            key_map: HashMap::new(),
        }
    }
    pub fn input_event(&mut self, e: &KeyboardInput) {
        let KeyboardInput {
            scancode,
            virtual_keycode,
            state,
            ..
        } = e;
        self.key_map.insert(Key::PhysicKey(*scancode), *state);
        if let Some(code) = virtual_keycode {
            self.key_map.insert(Key::LogicKey(*code), *state);
        }
    }
}

impl Keyboard {
    pub fn create_sub_controller(
        parent: &Arc<Mutex<Keyboard>>,
        movement_keys: [Key; 4],
    ) -> Controller {
        Controller {
            movement_keys,
            parent: Arc::clone(parent),
        }
    }
}

pub struct Controller {
    movement_keys: [Key; 4],
    parent: Arc<Mutex<Keyboard>>,
}

impl Controller {
    pub(crate) fn movement_status(&self) -> (f32, f32) {
        let parent = &self.parent.lock().unwrap().key_map;
        let get_value = |key, pressed| match parent.get(&self.movement_keys[key]) {
            Some(ElementState::Pressed) => pressed,
            _ => 0.0,
        };
        (
            get_value(3, 1.0) - get_value(2, 1.0),
            get_value(0, 1.0) - get_value(1, 0.6),
        )
    }
}
