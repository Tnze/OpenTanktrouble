use crate::maze::Controller;
use winit::event::{KeyboardInput, VirtualKeyCode, ElementState, ScanCode};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::borrow::Borrow;

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
            key_map: HashMap::new()
        }
    }
    pub fn input_event(&mut self, e: &KeyboardInput) {
        match e {
            KeyboardInput {
                scancode,
                virtual_keycode,
                state, ..
            } => {
                self.key_map.insert(match virtual_keycode {
                    Some(vk) => Key::LogicKey(*vk),
                    None => Key::PhysicKey(*scancode),
                }, *state)
            }
        };
    }
}

impl KeyboardController {
    pub fn create_sub_controller(parent: &Rc<RefCell<KeyboardController>>, keys: [Key; 4]) -> SubKeyboardController {
        SubKeyboardController {
            keys,
            parent: Rc::clone(parent),
        }
    }
}

pub struct SubKeyboardController {
    keys: [Key; 4],
    parent: Rc<RefCell<KeyboardController>>,
}

impl SubKeyboardController {}

#[inline]
fn pressed_or_default<T>(state: Option<&ElementState>, pressed: T, otherwise: T) -> T {
    match state {
        Some(ElementState::Pressed) => pressed,
        _ => otherwise,
    }
}

impl Controller for SubKeyboardController {
    fn status(&self) -> (f64, f64) {
        let parent = &self.parent.borrow_mut().key_map;
        (
            0.0 + pressed_or_default(parent.get(&self.keys[2]), 1.0, 0.0)
                - pressed_or_default(parent.get(&self.keys[3]), 1.0, 0.0),
            0.0 + pressed_or_default(parent.get(&self.keys[0]), 1.0, 0.0)
                - pressed_or_default(parent.get(&self.keys[1]), 0.6, 0.0),
        )
    }
}