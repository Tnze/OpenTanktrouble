use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use winit::event::{ElementState, KeyboardInput, ScanCode, VirtualKeyCode};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Key {
    LogicKey(VirtualKeyCode),
    PhysicKey(ScanCode),
}

pub struct KeyboardController {
    key_map: HashMap<Key, ElementState>,
}

impl KeyboardController {
    pub fn new() -> KeyboardController {
        KeyboardController {
            key_map: HashMap::new(),
        }
    }
    pub fn input_event(&mut self, e: &KeyboardInput) {
        match e {
            KeyboardInput {
                scancode,
                virtual_keycode,
                state,
                ..
            } => {
                self.key_map.insert(Key::PhysicKey(*scancode), *state);
                if let Some(code) = virtual_keycode {
                    self.key_map.insert(Key::LogicKey(*code), *state);
                }
            }
        };
    }
}

impl KeyboardController {
    pub fn create_sub_controller(
        parent: &Arc<Mutex<KeyboardController>>,
        movement_keys: [Key; 4],
    ) -> SubKeyboardController {
        SubKeyboardController {
            movement_keys,
            parent: Arc::clone(parent),
        }
    }
}

pub struct SubKeyboardController {
    movement_keys: [Key; 4],
    parent: Arc<Mutex<KeyboardController>>,
}

impl SubKeyboardController {
    pub(crate) fn movement_status(&self) -> (f32, f32) {
        let parent = &self.parent.lock().unwrap().key_map;
        let get_value = |key, pressed| match parent.get(&self.movement_keys[key]) {
            Some(ElementState::Pressed) => pressed,
            _ => 0.0,
        };
        (
            get_value(2, 1.0) - get_value(3, 1.0),
            get_value(0, 1.0) - get_value(1, 0.6),
        )
    }
}
