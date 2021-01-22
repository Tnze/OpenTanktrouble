use std::{
    sync::{Arc, Mutex},
    thread,
};
use std::error::Error;

use futures::executor::block_on;
use gilrs::EventType;
use log::{debug, error, info, Level, log_enabled};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowBuilder},
};

use crate::input::{
    gamepad_controller::{Controller, Gamepad},
    keyboard_controller::{Key::LogicKey, Keyboard},
};
use crate::scene::playground::GameScene;

mod input;
mod scene;
mod window;

fn abort<E: Error>(err: E) -> ! {
    error!("Error in main: {}", err);
    msgbox::create("Error", &*err.to_string(), msgbox::IconType::Error);
    panic!(err)
}

fn main() {
    // Init logger
    env_logger::init();

    // Create window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .unwrap_or_else(abort);
    let mut state = block_on(window::WindowState::new(&window)).unwrap_or_else(abort);

    // Init controller
    let keyboard_controller = Keyboard::new();
    let mut gamepad_controller = Gamepad::new();

    event_loop.run(move |event, _, control_flow| {
        while let Some(e) = gamepad_controller.next() {
            if let gilrs::Event {
                id,
                event: EventType::Connected,
                ..
            } = e
            {
                println!("change tank: {}", id);
                my_maze.add_tank(input::Controller::Gamepad(
                    Controller::create_gamepad_controller(&mut gamepad_controller, id),
                ));
            }
        }
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::KeyboardInput { input, .. } => match input {
                    // Press Esc to quit
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    // Press F11 to enter fullscreen mode
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::F11),
                        ..
                    } => {
                        let window = surface.window();
                        window.set_fullscreen(match window.fullscreen() {
                            None => Some(Fullscreen::Borderless(None)),
                            Some(_) => None,
                        });
                    }
                    // Other keyboard event
                    _ => keyboard_controller.lock().unwrap().input_event(&input),
                },
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(physical_size) => state.resize(Some(*physical_size)),
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    state.resize(Some(**new_inner_size))
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Recreate the swap_chain if lost
                    Err(wgpu::SwapChainError::Lost) => state.resize(None),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SwapChainError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => (),
        }
    });
}
